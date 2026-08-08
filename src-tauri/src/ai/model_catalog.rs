use crate::ai::error::AiError;
use crate::ai::models::ModelInfo;

/// Modèles audio / transcription proposés à l'utilisateur (catalogue produit, pas la liste brute `/models`).
pub fn transcription_models(provider_id: &str) -> Vec<ModelInfo> {
    match provider_id {
        "mistral" => vec![ModelInfo {
            id: "voxtral-mini-latest".into(),
            name: "Voxtral Mini".into(),
            description: Some(
                "Transcription batch Mistral (Voxtral Transcribe 2), avec diarisation optionnelle."
                    .into(),
            ),
        }],
        "openai" => vec![
            ModelInfo {
                id: "gpt-4o-mini-transcribe".into(),
                name: "GPT-4o Mini Transcribe".into(),
                description: Some("Modèle audio recommandé — rapide et économique.".into()),
            },
            ModelInfo {
                id: "gpt-4o-transcribe".into(),
                name: "GPT-4o Transcribe".into(),
                description: Some("Meilleure qualité de transcription OpenAI.".into()),
            },
            ModelInfo {
                id: "whisper-1".into(),
                name: "Whisper-1".into(),
                description: Some("Modèle Whisper classique (compatibilité).".into()),
            },
        ],
        _ => vec![],
    }
}

/// Modèles LLM pour le compte-rendu structuré.
pub fn summary_models(provider_id: &str) -> Vec<ModelInfo> {
    match provider_id {
        "mistral" => vec![
            ModelInfo {
                id: "mistral-small-latest".into(),
                name: "Mistral Small".into(),
                description: Some("Rapide et économique pour le compte-rendu.".into()),
            },
            ModelInfo {
                id: "mistral-medium-latest".into(),
                name: "Mistral Medium".into(),
                description: Some("Plus précis pour des réunions complexes.".into()),
            },
        ],
        "openai" => vec![
            ModelInfo {
                id: "gpt-4o-mini".into(),
                name: "GPT-4o Mini".into(),
                description: Some("Compte-rendu rapide et économique.".into()),
            },
            ModelInfo {
                id: "gpt-4o".into(),
                name: "GPT-4o".into(),
                description: Some("Meilleure qualité de synthèse.".into()),
            },
            ModelInfo {
                id: "gpt-4.1-mini".into(),
                name: "GPT-4.1 Mini".into(),
                description: Some("Dernière génération, profil économique.".into()),
            },
            ModelInfo {
                id: "gpt-4.1".into(),
                name: "GPT-4.1".into(),
                description: Some("Dernière génération, haute qualité.".into()),
            },
        ],
        _ => vec![],
    }
}

pub fn default_transcription_model(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "mistral" => Some("voxtral-mini-latest"),
        "openai" => Some("gpt-4o-mini-transcribe"),
        _ => None,
    }
}

pub fn default_summary_model(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        "mistral" => Some("mistral-small-latest"),
        "openai" => Some("gpt-4o-mini"),
        _ => None,
    }
}

pub fn supports_diarization(provider_id: &str) -> bool {
    matches!(provider_id, "mistral" | "openai")
}

/// OpenAI utilise un modèle dédié dès que la diarisation est activée.
pub const OPENAI_DIARIZE_MODEL: &str = "gpt-4o-transcribe-diarize";

pub fn validate_transcription_model(
    provider_id: &str,
    model: Option<String>,
) -> Result<Option<String>, AiError> {
    validate_catalog_model(
        provider_id,
        model,
        transcription_models(provider_id),
        "transcription",
    )
}

pub fn validate_summary_model(
    provider_id: &str,
    model: Option<String>,
) -> Result<Option<String>, AiError> {
    validate_catalog_model(
        provider_id,
        model,
        summary_models(provider_id),
        "compte-rendu",
    )
}

fn validate_catalog_model(
    provider_id: &str,
    model: Option<String>,
    catalog: Vec<ModelInfo>,
    label: &str,
) -> Result<Option<String>, AiError> {
    let Some(model) = model else {
        return Ok(None);
    };

    let trimmed = model.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if catalog.iter().any(|entry| entry.id == trimmed) {
        return Ok(Some(trimmed));
    }

    Err(AiError::Other(format!(
        "Modèle de {label} « {trimmed} » non supporté pour {provider_id}."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mistral_offers_small_and_medium_for_summary() {
        let ids: Vec<_> = summary_models("mistral")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert!(ids.contains(&"mistral-small-latest".to_string()));
        assert!(ids.contains(&"mistral-medium-latest".to_string()));
    }

    #[test]
    fn openai_offers_audio_and_llm_models() {
        assert!(!transcription_models("openai").is_empty());
        assert!(!summary_models("openai").is_empty());
        assert_eq!(
            default_transcription_model("openai"),
            Some("gpt-4o-mini-transcribe")
        );
    }

    #[test]
    fn rejects_models_outside_catalog() {
        assert!(validate_summary_model("mistral", Some("mistral-small-latest".into())).is_ok());
        assert!(validate_summary_model("mistral", Some("not-a-catalog-model".into())).is_err());
        assert!(validate_transcription_model("openai", Some("whisper-1".into())).is_ok());
        assert!(validate_transcription_model("openai", Some("gpt-5-transcribe".into())).is_err());
    }
}
