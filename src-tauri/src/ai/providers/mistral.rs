use async_trait::async_trait;
use serde::Deserialize;

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::models::{
    KeyValidationResult, ModelInfo, SummaryOptions, SummaryResult, TranscriptionOptions,
    TranscriptionResult,
};
use crate::ai::provider::AiProvider;
use crate::ai::summary::SummaryProvider;
use crate::ai::transcription::TranscriptionProvider;

const MISTRAL_API_BASE: &str = "https://api.mistral.ai/v1";

pub struct MistralProvider {
    client: reqwest::Client,
}

impl MistralProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    async fn fetch_models(&self, api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
        let response = self
            .client
            .get(format!("{MISTRAL_API_BASE}/models"))
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Ok(vec![]);
        }

        if !response.status().is_success() {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: format!("réponse inattendue ({})", response.status()),
            });
        }

        let payload: MistralModelsResponse = response.json().await?;
        Ok(payload
            .data
            .into_iter()
            .map(|model| ModelInfo {
                id: model.id.clone(),
                name: model.id,
                description: model.object,
            })
            .collect())
    }
}

impl Default for MistralProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct MistralModelsResponse {
    data: Vec<MistralModel>,
}

#[derive(Debug, Deserialize)]
struct MistralModel {
    id: String,
    object: Option<String>,
}

#[async_trait]
impl AiProvider for MistralProvider {
    fn id(&self) -> &str {
        "mistral"
    }

    fn display_name(&self) -> &str {
        "Mistral AI"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::mistral()
    }

    async fn validate_key(&self, api_key: &str) -> Result<KeyValidationResult, AiError> {
        if api_key.trim().is_empty() {
            return Ok(KeyValidationResult {
                valid: false,
                message: "La clé API est vide.".to_string(),
                models: None,
            });
        }

        let models = self.fetch_models(api_key).await?;
        if models.is_empty() {
            return Ok(KeyValidationResult {
                valid: false,
                message: "Clé API invalide ou expirée.".to_string(),
                models: None,
            });
        }

        Ok(KeyValidationResult {
            valid: true,
            message: format!("Clé API valide — {} modèle(s) disponible(s).", models.len()),
            models: Some(models),
        })
    }

    async fn list_models(&self, api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
        self.fetch_models(api_key).await
    }
}

#[async_trait]
impl TranscriptionProvider for MistralProvider {
    async fn transcribe(
        &self,
        _api_key: &str,
        _audio: &[u8],
        _options: TranscriptionOptions,
    ) -> Result<TranscriptionResult, AiError> {
        Err(AiError::NotImplemented(
            "Transcription Mistral — implémentation complète prévue dans JUL-148.".to_string(),
        ))
    }
}

#[async_trait]
impl SummaryProvider for MistralProvider {
    async fn summarize(
        &self,
        _api_key: &str,
        _text: &str,
        _options: SummaryOptions,
    ) -> Result<SummaryResult, AiError> {
        Err(AiError::NotImplemented(
            "Résumé Mistral — hors périmètre JUL-152.".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn validate_key_rejects_empty_key() {
        let provider = MistralProvider::new();
        let result = provider.validate_key("   ").await.expect("validation");
        assert!(!result.valid);
        assert!(result.message.contains("vide"));
    }

    #[tokio::test]
    async fn transcribe_returns_not_implemented() {
        let provider = MistralProvider::new();
        let error = provider
            .transcribe("sk-test", &[], TranscriptionOptions {
                model: None,
                language: None,
            })
            .await
            .unwrap_err();
        assert!(matches!(error, AiError::NotImplemented(_)));
    }
}
