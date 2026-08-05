use serde::{Deserialize, Serialize};

use crate::ai::error::AiError;

/// Schéma JSON stable pour un compte-rendu de réunion structuré.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredSummary {
    pub synthese: String,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub actions: Vec<StructuredActionItem>,
    #[serde(default)]
    pub risques: Vec<String>,
    #[serde(default)]
    pub questions_ouvertes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StructuredActionItem {
    pub titre: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub responsable: Option<String>,
    #[serde(default)]
    pub echeance: Option<String>,
}

impl StructuredSummary {
    pub fn validate(&self) -> Result<(), AiError> {
        if self.synthese.trim().is_empty() {
            return Err(AiError::Other(
                "le champ « synthese » est obligatoire et ne peut pas être vide".into(),
            ));
        }

        for (index, action) in self.actions.iter().enumerate() {
            if action.titre.trim().is_empty() {
                return Err(AiError::Other(format!(
                    "l'action #{index} doit avoir un titre non vide"
                )));
            }
        }

        Ok(())
    }
}

/// Extrait et valide le JSON structuré depuis la réponse brute du modèle.
pub fn parse_structured_summary(raw: &str) -> Result<StructuredSummary, AiError> {
    let json_str = extract_json_payload(raw);
    let summary: StructuredSummary = serde_json::from_str(&json_str)
        .map_err(|error| AiError::Other(format!("JSON de compte-rendu invalide : {error}")))?;
    summary.validate()?;
    Ok(summary)
}

fn extract_json_payload(raw: &str) -> String {
    let trimmed = raw.trim();

    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        let content = after_fence
            .strip_prefix("json")
            .unwrap_or(after_fence)
            .trim_start();
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }

    trimmed.to_string()
}

pub const SYSTEM_PROMPT: &str = r#"Tu es un assistant expert en rédaction de comptes-rendus de réunion en français.

À partir d'une transcription, produis un compte-rendu structuré au format JSON strict, sans texte avant ni après.

Schéma attendu :
{
  "synthese": "string — synthèse concise de la réunion (2 à 4 phrases)",
  "decisions": ["string — chaque décision prise"],
  "actions": [
    {
      "titre": "string — action concrète",
      "description": "string ou null — détails optionnels",
      "responsable": "string ou null — personne assignée si mentionnée",
      "echeance": "string ou null — date ou délai si mentionné"
    }
  ],
  "risques": ["string — risques ou points de vigilance identifiés"],
  "questionsOuvertes": ["string — questions restant sans réponse"]
}

Règles :
- Réponds UNIQUEMENT avec un objet JSON valide.
- Utilise des tableaux vides [] si une section n'a pas de contenu.
- N'invente pas de responsables ni d'échéances non mentionnés dans la transcription.
- Rédige en français clair et professionnel."#;

pub fn build_user_prompt(transcription: &str) -> String {
    format!(
        "Transcription de la réunion :\n\n{transcription}\n\nGénère le compte-rendu structuré au format JSON."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_JSON: &str = r#"{
        "synthese": "La réunion a porté sur le lancement du produit.",
        "decisions": ["Valider le planning Q4"],
        "actions": [
            {
                "titre": "Envoyer le devis",
                "description": "Au client principal",
                "responsable": "Marie",
                "echeance": "vendredi"
            }
        ],
        "risques": ["Retard fournisseur"],
        "questionsOuvertes": ["Budget marketing ?"]
    }"#;

    #[test]
    fn parse_valid_json() {
        let summary = parse_structured_summary(VALID_JSON).expect("parse");
        assert_eq!(
            summary.synthese,
            "La réunion a porté sur le lancement du produit."
        );
        assert_eq!(summary.decisions, vec!["Valider le planning Q4"]);
        assert_eq!(summary.actions.len(), 1);
        assert_eq!(summary.actions[0].titre, "Envoyer le devis");
        assert_eq!(summary.actions[0].responsable.as_deref(), Some("Marie"));
        assert_eq!(summary.risques, vec!["Retard fournisseur"]);
        assert_eq!(summary.questions_ouvertes, vec!["Budget marketing ?"]);
    }

    #[test]
    fn parse_json_wrapped_in_markdown_fence() {
        let wrapped = format!("```json\n{VALID_JSON}\n```");
        let summary = parse_structured_summary(&wrapped).expect("parse");
        assert!(!summary.synthese.is_empty());
        assert_eq!(summary.decisions.len(), 1);
    }

    #[test]
    fn reject_empty_synthese() {
        let json = r#"{"synthese": "   ", "decisions": []}"#;
        let err = parse_structured_summary(json).unwrap_err();
        assert!(err.to_string().contains("synthese"));
    }

    #[test]
    fn reject_action_without_title() {
        let json = r#"{
            "synthese": "OK",
            "actions": [{"titre": "  "}]
        }"#;
        let err = parse_structured_summary(json).unwrap_err();
        assert!(err.to_string().contains("action"));
    }

    #[test]
    fn reject_invalid_json() {
        let err = parse_structured_summary("{not json").unwrap_err();
        assert!(err.to_string().contains("JSON"));
    }

    #[test]
    fn accepts_minimal_schema_with_defaults() {
        let json = r#"{"synthese": "Réunion courte."}"#;
        let summary = parse_structured_summary(json).expect("parse");
        assert!(summary.decisions.is_empty());
        assert!(summary.actions.is_empty());
        assert!(summary.risques.is_empty());
        assert!(summary.questions_ouvertes.is_empty());
    }

    #[test]
    fn build_user_prompt_includes_transcription() {
        let prompt = build_user_prompt("Bonjour à tous");
        assert!(prompt.contains("Bonjour à tous"));
        assert!(prompt.contains("Transcription"));
    }
}
