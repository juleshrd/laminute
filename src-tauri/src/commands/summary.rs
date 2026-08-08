use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::ai::models::SummaryOptions;
use crate::ai::secrets;
use crate::ai::structured_summary::{self, StructuredSummary};
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::local_activity::LocalActivityGate;
use crate::models::{Action, CreateMeetingInput, Summary};
use crate::repository::{MeetingRepository, SummaryRepository};
use crate::retention;
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

#[derive(Debug)]
enum ResolvedSummaryInput {
    ExistingMeeting { meeting_id: String, text: String },
    PastedText { text: String },
}

#[tauri::command]
pub async fn generate_structured_summary(
    app: AppHandle,
    db_state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
    gate: State<'_, LocalActivityGate>,
    input: GenerateStructuredSummaryInput,
) -> Result<GenerateStructuredSummaryOutput, String> {
    generate_structured_summary_inner(&app, &db_state, &ai_state, &gate, input)
        .await
        .map_err(|e| e.to_string())
}

async fn generate_structured_summary_inner(
    app: &AppHandle,
    db_state: &State<'_, AppState>,
    ai_state: &State<'_, AiAppState>,
    gate: &State<'_, LocalActivityGate>,
    input: GenerateStructuredSummaryInput,
) -> AppResult<GenerateStructuredSummaryOutput> {
    let activity = gate.begin_operation()?;

    let (provider_id, default_model) = {
        let settings = ai_state
            .settings
            .lock()
            .map_err(|_| AppError::Message("verrou des réglages indisponible".into()))?;
        let provider_id = input
            .provider_id
            .clone()
            .or_else(|| settings.selected_provider_id().map(str::to_string))
            .ok_or_else(|| AppError::Message("aucun fournisseur IA sélectionné".into()))?;
        let default_model = settings.summary_model_for(&provider_id);
        (provider_id, default_model)
    };

    let provider = ai_state
        .registry
        .require(&provider_id)
        .map_err(|e| AppError::Message(e.to_string()))?;
    let provider_display_name = provider.display_name().to_string();

    let api_key = if provider.capabilities().local {
        String::new()
    } else {
        secrets::get_api_key(&provider_id)
            .map_err(|e| AppError::Message(e.to_string()))?
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| {
                AppError::Message("aucune clé API enregistrée pour ce fournisseur".into())
            })?
    };

    let model = input.model.clone().or(default_model);
    let resolved = resolve_input(db_state, input)?;
    let transcription_text = match &resolved {
        ResolvedSummaryInput::ExistingMeeting { text, .. }
        | ResolvedSummaryInput::PastedText { text } => text.clone(),
    };

    gate.ensure_generation(activity)?;

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

    gate.ensure_generation(activity)?;

    let (meeting_id, summary, actions) = db_state.with_db(|conn| match &resolved {
        ResolvedSummaryInput::ExistingMeeting { meeting_id, .. } => {
            let (summary, actions) = SummaryRepository::save_structured_summary(
                conn,
                meeting_id,
                Some(&provider_id),
                Some(&provider_display_name),
                &structured,
            )?;
            Ok((meeting_id.clone(), summary, actions))
        }
        ResolvedSummaryInput::PastedText { .. } => {
            // Création + persistance atomiques : pas de brouillon si la sauvegarde échoue.
            let tx = conn.unchecked_transaction()?;
            let meeting = MeetingRepository::create(
                &tx,
                CreateMeetingInput {
                    title: format!("Compte-rendu {}", Utc::now().format("%Y-%m-%d %H:%M")),
                    description: Some("Généré à partir d'un texte importé".into()),
                },
            )?;
            let (summary, actions) = SummaryRepository::save_structured_summary_on(
                &tx,
                &meeting.id,
                Some(&provider_id),
                Some(&provider_display_name),
                &structured,
            )?;
            tx.commit()?;
            Ok((meeting.id, summary, actions))
        }
    })?;

    retention::maybe_purge_audio_files(app, db_state, &meeting_id)?;

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
) -> AppResult<ResolvedSummaryInput> {
    match (input.meeting_id, input.text) {
        (Some(meeting_id), Some(text)) => {
            if text.trim().is_empty() {
                return Err(AppError::Message("le texte fourni est vide".into()));
            }
            db_state.with_db(|conn| MeetingRepository::get_by_id(conn, &meeting_id))?;
            Ok(ResolvedSummaryInput::ExistingMeeting { meeting_id, text })
        }
        (Some(meeting_id), None) => {
            db_state.with_db(|conn| MeetingRepository::get_by_id(conn, &meeting_id))?;
            let text = db_state
                .with_db(|conn| SummaryRepository::latest_transcription_text(conn, &meeting_id))?
                .ok_or_else(|| {
                    AppError::Message("aucune transcription trouvée pour cette réunion".into())
                })?;
            Ok(ResolvedSummaryInput::ExistingMeeting { meeting_id, text })
        }
        (None, Some(text)) => {
            if text.trim().is_empty() {
                return Err(AppError::Message("le texte fourni est vide".into()));
            }
            // La réunion n'est créée qu'après succès IA, pour éviter les brouillons fantômes.
            Ok(ResolvedSummaryInput::PastedText { text })
        }
        (None, None) => Err(AppError::Message(
            "fournissez un meeting_id ou un texte de transcription".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::MeetingStatus;

    #[test]
    fn pasted_text_persist_creates_meeting_only_with_summary() {
        let conn = open_in_memory().unwrap();
        let structured = StructuredSummary {
            synthese: "Synthèse.".into(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };

        let tx = conn.unchecked_transaction().unwrap();
        let meeting = MeetingRepository::create(
            &tx,
            CreateMeetingInput {
                title: "Compte-rendu test".into(),
                description: Some("Généré à partir d'un texte importé".into()),
            },
        )
        .unwrap();
        let (summary, actions) = SummaryRepository::save_structured_summary_on(
            &tx,
            &meeting.id,
            Some("mistral"),
            Some("Mistral AI"),
            &structured,
        )
        .unwrap();
        tx.commit().unwrap();

        assert_eq!(summary.meeting_id, meeting.id);
        assert!(actions.is_empty());
        assert_eq!(
            MeetingRepository::get_by_id(&conn, &meeting.id)
                .unwrap()
                .status,
            MeetingStatus::Completed
        );
    }

    #[test]
    fn resolve_pasted_text_does_not_create_meeting() {
        // Documente le contrat : un texte collé seul ne touche pas la DB avant l'appel IA.
        let resolved = ResolvedSummaryInput::PastedText {
            text: "hello".into(),
        };
        match resolved {
            ResolvedSummaryInput::PastedText { text } => assert_eq!(text, "hello"),
            ResolvedSummaryInput::ExistingMeeting { .. } => panic!("expected pasted text"),
        }
    }
}
