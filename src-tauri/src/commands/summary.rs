use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ai::models::SummaryOptions;
use crate::ai::secrets;
use crate::ai::structured_summary::{self, StructuredSummary};
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{Action, CreateMeetingInput, Summary};
use crate::repository::{MeetingRepository, SummaryRepository};
use crate::AiAppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateStructuredSummaryInput {
    pub meeting_id: Option<String>,
    pub text: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateStructuredSummaryOutput {
    pub meeting_id: String,
    pub summary: Summary,
    pub structured: StructuredSummary,
    pub actions: Vec<Action>,
}

#[tauri::command]
pub async fn generate_structured_summary(
    db_state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
    input: GenerateStructuredSummaryInput,
) -> Result<GenerateStructuredSummaryOutput, String> {
    generate_structured_summary_inner(&db_state, &ai_state, input)
        .await
        .map_err(|e| e.to_string())
}

async fn generate_structured_summary_inner(
    db_state: &State<'_, AppState>,
    ai_state: &State<'_, AiAppState>,
    input: GenerateStructuredSummaryInput,
) -> AppResult<GenerateStructuredSummaryOutput> {
    let provider_id = {
        let settings = ai_state
            .settings
            .lock()
            .map_err(|_| AppError::Message("verrou des réglages indisponible".into()))?;
        input
            .provider_id
            .clone()
            .or_else(|| settings.selected_provider_id().map(str::to_string))
            .ok_or_else(|| AppError::Message("aucun fournisseur IA sélectionné".into()))?
    };

    ai_state
        .registry
        .require(&provider_id)
        .map_err(|e| AppError::Message(e.to_string()))?;

    let api_key = secrets::get_api_key(&provider_id)
        .map_err(|e| AppError::Message(e.to_string()))?
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| AppError::Message("aucune clé API enregistrée pour ce fournisseur".into()))?;

    let model = input.model.clone();
    let (meeting_id, transcription_text) = resolve_input(db_state, input)?;

    let summary_result = ai_state
        .registry
        .summarize_text(
            &provider_id,
            &api_key,
            &transcription_text,
            SummaryOptions {
                model,
                max_tokens: Some(4096),
            },
        )
        .await
        .map_err(|e| AppError::Message(e.to_string()))?;

    let structured = structured_summary::parse_structured_summary(&summary_result.text)
        .map_err(|e| AppError::Message(e.to_string()))?;

    let (summary, actions) = with_db(db_state, |conn| {
        SummaryRepository::save_structured_summary(
            conn,
            &meeting_id,
            Some(&provider_id),
            &structured,
        )
    })?;

    Ok(GenerateStructuredSummaryOutput {
        meeting_id,
        summary,
        structured,
        actions,
    })
}

fn resolve_input(
    db_state: &State<'_, AppState>,
    input: GenerateStructuredSummaryInput,
) -> AppResult<(String, String)> {
    match (input.meeting_id, input.text) {
        (Some(meeting_id), Some(text)) => {
            if text.trim().is_empty() {
                return Err(AppError::Message("le texte fourni est vide".into()));
            }
            with_db(db_state, |conn| MeetingRepository::get_by_id(conn, &meeting_id))?;
            Ok((meeting_id, text))
        }
        (Some(meeting_id), None) => {
            with_db(db_state, |conn| MeetingRepository::get_by_id(conn, &meeting_id))?;
            let text = with_db(db_state, |conn| {
                SummaryRepository::latest_transcription_text(conn, &meeting_id)
            })?
            .ok_or_else(|| {
                AppError::Message("aucune transcription trouvée pour cette réunion".into())
            })?;
            Ok((meeting_id, text))
        }
        (None, Some(text)) => {
            if text.trim().is_empty() {
                return Err(AppError::Message("le texte fourni est vide".into()));
            }
            let meeting = with_db(db_state, |conn| {
                MeetingRepository::create(
                    conn,
                    CreateMeetingInput {
                        title: format!(
                            "Compte-rendu {}",
                            Utc::now().format("%Y-%m-%d %H:%M")
                        ),
                        description: Some("Généré à partir d'un texte importé".into()),
                    },
                )
            })?;
            Ok((meeting.id, text))
        }
        (None, None) => Err(AppError::Message(
            "fournissez un meeting_id ou un texte de transcription".into(),
        )),
    }
}

fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> AppResult<T>
where
    F: FnOnce(&rusqlite::Connection) -> AppResult<T>,
{
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Message("impossible d'accéder à la base de données".into()))?;
    f(&db)
}
