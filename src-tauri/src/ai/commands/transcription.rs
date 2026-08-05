use std::path::Path;
use std::sync::Mutex;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::models::TranscriptionOptions;
use crate::ai::secrets;
use crate::db::AppState;
use crate::models::{CreateMeetingInput, MeetingStatus, Transcription};
use crate::repository::MeetingRepository;
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
    let provider_id = ai_state
        .settings
        .lock()
        .map_err(|_| "verrou des réglages indisponible".to_string())?
        .selected_provider_id()
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_PROVIDER_ID.to_string());

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

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            phase: TranscriptionPhase::Preparing,
            message: "Préparation de la transcription…".to_string(),
            meeting_id: input.meeting_id.clone(),
        },
    );

    let path = Path::new(&input.file_path);
    if !path.exists() {
        let message = format!("Fichier audio introuvable : {}", input.file_path);
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

    let audio_bytes = std::fs::read(path).map_err(|err| {
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

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_string);

    let meeting_id = {
        let db = db_state
            .db
            .lock()
            .map_err(|_| "impossible d'accéder à la base de données".to_string())?;

        if let Some(existing_id) = &input.meeting_id {
            MeetingRepository::get_by_id(&db, existing_id).map_err(|e| e.to_string())?;
            existing_id.clone()
        } else {
            let title = input.meeting_title.clone().unwrap_or_else(|| {
                Local::now()
                    .format("Enregistrement %d/%m/%Y %H:%M")
                    .to_string()
            });
            let meeting = MeetingRepository::create(
                &db,
                CreateMeetingInput {
                    title,
                    description: None,
                },
            )
            .map_err(|e| e.to_string())?;
            meeting.id
        }
    };

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            phase: TranscriptionPhase::Preparing,
            message: "Enregistrement de l'audio…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    let audio_file = {
        let db = db_state
            .db
            .lock()
            .map_err(|_| "impossible d'accéder à la base de données".to_string())?;

        MeetingRepository::update_status(&db, &meeting_id, MeetingStatus::Processing)
            .map_err(|e| e.to_string())?;

        MeetingRepository::attach_audio_file(
            &db,
            &meeting_id,
            &input.file_path,
            input.duration_ms,
            format.as_deref(),
        )
        .map_err(|e| e.to_string())?
    };

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
            &audio_bytes,
            TranscriptionOptions {
                model: None,
                language: input.language.clone(),
                file_name,
            },
        )
        .await
        .map_err(|err| {
            let message = err.to_string();
            if let Ok(db) = db_state.db.lock() {
                let _ = MeetingRepository::update_status(&db, &meeting_id, MeetingStatus::Draft);
            }
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

    let transcription = {
        let db = db_state
            .db
            .lock()
            .map_err(|_| "impossible d'accéder à la base de données".to_string())?;

        let saved = MeetingRepository::create_transcription(
            &db,
            &meeting_id,
            Some(&audio_file.id),
            &provider_id,
            &provider_display_name,
            &result.text,
            result.language.as_deref(),
        )
        .map_err(|e| e.to_string())?;

        MeetingRepository::update_status(&db, &meeting_id, MeetingStatus::Completed)
            .map_err(|e| e.to_string())?;

        saved
    };

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
