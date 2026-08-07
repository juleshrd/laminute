use std::path::Path;
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::limits::validate_transcription_audio_size;
use crate::ai::models::TranscriptionOptions;
use crate::ai::secrets;
use crate::audio::paths::{ingest_if_needed, resolve_owned, ManagedAudioRoots};
use crate::db::AppState;
use crate::models::{CreateMeetingInput, MeetingStatus, Transcription};
use crate::repository::MeetingRepository;
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
    pub phase: TranscriptionPhase,
    pub message: String,
    pub meeting_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeAudioInput {
    pub file_path: String,
    pub meeting_id: Option<String>,
    pub meeting_title: Option<String>,
    pub language: Option<String>,
    pub duration_ms: Option<i64>,
}

pub struct TranscriptionState {
    pub last_progress: Mutex<TranscriptionProgress>,
}

impl TranscriptionState {
    pub fn new() -> Self {
        Self {
            last_progress: Mutex::new(TranscriptionProgress {
                phase: TranscriptionPhase::Idle,
                message: "En attente.".to_string(),
                meeting_id: None,
            }),
        }
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
    progress: TranscriptionProgress,
) {
    if let Ok(mut last) = transcription_state.last_progress.lock() {
        *last = progress.clone();
    }
    let _ = app.emit(TRANSCRIPTION_PROGRESS_EVENT, &progress);
}

#[tauri::command]
pub fn get_transcription_progress(
    state: State<'_, TranscriptionState>,
) -> Result<TranscriptionProgress, String> {
    state
        .last_progress
        .lock()
        .map(|progress| progress.clone())
        .map_err(|_| "verrou transcription indisponible".to_string())
}

#[tauri::command]
pub async fn transcribe_audio_file(
    app: AppHandle,
    ai_state: State<'_, AiAppState>,
    db_state: State<'_, AppState>,
    transcription_state: State<'_, TranscriptionState>,
    input: TranscribeAudioInput,
) -> Result<Transcription, String> {
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
        TranscriptionProgress {
            phase: TranscriptionPhase::Preparing,
            message: "Préparation de la transcription…".to_string(),
            meeting_id: input.meeting_id.clone(),
        },
    );

    let roots = ManagedAudioRoots::from_app(&app).map_err(|err| {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
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
            TranscriptionProgress {
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
            TranscriptionProgress {
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
            TranscriptionProgress {
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
        TranscriptionProgress {
            phase: TranscriptionPhase::Preparing,
            message: "Enregistrement de l'audio…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    let audio_file = db_state
        .with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Processing)?;
            MeetingRepository::attach_audio_file(
                conn,
                &meeting_id,
                &owned_path_str,
                input.duration_ms,
                format.as_deref(),
            )
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
            TranscriptionProgress {
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
        TranscriptionProgress {
            phase: TranscriptionPhase::Uploading,
            message: format!("Envoi de l'audio à {provider_display_name}…"),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    let result = ai_state
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
        )
        .await
        .map_err(|err| {
            let message = err.to_string();
            let _ = db_state.with_db(|conn| {
                MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
            });
            emit_progress(
                &app,
                &transcription_state,
                TranscriptionProgress {
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
        TranscriptionProgress {
            phase: TranscriptionPhase::Saving,
            message: "Enregistrement de la transcription…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

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
        TranscriptionProgress {
            phase: TranscriptionPhase::Completed,
            message: "Transcription terminée.".to_string(),
            meeting_id: Some(meeting_id),
        },
    );

    Ok(transcription)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn default_progress_is_idle() {
        let state = TranscriptionState::new();
        let progress = state.last_progress.lock().expect("lock");
        assert!(matches!(progress.phase, TranscriptionPhase::Idle));
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
