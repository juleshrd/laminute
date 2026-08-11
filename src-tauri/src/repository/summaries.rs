use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::ai::speaker::apply_speaker_map_to_structured;
use crate::ai::structured_summary::{
    DecisionEntry, ItemOrigin, StructuredActionItem, StructuredSummary, SummaryValidationState,
};
use crate::error::{AppError, AppResult};
use crate::models::{Action, ActionStatus, MeetingStatus, Summary, SummaryRevision};
use crate::repository::MeetingRepository;

pub struct SummaryRepository;

impl SummaryRepository {
    pub fn latest_transcription_text(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Option<String>> {
        conn.query_row(
            "SELECT content FROM transcriptions
             WHERE meeting_id = ?1
             ORDER BY created_at DESC
             LIMIT 1",
            [meeting_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    /// Persiste le résumé, les actions et le statut `completed` dans une transaction unique.
    /// Si `provider_id` est fourni, garantit la ligne `ai_providers` (FK).
    pub fn save_structured_summary(
        conn: &Connection,
        meeting_id: &str,
        provider_id: Option<&str>,
        provider_display_name: Option<&str>,
        structured: &StructuredSummary,
    ) -> AppResult<(Summary, Vec<Action>)> {
        Self::save_structured_summary_with_meta(
            conn,
            meeting_id,
            provider_id,
            provider_display_name,
            structured,
            None,
            None,
        )
    }

    pub fn save_structured_summary_with_meta(
        conn: &Connection,
        meeting_id: &str,
        provider_id: Option<&str>,
        provider_display_name: Option<&str>,
        structured: &StructuredSummary,
        model: Option<&str>,
        preserve_from: Option<&StructuredSummary>,
    ) -> AppResult<(Summary, Vec<Action>)> {
        let tx = conn.unchecked_transaction()?;
        let result = Self::save_structured_summary_on(
            &tx,
            meeting_id,
            provider_id,
            provider_display_name,
            structured,
            model,
            preserve_from,
        )?;
        tx.commit()?;
        Ok(result)
    }

    /// Variante sans transaction, à utiliser dans une transaction déjà ouverte par l'appelant.
    pub(crate) fn save_structured_summary_on(
        conn: &Connection,
        meeting_id: &str,
        provider_id: Option<&str>,
        provider_display_name: Option<&str>,
        structured: &StructuredSummary,
        model: Option<&str>,
        preserve_from: Option<&StructuredSummary>,
    ) -> AppResult<(Summary, Vec<Action>)> {
        let meeting = MeetingRepository::get_by_id(conn, meeting_id)?;
        let mut structured_to_save = structured.clone();
        if let Some(ref map) = meeting.speaker_map {
            if !map.is_empty() {
                apply_speaker_map_to_structured(&mut structured_to_save, map);
            }
        }
        if let Some(previous) = preserve_from {
            structured_to_save = merge_preserving_locked(previous, &structured_to_save);
        }
        ensure_item_keys(&mut structured_to_save);

        if let Some(provider_id) = provider_id {
            let name = provider_display_name.unwrap_or(provider_id);
            MeetingRepository::ensure_ai_provider(conn, provider_id, name, provider_id)?;
        }

        let now = Utc::now().to_rfc3339();
        let summary_id = Uuid::new_v4().to_string();
        let content = serde_json::to_string(&structured_to_save).map_err(|error| {
            AppError::Message(format!("sérialisation du compte-rendu : {error}"))
        })?;

        conn.execute(
            "INSERT INTO summaries (
                id, meeting_id, provider_id, content, model, validation_state, validated_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
            params![
                summary_id,
                meeting_id,
                provider_id,
                content,
                model,
                SummaryValidationState::Generated.as_str(),
                now,
                now,
            ],
        )?;

        Self::insert_revision(
            conn,
            &summary_id,
            meeting_id,
            &content,
            SummaryValidationState::Generated,
            model,
            provider_id,
            Some("generated"),
            &now,
        )?;

        // Remplace les actions non préservées : on purge celles dont l'item_key n'est pas locked/validated
        // en pratique on recrée depuis le CR fusionné.
        conn.execute("DELETE FROM actions WHERE meeting_id = ?1", [meeting_id])?;

        let mut actions = Vec::with_capacity(structured_to_save.actions.len());
        for item in &structured_to_save.actions {
            let action_id = Uuid::new_v4().to_string();
            let item_key = item
                .id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let sources_json = serde_json::to_string(&item.sources).ok();
            conn.execute(
                "INSERT INTO actions (
                    id, meeting_id, title, description, assignee, due_date, status,
                    item_key, sources_json, origin, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    action_id,
                    meeting_id,
                    item.titre.trim(),
                    item.description,
                    item.responsable,
                    item.echeance,
                    ActionStatus::Pending.as_str(),
                    item_key,
                    sources_json,
                    item.origin.as_str(),
                    now,
                    now,
                ],
            )?;

            actions.push(Action {
                id: action_id,
                meeting_id: meeting_id.to_string(),
                title: item.titre.trim().to_string(),
                description: item.description.clone(),
                assignee: item.responsable.clone(),
                due_date: item.echeance.clone(),
                status: ActionStatus::Pending,
                item_key: Some(item_key),
                sources: item.sources.clone(),
                origin: item.origin,
                created_at: now.clone(),
                updated_at: now.clone(),
            });
        }

        MeetingRepository::update_status(conn, meeting_id, MeetingStatus::Completed)?;

        let summary = Summary {
            id: summary_id,
            meeting_id: meeting_id.to_string(),
            provider_id: provider_id.map(str::to_string),
            content,
            model: model.map(str::to_string),
            validation_state: SummaryValidationState::Generated,
            validated_at: None,
            created_at: now.clone(),
            updated_at: now,
        };

        Ok((summary, actions))
    }

    pub fn update_structured_summary(
        conn: &Connection,
        meeting_id: &str,
        structured: &StructuredSummary,
        validation_state: SummaryValidationState,
        note: Option<&str>,
    ) -> AppResult<Summary> {
        let mut latest = MeetingRepository::latest_summary(conn, meeting_id)?
            .ok_or_else(|| AppError::Message("aucun compte-rendu à mettre à jour".into()))?;

        let mut structured = structured.clone();
        ensure_item_keys(&mut structured);
        structured.validate().map_err(|e| AppError::Message(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let content = serde_json::to_string(&structured).map_err(|error| {
            AppError::Message(format!("sérialisation du compte-rendu : {error}"))
        })?;
        let validated_at = if validation_state == SummaryValidationState::Validated {
            Some(now.clone())
        } else {
            None
        };

        conn.execute(
            "UPDATE summaries
             SET content = ?1, validation_state = ?2, validated_at = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                content,
                validation_state.as_str(),
                validated_at,
                now,
                latest.id
            ],
        )?;

        Self::insert_revision(
            conn,
            &latest.id,
            meeting_id,
            &content,
            validation_state,
            latest.model.as_deref(),
            latest.provider_id.as_deref(),
            note,
            &now,
        )?;

        // Resync actions rows from structured content while preserving action status when possible.
        let existing = MeetingRepository::get_detail(conn, meeting_id)?.actions;
        let status_by_key: std::collections::HashMap<String, ActionStatus> = existing
            .into_iter()
            .filter_map(|action| action.item_key.map(|key| (key, action.status)))
            .collect();

        conn.execute("DELETE FROM actions WHERE meeting_id = ?1", [meeting_id])?;
        for item in &structured.actions {
            let action_id = Uuid::new_v4().to_string();
            let item_key = item
                .id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            let status = status_by_key
                .get(&item_key)
                .copied()
                .unwrap_or(ActionStatus::Pending);
            let sources_json = serde_json::to_string(&item.sources).ok();
            conn.execute(
                "INSERT INTO actions (
                    id, meeting_id, title, description, assignee, due_date, status,
                    item_key, sources_json, origin, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    action_id,
                    meeting_id,
                    item.titre.trim(),
                    item.description,
                    item.responsable,
                    item.echeance,
                    status.as_str(),
                    item_key,
                    sources_json,
                    item.origin.as_str(),
                    now,
                    now,
                ],
            )?;
        }

        latest.content = content;
        latest.validation_state = validation_state;
        latest.validated_at = validated_at;
        latest.updated_at = now;
        Ok(latest)
    }

    pub fn set_action_status(
        conn: &Connection,
        meeting_id: &str,
        action_id: &str,
        status: ActionStatus,
    ) -> AppResult<Action> {
        MeetingRepository::get_by_id(conn, meeting_id)?;
        let now = Utc::now().to_rfc3339();
        let updated = conn.execute(
            "UPDATE actions SET status = ?1, updated_at = ?2
             WHERE id = ?3 AND meeting_id = ?4",
            params![status.as_str(), now, action_id, meeting_id],
        )?;
        if updated == 0 {
            return Err(AppError::Message("action introuvable".into()));
        }
        MeetingRepository::get_detail(conn, meeting_id)?
            .actions
            .into_iter()
            .find(|action| action.id == action_id)
            .ok_or_else(|| AppError::Message("action introuvable après mise à jour".into()))
    }

    pub fn list_revisions(
        conn: &Connection,
        meeting_id: &str,
    ) -> AppResult<Vec<SummaryRevision>> {
        MeetingRepository::get_by_id(conn, meeting_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, summary_id, meeting_id, content, validation_state, model, provider_id, note, created_at
             FROM summary_revisions
             WHERE meeting_id = ?1
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([meeting_id], |row| {
            let validation_str: String = row.get(4)?;
            Ok(SummaryRevision {
                id: row.get(0)?,
                summary_id: row.get(1)?,
                meeting_id: row.get(2)?,
                content: row.get(3)?,
                validation_state: SummaryValidationState::from_str(&validation_str)
                    .unwrap_or_default(),
                model: row.get(5)?,
                provider_id: row.get(6)?,
                note: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn insert_revision(
        conn: &Connection,
        summary_id: &str,
        meeting_id: &str,
        content: &str,
        validation_state: SummaryValidationState,
        model: Option<&str>,
        provider_id: Option<&str>,
        note: Option<&str>,
        created_at: &str,
    ) -> AppResult<()> {
        conn.execute(
            "INSERT INTO summary_revisions (
                id, summary_id, meeting_id, content, validation_state, model, provider_id, note, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                summary_id,
                meeting_id,
                content,
                validation_state.as_str(),
                model,
                provider_id,
                note,
                created_at,
            ],
        )?;
        Ok(())
    }
}

fn ensure_item_keys(structured: &mut StructuredSummary) {
    for decision in &mut structured.decisions {
        let mut item = decision.as_item();
        if item.id.as_ref().map(|id| id.trim().is_empty()).unwrap_or(true) {
            item.id = Some(Uuid::new_v4().to_string());
        }
        *decision = DecisionEntry::Item(item);
    }
    for action in &mut structured.actions {
        if action.id.as_ref().map(|id| id.trim().is_empty()).unwrap_or(true) {
            action.id = Some(Uuid::new_v4().to_string());
        }
    }
}

fn merge_preserving_locked(
    previous: &StructuredSummary,
    generated: &StructuredSummary,
) -> StructuredSummary {
    let mut next = generated.clone();

    let preserved_decisions: Vec<DecisionEntry> = previous
        .decisions
        .iter()
        .map(DecisionEntry::as_item)
        .filter(|item| item.origin.is_preserved())
        .map(DecisionEntry::Item)
        .collect();
    if !preserved_decisions.is_empty() {
        let preserved_keys: std::collections::HashSet<String> = preserved_decisions
            .iter()
            .filter_map(|entry| entry.as_item().id)
            .collect();
        next.decisions
            .retain(|entry| match entry.as_item().id {
                Some(id) => !preserved_keys.contains(&id),
                None => true,
            });
        next.decisions.extend(preserved_decisions);
    }

    let preserved_actions: Vec<StructuredActionItem> = previous
        .actions
        .iter()
        .filter(|item| item.origin.is_preserved())
        .cloned()
        .collect();
    if !preserved_actions.is_empty() {
        let preserved_keys: std::collections::HashSet<String> = preserved_actions
            .iter()
            .filter_map(|item| item.id.clone())
            .collect();
        next.actions.retain(|item| match &item.id {
            Some(id) => !preserved_keys.contains(id),
            None => true,
        });
        next.actions.extend(preserved_actions);
    }

    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::{EvidenceSource, StructuredActionItem};
    use crate::db::open_in_memory;
    use crate::models::CreateMeetingInput;

    fn sample_structured() -> StructuredSummary {
        StructuredSummary {
            synthese: "Réunion productive.".into(),
            decisions: vec!["Lancer la V2".into()],
            actions: vec![StructuredActionItem {
                titre: "Préparer la démo".into(),
                description: Some("Pour le comité".into()),
                responsable: Some("Paul".into()),
                echeance: Some("lundi".into()),
                ..Default::default()
            }],
            risques: vec!["Charge équipe".into()],
            questions_ouvertes: vec!["Budget ?".into()],
        }
    }

    fn sample_structured_two_actions() -> StructuredSummary {
        let mut structured = sample_structured();
        structured.actions.push(StructuredActionItem {
            titre: "Envoyer le compte-rendu".into(),
            ..Default::default()
        });
        structured
    }

    #[test]
    fn save_structured_summary_persists_summary_and_actions() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sprint".into(),
                description: None,
            },
        )
        .unwrap();

        let (summary, actions) = SummaryRepository::save_structured_summary(
            &conn,
            &meeting.id,
            None,
            None,
            &sample_structured(),
        )
        .unwrap();

        assert_eq!(summary.meeting_id, meeting.id);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Préparer la démo");
        assert_eq!(actions[0].assignee.as_deref(), Some("Paul"));
        assert!(actions[0].item_key.is_some());

        let detail = MeetingRepository::get_detail(&conn, &meeting.id).unwrap();
        assert_eq!(detail.summaries.len(), 1);
        assert_eq!(detail.actions.len(), 1);
        assert_eq!(detail.meeting.status, MeetingStatus::Completed);
    }

    #[test]
    fn save_structured_summary_ensures_provider_on_fresh_db() {
        for (provider_id, display_name) in [
            ("mistral", "Mistral AI"),
            ("openai", "OpenAI"),
            ("ollama", "Ollama"),
        ] {
            let conn = open_in_memory().unwrap();
            let meeting = MeetingRepository::create(
                &conn,
                CreateMeetingInput {
                    title: "Sprint".into(),
                    description: None,
                },
            )
            .unwrap();

            let (summary, _) = SummaryRepository::save_structured_summary(
                &conn,
                &meeting.id,
                Some(provider_id),
                Some(display_name),
                &sample_structured(),
            )
            .unwrap();

            assert_eq!(summary.provider_id.as_deref(), Some(provider_id));
        }
    }

    #[test]
    fn save_structured_summary_rolls_back_on_action_failure() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sprint".into(),
                description: None,
            },
        )
        .unwrap();

        conn.execute_batch(
            "CREATE TRIGGER fail_second_action
             AFTER INSERT ON actions
             WHEN (SELECT COUNT(*) FROM actions WHERE meeting_id = NEW.meeting_id) >= 2
             BEGIN
               SELECT RAISE(ABORT, 'simulated action failure');
             END;",
        )
        .unwrap();

        let err = SummaryRepository::save_structured_summary(
            &conn,
            &meeting.id,
            None,
            None,
            &sample_structured_two_actions(),
        )
        .expect_err("doit échouer");
        let _ = err;

        let summary_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))
            .unwrap();
        let action_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM actions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(summary_count, 0);
        assert_eq!(action_count, 0);
    }

    #[test]
    fn update_and_validate_creates_revision_and_preserves_locked_on_regen() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sprint".into(),
                description: None,
            },
        )
        .unwrap();

        let (summary, _) = SummaryRepository::save_structured_summary_with_meta(
            &conn,
            &meeting.id,
            Some("mistral"),
            Some("Mistral AI"),
            &sample_structured(),
            Some("mistral-small-latest"),
            None,
        )
        .unwrap();
        assert_eq!(summary.model.as_deref(), Some("mistral-small-latest"));

        let mut edited = sample_structured();
        ensure_item_keys(&mut edited);
        if let Some(DecisionEntry::Item(item)) = edited.decisions.first_mut() {
            item.origin = ItemOrigin::Locked;
            item.texte = "Décision verrouillée".into();
            item.sources = vec![EvidenceSource {
                segment_index: Some(0),
                start_ms: Some(1000),
                end_ms: Some(2500),
                quote: Some("on lance la V2".into()),
            }];
        }
        if let Some(action) = edited.actions.first_mut() {
            action.origin = ItemOrigin::Validated;
            action.titre = "Action validée".into();
        }

        let updated = SummaryRepository::update_structured_summary(
            &conn,
            &meeting.id,
            &edited,
            SummaryValidationState::Edited,
            Some("correction humaine"),
        )
        .unwrap();
        assert_eq!(updated.validation_state, SummaryValidationState::Edited);

        let mut regenerated = sample_structured();
        regenerated.decisions = vec!["Nouvelle décision IA".into()];
        regenerated.actions = vec![StructuredActionItem {
            titre: "Nouvelle action IA".into(),
            ..Default::default()
        }];

        let (next, actions) = SummaryRepository::save_structured_summary_with_meta(
            &conn,
            &meeting.id,
            Some("mistral"),
            Some("Mistral AI"),
            &regenerated,
            Some("mistral-small-latest"),
            Some(&edited),
        )
        .unwrap();

        let parsed: StructuredSummary = serde_json::from_str(&next.content).unwrap();
        assert!(
            parsed
                .decisions
                .iter()
                .any(|d| d.text() == "Décision verrouillée")
        );
        assert!(actions.iter().any(|a| a.title == "Action validée"));
        assert!(actions.iter().any(|a| a.title == "Nouvelle action IA"));

        let revisions = SummaryRepository::list_revisions(&conn, &meeting.id).unwrap();
        assert!(revisions.len() >= 2);
    }

    #[test]
    fn set_action_status_updates_row() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sprint".into(),
                description: None,
            },
        )
        .unwrap();
        let (_, actions) = SummaryRepository::save_structured_summary(
            &conn,
            &meeting.id,
            None,
            None,
            &sample_structured(),
        )
        .unwrap();
        let updated = SummaryRepository::set_action_status(
            &conn,
            &meeting.id,
            &actions[0].id,
            ActionStatus::Done,
        )
        .unwrap();
        assert_eq!(updated.status, ActionStatus::Done);
    }
}
