use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::http;
use crate::ai::limits::truncate_error_message;
use crate::ai::models::{KeyValidationResult, ModelInfo, SummaryOptions, SummaryResult};
use crate::ai::ollama_url;
use crate::ai::provider::AiProvider;
use crate::ai::structured_summary;
use crate::ai::summary::SummaryProvider;

pub const DEFAULT_OLLAMA_BASE: &str = "http://127.0.0.1:11434";
const DEFAULT_SUMMARY_MODEL: &str = "llama3.2";

pub struct OllamaProvider {
    client: reqwest::Client,
    api_base: Arc<RwLock<String>>,
    allow_remote: Arc<RwLock<bool>>,
}

impl OllamaProvider {
    pub fn new() -> Self {
        Self::with_base_url(
            DEFAULT_OLLAMA_BASE.to_string(),
            false,
            http::build_ollama_client(),
        )
    }

    pub fn with_base_url(base_url: String, allow_remote: bool, client: reqwest::Client) -> Self {
        Self {
            client,
            api_base: Arc::new(RwLock::new(base_url)),
            allow_remote: Arc::new(RwLock::new(allow_remote)),
        }
    }

    pub fn configure(&self, base_url: String, allow_remote: bool) {
        if let Ok(mut current) = self.api_base.write() {
            *current = base_url;
        }
        if let Ok(mut current) = self.allow_remote.write() {
            *current = allow_remote;
        }
    }

    fn allow_remote(&self) -> bool {
        self.allow_remote.read().map(|flag| *flag).unwrap_or(false)
    }

    fn raw_base_url(&self) -> String {
        self.api_base
            .read()
            .map(|url| url.clone())
            .unwrap_or_else(|_| DEFAULT_OLLAMA_BASE.to_string())
    }

    /// Valide la destination avant tout appel réseau (aucune donnée envoyée sinon).
    fn validated_base_url(&self) -> Result<String, AiError> {
        ollama_url::normalize(&self.raw_base_url(), self.allow_remote())
            .map(|url| url.into_string())
            .map_err(|err| AiError::Provider {
                provider: self.id().to_string(),
                message: err.to_string(),
            })
    }

    async fn fetch_models(&self) -> Result<Vec<ModelInfo>, AiError> {
        let base = self.validated_base_url()?;
        let response = self.client.get(format!("{base}/api/tags")).send().await?;

        if !response.status().is_success() {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: format!("impossible de joindre Ollama ({})", response.status()),
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
        if models
            .iter()
            .any(|m| m.id.starts_with(DEFAULT_SUMMARY_MODEL))
        {
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
    /// Schéma JSON structuré (Ollama `/api/chat` — `format`).
    format: serde_json::Value,
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
        cancel: &CancellationToken,
    ) -> Result<SummaryResult, AiError> {
        let base = self.validated_base_url()?;
        let models = self.fetch_models().await.unwrap_or_default();
        let model = self.resolve_model(&options, &models);

        let prompt_mode = options.prompt_mode;
        let request = OllamaChatRequest {
            model: model.clone(),
            messages: vec![
                OllamaChatMessage {
                    role: "system".to_string(),
                    content: structured_summary::system_prompt_for(prompt_mode).to_string(),
                },
                OllamaChatMessage {
                    role: "user".to_string(),
                    content: structured_summary::build_user_prompt_for(
                        prompt_mode,
                        text,
                        options.speaker_identity.as_deref(),
                    ),
                },
            ],
            stream: false,
            format: structured_summary::json_schema(),
        };

        let response = http::send_cancellable(
            self.client.post(format!("{base}/api/chat")).json(&request),
            cancel,
        )
        .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = http::read_provider_error(response, Some(cancel)).await?;
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: if status == StatusCode::NOT_FOUND {
                    format!("modèle « {model} » introuvable sur Ollama")
                } else {
                    truncate_error_message(&format!("réponse inattendue ({status}) : {body}"))
                },
            });
        }

        let body = http::read_provider_response(response, Some(cancel)).await?;
        let payload: OllamaChatResponse =
            serde_json::from_str(&body).map_err(|err| AiError::Provider {
                provider: self.id().to_string(),
                message: format!("réponse compte-rendu illisible : {err}"),
            })?;
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
    use wiremock::matchers::{body_partial_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn no_cancel() -> CancellationToken {
        CancellationToken::new()
    }

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

        let provider =
            OllamaProvider::with_base_url(mock_server.uri(), false, http::build_ollama_client());
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
            .and(body_partial_json(serde_json::json!({
                "format": { "type": "object" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "model": "llama3.2",
                "message": {
                    "role": "assistant",
                    "content": "{\"synthese\":\"Résumé local\",\"decisions\":[],\"actions\":[]}"
                }
            })))
            .mount(&mock_server)
            .await;

        let provider =
            OllamaProvider::with_base_url(mock_server.uri(), false, http::build_ollama_client());
        let result = provider
            .summarize(
                "",
                "Texte de réunion",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                    ..Default::default()
                },
                &no_cancel(),
            )
            .await
            .expect("summary");

        assert!(result.text.contains("synthese"));
        assert_eq!(result.model, "llama3.2");
    }

    #[tokio::test]
    async fn summarize_rejects_invalid_base_url_before_sending() {
        let provider = OllamaProvider::with_base_url(
            "http://169.254.169.254".into(),
            true,
            http::build_ollama_client(),
        );
        let err = provider
            .summarize(
                "",
                "secret transcript",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                    ..Default::default()
                },
                &no_cancel(),
            )
            .await
            .expect_err("must reject");
        assert!(err.to_string().contains("link-local") || err.to_string().contains("metadata"));
    }

    #[tokio::test]
    async fn summarize_rejects_oversized_response() {
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
            .respond_with(
                ResponseTemplate::new(200).set_body_string(
                    "x".repeat(crate::ai::limits::MAX_PROVIDER_RESPONSE_BYTES + 1),
                ),
            )
            .mount(&mock_server)
            .await;

        let provider =
            OllamaProvider::with_base_url(mock_server.uri(), false, http::build_ollama_client());
        let err = provider
            .summarize(
                "",
                "Texte de réunion",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                    ..Default::default()
                },
                &no_cancel(),
            )
            .await
            .expect_err("oversized response");

        assert!(err.to_string().contains("trop volumineuse"));
    }

    #[tokio::test]
    async fn rejects_cross_origin_redirect() {
        let victim = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "models": [{ "name": "llama3.2:latest" }]
            })))
            .mount(&victim)
            .await;

        let gateway = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(
                ResponseTemplate::new(302).insert_header("Location", victim.uri() + "/api/tags"),
            )
            .mount(&gateway)
            .await;

        let provider =
            OllamaProvider::with_base_url(gateway.uri(), false, http::build_ollama_client());
        let err = provider
            .validate_key("")
            .await
            .expect_err("cross-origin redirect must fail");
        let message = err.to_string();
        assert!(
            message.contains("redirection")
                || message.contains("error sending request")
                || message.contains("redirect"),
            "unexpected error: {message}"
        );
    }
}
