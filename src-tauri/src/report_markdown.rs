//! Génération Markdown des comptes-rendus (miroir de `src/lib/reportExport.ts`).

use crate::ai::structured_summary::{StructuredActionItem, StructuredSummary};
use crate::models::MeetingStatus;

pub struct MeetingReportMarkdownInput<'a> {
    pub title: &'a str,
    pub status: MeetingStatus,
    pub display_date: &'a str,
    pub duration_label: &'a str,
    pub summary: &'a StructuredSummary,
}

pub fn build_meeting_report_markdown(input: MeetingReportMarkdownInput<'_>) -> String {
    let mut sections = vec![
        format!("# {}", input.title),
        String::new(),
        "*Compte-rendu exporté depuis La Minute*".to_string(),
        String::new(),
        "| | |".to_string(),
        "| --- | --- |".to_string(),
        format!("| Statut | {} |", status_label_fr(input.status)),
        format!("| Date | {} |", input.display_date),
        format!("| Durée | {} |", input.duration_label),
        String::new(),
        "## Synthèse".to_string(),
        String::new(),
        {
            let synthese = input.summary.synthese.trim();
            if synthese.is_empty() {
                "_Synthèse vide._".to_string()
            } else {
                synthese.to_string()
            }
        },
        String::new(),
        "## Décisions".to_string(),
        String::new(),
        bullet_list(&input.summary.decisions),
        String::new(),
        "## Actions".to_string(),
        String::new(),
        format_actions(&input.summary.actions),
    ];

    if !input.summary.risques.is_empty() {
        sections.extend([
            String::new(),
            "## Risques".to_string(),
            String::new(),
            bullet_list(&input.summary.risques),
        ]);
    }

    if !input.summary.questions_ouvertes.is_empty() {
        sections.extend([
            String::new(),
            "## Questions ouvertes".to_string(),
            String::new(),
            bullet_list(&input.summary.questions_ouvertes),
        ]);
    }

    sections.push(String::new());
    sections.join("\n")
}

fn status_label_fr(status: MeetingStatus) -> &'static str {
    match status {
        MeetingStatus::Draft => "Brouillon",
        MeetingStatus::Recording => "Enregistrement",
        MeetingStatus::Processing => "Traitement",
        MeetingStatus::Completed => "Terminée",
    }
}

fn bullet_list(items: &[String]) -> String {
    if items.is_empty() {
        return "_Aucun élément._".to_string();
    }
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_actions(actions: &[StructuredActionItem]) -> String {
    if actions.is_empty() {
        return "_Aucune action identifiée._".to_string();
    }

    actions
        .iter()
        .map(|action| {
            let mut bits = vec![format!("**{}**", action.titre)];
            if let Some(description) = action.description.as_deref().filter(|s| !s.is_empty()) {
                bits.push(description.to_string());
            }
            let line = bits.join(" — ");

            let mut meta = Vec::new();
            if let Some(responsable) = action.responsable.as_deref().filter(|s| !s.is_empty()) {
                meta.push(format!("responsable : {responsable}"));
            }
            if let Some(echeance) = action.echeance.as_deref().filter(|s| !s.is_empty()) {
                meta.push(format!("échéance : {echeance}"));
            }

            if meta.is_empty() {
                format!("- {line}")
            } else {
                format!("- {line} ({})", meta.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::StructuredActionItem;

    #[test]
    fn markdown_includes_core_sections() {
        let summary = StructuredSummary {
            synthese: "Point d’avancement du trimestre.".into(),
            decisions: vec!["Valider le roadmap Q3".into()],
            actions: vec![StructuredActionItem {
                titre: "Rédiger le brief".into(),
                description: Some("Version courte".into()),
                responsable: Some("Alice".into()),
                echeance: Some("2026-08-12".into()),
            }],
            risques: vec!["Délai serré".into()],
            questions_ouvertes: vec!["Budget marketing ?".into()],
        };

        let md = build_meeting_report_markdown(MeetingReportMarkdownInput {
            title: "Comité produit",
            status: MeetingStatus::Completed,
            display_date: "5 août 2026 à 14:00",
            duration_label: "12:30",
            summary: &summary,
        });

        assert!(md.contains("# Comité produit"));
        assert!(md.contains("La Minute"));
        assert!(md.contains("| Statut | Terminée |"));
        assert!(md.contains("| Date | 5 août 2026 à 14:00 |"));
        assert!(md.contains("| Durée | 12:30 |"));
        assert!(md.contains("## Synthèse"));
        assert!(md.contains("Point d’avancement du trimestre."));
        assert!(md.contains("## Décisions"));
        assert!(md.contains("- Valider le roadmap Q3"));
        assert!(md.contains("## Actions"));
        assert!(md.contains("**Rédiger le brief**"));
        assert!(md.contains("responsable : Alice"));
        assert!(md.contains("échéance : 2026-08-12"));
        assert!(md.contains("## Risques"));
        assert!(md.contains("- Délai serré"));
        assert!(md.contains("## Questions ouvertes"));
        assert!(md.contains("- Budget marketing ?"));
    }

    #[test]
    fn markdown_omits_empty_optional_sections() {
        let summary = StructuredSummary {
            synthese: "Synthèse seule".into(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };

        let md = build_meeting_report_markdown(MeetingReportMarkdownInput {
            title: "Réunion",
            status: MeetingStatus::Draft,
            display_date: "01/01/2026",
            duration_label: "—",
            summary: &summary,
        });

        assert!(md.contains("_Aucun élément._"));
        assert!(md.contains("_Aucune action identifiée._"));
        assert!(!md.contains("## Risques"));
        assert!(!md.contains("## Questions ouvertes"));
    }
}
