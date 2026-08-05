use serde::{Deserialize, Serialize};

use crate::ai::capabilities::ProviderCapabilities;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValidationResult {
    pub valid: bool,
    pub message: String,
    pub models: Option<Vec<ModelInfo>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub selected_provider_id: Option<String>,
    /// Indique si une clé est enregistrée pour le fournisseur sélectionné (jamais la clé elle-même).
    pub has_api_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_base_url: Option<String>,
}

/// Options de transcription (consommées par JUL-148+).
#[derive(Debug, Clone)]
pub struct TranscriptionOptions {
    pub model: Option<String>,
    pub language: Option<String>,
    pub file_name: Option<String>,
}

/// Résultat de transcription (consommée par JUL-148+).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Options de résumé (consommées par JUL-153+).
#[derive(Debug, Clone)]
pub struct SummaryOptions {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}

/// Résultat de résumé (consommé par JUL-153+).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResult {
    pub text: String,
    pub model: String,
}
