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
pub struct ProviderModelPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    pub selected_provider_id: Option<String>,
    /// Indique si une clé est enregistrée pour le fournisseur sélectionné (jamais la clé elle-même).
    pub has_api_key: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama_base_url: Option<String>,
    /// Autorisation explicite d'un serveur Ollama hors loopback (LAN / distant).
    pub ollama_allow_remote: bool,
    /// Diarisation locuteurs activée pour la transcription (si le fournisseur le permet).
    pub diarization_enabled: bool,
    /// Langue de transcription forcée. Absent ou `auto` = détection automatique.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcription_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary_model: Option<String>,
    pub transcription_models: Vec<ModelInfo>,
    pub summary_models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetModelPreferencesInput {
    pub provider_id: String,
    #[serde(default)]
    pub transcription_model: Option<String>,
    #[serde(default)]
    pub summary_model: Option<String>,
    #[serde(default)]
    pub diarization_enabled: Option<bool>,
    /// Langue de transcription forcée (`fr`, `en`, …). `null` / absent = détection automatique.
    #[serde(default)]
    pub transcription_language: Option<String>,
}

/// Segment structuré d'une transcription (diarisation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
}

/// Options de transcription (consommées par JUL-148+ / JUL-165).
#[derive(Debug, Clone)]
pub struct TranscriptionOptions {
    pub model: Option<String>,
    pub language: Option<String>,
    pub file_name: Option<String>,
    pub diarize: bool,
}

/// Résultat de transcription (consommée par JUL-148+).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,
}

/// Options de résumé (consommées par JUL-153+).
#[derive(Debug, Clone)]
pub struct SummaryOptions {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub prompt_mode: crate::ai::structured_summary::SummaryPromptMode,
    /// Labels techniques → noms confirmés, injectés dans le prompt compte-rendu.
    pub speaker_identity: Option<Vec<(String, String)>>,
}

impl Default for SummaryOptions {
    fn default() -> Self {
        Self {
            model: None,
            max_tokens: None,
            prompt_mode: crate::ai::structured_summary::SummaryPromptMode::Full,
            speaker_identity: None,
        }
    }
}

/// Résultat de résumé (consommé par JUL-153+).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryResult {
    pub text: String,
    pub model: String,
}
