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
}

#[derive(Debug, Clone)]
pub struct TranscriptionOptions {
    pub model: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct SummaryOptions {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResult {
    pub text: String,
    pub model: String,
}
