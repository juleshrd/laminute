use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::ai::jobs::{AiJobKind, AiJobStatus};
use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct AiJobRecord {
    pub job_id: String,
    pub kind: AiJobKind,
    pub meeting_id: Option<String>,
    #[allow(dead_code)]
    pub audio_file_id: Option<String>,
    #[allow(dead_code)]
    pub phase: String,
    pub status: AiJobStatus,
    #[allow(dead_code)]
    pub created_at: String,
    #[allow(dead_code)]
    pub updated_at: String,
}

pub struct AiJobRepository;

impl AiJobRepository {
    pub fn insert_running(
        conn: &Connection,
        job_id: &str,
        kind: AiJobKind,
        meeting_id: Option<&str>,
        audio_file_id: Option<&str>,
        phase: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO ai_jobs (job_id, kind, meeting_id, audio_file_id, phase, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                job_id,
                kind.as_str(),
                meeting_id,
                audio_file_id,
                phase,
                AiJobStatus::Running.as_str(),
                now,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn update_phase(conn: &Connection, job_id: &str, phase: &str) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE ai_jobs SET phase = ?1, updated_at = ?2 WHERE job_id = ?3",
            params![phase, now, job_id],
        )?;
        Ok(())
    }

    pub fn update_status(conn: &Connection, job_id: &str, status: AiJobStatus) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE ai_jobs SET status = ?1, updated_at = ?2 WHERE job_id = ?3",
            params![status.as_str(), now, job_id],
        )?;
        Ok(())
    }

    pub fn update_audio_file_id(
        conn: &Connection,
        job_id: &str,
        audio_file_id: &str,
    ) -> AppResult<()> {
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE ai_jobs SET audio_file_id = ?1, updated_at = ?2 WHERE job_id = ?3",
            params![audio_file_id, now, job_id],
        )?;
        Ok(())
    }

    pub fn list_running(conn: &Connection) -> AppResult<Vec<AiJobRecord>> {
        let mut stmt = conn.prepare(
            "SELECT job_id, kind, meeting_id, audio_file_id, phase, status, created_at, updated_at
             FROM ai_jobs WHERE status = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([AiJobStatus::Running.as_str()], map_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn latest_for_meeting(
        conn: &Connection,
        meeting_id: &str,
        kind: AiJobKind,
    ) -> AppResult<Option<AiJobRecord>> {
        conn.query_row(
            "SELECT job_id, kind, meeting_id, audio_file_id, phase, status, created_at, updated_at
             FROM ai_jobs
             WHERE meeting_id = ?1 AND kind = ?2
             ORDER BY created_at DESC, job_id DESC
             LIMIT 1",
            params![meeting_id, kind.as_str()],
            map_row,
        )
        .optional()
        .map_err(Into::into)
    }
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiJobRecord> {
    let kind_str: String = row.get(1)?;
    let status_str: String = row.get(5)?;
    Ok(AiJobRecord {
        job_id: row.get(0)?,
        kind: AiJobKind::from_str(&kind_str).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(1, "kind".into(), rusqlite::types::Type::Text)
        })?,
        meeting_id: row.get(2)?,
        audio_file_id: row.get(3)?,
        phase: row.get(4)?,
        status: AiJobStatus::from_str(&status_str).ok_or_else(|| {
            rusqlite::Error::InvalidColumnType(5, "status".into(), rusqlite::types::Type::Text)
        })?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::{CreateMeetingInput, MeetingStatus};
    use crate::repository::MeetingRepository;

    #[test]
    fn insert_and_update_ai_job() {
        let conn = open_in_memory().expect("db");
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Test".into(),
                description: None,
            },
        )
        .expect("meeting");

        AiJobRepository::insert_running(
            &conn,
            "job-1",
            AiJobKind::Transcription,
            Some(&meeting.id),
            None,
            "preparing",
        )
        .expect("insert");

        AiJobRepository::update_phase(&conn, "job-1", "uploading").expect("phase");
        AiJobRepository::update_status(&conn, "job-1", AiJobStatus::Cancelled).expect("status");

        let latest =
            AiJobRepository::latest_for_meeting(&conn, &meeting.id, AiJobKind::Transcription)
                .expect("latest")
                .expect("record");
        assert_eq!(latest.status, AiJobStatus::Cancelled);
        assert_eq!(latest.phase, "uploading");
    }

    #[test]
    fn reconcile_scenario_running_job_without_result() {
        let conn = open_in_memory().expect("db");
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Crash".into(),
                description: None,
            },
        )
        .expect("meeting");
        MeetingRepository::update_status(&conn, &meeting.id, MeetingStatus::Processing)
            .expect("status");
        MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            "/tmp/audio.wav",
            Some(1000),
            Some("wav"),
        )
        .expect("audio");

        AiJobRepository::insert_running(
            &conn,
            "job-crash",
            AiJobKind::Transcription,
            Some(&meeting.id),
            None,
            "transcribing",
        )
        .expect("insert");

        crate::ai::reconcile::reconcile_ai_jobs(&conn).expect("reconcile");

        let job = AiJobRepository::latest_for_meeting(&conn, &meeting.id, AiJobKind::Transcription)
            .expect("latest")
            .expect("record");
        assert_eq!(job.status, AiJobStatus::Cancelled);

        let meeting = MeetingRepository::get_by_id(&conn, &meeting.id).expect("meeting");
        assert_eq!(meeting.status, MeetingStatus::Processing);
    }
}
