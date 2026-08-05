use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::audio::import::{import_mp3, title_from_path, ImportedAudio};
use crate::audio::AudioError;
use crate::error::{AppError, AppResult};
use crate::models::{
    Action, ActionStatus, AudioFile, CreateMeetingInput, Meeting, MeetingDetail, MeetingStatus,
    MeetingSummary, Summary, Transcription,
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
        .ok_or_else(|| AppError::MeetingNotFound {
            id: id.to_string(),
        })
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
        let deleted = conn.execute("DELETE FROM meetings WHERE id = ?1", [id])?;
        if deleted == 0 {
            return Err(AppError::MeetingNotFound {
                id: id.to_string(),
            });
        }
        Ok(())
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

fn map_audio_error(error: AudioError) -> AppError {
    AppError::Message(error.to_string())
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
    fn create_from_imported_audio_sets_processing_status() {
        let conn = open_in_memory().unwrap();
        let imports_dir = std::env::temp_dir().join(format!(
            "laminute-imports-repo-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&imports_dir);
        let dest_path = imports_dir.join("sample.mp3");
        std::fs::write(&dest_path, b"fixture").unwrap();

        let imported = ImportedAudio {
            dest_path: dest_path.clone(),
            duration_ms: 120_000,
            format: "mp3".into(),
        };

        let detail = MeetingRepository::create_from_imported_audio(
            &conn,
            "Comité produit",
            &imported,
        )
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
}
