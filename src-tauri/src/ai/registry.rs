use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::ai::error::AiError;
use crate::ai::http;
use crate::ai::models::{
    ProviderInfo, SummaryOptions, SummaryResult, TranscriptionOptions, TranscriptionResult,
};
use crate::ai::provider::AiProvider;
use crate::ai::providers::mistral::{MistralProvider, MISTRAL_API_BASE};
use crate::ai::providers::ollama::{OllamaProvider, DEFAULT_OLLAMA_BASE};
use crate::ai::providers::openai::{OpenAiProvider, OPENAI_API_BASE};
use crate::ai::summary::SummaryProvider;
use crate::ai::transcription::TranscriptionProvider;

/// Registre central des fournisseurs IA. Ajouter un fournisseur ici suffit — aucun changement UI requis.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    transcription: HashMap<String, Arc<dyn TranscriptionProvider>>,
    summary: HashMap<String, Arc<dyn SummaryProvider>>,
    ollama: Arc<OllamaProvider>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let client = http::build_client();
        let mistral = Arc::new(MistralProvider::with_api_base(
            MISTRAL_API_BASE.to_string(),
            client.clone(),
        ));
        let openai = Arc::new(OpenAiProvider::with_api_base(
            OPENAI_API_BASE.to_string(),
            client.clone(),
        ));
        let ollama = Arc::new(OllamaProvider::with_base_url(
            DEFAULT_OLLAMA_BASE.to_string(),
            false,
            http::build_ollama_client(),
        ));

        let mut registry = Self {
            providers: HashMap::new(),
            transcription: HashMap::new(),
            summary: HashMap::new(),
            ollama: ollama.clone(),
        };

        registry.register_ai(mistral.clone());
        registry.register_transcription(mistral.clone());
        registry.register_summary(mistral);

        registry.register_ai(openai.clone());
        registry.register_transcription(openai.clone());
        registry.register_summary(openai);

        registry.register_ai(ollama.clone());
        registry.register_summary(ollama);

        registry
    }

    fn register_ai(&mut self, provider: Arc<dyn AiProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    fn register_transcription(&mut self, provider: Arc<dyn TranscriptionProvider>) {
        self.transcription
            .insert(provider.id().to_string(), provider);
    }

    fn register_summary(&mut self, provider: Arc<dyn SummaryProvider>) {
        self.summary.insert(provider.id().to_string(), provider);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(id).cloned()
    }

    pub fn list(&self) -> Vec<ProviderInfo> {
        let mut providers: Vec<_> = self.providers.values().map(|p| p.info()).collect();
        providers.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        providers
    }

    pub fn require(&self, id: &str) -> Result<Arc<dyn AiProvider>, AiError> {
        self.get(id)
            .ok_or_else(|| AiError::UnknownProvider(id.to_string()))
    }

    pub fn ollama(&self) -> &OllamaProvider {
        &self.ollama
    }

    pub async fn summarize_text(
        &self,
        provider_id: &str,
        api_key: &str,
        text: &str,
        options: SummaryOptions,
        cancel: &CancellationToken,
    ) -> Result<SummaryResult, AiError> {
        let provider = self.summary.get(provider_id).ok_or_else(|| {
            AiError::Other(format!(
                "le fournisseur « {provider_id} » ne prend pas en charge le résumé structuré"
            ))
        })?;
        SummaryProvider::summarize(provider.as_ref(), api_key, text, options, cancel).await
    }

    pub async fn transcribe_audio(
        &self,
        provider_id: &str,
        api_key: &str,
        audio_path: &Path,
        options: TranscriptionOptions,
        cancel: &CancellationToken,
    ) -> Result<TranscriptionResult, AiError> {
        let provider = self.transcription.get(provider_id).ok_or_else(|| {
            AiError::Other(format!(
                "le fournisseur « {provider_id} » ne prend pas en charge la transcription audio"
            ))
        })?;
        TranscriptionProvider::transcribe(provider.as_ref(), api_key, audio_path, options, cancel)
            .await
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::capabilities::ProviderCapabilities;
    use crate::ai::models::{KeyValidationResult, ModelInfo};
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    struct StubSummaryProvider;

    #[async_trait]
    impl AiProvider for StubSummaryProvider {
        fn id(&self) -> &str {
            "stub"
        }

        fn display_name(&self) -> &str {
            "Stub"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                transcription: false,
                summary: true,
                local: true,
                streaming: false,
                diarization: false,
            }
        }

        async fn validate_key(&self, _api_key: &str) -> Result<KeyValidationResult, AiError> {
            Ok(KeyValidationResult {
                valid: true,
                message: "ok".into(),
                models: None,
            })
        }

        async fn list_models(&self, _api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl SummaryProvider for StubSummaryProvider {
        async fn summarize(
            &self,
            _api_key: &str,
            text: &str,
            _options: SummaryOptions,
            _cancel: &CancellationToken,
        ) -> Result<SummaryResult, AiError> {
            Ok(SummaryResult {
                text: format!("stub:{text}"),
                model: "stub-model".into(),
            })
        }
    }

    #[test]
    fn registry_lists_all_providers() {
        let registry = ProviderRegistry::new();
        let providers = registry.list();
        assert_eq!(providers.len(), 3);
        let ids: Vec<_> = providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"mistral"));
        assert!(ids.contains(&"openai"));
        assert!(ids.contains(&"ollama"));
    }

    #[test]
    fn registry_returns_unknown_provider_error() {
        let registry = ProviderRegistry::new();
        let result = registry.require("unknown");
        assert!(matches!(result, Err(AiError::UnknownProvider(_))));
    }

    #[tokio::test]
    async fn summarize_text_rejects_unknown_provider() {
        let registry = ProviderRegistry::new();
        let result = registry
            .summarize_text(
                "unknown",
                "sk-test",
                "texte",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                },
                &CancellationToken::new(),
            )
            .await;
        assert!(matches!(result, Err(AiError::Other(_))));
    }

    #[tokio::test]
    async fn transcribe_audio_rejects_ollama() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio_path = dir.path().join("audio.wav");
        std::fs::write(&audio_path, b"audio").expect("write");

        let registry = ProviderRegistry::new();
        let result = registry
            .transcribe_audio(
                "ollama",
                "",
                &audio_path,
                TranscriptionOptions {
                    model: None,
                    language: None,
                    file_name: None,
                    diarize: false,
                },
                &CancellationToken::new(),
            )
            .await;
        assert!(matches!(result, Err(AiError::Other(_))));
    }

    #[tokio::test]
    async fn register_summary_stub_dispatches_without_match() {
        let mut registry = ProviderRegistry::new();
        let stub = Arc::new(StubSummaryProvider);
        registry.register_ai(stub.clone());
        registry.register_summary(stub);

        let result = registry
            .summarize_text(
                "stub",
                "",
                "hello",
                SummaryOptions {
                    model: None,
                    max_tokens: None,
                },
                &CancellationToken::new(),
            )
            .await
            .expect("stub summary");

        assert_eq!(result.text, "stub:hello");
        assert_eq!(result.model, "stub-model");
    }

    struct StubTranscriptionProvider;

    #[async_trait]
    impl AiProvider for StubTranscriptionProvider {
        fn id(&self) -> &str {
            "stub-transcribe"
        }

        fn display_name(&self) -> &str {
            "Stub Transcribe"
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                transcription: true,
                summary: false,
                local: false,
                streaming: false,
                diarization: false,
            }
        }

        async fn validate_key(&self, _api_key: &str) -> Result<KeyValidationResult, AiError> {
            Ok(KeyValidationResult {
                valid: true,
                message: "ok".into(),
                models: None,
            })
        }

        async fn list_models(&self, _api_key: &str) -> Result<Vec<ModelInfo>, AiError> {
            Ok(vec![])
        }
    }

    #[async_trait]
    impl TranscriptionProvider for StubTranscriptionProvider {
        async fn transcribe(
            &self,
            _api_key: &str,
            _audio_path: &Path,
            _options: TranscriptionOptions,
            _cancel: &CancellationToken,
        ) -> Result<TranscriptionResult, AiError> {
            Ok(TranscriptionResult {
                text: "stub transcription".into(),
                model: "stub-model".into(),
                language: None,
            })
        }
    }

    #[tokio::test]
    async fn register_transcription_stub_dispatches_without_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio_path = dir.path().join("audio.wav");
        std::fs::write(&audio_path, b"audio").expect("write");

        let mut registry = ProviderRegistry::new();
        let stub = Arc::new(StubTranscriptionProvider);
        registry.register_ai(stub.clone());
        registry.register_transcription(stub);

        let result = registry
            .transcribe_audio(
                "stub-transcribe",
                "",
                &audio_path,
                TranscriptionOptions {
                    model: None,
                    language: None,
                    file_name: None,
                    diarize: false,
                },
                &CancellationToken::new(),
            )
            .await
            .expect("stub transcription");

        assert_eq!(result.text, "stub transcription");
        assert_eq!(result.model, "stub-model");
    }
}
