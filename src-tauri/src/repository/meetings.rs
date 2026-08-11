use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::audio::import::ImportedAudio;
use crate::error::{AppError, AppResult};
use crate::models::{
    Action, ActionStatus, AudioFile, CreateMeetingInput, Meeting, MeetingDetail, MeetingFullDetail,
    MeetingListItem, MeetingSearchFilters, MeetingSearchPage, MeetingStatus, MeetingSummary,
    Summary, SummaryMetadata, Transcription, TranscriptionMetadata,
};

pub struct MeetingRepository;

const SEARCH_PAGE_SIZE: usize = 50;
const SEARCH_QUERY_LIMIT: i64 = (SEARCH_PAGE_SIZE as i64) + 1;
const SEARCH_CURSOR_SEPARATOR: char = '|';

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
            transcriptions: Self::list_transcription_metadata(conn, id)?,
            summaries: Self::list_summary_metadata(conn, id)?,
            actions: Self::list_actions(conn, id)?,
        })
    }

    pub fn get_full_detail(conn: &Connection, id: &str) -> AppResult<MeetingFullDetail> {
        let meeting = Self::get_by_id(conn, id)?;

        Ok(MeetingFullDetail {
            meeting,
            audio_files: Self::list_audio_files(conn, id)?,
            transcriptions: Self::list_transcriptions(conn, id)?,
            summaries: Self::list_summaries(conn, id)?,
            actions: Self::list_actions(conn, id)?,
        })
    }

    pub fn latest_transcription(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Option<Transcription>> {
        Self::get_by_id(conn, meeting_id)?;
        conn.query_row(
            "SELECT id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at
             FROM transcriptions WHERE meeting_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
            [meeting_id],
            map_transcription_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn latest_summary(conn: &Connection, meeting_id: &str) -> AppResult<Option<Summary>> {
        Self::get_by_id(conn, meeting_id)?;
        conn.query_row(
            "SELECT id, meeting_id, provider_id, content, created_at, updated_at
             FROM summaries WHERE meeting_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
            [meeting_id],
            map_summary_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_transcription_versions(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Vec<TranscriptionMetadata>> {
        Self::get_by_id(conn, meeting_id)?;
        Self::list_transcription_metadata(conn, meeting_id)
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
    ) -> AppResult<MeetingSearchPage> {
        let query = filters
            .query
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        let fts_query = query.map(escape_fts);

        let status_str = filters.status.map(|status| status.as_str().to_string());
        let cursor = filters
            .cursor
            .as_deref()
            .map(parse_search_cursor)
            .transpose()?;
        let (cursor_created_at, cursor_id) = cursor
            .map(|cursor| (Some(cursor.created_at), Some(cursor.id)))
            .unwrap_or((None, None));

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
                    WHEN ?1 IS NOT NULL THEN (
                        SELECT snippet(meetings_fts, 2, '', '', '…', 24)
                        FROM meetings_fts
                        WHERE meeting_id = m.id
                          AND meetings_fts MATCH ?2
                        ORDER BY CASE source
                            WHEN 'title' THEN 0
                            WHEN 'transcription' THEN 1
                            WHEN 'summary' THEN 2
                            ELSE 3
                        END
                        LIMIT 1
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
                    OR m.id IN (
                        SELECT meeting_id
                        FROM meetings_fts
                        WHERE meetings_fts MATCH ?2
                    )
              )
              AND (
                    ?7 IS NULL
                    OR m.created_at < ?7
                    OR (m.created_at = ?7 AND m.id < ?8)
              )
            ORDER BY m.created_at DESC, m.id DESC
            LIMIT ?9
        "#;

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(
            params![
                query,
                fts_query,
                status_str,
                filters.date_from,
                filters.date_to,
                filters.provider_id,
                cursor_created_at,
                cursor_id,
                SEARCH_QUERY_LIMIT,
            ],
            |row| {
                let status_value: String = row.get(2)?;
                Ok(MeetingListItem {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    status: MeetingStatus::from_str(&status_value).unwrap_or(MeetingStatus::Draft),
                    created_at: row.get(3)?,
                    started_at: row.get(4)?,
                    ended_at: row.get(5)?,
                    updated_at: row.get(6)?,
                    snippet: row.get(7)?,
                })
            },
        )?;

        let mut items = rows.collect::<Result<Vec<_>, _>>()?;
        let next_cursor = if items.len() > SEARCH_PAGE_SIZE {
            items.truncate(SEARCH_PAGE_SIZE);
            items.last().map(search_cursor_for)
        } else {
            None
        };

        Ok(MeetingSearchPage { items, next_cursor })
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
        // La suppression disque est gérée hors repository (voir retention::purge_meeting_audio).
        // CASCADE efface aussi audio_files si des lignes restent.
        let deleted = conn.execute("DELETE FROM meetings WHERE id = ?1", [id])?;
        if deleted == 0 {
            return Err(AppError::MeetingNotFound { id: id.to_string() });
        }
        Ok(())
    }

    /// Supprime les lignes `audio_files` d'une réunion (pas d'I/O disque).
    /// Les transcriptions liées passent en `audio_file_id = NULL` (ON DELETE SET NULL).
    pub fn delete_audio_file_rows(conn: &Connection, meeting_id: &str) -> AppResult<()> {
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

    pub fn list_audio_files(conn: &Connection, meeting_id: &str) -> AppResult<Vec<AudioFile>> {
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

        let rows = stmt.query_map([meeting_id], map_transcription_row)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_summaries(conn: &Connection, meeting_id: &str) -> AppResult<Vec<Summary>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, provider_id, content, created_at, updated_at
             FROM summaries WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([meeting_id], map_summary_row)?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_transcription_metadata(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Vec<TranscriptionMetadata>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, audio_file_id, provider_id, language, created_at, updated_at
             FROM transcriptions WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([meeting_id], |row| {
            Ok(TranscriptionMetadata {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                audio_file_id: row.get(2)?,
                provider_id: row.get(3)?,
                language: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn list_summary_metadata(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Vec<SummaryMetadata>> {
        let mut stmt = conn.prepare(
            "SELECT id, meeting_id, provider_id, created_at, updated_at
             FROM summaries WHERE meeting_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([meeting_id], |row| {
            Ok(SummaryMetadata {
                id: row.get(0)?,
                meeting_id: row.get(1)?,
                provider_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
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

fn map_transcription_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Transcription> {
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
}

fn map_summary_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Summary> {
    Ok(Summary {
        id: row.get(0)?,
        meeting_id: row.get(1)?,
        provider_id: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Escapes a user query for FTS5 MATCH (trigram substring search).
fn escape_fts(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

struct SearchCursor {
    created_at: String,
    id: String,
}

fn search_cursor_for(item: &MeetingListItem) -> String {
    format!("{}{}{}", item.created_at, SEARCH_CURSOR_SEPARATOR, item.id)
}

fn parse_search_cursor(value: &str) -> AppResult<SearchCursor> {
    let (created_at, id) = value
        .split_once(SEARCH_CURSOR_SEPARATOR)
        .ok_or_else(|| AppError::Message("curseur de pagination de réunion invalide".into()))?;
    if created_at.is_empty() || id.is_empty() {
        return Err(AppError::Message(
            "curseur de pagination de réunion invalide".into(),
        ));
    }
    Ok(SearchCursor {
        created_at: created_at.to_string(),
        id: id.to_string(),
    })
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
    fn get_detail_excludes_heavy_content_and_full_detail_keeps_it_for_exports() {
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

        let ipc_payload = serde_json::to_value(&detail).unwrap();
        assert!(ipc_payload["transcriptions"][0].get("content").is_none());
        assert!(ipc_payload["summaries"][0].get("content").is_none());

        let full_detail = MeetingRepository::get_full_detail(&conn, &meeting.id).unwrap();
        assert_eq!(full_detail.transcriptions[0].content, "Bonjour à tous");
        assert_eq!(full_detail.summaries[0].content, "Réunion de lancement");

        let latest = MeetingRepository::latest_transcription(&conn, &meeting.id).unwrap();
        assert_eq!(latest.unwrap().content, "Bonjour à tous");
        assert_eq!(
            MeetingRepository::list_transcription_versions(&conn, &meeting.id)
                .unwrap()
                .len(),
            1
        );
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
        use crate::audio::paths::ManagedAudioRoots;
        use crate::retention::purge_meeting_audio;

        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Avec fichier".into(),
                description: None,
            },
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let roots = ManagedAudioRoots::from_app_data_dir(tmp.path().to_path_buf());
        roots.ensure_dirs().unwrap();
        let audio_path = roots.imports_dir.join("meeting.mp3");
        std::fs::write(&audio_path, b"audio").unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES ('audio-del', ?1, ?2, 1000, 'mp3', ?3)",
            params![meeting.id, audio_path.to_string_lossy().to_string(), now],
        )
        .unwrap();

        assert!(audio_path.is_file());
        purge_meeting_audio(&roots, &conn, &meeting.id).unwrap();
        MeetingRepository::delete(&conn, &meeting.id).unwrap();
        assert!(!audio_path.exists());
    }

    #[test]
    fn delete_meeting_refuses_external_audio_path() {
        use crate::audio::paths::ManagedAudioRoots;
        use crate::retention::purge_meeting_audio;

        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Chemin externe".into(),
                description: None,
            },
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let roots = ManagedAudioRoots::from_app_data_dir(tmp.path().join("app").to_path_buf());
        roots.ensure_dirs().unwrap();

        let external = tmp.path().join("external.mp3");
        std::fs::write(&external, b"audio").unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES ('audio-ext', ?1, ?2, 1000, 'mp3', ?3)",
            params![meeting.id, external.to_string_lossy().to_string(), now],
        )
        .unwrap();

        let err = purge_meeting_audio(&roots, &conn, &meeting.id).unwrap_err();
        assert!(err.to_string().contains("hors des répertoires gérés"));
        assert!(external.exists());

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audio_files WHERE meeting_id = ?1",
                [&meeting.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 1,
            "la ligne DB doit rester si la suppression disque échoue"
        );
    }

    #[cfg(unix)]
    #[test]
    fn purge_keeps_db_row_when_disk_remove_fails() {
        use std::os::unix::fs::PermissionsExt;

        use crate::audio::paths::ManagedAudioRoots;
        use crate::retention::purge_meeting_audio;

        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Verrou disque".into(),
                description: None,
            },
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let roots = ManagedAudioRoots::from_app_data_dir(tmp.path().to_path_buf());
        roots.ensure_dirs().unwrap();
        let audio_path = roots.imports_dir.join("locked.mp3");
        std::fs::write(&audio_path, b"audio").unwrap();

        MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            &audio_path.to_string_lossy(),
            Some(1000),
            Some("mp3"),
        )
        .unwrap();

        let mut perms = std::fs::metadata(&roots.imports_dir).unwrap().permissions();
        perms.set_mode(0o555);
        std::fs::set_permissions(&roots.imports_dir, perms).unwrap();

        let result = purge_meeting_audio(&roots, &conn, &meeting.id);

        let mut restore = std::fs::metadata(&roots.imports_dir).unwrap().permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(&roots.imports_dir, restore).unwrap();

        assert!(result.is_err());
        assert!(audio_path.exists());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audio_files WHERE meeting_id = ?1",
                [&meeting.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
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
        use crate::audio::paths::ManagedAudioRoots;
        use crate::retention::purge_meeting_audio;

        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Purge audio".into(),
                description: None,
            },
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let roots = ManagedAudioRoots::from_app_data_dir(tmp.path().to_path_buf());
        roots.ensure_dirs().unwrap();
        let audio_path = roots.imports_dir.join("meeting.mp3");
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
        purge_meeting_audio(&roots, &conn, &meeting.id).unwrap();
        assert!(!audio_path.exists());

        let detail = MeetingRepository::get_detail(&conn, &meeting.id).unwrap();
        assert!(detail.audio_files.is_empty());
        assert_eq!(detail.transcriptions.len(), 1);
        assert!(detail.transcriptions[0].audio_file_id.is_none());
    }

    fn empty_filters() -> MeetingSearchFilters {
        MeetingSearchFilters {
            query: None,
            status: None,
            provider_id: None,
            date_from: None,
            date_to: None,
            cursor: None,
        }
    }

    fn insert_meeting_at(conn: &Connection, id: &str, title: &str, created_at: &str) {
        conn.execute(
            "INSERT INTO meetings (id, title, description, status, created_at, updated_at)
             VALUES (?1, ?2, NULL, 'draft', ?3, ?3)",
            params![id, title, created_at],
        )
        .unwrap();
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
        assert_eq!(results.items.len(), 2);
    }

    #[test]
    fn search_limits_results_and_paginates_without_duplicates() {
        let conn = open_in_memory().unwrap();
        let created_at = "2026-08-08T10:00:00Z";
        for index in 0..55 {
            let id = format!("meeting-{index:02}");
            insert_meeting_at(&conn, &id, &format!("Réunion {index:02}"), created_at);
        }

        let first_page = MeetingRepository::search(&conn, &empty_filters()).unwrap();
        assert_eq!(first_page.items.len(), SEARCH_PAGE_SIZE);
        assert_eq!(first_page.items[0].id, "meeting-54");
        assert_eq!(first_page.items[SEARCH_PAGE_SIZE - 1].id, "meeting-05");

        let first_ids: std::collections::HashSet<_> = first_page
            .items
            .iter()
            .map(|item| item.id.clone())
            .collect();
        assert_eq!(first_ids.len(), SEARCH_PAGE_SIZE);

        let second_page = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                cursor: first_page.next_cursor.clone(),
                ..empty_filters()
            },
        )
        .unwrap();

        assert_eq!(second_page.items.len(), 5);
        assert!(second_page.next_cursor.is_none());
        assert_eq!(second_page.items[0].id, "meeting-04");
        for item in &second_page.items {
            assert!(
                !first_ids.contains(&item.id),
                "cursor pagination must not repeat {}",
                item.id
            );
        }
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Comité produit");
        assert!(results.items[0].snippet.is_some());
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Terminée");
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Avec IA");
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Réunion du jour");
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Point commercial");
        let snippet = results.items[0].snippet.as_deref().unwrap();
        assert!(snippet.contains("Dufour"));
        assert!(snippet.starts_with('…'));
    }

    #[test]
    fn search_snippet_does_not_return_full_fts_body() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Long verbatim".into(),
                description: None,
            },
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        let full_body = format!(
            "{} Dufour {} forbidden-tail-marker",
            "avant ".repeat(250),
            "après ".repeat(250)
        );
        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at)
             VALUES ('tx-long', ?1, NULL, NULL, ?2, 'fr', ?3, ?3)",
            params![meeting.id, &full_body, now],
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

        let snippet = results.items[0].snippet.as_deref().unwrap();
        assert!(snippet.contains("Dufour"));
        assert!(snippet.len() < 240);
        assert!(!snippet.contains("forbidden-tail-marker"));
        assert_ne!(snippet, full_body);
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

        assert_eq!(results.items.len(), 1);
        assert_eq!(results.items[0].title, "Comité");
        assert!(results.items[0]
            .snippet
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("dufour"));
    }
}
