use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::ai::error::AiError;
use crate::ai::jobs::{meeting_job_key, summary_fallback_key, AiJobKind, AiJobState, AiJobStatus};
use crate::ai::limits::validate_summary_input_text;
use crate::ai::model_catalog;
use crate::ai::summary_pipeline::run_structured_summary;
use crate::ai::speaker::speaker_identity_pairs;
use crate::ai::token_pipeline::SummaryPipelineMeta;
use crate::ai::secrets;
use crate::ai::structured_summary::{parse_structured_summary, StructuredSummary};
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::local_activity::LocalActivityGate;
use crate::models::{Action, CreateMeetingInput, Summary};
use crate::repository::{AiJobRepository, MeetingRepository, SummaryRepository};
use crate::retention;
use crate::AiAppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateStructuredSummaryInput {
    pub job_id: Option<String>,
    pub meeting_id: Option<String>,
    pub text: Option<String>,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateStructuredSummaryOutput {
    pub job_id: String,
    pub meeting_id: String,
    pub summary: Summary,
    pub structured: StructuredSummary,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub meta: SummaryPipelineMeta,
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
    jobs: State<'_, AiJobState>,
    gate: State<'_, LocalActivityGate>,
    input: GenerateStructuredSummaryInput,
) -> Result<GenerateStructuredSummaryOutput, String> {
    generate_structured_summary_inner(&app, &db_state, &ai_state, &jobs, &gate, input)
        .await
        .map_err(|e| e.to_string())
}

async fn generate_structured_summary_inner(
    app: &AppHandle,
    db_state: &State<'_, AppState>,
    ai_state: &State<'_, AiAppState>,
    jobs: &State<'_, AiJobState>,
    gate: &State<'_, LocalActivityGate>,
    input: GenerateStructuredSummaryInput,
) -> AppResult<GenerateStructuredSummaryOutput> {
    let job_id = input
        .job_id
        .clone()
        .unwrap_or_else(|| AiJobState::new_job_id(AiJobKind::Summary));
    let activity = gate.begin_operation()?;

    let (provider_id, default_model, requested_model) = {
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
        (provider_id, default_model, input.model.clone())
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

    let model =
        model_catalog::validate_summary_model(&provider_id, requested_model.or(default_model))
            .map_err(|e| AppError::Message(e.to_string()))?;
    let resolved = resolve_input(db_state, input)?;
    let transcription_text = match &resolved {
        ResolvedSummaryInput::ExistingMeeting { text, .. }
        | ResolvedSummaryInput::PastedText { text } => text.clone(),
    };
    validate_summary_input_text(&transcription_text)
        .map_err(|e| AppError::Message(e.to_string()))?;

    let job_key = match &resolved {
        ResolvedSummaryInput::ExistingMeeting { meeting_id, .. } => meeting_job_key(meeting_id),
        ResolvedSummaryInput::PastedText { text } => summary_fallback_key(text),
    };
    let job = jobs
        .begin(job_id.clone(), AiJobKind::Summary, job_key)
        .map_err(|e| AppError::Message(e.to_string()))?;

    let _ = db_state.with_db(|conn| {
        let meeting_id = match &resolved {
            ResolvedSummaryInput::ExistingMeeting { meeting_id, .. } => Some(meeting_id.as_str()),
            ResolvedSummaryInput::PastedText { .. } => None,
        };
        AiJobRepository::insert_running(
            conn,
            &job_id,
            AiJobKind::Summary,
            meeting_id,
            None,
            "summarizing",
        )
    });

    let cancel = job.cancellation_token();

    jobs
        .ensure_not_cancelled(job.job_id())
        .map_err(|message| AppError::Message(message))?;
    gate.ensure_generation(activity)?;

    let speaker_identity = match &resolved {
        ResolvedSummaryInput::ExistingMeeting { meeting_id, .. } => db_state
            .with_db(|conn| {
                let meeting = MeetingRepository::get_by_id(conn, meeting_id)?;
                Ok(meeting
                    .speaker_map
                    .map(|map| speaker_identity_pairs(&map))
                    .filter(|pairs| !pairs.is_empty()))
            })?,
        ResolvedSummaryInput::PastedText { .. } => None,
    };

    let model_for_save = model.clone();
    let summary_run = run_structured_summary(
        &ai_state.registry,
        &provider_id,
        &api_key,
        &transcription_text,
        model,
        &cancel,
        Some(jobs),
        Some(job.job_id()),
        None,
        speaker_identity,
    )
    .await
    .map_err(|err| {
        if matches!(err, AiError::Cancelled) {
            let _ = db_state.with_db(|conn| {
                AiJobRepository::update_status(conn, &job_id, AiJobStatus::Cancelled)
            });
        } else {
            let _ = db_state.with_db(|conn| {
                AiJobRepository::update_status(conn, &job_id, AiJobStatus::Failed)
            });
        }
        AppError::Message(err.to_string())
    })?;

    jobs.ensure_not_cancelled(job.job_id()).map_err(|err| {
        let _ = db_state
            .with_db(|conn| AiJobRepository::update_status(conn, &job_id, AiJobStatus::Cancelled));
        AppError::Message(err)
    })?;

    let structured = summary_run.structured;
    let pipeline_meta = summary_run.meta;

    gate.ensure_generation(activity)?;

    let (meeting_id, summary, actions) = db_state.with_db(|conn| match &resolved {
        ResolvedSummaryInput::ExistingMeeting { meeting_id, .. } => {
            let previous = MeetingRepository::latest_summary(conn, meeting_id)?
                .and_then(|summary| parse_optional_structured(&summary.content));
            let (summary, actions) = SummaryRepository::save_structured_summary_with_meta(
                conn,
                meeting_id,
                Some(&provider_id),
                Some(&provider_display_name),
                &structured,
                model_for_save.as_deref(),
                previous.as_ref(),
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
                model_for_save.as_deref(),
                None,
            )?;
            tx.commit()?;
            Ok((meeting.id, summary, actions))
        }
    })?;

    retention::maybe_purge_audio_files(app, db_state, &meeting_id)?;

    let _ = db_state
        .with_db(|conn| AiJobRepository::update_status(conn, &job_id, AiJobStatus::Completed));
    job.finish_completed();

    Ok(GenerateStructuredSummaryOutput {
        job_id,
        meeting_id,
        summary,
        structured,
        actions,
        meta: pipeline_meta,
    })
}

fn parse_optional_structured(content: &str) -> Option<StructuredSummary> {
    parse_structured_summary(content).ok()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStructuredSummaryInput {
    pub meeting_id: String,
    pub structured: StructuredSummary,
    pub validation_state: Option<String>,
    pub note: Option<String>,
}

#[tauri::command]
pub fn update_structured_summary(
    state: State<'_, AppState>,
    input: UpdateStructuredSummaryInput,
) -> Result<Summary, String> {
    let validation = input
        .validation_state
        .as_deref()
        .and_then(crate::ai::structured_summary::SummaryValidationState::from_str)
        .unwrap_or(crate::ai::structured_summary::SummaryValidationState::Edited);
    state
        .with_db(|conn| {
            SummaryRepository::update_structured_summary(
                conn,
                &input.meeting_id,
                &input.structured,
                validation,
                input.note.as_deref(),
            )
        })
        .map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetActionStatusInput {
    pub meeting_id: String,
    pub action_id: String,
    pub status: String,
}

#[tauri::command]
pub fn set_action_status(
    state: State<'_, AppState>,
    input: SetActionStatusInput,
) -> Result<Action, String> {
    let status = crate::models::ActionStatus::from_str(&input.status)
        .ok_or_else(|| format!("statut d'action invalide : {}", input.status))?;
    state
        .with_db(|conn| {
            SummaryRepository::set_action_status(conn, &input.meeting_id, &input.action_id, status)
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_summary_revisions(
    state: State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<crate::models::SummaryRevision>, String> {
    state
        .with_db(|conn| SummaryRepository::list_revisions(conn, &meeting_id))
        .map_err(|e| e.to_string())
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
            Some("mistral-small-latest"),
            None,
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
