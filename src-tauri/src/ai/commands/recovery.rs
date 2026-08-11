use serde::Serialize;
use tauri::State;

use crate::ai::commands::transcription::{transcribe_audio_file, TranscribeAudioInput};
use crate::ai::jobs::{AiJobKind, AiJobStatus};
use crate::commands::{
    generate_structured_summary, GenerateStructuredSummaryInput, GenerateStructuredSummaryOutput,
};
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::models::MeetingStatus;
use crate::repository::{AiJobRepository, MeetingRepository, SummaryRepository};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRecoveryActions {
    pub meeting_id: String,
    pub can_resume_transcription: bool,
    pub can_retry_summary: bool,
    pub audio_file_path: Option<String>,
}

fn compute_recovery_actions(conn: &rusqlite::Connection, meeting_id: &str) -> AppResult<AiRecoveryActions> {
    let detail = MeetingRepository::get_detail(conn, meeting_id)?;
    let has_transcription = !detail.transcriptions.is_empty();
    let has_summary = !detail.summaries.is_empty();
    let has_audio = !detail.audio_files.is_empty();
    let recoverable = detail.meeting.status == MeetingStatus::Processing;

    let interrupted_transcription = AiJobRepository::latest_for_meeting(
        conn,
        meeting_id,
        AiJobKind::Transcription,
    )?
    .is_some_and(|job| job.status == AiJobStatus::Cancelled);

    let interrupted_summary = AiJobRepository::latest_for_meeting(
        conn,
        meeting_id,
        AiJobKind::Summary,
    )?
    .is_some_and(|job| job.status == AiJobStatus::Cancelled);

    let can_resume_transcription =
        has_audio && !has_transcription && (recoverable || interrupted_transcription);
    let can_retry_summary =
        has_transcription && !has_summary && (recoverable || interrupted_summary);

    Ok(AiRecoveryActions {
        meeting_id: meeting_id.to_string(),
        can_resume_transcription,
        can_retry_summary,
        audio_file_path: detail.audio_files.first().map(|file| file.file_path.clone()),
    })
}

#[tauri::command]
pub fn get_ai_recovery_actions(
    db_state: State<'_, AppState>,
    meeting_id: String,
) -> Result<AiRecoveryActions, String> {
    db_state
        .with_db(|conn| compute_recovery_actions(conn, &meeting_id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_transcription_for_meeting(
    app: tauri::AppHandle,
    ai_state: State<'_, crate::AiAppState>,
    db_state: State<'_, AppState>,
    transcription_state: State<'_, crate::ai::TranscriptionState>,
    jobs: State<'_, crate::ai::jobs::AiJobState>,
    gate: State<'_, crate::local_activity::LocalActivityGate>,
    meeting_id: String,
) -> Result<crate::ai::commands::transcription::TranscribeAudioOutput, String> {
    let input = db_state
        .with_db(|conn| {
            let actions = compute_recovery_actions(conn, &meeting_id)?;
            if !actions.can_resume_transcription {
                return Err(AppError::Message(
                    "aucune transcription à reprendre pour cette réunion".into(),
                ));
            }
            let file_path = actions.audio_file_path.ok_or_else(|| {
                AppError::Message("fichier audio introuvable pour cette réunion".into())
            })?;
            let audio = MeetingRepository::list_audio_files(conn, &meeting_id)?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Message("fichier audio introuvable".into()))?;
            Ok(TranscribeAudioInput {
                job_id: None,
                file_path,
                meeting_id: Some(meeting_id.clone()),
                meeting_title: None,
                language: None,
                duration_ms: audio.duration_ms,
            })
        })
        .map_err(|e| e.to_string())?;

    transcribe_audio_file(
        app,
        ai_state,
        db_state,
        transcription_state,
        jobs,
        gate,
        input,
    )
    .await
}

#[tauri::command]
pub async fn resume_summary_for_meeting(
    app: tauri::AppHandle,
    db_state: State<'_, AppState>,
    ai_state: State<'_, crate::AiAppState>,
    jobs: State<'_, crate::ai::jobs::AiJobState>,
    gate: State<'_, crate::local_activity::LocalActivityGate>,
    meeting_id: String,
) -> Result<GenerateStructuredSummaryOutput, String> {
    db_state
        .with_db(|conn| {
            let actions = compute_recovery_actions(conn, &meeting_id)?;
            if !actions.can_retry_summary {
                return Err(AppError::Message(
                    "aucun compte-rendu à relancer pour cette réunion".into(),
                ));
            }
            SummaryRepository::latest_transcription_text(conn, &meeting_id)?
                .ok_or_else(|| AppError::Message("aucune transcription trouvée".into()))?;
            Ok(())
        })
        .map_err(|e| e.to_string())?;

    generate_structured_summary(
        app,
        db_state,
        ai_state,
        jobs,
        gate,
        GenerateStructuredSummaryInput {
            job_id: None,
            meeting_id: Some(meeting_id),
            text: None,
            provider_id: None,
            model: None,
        },
    )
    .await
}
