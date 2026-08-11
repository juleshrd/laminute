use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::ai::speaker::apply_speaker_map_to_structured;
use crate::ai::structured_summary::StructuredSummary;
use crate::error::{AppError, AppResult};
use crate::models::{Action, ActionStatus, MeetingStatus, Summary};
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
        let tx = conn.unchecked_transaction()?;
        let result = Self::save_structured_summary_on(
            &tx,
            meeting_id,
            provider_id,
            provider_display_name,
            structured,
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
    ) -> AppResult<(Summary, Vec<Action>)> {
        let meeting = MeetingRepository::get_by_id(conn, meeting_id)?;
        let mut structured_to_save = structured.clone();
        if let Some(ref map) = meeting.speaker_map {
            if !map.is_empty() {
                apply_speaker_map_to_structured(&mut structured_to_save, map);
            }
        }

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
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![summary_id, meeting_id, provider_id, content, now, now],
        )?;

        let mut actions = Vec::with_capacity(structured_to_save.actions.len());
        for item in &structured_to_save.actions {
            let action_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO actions (id, meeting_id, title, description, assignee, due_date, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    action_id,
                    meeting_id,
                    item.titre.trim(),
                    item.description,
                    item.responsable,
                    item.echeance,
                    ActionStatus::Pending.as_str(),
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
            created_at: now.clone(),
            updated_at: now,
        };

        Ok((summary, actions))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::StructuredActionItem;
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
            }],
            risques: vec!["Charge équipe".into()],
            questions_ouvertes: vec!["Budget ?".into()],
        }
    }

    fn sample_structured_two_actions() -> StructuredSummary {
        let mut structured = sample_structured();
        structured.actions.push(StructuredActionItem {
            titre: "Envoyer le compte-rendu".into(),
            description: None,
            responsable: None,
            echeance: None,
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
                    title: format!("CR {provider_id}"),
                    description: None,
                },
            )
            .unwrap();

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ai_providers WHERE id = ?1",
                    [provider_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "provider {provider_id} must be absent initially");

            let (summary, _) = SummaryRepository::save_structured_summary(
                &conn,
                &meeting.id,
                Some(provider_id),
                Some(display_name),
                &sample_structured(),
            )
            .unwrap();

            assert_eq!(summary.provider_id.as_deref(), Some(provider_id));
            let name: String = conn
                .query_row(
                    "SELECT name FROM ai_providers WHERE id = ?1",
                    [provider_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(name, display_name);
        }
    }

    #[test]
    fn save_structured_summary_rolls_back_on_action_failure() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Rollback".into(),
                description: None,
            },
        )
        .unwrap();

        conn.execute_batch(
            "CREATE TRIGGER fail_second_action BEFORE INSERT ON actions
             WHEN (SELECT COUNT(*) FROM actions WHERE meeting_id = NEW.meeting_id) >= 1
             BEGIN
               SELECT RAISE(ABORT, 'simulated action failure');
             END;",
        )
        .unwrap();

        let err = SummaryRepository::save_structured_summary(
            &conn,
            &meeting.id,
            Some("mistral"),
            Some("Mistral AI"),
            &sample_structured_two_actions(),
        )
        .expect_err("second action insert must fail");
        assert!(
            err.to_string().contains("simulated action failure"),
            "unexpected error: {err}"
        );

        let summaries: i64 = conn
            .query_row("SELECT COUNT(*) FROM summaries", [], |row| row.get(0))
            .unwrap();
        let actions: i64 = conn
            .query_row("SELECT COUNT(*) FROM actions", [], |row| row.get(0))
            .unwrap();
        let providers: i64 = conn
            .query_row("SELECT COUNT(*) FROM ai_providers", [], |row| row.get(0))
            .unwrap();
        assert_eq!(summaries, 0);
        assert_eq!(actions, 0);
        assert_eq!(providers, 0);

        let meeting = MeetingRepository::get_by_id(&conn, &meeting.id).unwrap();
        assert_eq!(meeting.status, MeetingStatus::Draft);
    }

    #[test]
    fn latest_transcription_text_returns_most_recent() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Daily".into(),
                description: None,
            },
        )
        .unwrap();
        let older = "2024-01-01T10:00:00Z";
        let newer = "2024-01-01T11:00:00Z";

        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, content, created_at, updated_at)
             VALUES ('tx-1', ?1, 'ancienne', ?2, ?2)",
            params![meeting.id, older],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, content, created_at, updated_at)
             VALUES ('tx-2', ?1, 'récente', ?2, ?2)",
            params![meeting.id, newer],
        )
        .unwrap();

        let text = SummaryRepository::latest_transcription_text(&conn, &meeting.id)
            .unwrap()
            .expect("transcription");
        assert_eq!(text, "récente");
    }
}
