use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::models::{KeyValidationResult, ModelInfo, SummaryOptions, SummaryResult};
use crate::ai::provider::AiProvider;
use crate::ai::structured_summary::{self, SYSTEM_PROMPT};
use crate::ai::summary::SummaryProvider;

pub const DEFAULT_OLLAMA_BASE: &str = "http://127.0.0.1:11434";
const DEFAULT_SUMMARY_MODEL: &str = "llama3.2";

pub struct OllamaProvider {
    client: reqwest::Client,
    api_base: Arc<RwLock<String>>,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_OLLAMA_BASE.to_string())
    }

    pub fn with_base_url(base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base: Arc::new(RwLock::new(base_url)),
        }
    }

    pub fn set_base_url(&self, base_url: String) {
        if let Ok(mut current) = self.api_base.write() {
            *current = base_url;
        }
    }

    fn base_url(&self) -> String {
        self.api_base
            .read()
            .map(|url| url.clone())
            .unwrap_or_else(|_| DEFAULT_OLLAMA_BASE.to_string())
    }

    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url()))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: format!(
                    "impossible de joindre Ollama ({})",
                    response.status()
                ),
            });
        }

        let payload: OllamaTagsResponse = response.json().await?;
        Ok(payload
            .models
            .into_iter()
            .map(|model| {
                let name = model.name.clone();
                ModelInfo {
                    id: name.clone(),
                    name,
                    description: None,
                }
            })
            .collect())
    }

    fn resolve_model(&self, options: &SummaryOptions, models: &[ModelInfo]) -> String {
        if let Some(model) = &options.model {
            return model.clone();
        }
        if models.iter().any(|m| m.id.starts_with(DEFAULT_SUMMARY_MODEL)) {
            return DEFAULT_SUMMARY_MODEL.to_string();
        }
        models
            .first()
            .map(|m| m.id.clone())
            .unwrap_or_else(|| DEFAULT_SUMMARY_MODEL.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    model: String,
    message: OllamaChatMessageResponse,
}

#[derive(Debug, Deserialize)]
struct OllamaChatMessageResponse {
    content: String,
}

#[async_trait]
impl AiProvider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn display_name(&self) -> &str {
        "Ollama"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::ollama()
    }

    async fn validate_key(&self, _api_key: &str) -> Result<KeyValidationResult, AiError> {
        let models = self.fetch_models().await?;
        if models.is_empty() {
            return Ok(KeyValidationResult {
                valid: false,
                message: "Ollama est joignable mais aucun modèle n'est installé.".to_string(),
                models: Some(vec![]),
            });
        }

        Ok(KeyValidationResult {
            valid: true,
            message: format!(
                "Connexion Ollama OK — {} modèle(s) disponible(s).",
                models.len()
            ),
            models: Some(models),
        })
    }

    async fn list_models(&self, _api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
        self.fetch_models().await
    }
}

#[async_trait]
impl SummaryProvider for OllamaProvider {
    async fn summarize(
        &self,
        _api_key: &str,
        text: &str,
        options: SummaryOptions,
    ) -> Result<SummaryResult, AiError> {
        let models = self.fetch_models().await.unwrap_or_default();
        let model = self.resolve_model(&options, &models);

        let request = OllamaChatRequest {
            model: model.clone(),
            messages: vec![
                OllamaChatMessage {
                    role: "system".to_string(),
                    content: SYSTEM_PROMPT.to_string(),
                },
                OllamaChatMessage {
                    role: "user".to_string(),
                    content: structured_summary::build_user_prompt(text),
                },
            ],
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url()))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: if status == StatusCode::NOT_FOUND {
                    format!("modèle « {model} » introuvable sur Ollama")
                } else {
                    format!("réponse inattendue ({status}) : {body}")
                },
            });
        }

        let payload: OllamaChatResponse = response.json().await?;
        if payload.message.content.trim().is_empty() {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: "réponse vide du modèle".to_string(),
            });
        }

        Ok(SummaryResult {
            text: payload.message.content,
            model: payload.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn validate_key_accepts_ping_without_api_key() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{ "name": "llama3.2:latest" }]
            })))
            .mount(&mock_server)
            .await;

        let provider = OllamaProvider::with_base_url(mock_server.uri());
        let result = provider.validate_key("").await.expect("validation");
        assert!(result.valid);
        assert!(result.message.contains("Connexion Ollama OK"));
    }

    #[tokio::test]
    async fn summarize_returns_text_from_api() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{ "name": "llama3.2:latest" }]
            })))
            .mount(&mock_server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "llama3.2",
                "message": {
                    "role": "assistant",
                    "content": "{\"synthese\":\"Résumé local\",\"decisions\":[],\"actions\":[]}"
                }
            })))
            .mount(&mock_server)
            .await;

        let provider = OllamaProvider::with_base_url(mock_server.uri());
        let result = provider
            .summarize(
                "",
                "Texte de réunion",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                },
            )
            .await
            .expect("summary");

        assert!(result.text.contains("synthese"));
        assert_eq!(result.model, "llama3.2");
    }
}
