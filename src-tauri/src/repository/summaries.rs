use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::ai::structured_summary::StructuredSummary;
use crate::error::{AppError, AppResult};
use crate::models::{Action, ActionStatus, Summary};

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

    pub fn save_structured_summary(
        conn: &Connection,
        meeting_id: &str,
        provider_id: Option<&str>,
        structured: &StructuredSummary,
    ) -> AppResult<(Summary, Vec<Action>)> {
        let now = Utc::now().to_rfc3339();
        let summary_id = Uuid::new_v4().to_string();
        let content = serde_json::to_string(structured).map_err(|error| {
            AppError::Message(format!("sérialisation du compte-rendu : {error}"))
        })?;

        conn.execute(
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![summary_id, meeting_id, provider_id, content, now, now],
        )?;

        let mut actions = Vec::with_capacity(structured.actions.len());
        for item in &structured.actions {
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
    use crate::repository::MeetingRepository;

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
