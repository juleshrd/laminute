use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::audio::import::{import_mp3, title_from_path, ImportedAudio};
use crate::audio::AudioError;
use crate::error::{AppError, AppResult};
use crate::models::{
    Action, ActionStatus, AudioFile, CreateMeetingInput, Meeting, MeetingDetail, MeetingListItem,
    MeetingSearchFilters, MeetingStatus, MeetingSummary, Summary, Transcription,
};

pub struct MeetingRepository;

impl MeetingRepository {
    pub fn create(conn: &Connection, input: CreateMeetingInput) -> AppResult<Meeting> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let title = input.title.trim();
        if title.is_empty() {
            return Err(AppError::Message("le titre est obligatoire".into()));
        }

        conn.execute(
            "INSERT INTO meetings (id, title, description, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                title,
                input.description,
                MeetingStatus::Draft.as_str(),
                now,
                now,
            ],
        )?;

        Self::get_by_id(conn, &id)
    }

    pub fn get_by_id(conn: &Connection, id: &str) -> AppResult<Meeting> {
        conn.query_row(
            "SELECT id, title, description, status, started_at, ended_at, created_at, updated_at
             FROM meetings WHERE id = ?1",
            [id],
            map_meeting_row,
        )
        .optional()?
        .ok_or_else(|| AppError::MeetingNotFound { id: id.to_string() })
    }

    pub fn get_detail(conn: &Connection, id: &str) -> AppResult<MeetingDetail> {
        let meeting = Self::get_by_id(conn, id)?;

        Ok(MeetingDetail {
            meeting,
            audio_files: Self::list_audio_files(conn, id)?,
            transcriptions: Self::list_transcriptions(conn, id)?,
            summaries: Self::list_summaries(conn, id)?,
            actions: Self::list_actions(conn, id)?,
        })
    }

    pub fn list(conn: &Connection) -> AppResult<Vec<MeetingSummary>> {
        let mut stmt = conn.prepare(
            "SELECT id, title, status, started_at, ended_at, created_at, updated_at
             FROM meetings ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let status_str: String = row.get(2)?;
            Ok(MeetingSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                status: MeetingStatus::from_str(&status_str).unwrap_or(MeetingStatus::Draft),
                started_at: row.get(3)?,
                ended_at: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn search(
        conn: &Connection,
        filters: &MeetingSearchFilters,
    ) -> AppResult<Vec<MeetingListItem>> {
        let query = filters
            .query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let like_pattern = query.map(|value| format!("%{}%", escape_like(value)));

        let status_str = filters.status.map(|status| status.as_str().to_string());

        let sql = r#"
            SELECT
                m.id,
                m.title,
                m.status,
                m.created_at,
                m.started_at,
                m.ended_at,
                m.updated_at,
                CASE
                    WHEN ?1 IS NOT NULL THEN COALESCE(
                        CASE WHEN m.title LIKE ?2 ESCAPE '\' THEN m.title END,
                        (
                            SELECT t.content
                            FROM transcriptions t
                            WHERE t.meeting_id = m.id AND t.content LIKE ?2 ESCAPE '\'
                            LIMIT 1
                        ),
                        (
                            SELECT s.content
                            FROM summaries s
                            WHERE s.meeting_id = m.id AND s.content LIKE ?2 ESCAPE '\'
                            LIMIT 1
                        )
                    )
                END AS snippet
            FROM meetings m
            WHERE (?3 IS NULL OR m.status = ?3)
              AND (?4 IS NULL OR date(COALESCE(m.started_at, m.created_at)) >= date(?4))
              AND (?5 IS NULL OR date(COALESCE(m.started_at, m.created_at)) <= date(?5))
              AND (
                    ?6 IS NULL
                    OR EXISTS (
                        SELECT 1 FROM transcriptions t
                        WHERE t.meeting_id = m.id AND t.provider_id = ?6
                    )
                    OR EXISTS (
                        SELECT 1 FROM summaries s
                        WHERE s.meeting_id = m.id AND s.provider_id = ?6
                    )
              )
              AND (
                    ?1 IS NULL
                    OR m.title LIKE ?2 ESCAPE '\'
                    OR EXISTS (
                        SELECT 1 FROM transcriptions t
                        WHERE t.meeting_id = m.id AND t.content LIKE ?2 ESCAPE '\'
                    )
                    OR EXISTS (
                        SELECT 1 FROM summaries s
                        WHERE s.meeting_id = m.id AND s.content LIKE ?2 ESCAPE '\'
                    )
              )
            ORDER BY m.created_at DESC
        "#;

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(
            params![
                query,
                like_pattern,
                status_str,
                filters.date_from,
                filters.date_to,
                filters.provider_id,
            ],
            |row| {
                let status_value: String = row.get(2)?;
                let raw_snippet: Option<String> = row.get(7)?;
                let snippet = match (raw_snippet, query) {
                    (Some(text), Some(needle)) => Some(centered_snippet(&text, needle, 120)),
                    (other, _) => other,
                };
                Ok(MeetingListItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    status: MeetingStatus::from_str(&status_value).unwrap_or(MeetingStatus::Draft),
                    created_at: row.get(3)?,
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    snippet,
                })
            },
        )?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_from_mp3_import(
        conn: &Connection,
        source: &Path,
        imports_dir: &Path,
    ) -> AppResult<MeetingDetail> {
        let imported = import_mp3(source, imports_dir).map_err(map_audio_error)?;
        let title = title_from_path(source);
        Self::create_from_imported_audio(conn, &title, &imported)
    }

    pub fn create_from_imported_audio(
        conn: &Connection,
        title: &str,
        imported: &ImportedAudio,
    ) -> AppResult<MeetingDetail> {
        let meeting_id = Uuid::new_v4().to_string();
        let audio_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let trimmed_title = title.trim();
        if trimmed_title.is_empty() {
            return Err(AppError::Message("le titre est obligatoire".into()));
        }

        let tx = conn.unchecked_transaction()?;

        tx.execute(
            "INSERT INTO meetings (id, title, description, status, created_at, updated_at)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5)",
            params![
                meeting_id,
                trimmed_title,
                MeetingStatus::Processing.as_str(),
                now,
                now,
            ],
        )?;

        tx.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                audio_id,
                meeting_id,
                imported.dest_path.to_string_lossy().to_string(),
                imported.duration_ms,
                imported.format,
                now,
            ],
        )?;

        tx.commit()?;

        Self::get_detail(conn, &meeting_id)
    }

    pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
        Self::delete_audio_files_for_meeting(conn, id)?;

        let deleted = conn.execute("DELETE FROM meetings WHERE id = ?1", [id])?;
        if deleted == 0 {
            return Err(AppError::MeetingNotFound { id: id.to_string() });
        }
        Ok(())
    }

    /// Supprime les fichiers audio disque et les lignes `audio_files` d'une réunion.
    /// Les transcriptions liées passent en `audio_file_id = NULL` (ON DELETE SET NULL).
    pub fn delete_audio_files_for_meeting(conn: &Connection, meeting_id: &str) -> AppResult<()> {
        let audio_files = Self::list_audio_files(conn, meeting_id)?;
        for audio in &audio_files {
            let path = Path::new(&audio.file_path);
            if path.is_file() {
                let _ = std::fs::remove_file(path);
            }
        }

        conn.execute(
            "DELETE FROM audio_files WHERE meeting_id = ?1",
            [meeting_id],
        )?;
        Ok(())
    }

    pub fn ensure_ai_provider(
        conn: &Connection,
        provider_id: &str,
        name: &str,
        provider_type: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO ai_providers (id, name, provider_type, is_enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            params![provider_id, name, provider_type, now],
        )?;
        Ok(())
    }

    pub fn attach_audio_file(
        conn: &Connection,
        meeting_id: &str,
        file_path: &str,
        duration_ms: Option<i64>,
        format: Option<&str>,
    ) -> AppResult<AudioFile> {
        Self::get_by_id(conn, meeting_id)?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, meeting_id, file_path, duration_ms, format, now],
        )?;

        Ok(AudioFile {
            id,
            meeting_id: meeting_id.to_string(),
            file_path: file_path.to_string(),
            duration_ms,
            format: format.map(str::to_string),
            created_at: now,
        })
    }

    pub fn create_transcription(
        conn: &Connection,
        meeting_id: &str,
        audio_file_id: Option<&str>,
        provider_id: &str,
        provider_display_name: &str,
        content: &str,
        language: Option<&str>,
    ) -> AppResult<Transcription> {
        Self::get_by_id(conn, meeting_id)?;
        Self::ensure_ai_provider(conn, provider_id, provider_display_name, provider_id)?;

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                id,
                meeting_id,
                audio_file_id,
                provider_id,
                content,
                language,
                now,
            ],
        )?;

        Ok(Transcription {
            id,
            meeting_id: meeting_id.to_string(),
            audio_file_id: audio_file_id.map(str::to_string),
            provider_id: Some(provider_id.to_string()),
            content: content.to_string(),
            language: language.map(str::to_string),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update_title(conn: &Connection, id: &str, title: &str) -> AppResult<Meeting> {
        let trimmed = title.trim();
        if trimmed.is_empty() {
            return Err(AppError::Message("le titre est obligatoire".into()));
        }

        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE meetings SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![trimmed, now, id],
        )?;

        if updated == 0 {
            return Err(AppError::MeetingNotFound { id: id.to_string() });
        }

        Self::get_by_id(conn, id)
    }

    pub fn update_status(
        conn: &Connection,
        meeting_id: &str,
        status: MeetingStatus,
    ) -> AppResult<Meeting> {
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE meetings SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.as_str(), now, meeting_id],
        )?;

        if updated == 0 {
            return Err(AppError::MeetingNotFound {
                id: meeting_id.to_string(),
            });
        }

        Self::get_by_id(conn, meeting_id)
    }

    fn list_audio_files(conn: &Connection, meeting_id: &str) -> AppResult<Vec<AudioFile>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, file_path, duration_ms, format, created_at
             FROM audio_files WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([meeting_id], |row| {
            Ok(AudioFile {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                file_path: row.get(2)?,
                duration_ms: row.get(3)?,
                format: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_transcriptions(conn: &Connection, meeting_id: &str) -> AppResult<Vec<Transcription>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at
             FROM transcriptions WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([meeting_id], |row| {
            Ok(Transcription {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                audio_file_id: row.get(2)?,
                provider_id: row.get(3)?,
                content: row.get(4)?,
                language: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_summaries(conn: &Connection, meeting_id: &str) -> AppResult<Vec<Summary>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, provider_id, content, created_at, updated_at
             FROM summaries WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([meeting_id], |row| {
            Ok(Summary {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                provider_id: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_actions(conn: &Connection, meeting_id: &str) -> AppResult<Vec<Action>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, title, description, assignee, due_date, status, created_at, updated_at
             FROM actions WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([meeting_id], |row| {
            let status_str: String = row.get(6)?;
            Ok(Action {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                title: row.get(2)?,
                description: row.get(3)?,
                assignee: row.get(4)?,
                due_date: row.get(5)?,
                status: ActionStatus::from_str(&status_str).unwrap_or(ActionStatus::Pending),
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn map_meeting_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Meeting> {
    let status_str: String = row.get(3)?;
    Ok(Meeting {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        status: MeetingStatus::from_str(&status_str).unwrap_or(MeetingStatus::Draft),
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn map_audio_error(error: AudioError) -> AppError {
    AppError::Message(error.to_string())
}

fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Builds a short excerpt centered on the first case-insensitive match of `needle`.
fn centered_snippet(haystack: &str, needle: &str, max_chars: usize) -> String {
    if max_chars == 0 || haystack.is_empty() {
        return String::new();
    }

    let needle = needle.trim();
    if needle.is_empty() {
        return truncate_chars(haystack, max_chars);
    }

    let lower_haystack = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    let byte_pos = lower_haystack.find(&lower_needle);

    let char_index = match byte_pos {
        Some(pos) => haystack[..pos].chars().count(),
        None => 0,
    };

    let total_chars = haystack.chars().count();
    if total_chars <= max_chars {
        return haystack.to_string();
    }

    let needle_chars = needle.chars().count();
    let context = max_chars.saturating_sub(needle_chars) / 2;
    let mut start = char_index.saturating_sub(context);
    let end = (start + max_chars).min(total_chars);
    if end - start < max_chars {
        start = end.saturating_sub(max_chars);
    }

    let excerpt: String = haystack.chars().skip(start).take(end - start).collect();

    let mut result = String::new();
    if start > 0 {
        result.push('…');
    }
    result.push_str(&excerpt);
    if end < total_chars {
        result.push('…');
    }
    result
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut truncated: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        truncated.push('…');
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::ActionStatus;

    fn seed_related_data(conn: &Connection, meeting_id: &str) {
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO ai_providers (id, name, provider_type, is_enabled, credential_key_id, created_at, updated_at)
             VALUES ('provider-1', 'OpenAI', 'openai', 1, 'keychain:openai', ?1, ?1)",
            [&now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES ('audio-1', ?1, '/tmp/meeting.mp3', 60000, 'mp3', ?2)",
            params![meeting_id, now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at)
             VALUES ('tx-1', ?1, 'audio-1', 'provider-1', 'Bonjour à tous', 'fr', ?2, ?2)",
            params![meeting_id, now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES ('sum-1', ?1, 'provider-1', 'Réunion de lancement', ?2, ?2)",
            params![meeting_id, now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO actions (id, meeting_id, title, status, created_at, updated_at)
             VALUES ('act-1', ?1, 'Envoyer le compte-rendu', 'pending', ?2, ?2)",
            params![meeting_id, now],
        )
        .unwrap();
    }

    #[test]
    fn create_and_get_meeting() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Stand-up".into(),
                description: Some("Daily".into()),
            },
        )
        .unwrap();

        assert_eq!(meeting.title, "Stand-up");
        assert_eq!(meeting.status, MeetingStatus::Draft);

        let fetched = MeetingRepository::get_by_id(&conn, &meeting.id).unwrap();
        assert_eq!(fetched.title, "Stand-up");
    }

    #[test]
    fn reject_empty_title() {
        let conn = open_in_memory().unwrap();
        let err = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "   ".into(),
                description: None,
            },
        )
        .unwrap_err();

        assert!(err.to_string().contains("titre"));
    }

    #[test]
    fn update_title_ok() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Stand-up".into(),
                description: None,
            },
        )
        .unwrap();

        let updated =
            MeetingRepository::update_title(&conn, &meeting.id, "Comité produit").unwrap();
        assert_eq!(updated.title, "Comité produit");

        let fetched = MeetingRepository::get_by_id(&conn, &meeting.id).unwrap();
        assert_eq!(fetched.title, "Comité produit");
    }

    #[test]
    fn reject_empty_title_on_update() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Stand-up".into(),
                description: None,
            },
        )
        .unwrap();

        let err = MeetingRepository::update_title(&conn, &meeting.id, "   ").unwrap_err();
        assert!(err.to_string().contains("titre"));
    }

    #[test]
    fn list_meetings_returns_newest_first() {
        let conn = open_in_memory().unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Ancienne".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Récente".into(),
                description: None,
            },
        )
        .unwrap();

        let meetings = MeetingRepository::list(&conn).unwrap();
        assert_eq!(meetings.len(), 2);
        assert_eq!(meetings[0].title, "Récente");
    }

    #[test]
    fn get_detail_includes_related_entities() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Comité".into(),
                description: None,
            },
        )
        .unwrap();
        seed_related_data(&conn, &meeting.id);

        let detail = MeetingRepository::get_detail(&conn, &meeting.id).unwrap();
        assert_eq!(detail.audio_files.len(), 1);
        assert_eq!(detail.transcriptions.len(), 1);
        assert_eq!(detail.summaries.len(), 1);
        assert_eq!(detail.actions.len(), 1);
        assert_eq!(detail.actions[0].status, ActionStatus::Pending);
    }

    #[test]
    fn delete_meeting_cascades_children() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "À supprimer".into(),
                description: None,
            },
        )
        .unwrap();
        seed_related_data(&conn, &meeting.id);

        MeetingRepository::delete(&conn, &meeting.id).unwrap();

        let audio_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audio_files", [], |row| row.get(0))
            .unwrap();
        assert_eq!(audio_count, 0);

        assert!(MeetingRepository::get_by_id(&conn, &meeting.id).is_err());
    }

    #[test]
    fn delete_unknown_meeting_returns_error() {
        let conn = open_in_memory().unwrap();
        let err = MeetingRepository::delete(&conn, "missing").unwrap_err();
        assert!(err.to_string().contains("introuvable"));
    }

    #[test]
    fn delete_meeting_removes_audio_file_from_disk() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Avec fichier".into(),
                description: None,
            },
        )
        .unwrap();

        let dir =
            std::env::temp_dir().join(format!("laminute-delete-audio-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let audio_path = dir.join("meeting.mp3");
        std::fs::write(&audio_path, b"audio").unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES ('audio-del', ?1, ?2, 1000, 'mp3', ?3)",
            params![meeting.id, audio_path.to_string_lossy().to_string(), now],
        )
        .unwrap();

        assert!(audio_path.is_file());
        MeetingRepository::delete(&conn, &meeting.id).unwrap();
        assert!(!audio_path.exists());

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn create_from_imported_audio_sets_processing_status() {
        let conn = open_in_memory().unwrap();
        let imports_dir =
            std::env::temp_dir().join(format!("laminute-imports-repo-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&imports_dir);
        let dest_path = imports_dir.join("sample.mp3");
        std::fs::write(&dest_path, b"fixture").unwrap();

        let imported = ImportedAudio {
            dest_path: dest_path.clone(),
            duration_ms: 120_000,
            format: "mp3".into(),
        };

        let detail =
            MeetingRepository::create_from_imported_audio(&conn, "Comité produit", &imported)
                .unwrap();

        assert_eq!(detail.meeting.title, "Comité produit");
        assert_eq!(detail.meeting.status, MeetingStatus::Processing);
        assert_eq!(detail.audio_files.len(), 1);
        assert_eq!(detail.audio_files[0].duration_ms, Some(120_000));
        assert_eq!(detail.audio_files[0].format.as_deref(), Some("mp3"));
        assert_eq!(
            detail.audio_files[0].file_path,
            dest_path.to_string_lossy().to_string()
        );

        let _ = std::fs::remove_dir_all(imports_dir);
    }

    #[test]
    fn delete_audio_files_for_meeting_keeps_transcription() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Purge audio".into(),
                description: None,
            },
        )
        .unwrap();

        let dir = std::env::temp_dir().join(format!("laminute-purge-audio-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let audio_path = dir.join("meeting.mp3");
        std::fs::write(&audio_path, b"audio").unwrap();

        let audio = MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            &audio_path.to_string_lossy(),
            Some(1000),
            Some("mp3"),
        )
        .unwrap();

        MeetingRepository::create_transcription(
            &conn,
            &meeting.id,
            Some(&audio.id),
            "mistral",
            "Mistral AI",
            "Bonjour",
            Some("fr"),
        )
        .unwrap();

        assert!(audio_path.is_file());
        MeetingRepository::delete_audio_files_for_meeting(&conn, &meeting.id).unwrap();
        assert!(!audio_path.exists());

        let detail = MeetingRepository::get_detail(&conn, &meeting.id).unwrap();
        assert!(detail.audio_files.is_empty());
        assert_eq!(detail.transcriptions.len(), 1);
        assert!(detail.transcriptions[0].audio_file_id.is_none());

        let _ = std::fs::remove_dir_all(dir);
    }

    fn empty_filters() -> MeetingSearchFilters {
        MeetingSearchFilters {
            query: None,
            status: None,
            provider_id: None,
            date_from: None,
            date_to: None,
        }
    }

    #[test]
    fn search_empty_query_returns_all_meetings() {
        let conn = open_in_memory().unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Alpha".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Beta".into(),
                description: None,
            },
        )
        .unwrap();

        let results = MeetingRepository::search(&conn, &empty_filters()).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_by_title() {
        let conn = open_in_memory().unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Comité produit".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Stand-up".into(),
                description: None,
            },
        )
        .unwrap();

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                query: Some("produit".into()),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Comité produit");
        assert!(results[0].snippet.is_some());
    }

    #[test]
    fn search_by_status() {
        let conn = open_in_memory().unwrap();
        let draft = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Brouillon".into(),
                description: None,
            },
        )
        .unwrap();
        let completed = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Terminée".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::update_status(&conn, &completed.id, MeetingStatus::Completed).unwrap();
        let _ = draft;

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                status: Some(MeetingStatus::Completed),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Terminée");
    }

    #[test]
    fn search_by_provider_id() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Avec IA".into(),
                description: None,
            },
        )
        .unwrap();
        seed_related_data(&conn, &meeting.id);

        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sans IA".into(),
                description: None,
            },
        )
        .unwrap();

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                provider_id: Some("provider-1".into()),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Avec IA");
    }

    #[test]
    fn search_by_date_range() {
        let conn = open_in_memory().unwrap();
        let now = Utc::now().to_rfc3339();
        let yesterday = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();

        conn.execute(
            "INSERT INTO meetings (id, title, description, status, started_at, created_at, updated_at)
             VALUES ('old', 'Ancienne réunion', NULL, 'completed', ?1, ?1, ?1)",
            [&yesterday],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO meetings (id, title, description, status, started_at, created_at, updated_at)
             VALUES ('new', 'Réunion du jour', NULL, 'completed', ?1, ?1, ?1)",
            [&now],
        )
        .unwrap();

        let today = Utc::now().format("%Y-%m-%d").to_string();

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                date_from: Some(today.clone()),
                date_to: Some(today),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Réunion du jour");
    }

    #[test]
    fn search_by_transcription_content() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Point commercial".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Autre sujet".into(),
                description: None,
            },
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        let long_prefix = "x".repeat(80);
        let content =
            format!("{long_prefix} Discussion avec le client Dufour sur le renouvellement.");
        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at)
             VALUES ('tx-dufour', ?1, NULL, NULL, ?2, 'fr', ?3, ?3)",
            params![meeting.id, content, now],
        )
        .unwrap();

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                query: Some("Dufour".into()),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Point commercial");
        let snippet = results[0].snippet.as_deref().unwrap();
        assert!(snippet.contains("Dufour"));
        assert!(snippet.starts_with('…'));
    }

    #[test]
    fn search_by_summary_content() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Comité".into(),
                description: None,
            },
        )
        .unwrap();
        MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sans résumé pertinent".into(),
                description: None,
            },
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES ('sum-1', ?1, NULL, 'Actions : relancer Dufour avant vendredi', ?2, ?2)",
            params![meeting.id, now],
        )
        .unwrap();

        let results = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                query: Some("dufour".into()),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Comité");
        assert!(results[0]
            .snippet
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("dufour"));
    }

    #[test]
    fn centered_snippet_focuses_on_match() {
        let haystack = format!("{}Dufour{}", "a".repeat(100), "b".repeat(100));
        let snippet = centered_snippet(&haystack, "Dufour", 40);
        assert!(snippet.contains("Dufour"));
        assert!(snippet.starts_with('…'));
        assert!(snippet.ends_with('…'));
        assert!(snippet.chars().count() <= 42); // excerpt + ellipses
    }
}
