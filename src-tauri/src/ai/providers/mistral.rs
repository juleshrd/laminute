use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::limits::validate_transcription_audio_size;
use crate::ai::models::{
    KeyValidationResult, ModelInfo, SummaryOptions, SummaryResult, TranscriptionOptions,
    TranscriptionResult,
};
use crate::ai::provider::AiProvider;
use crate::ai::structured_summary::{self, SYSTEM_PROMPT};
use crate::ai::summary::SummaryProvider;
use crate::ai::transcription::TranscriptionProvider;

use crate::ai::model_catalog;

const MISTRAL_API_BASE: &str = "https://api.mistral.ai/v1";

pub struct MistralProvider {
    client: reqwest::Client,
    api_base: String,
}

impl MistralProvider {
    pub fn new() -> Self {
        Self::with_api_base(MISTRAL_API_BASE.to_string())
    }

    pub fn with_api_base(api_base: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_base,
        }
    }

    async fn fetch_models(&self, api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
        let response = self
            .client
            .get(format!("{}/models", self.api_base))
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await?;

        if response.status() == StatusCode::UNAUTHORIZED {
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

    fn resolve_upload_name(options: &TranscriptionOptions) -> (String, &'static str) {
        let file_name = options
            .file_name
            .as_deref()
            .unwrap_or("audio.wav")
            .to_string();
        let mime = match file_name.rsplit('.').next() {
            Some("mp3") => "audio/mpeg",
            Some("m4a") => "audio/mp4",
            Some("ogg") => "audio/ogg",
            Some("flac") => "audio/flac",
            _ => "audio/wav",
        };
        (file_name, mime)
    }

    fn map_http_error(status: StatusCode, body: &str) -> AiError {
        let message = match status {
            StatusCode::UNAUTHORIZED => "Clé API invalide ou expirée.".to_string(),
            StatusCode::PAYLOAD_TOO_LARGE => {
                "Fichier audio trop volumineux pour l'API Mistral.".to_string()
            }
            StatusCode::TOO_MANY_REQUESTS => {
                "Limite de requêtes Mistral atteinte — réessayez plus tard.".to_string()
            }
            StatusCode::UNSUPPORTED_MEDIA_TYPE => {
                "Format audio non supporté par Mistral.".to_string()
            }
            _ if status.is_client_error() => {
                extract_api_message(body).unwrap_or_else(|| format!("requête refusée ({status})"))
            }
            _ => extract_api_message(body)
                .unwrap_or_else(|| format!("erreur serveur Mistral ({status})")),
        };

        AiError::Provider {
            provider: "mistral".to_string(),
            message,
        }
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

#[derive(Debug, Deserialize)]
struct MistralTranscriptionResponse {
    text: String,
    model: Option<String>,
    language: Option<String>,
    #[serde(default)]
    segments: Vec<MistralTranscriptionSegment>,
}

#[derive(Debug, Deserialize)]
struct MistralTranscriptionSegment {
    text: String,
    #[serde(default)]
    speaker_id: Option<String>,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
}

fn format_diarized_text(segments: &[MistralTranscriptionSegment], fallback: &str) -> String {
    if segments.is_empty() {
        return fallback.to_string();
    }

    let mut lines = Vec::with_capacity(segments.len());
    for segment in segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }
        let speaker = segment
            .speaker_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Locuteur");
        match (segment.start, segment.end) {
            (Some(start), Some(end)) => {
                lines.push(format!("[{speaker} {start:.1}s–{end:.1}s] {text}"));
            }
            _ => lines.push(format!("[{speaker}] {text}")),
        }
    }

    if lines.is_empty() {
        fallback.to_string()
    } else {
        lines.join("\n")
    }
}

#[derive(Debug, Deserialize)]
struct MistralErrorResponse {
    message: Option<String>,
    detail: Option<String>,
}

fn extract_api_message(body: &str) -> Option<String> {
    serde_json::from_str::<MistralErrorResponse>(body)
        .ok()
        .and_then(|payload| payload.message.or(payload.detail))
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
        api_key: &str,
        audio_path: &Path,
        options: TranscriptionOptions,
    ) -> Result<TranscriptionResult, AiError> {
        if api_key.trim().is_empty() {
            return Err(AiError::Other(
                "Aucune clé API Mistral configurée.".to_string(),
            ));
        }

        let metadata = std::fs::metadata(audio_path).map_err(|err| {
            AiError::Other(format!("Impossible de lire le fichier audio : {err}"))
        })?;
        validate_transcription_audio_size(metadata.len())?;

        let model = options.model.as_deref().unwrap_or_else(|| {
            model_catalog::default_transcription_model("mistral").unwrap_or("voxtral-mini-latest")
        });
        let (file_name, mime) = Self::resolve_upload_name(&options);

        let file_part = reqwest::multipart::Part::file(audio_path)
            .await
            .map_err(|err| {
                AiError::Other(format!("Impossible de lire le fichier audio : {err}"))
            })?
            .file_name(file_name)
            .mime_str(mime)
            .map_err(|err| AiError::Other(err.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .text("model", model.to_string())
            .part("file", file_part);

        // Avec diarize/timestamps, Mistral déconseille de forcer `language`.
        if options.diarize {
            form = form
                .text("diarize", "true")
                .text("timestamp_granularities", "segment");
        } else if let Some(language) = &options.language {
            form = form.text("language", language.clone());
        }

        let response = self
            .client
            .post(format!("{}/audio/transcriptions", self.api_base))
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(Self::map_http_error(status, &body));
        }

        let payload: MistralTranscriptionResponse =
            serde_json::from_str(&body).map_err(|err| AiError::Provider {
                provider: self.id().to_string(),
                message: format!("réponse transcription illisible : {err}"),
            })?;

        let text = if options.diarize {
            format_diarized_text(&payload.segments, &payload.text)
        } else {
            payload.text
        };

        if text.trim().is_empty() {
            return Err(AiError::Provider {
                provider: self.id().to_string(),
                message: "Mistral a renvoyé une transcription vide.".to_string(),
            });
        }

        Ok(TranscriptionResult {
            text,
            model: payload.model.unwrap_or_else(|| model.to_string()),
            language: payload.language.or(options.language),
        })
    }
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
impl SummaryProvider for MistralProvider {
    async fn summarize(
        &self,
        api_key: &str,
        text: &str,
        options: SummaryOptions,
    ) -> Result<SummaryResult, AiError> {
        let model = options.model.unwrap_or_else(|| {
            model_catalog::default_summary_model("mistral")
                .unwrap_or("mistral-small-latest")
                .to_string()
        });

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
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&request)
            .send()
            .await?;

        if response.status() == StatusCode::UNAUTHORIZED {
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
    use std::path::PathBuf;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn write_temp_audio(name: &str, bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join(name);
        if bytes.is_empty() {
            std::fs::File::create(&path).expect("create empty file");
        } else {
            std::fs::write(&path, bytes).expect("write audio");
        }
        (dir, path)
    }

    fn opts(
        model: Option<&str>,
        language: Option<&str>,
        file_name: Option<&str>,
        diarize: bool,
    ) -> TranscriptionOptions {
        TranscriptionOptions {
            model: model.map(str::to_string),
            language: language.map(str::to_string),
            file_name: file_name.map(str::to_string),
            diarize,
        }
    }

    #[tokio::test]
    async fn validate_key_rejects_empty_key() {
        let provider = MistralProvider::new();
        let result = provider.validate_key("   ").await.expect("validation");
        assert!(!result.valid);
        assert!(result.message.contains("vide"));
    }

    #[tokio::test]
    async fn transcribe_rejects_empty_audio() {
        let (_dir, audio_path) = write_temp_audio("empty.wav", &[]);
        let provider = MistralProvider::new();
        let error = provider
            .transcribe("sk-test", &audio_path, opts(None, None, None, false))
            .await
            .unwrap_err();
        assert!(matches!(error, AiError::Other(_)));
    }

    #[tokio::test]
    async fn transcribe_returns_text_from_api() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "text": "Bonjour à tous",
                "model": "voxtral-mini-latest",
                "language": "fr"
            })))
            .mount(&mock_server)
            .await;

        let provider = MistralProvider::with_api_base(format!("{}/v1", mock_server.uri()));
        let (_dir, audio_path) = write_temp_audio("sample.wav", b"fake-audio-bytes");
        let result = provider
            .transcribe(
                "sk-test",
                &audio_path,
                opts(None, Some("fr"), Some("sample.wav"), false),
            )
            .await
            .expect("transcription");

        assert_eq!(result.text, "Bonjour à tous");
        assert_eq!(result.model, "voxtral-mini-latest");
        assert_eq!(result.language.as_deref(), Some("fr"));
    }

    #[tokio::test]
    async fn transcribe_formats_diarized_segments() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "text": "Bonjour. Salut.",
                "model": "voxtral-mini-latest",
                "segments": [
                    { "text": "Bonjour.", "speaker_id": "SPEAKER_00", "start": 0.0, "end": 1.0 },
                    { "text": "Salut.", "speaker_id": "SPEAKER_01", "start": 1.2, "end": 2.0 }
                ]
            })))
            .mount(&mock_server)
            .await;

        let provider = MistralProvider::with_api_base(format!("{}/v1", mock_server.uri()));
        let (_dir, audio_path) = write_temp_audio("sample.wav", b"fake-audio-bytes");
        let result = provider
            .transcribe(
                "sk-test",
                &audio_path,
                opts(None, Some("fr"), Some("sample.wav"), true),
            )
            .await
            .expect("diarized transcription");

        assert!(result.text.contains("[SPEAKER_00"));
        assert!(result.text.contains("[SPEAKER_01"));
        assert!(result.text.contains("Bonjour."));
    }

    #[tokio::test]
    async fn transcribe_maps_unauthorized_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "message": "Unauthorized"
            })))
            .mount(&mock_server)
            .await;

        let provider = MistralProvider::with_api_base(format!("{}/v1", mock_server.uri()));
        let (_dir, audio_path) = write_temp_audio("sample.mp3", b"fake-audio-bytes");
        let error = provider
            .transcribe(
                "sk-invalid",
                &audio_path,
                opts(None, None, Some("sample.mp3"), false),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, AiError::Provider { .. }));
        assert!(error.to_string().contains("Clé API invalide"));
    }

    #[tokio::test]
    async fn transcribe_maps_rate_limit_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/audio/transcriptions"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
            .mount(&mock_server)
            .await;

        let provider = MistralProvider::with_api_base(format!("{}/v1", mock_server.uri()));
        let (_dir, audio_path) = write_temp_audio("audio.wav", b"fake-audio-bytes");
        let error = provider
            .transcribe("sk-test", &audio_path, opts(None, None, None, false))
            .await
            .unwrap_err();

        assert!(error.to_string().contains("Limite de requêtes"));
    }
}
