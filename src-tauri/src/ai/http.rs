use std::path::Path;
use std::time::Duration;

use reqwest::redirect;
use reqwest::StatusCode;
use serde::Deserialize;
use thiserror::Error;

use crate::ai::error::AiError;
use crate::ai::models::TranscriptionOptions;
use crate::ai::ollama_url;

#[derive(Debug, Error)]
#[error("redirection Ollama inter-origine ou vers une destination interdite")]
struct OllamaRedirectDenied;

/// Client HTTP partagé avec timeouts raisonnables pour les appels IA.
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .expect("reqwest client")
}

/// Client Ollama : mêmes timeouts, redirections limitées à la même origine.
pub fn build_ollama_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(30))
        .redirect(redirect::Policy::custom(|attempt| {
            let Some(original) = attempt.previous().first() else {
                return attempt.follow();
            };
            if ollama_url::same_origin(original, attempt.url())
                && ollama_url::is_allowed_redirect_target(attempt.url())
            {
                attempt.follow()
            } else {
                attempt.error(OllamaRedirectDenied)
            }
        }))
        .build()
        .expect("reqwest ollama client")
}

pub fn resolve_upload_name(options: &TranscriptionOptions) -> (String, &'static str) {
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

pub async fn audio_file_part(
    audio_path: &Path,
    file_name: String,
    mime: &str,
) -> Result<reqwest::multipart::Part, AiError> {
    reqwest::multipart::Part::file(audio_path)
        .await
        .map_err(|err| AiError::Other(format!("Impossible de lire le fichier audio : {err}")))?
        .file_name(file_name)
        .mime_str(mime)
        .map_err(|err| AiError::Other(err.to_string()))
}

pub fn map_http_error(status: StatusCode, body: &str, brand: &str, provider_id: &str) -> AiError {
    let message = match status {
        StatusCode::UNAUTHORIZED => "Clé API invalide ou expirée.".to_string(),
        StatusCode::PAYLOAD_TOO_LARGE => {
            format!("Fichier audio trop volumineux pour l'API {brand}.")
        }
        StatusCode::TOO_MANY_REQUESTS => {
            format!("Limite de requêtes {brand} atteinte — réessayez plus tard.")
        }
        StatusCode::UNSUPPORTED_MEDIA_TYPE => {
            format!("Format audio non supporté par {brand}.")
        }
        _ if status.is_client_error() => {
            extract_api_error_message(body).unwrap_or_else(|| format!("requête refusée ({status})"))
        }
        _ => extract_api_error_message(body)
            .unwrap_or_else(|| format!("erreur serveur {brand} ({status})")),
    };

    AiError::Provider {
        provider: provider_id.to_string(),
        message,
    }
}

#[derive(Debug, Deserialize)]
struct MistralStyleError {
    message: Option<String>,
    detail: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStyleError {
    error: Option<OpenAiErrorDetail>,
}

#[derive(Debug, Deserialize)]
struct OpenAiErrorDetail {
    message: Option<String>,
}

/// Extrait un message d'erreur depuis les formats JSON courants (Mistral, OpenAI, etc.).
pub fn extract_api_error_message(body: &str) -> Option<String> {
    if let Ok(payload) = serde_json::from_str::<OpenAiStyleError>(body) {
        if let Some(message) = payload.error.and_then(|e| e.message) {
            return Some(message);
        }
    }

    serde_json::from_str::<MistralStyleError>(body)
        .ok()
        .and_then(|payload| payload.message.or(payload.detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_http_error_unauthorized() {
        let err = map_http_error(StatusCode::UNAUTHORIZED, "", "Mistral", "mistral");
        assert!(matches!(
            &err,
            AiError::Provider { provider, .. } if provider == "mistral"
        ));
        assert!(err.to_string().contains("Clé API invalide"));
    }

    #[test]
    fn map_http_error_rate_limit_uses_brand() {
        let err = map_http_error(StatusCode::TOO_MANY_REQUESTS, "", "OpenAI", "openai");
        assert!(err.to_string().contains("Limite de requêtes OpenAI"));
    }

    #[test]
    fn map_http_error_extracts_mistral_message() {
        let body = r#"{"message":"quota exceeded"}"#;
        let err = map_http_error(StatusCode::BAD_REQUEST, body, "Mistral", "mistral");
        assert!(err.to_string().contains("quota exceeded"));
    }

    #[test]
    fn map_http_error_extracts_openai_message() {
        let body = r#"{"error":{"message":"invalid model"}}"#;
        let err = map_http_error(StatusCode::BAD_REQUEST, body, "OpenAI", "openai");
        assert!(err.to_string().contains("invalid model"));
    }
}
