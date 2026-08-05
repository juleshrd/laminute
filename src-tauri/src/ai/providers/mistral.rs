use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::models::{
    KeyValidationResult, ModelInfo, SummaryOptions, SummaryResult, TranscriptionOptions,
    TranscriptionResult,
};
use crate::ai::provider::AiProvider;
use crate::ai::structured_summary::{self, SYSTEM_PROMPT};
use crate::ai::summary::SummaryProvider;
use crate::ai::transcription::TranscriptionProvider;

const MISTRAL_API_BASE: &str = "https://api.mistral.ai/v1";
const DEFAULT_SUMMARY_MODEL: &str = "mistral-small-latest";

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

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    content: String,
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
        api_key: &str,
        text: &str,
        options: SummaryOptions,
    ) -> Result<SummaryResult, AiError> {
        let model = options
            .model
            .unwrap_or_else(|| DEFAULT_SUMMARY_MODEL.to_string());

        let request = ChatCompletionRequest {
            model: model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: structured_summary::build_user_prompt(text),
                },
            ],
            max_tokens: options.max_tokens,
        };

        let response = self
            .client
            .post(format!("{MISTRAL_API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&request)
            .send()
            .await?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: "Clé API invalide ou expirée.".to_string(),
            });
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: format!("réponse inattendue ({status}) : {body}"),
            });
        }

        let payload: ChatCompletionResponse = response.json().await?;
        let text = payload
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| AiError::Provider {
                provider: self.id().to_string(),
                message: "réponse vide du modèle".to_string(),
            })?;

        Ok(SummaryResult {
            text,
            model: payload.model,
        })
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
