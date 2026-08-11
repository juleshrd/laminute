use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::error::AiError;
use crate::ai::jobs::{
    meeting_job_key, transcription_fallback_key, AiJobKind, AiJobState, AiJobStatus,
    CancelAiJobOutput,
};
use crate::ai::limits::validate_transcription_audio_size;
use crate::ai::model_catalog;
use crate::ai::models::TranscriptionOptions;
use crate::ai::secrets;
use crate::audio::paths::{ingest_if_needed, resolve_owned, ManagedAudioRoots};
use crate::db::AppState;
use crate::local_activity::LocalActivityGate;
use crate::models::{CreateMeetingInput, MeetingStatus, Transcription};
use crate::repository::{AiJobRepository, MeetingRepository};
use crate::retention;
use crate::AiAppState;

const DEFAULT_PROVIDER_ID: &str = "mistral";
const TRANSCRIPTION_PROGRESS_EVENT: &str = "transcription-progress";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionPhase {
    Idle,
    Preparing,
    Uploading,
    Transcribing,
    Saving,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProgress {
    pub job_id: String,
    pub phase: TranscriptionPhase,
    pub message: String,
    pub meeting_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeAudioInput {
    pub job_id: Option<String>,
    pub file_path: String,
    pub meeting_id: Option<String>,
    pub meeting_title: Option<String>,
    pub language: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeAudioOutput {
    pub job_id: String,
    pub transcription: Transcription,
}

pub struct TranscriptionState {
    pub progress_by_job: Mutex<HashMap<String, TranscriptionProgress>>,
    pub latest_job_id: Mutex<Option<String>>,
}

impl TranscriptionState {
    pub fn new() -> Self {
        Self {
            progress_by_job: Mutex::new(HashMap::new()),
            latest_job_id: Mutex::new(None),
        }
    }

    pub fn reset(&self) -> Result<(), String> {
        self.progress_by_job
            .lock()
            .map_err(|_| "verrou transcription indisponible".to_string())?
            .clear();
        *self
            .latest_job_id
            .lock()
            .map_err(|_| "verrou transcription indisponible".to_string())? = None;
        Ok(())
    }
}

impl Default for TranscriptionState {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_progress(
    app: &AppHandle,
    transcription_state: &TranscriptionState,
    db_state: Option<&AppState>,
    progress: TranscriptionProgress,
) {
    if let Some(db_state) = db_state {
        persist_job_phase(db_state, &progress.job_id, &progress.phase);
    }
    if let Ok(mut by_job) = transcription_state.progress_by_job.lock() {
        by_job.insert(progress.job_id.clone(), progress.clone());
    }
    if let Ok(mut latest_job_id) = transcription_state.latest_job_id.lock() {
        *latest_job_id = Some(progress.job_id.clone());
    }
    let _ = app.emit(TRANSCRIPTION_PROGRESS_EVENT, &progress);
}

#[tauri::command]
pub fn get_transcription_progress(
    state: State<'_, TranscriptionState>,
    job_id: Option<String>,
) -> Result<Option<TranscriptionProgress>, String> {
    get_transcription_progress_for_job(&state, job_id)
}

fn phase_to_str(phase: &TranscriptionPhase) -> &'static str {
    match phase {
        TranscriptionPhase::Idle => "idle",
        TranscriptionPhase::Preparing => "preparing",
        TranscriptionPhase::Uploading => "uploading",
        TranscriptionPhase::Transcribing => "transcribing",
        TranscriptionPhase::Saving => "saving",
        TranscriptionPhase::Completed => "completed",
        TranscriptionPhase::Failed => "failed",
    }
}

fn persist_job_phase(
    db_state: &AppState,
    job_id: &str,
    phase: &TranscriptionPhase,
) {
    let _ = db_state.with_db(|conn| {
        AiJobRepository::update_phase(conn, job_id, phase_to_str(phase))
    });
}

fn persist_job_status(db_state: &AppState, job_id: &str, status: AiJobStatus) {
    let _ = db_state.with_db(|conn| AiJobRepository::update_status(conn, job_id, status));
}

#[tauri::command]
pub fn cancel_ai_job(
    jobs: State<'_, AiJobState>,
    db_state: State<'_, AppState>,
    job_id: String,
) -> Result<CancelAiJobOutput, String> {
    let output = jobs.cancel(&job_id)?;
    if output.cancelled {
        persist_job_status(&db_state, &job_id, AiJobStatus::Cancelled);
    }
    Ok(output)
}

fn get_transcription_progress_for_job(
    state: &TranscriptionState,
    job_id: Option<String>,
) -> Result<Option<TranscriptionProgress>, String> {
    let progress_by_job = state
        .progress_by_job
        .lock()
        .map_err(|_| "verrou transcription indisponible".to_string())?;

    if let Some(job_id) = job_id {
        return Ok(progress_by_job.get(&job_id).cloned());
    }

    let latest_job_id = state
        .latest_job_id
        .lock()
        .map_err(|_| "verrou transcription indisponible".to_string())?;
    Ok(latest_job_id
        .as_deref()
        .and_then(|id| progress_by_job.get(id))
        .cloned())
}

#[tauri::command]
pub async fn transcribe_audio_file(
    app: AppHandle,
    ai_state: State<'_, AiAppState>,
    db_state: State<'_, AppState>,
    transcription_state: State<'_, TranscriptionState>,
    jobs: State<'_, AiJobState>,
    gate: State<'_, LocalActivityGate>,
    input: TranscribeAudioInput,
) -> Result<TranscribeAudioOutput, String> {
    let job_id = input
        .job_id
        .clone()
        .unwrap_or_else(|| AiJobState::new_job_id(AiJobKind::Transcription));
    let job_key = input
        .meeting_id
        .as_deref()
        .map(meeting_job_key)
        .unwrap_or_else(|| transcription_fallback_key(&input.file_path));
    let job = jobs
        .begin(job_id.clone(), AiJobKind::Transcription, job_key)
        .map_err(|err| err.to_string())?;

    let _ = db_state.with_db(|conn| {
        AiJobRepository::insert_running(
            conn,
            &job_id,
            AiJobKind::Transcription,
            input.meeting_id.as_deref(),
            None,
            "preparing",
        )
    });

    let cancel = job.cancellation_token();
    let activity = gate.begin_operation().map_err(|e| e.to_string())?;

    let (provider_id, transcription_model, diarize) = {
        let settings = ai_state
            .settings
            .lock()
            .map_err(|_| "verrou des réglages indisponible".to_string())?;
        let provider_id = settings
            .selected_provider_id()
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_PROVIDER_ID.to_string());
        let transcription_model = settings.transcription_model_for(&provider_id);
        let diarize = settings.diarization_enabled();
        let transcription_model =
            model_catalog::validate_transcription_model(&provider_id, transcription_model)
                .map_err(|e| e.to_string())?;
        (provider_id, transcription_model, diarize)
    };

    let provider = ai_state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    if !provider.capabilities().transcription {
        return Err(format!(
            "Le fournisseur « {} » ne prend pas en charge la transcription audio. Choisissez OpenAI ou Mistral dans les réglages IA.",
            provider.display_name()
        ));
    }

    let api_key = secrets::get_api_key(&provider_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "Aucune clé API enregistrée — configurez {} dans les réglages.",
                provider.display_name()
            )
        })?;

    let provider_display_name = provider.display_name().to_string();
    let diarize = diarize && provider.capabilities().diarization;

    emit_progress(
        &app,
        &transcription_state,
        Some(&db_state),
        TranscriptionProgress {
            job_id: job.job_id().to_string(),
            phase: TranscriptionPhase::Preparing,
            message: "Préparation de la transcription…".to_string(),
            meeting_id: input.meeting_id.clone(),
        },
    );

    jobs.ensure_not_cancelled(job.job_id())?;
    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let roots = ManagedAudioRoots::from_app(&app).map_err(|err| {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    // Copie éventuelle hors verrou SQLite ; seul un chemin owned sera uploadé.
    let owned_path = ingest_if_needed(Path::new(&input.file_path), &roots).map_err(|err| {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    let metadata = std::fs::metadata(&owned_path).map_err(|err| {
        let message = format!("Impossible de lire le fichier audio : {err}");
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    if let Err(err) = validate_transcription_audio_size(metadata.len()) {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        return Err(message);
    }

    let file_name = owned_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let format = owned_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_string);
    let owned_path_str = owned_path.to_string_lossy().to_string();

    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let meeting_id = db_state
        .with_db(|conn| {
            if let Some(existing_id) = &input.meeting_id {
                MeetingRepository::get_by_id(conn, existing_id)?;
                Ok(existing_id.clone())
            } else {
                let title = input.meeting_title.clone().unwrap_or_else(|| {
                    Local::now()
                        .format("Enregistrement %d/%m/%Y %H:%M")
                        .to_string()
                });
                let meeting = MeetingRepository::create(
                    conn,
                    CreateMeetingInput {
                        title,
                        description: None,
                    },
                )?;
                Ok(meeting.id)
            }
        })
        .map_err(|e| e.to_string())?;

    emit_progress(
        &app,
        &transcription_state,
        Some(&db_state),
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Preparing,
            message: "Enregistrement de l'audio…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let audio_file = db_state
        .with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Processing)?;
            let audio_file = MeetingRepository::attach_audio_file(
                conn,
                &meeting_id,
                &owned_path_str,
                input.duration_ms,
                format.as_deref(),
            )?;
            AiJobRepository::update_audio_file_id(conn, &job_id, &audio_file.id)?;
            Ok(audio_file)
        })
        .map_err(|e| e.to_string())?;

    // Re-vérification anti-TOCTOU juste avant l'upload cloud.
    let upload_path = resolve_owned(&owned_path, &roots).map_err(|err| {
        let message = err.to_string();
        let _ = db_state.with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
        });
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        message
    })?;

    emit_progress(
        &app,
        &transcription_state,
        Some(&db_state),
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Uploading,
            message: format!("Envoi de l'audio à {provider_display_name}…"),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    jobs.ensure_not_cancelled(job.job_id())?;
    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let transcribe_result = ai_state
        .registry
        .transcribe_audio(
            &provider_id,
            &api_key,
            &upload_path,
            TranscriptionOptions {
                model: transcription_model,
                language: input.language.clone(),
                file_name,
                diarize,
            },
            &cancel,
        )
        .await;

    let result = match transcribe_result {
        Ok(result) => result,
        Err(err) => {
            let cancelled = matches!(err, AiError::Cancelled);
            persist_job_status(
                &db_state,
                &job_id,
                if cancelled {
                    AiJobStatus::Cancelled
                } else {
                    AiJobStatus::Failed
                },
            );
            let message = err.to_string();
            let _ = db_state.with_db(|conn| {
                MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
            });
            emit_progress(
                &app,
                &transcription_state,
                Some(&db_state),
                TranscriptionProgress {
                    job_id: job_id.clone(),
                    phase: TranscriptionPhase::Failed,
                    message: message.clone(),
                    meeting_id: Some(meeting_id.clone()),
                },
            );
            if cancelled {
                job.finish_cancelled();
            }
            return Err(message);
        }
    };

    if let Err(message) = jobs.ensure_not_cancelled(job.job_id()) {
        persist_job_status(&db_state, &job_id, AiJobStatus::Cancelled);
        let _ = db_state.with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
        });
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        job.finish_cancelled();
        return Err(message);
    }

    emit_progress(
        &app,
        &transcription_state,
        Some(&db_state),
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Saving,
            message: "Enregistrement de la transcription…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    gate.ensure_generation(activity).map_err(|e| {
        emit_progress(
            &app,
            &transcription_state,
            Some(&db_state),
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: e.to_string(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        e.to_string()
    })?;

    let transcription = db_state
        .with_db(|conn| {
            let saved = MeetingRepository::create_transcription(
                conn,
                &meeting_id,
                Some(&audio_file.id),
                &provider_id,
                &provider_display_name,
                &result.text,
                result.language.as_deref(),
            )?;
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Completed)?;
            Ok(saved)
        })
        .map_err(|e| e.to_string())?;

    retention::maybe_purge_audio_files(&app, &db_state, &meeting_id).map_err(|e| e.to_string())?;

    emit_progress(
        &app,
        &transcription_state,
        Some(&db_state),
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Completed,
            message: "Transcription terminée.".to_string(),
            meeting_id: Some(meeting_id),
        },
    );

    job.finish_completed();
    persist_job_status(&db_state, &job_id, AiJobStatus::Completed);

    Ok(TranscribeAudioOutput {
        job_id,
        transcription,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn default_progress_is_idle() {
        let state = TranscriptionState::new();
        let progress = get_transcription_progress_for_job(&state, None).expect("progress");
        assert!(progress.is_none());
    }

    #[test]
    fn repository_persists_transcription_for_meeting() {
        let conn = open_in_memory().expect("db");
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Test".into(),
                description: None,
            },
        )
        .expect("meeting");

        let audio = MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            "/tmp/test.wav",
            Some(1000),
            Some("wav"),
        )
        .expect("audio");

        let transcription = MeetingRepository::create_transcription(
            &conn,
            &meeting.id,
            Some(&audio.id),
            "mistral",
            "Mistral AI",
            "Bonjour",
            Some("fr"),
        )
        .expect("transcription");

        assert_eq!(transcription.content, "Bonjour");
        let detail = MeetingRepository::get_detail(&conn, &meeting.id).expect("detail");
        assert_eq!(detail.transcriptions.len(), 1);
        assert_eq!(detail.audio_files.len(), 1);
    }
}
