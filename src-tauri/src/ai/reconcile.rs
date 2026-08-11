use rusqlite::Connection;

use crate::ai::jobs::{AiJobKind, AiJobStatus};
use crate::error::AppResult;
use crate::models::MeetingStatus;
use crate::repository::{AiJobRepository, MeetingRepository};

/// Réconcilie les jobs IA laissés en `running` après un crash ou un redémarrage.
pub fn reconcile_ai_jobs(conn: &Connection) -> AppResult<()> {
    let running_jobs = AiJobRepository::list_running(conn)?;
    for job in running_jobs {
        let Some(meeting_id) = job.meeting_id.as_deref() else {
            AiJobRepository::update_status(conn, &job.job_id, AiJobStatus::Cancelled)?;
            continue;
        };

        let completed = match job.kind {
            AiJobKind::Transcription => {
                MeetingRepository::latest_transcription(conn, meeting_id)?.is_some()
            }
            AiJobKind::Summary => MeetingRepository::latest_summary(conn, meeting_id)?.is_some(),
        };

        if completed {
            AiJobRepository::update_status(conn, &job.job_id, AiJobStatus::Completed)?;
            let meeting = MeetingRepository::get_by_id(conn, meeting_id)?;
            if meeting.status == MeetingStatus::Processing {
                MeetingRepository::update_status(conn, meeting_id, MeetingStatus::Completed)?;
            }
        } else {
            AiJobRepository::update_status(conn, &job.job_id, AiJobStatus::Cancelled)?;
            let meeting = MeetingRepository::get_by_id(conn, meeting_id)?;
            if meeting.status == MeetingStatus::Processing {
                // Laisser la réunion en processing pour permettre la reprise.
                MeetingRepository::update_status(conn, meeting_id, MeetingStatus::Processing)?;
            }
        }
    }
    Ok(())
}
