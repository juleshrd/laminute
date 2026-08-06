use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::ai::error::AiError;
use crate::ai::models::{
    ProviderInfo, SummaryOptions, SummaryResult, TranscriptionOptions, TranscriptionResult,
};
use crate::ai::provider::AiProvider;
use crate::ai::providers::mistral::MistralProvider;
use crate::ai::providers::ollama::OllamaProvider;
use crate::ai::providers::openai::OpenAiProvider;
use crate::ai::summary::SummaryProvider;
use crate::ai::transcription::TranscriptionProvider;

/// Registre central des fournisseurs IA. Ajouter un fournisseur ici suffit — aucun changement UI requis.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    mistral: Arc<MistralProvider>,
    openai: Arc<OpenAiProvider>,
    ollama: Arc<OllamaProvider>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mistral = Arc::new(MistralProvider::new());
        let openai = Arc::new(OpenAiProvider::new());
        let ollama = Arc::new(OllamaProvider::new());
        let mut registry = Self {
            providers: HashMap::new(),
            mistral: mistral.clone(),
            openai: openai.clone(),
            ollama: ollama.clone(),
        };

        registry.register(mistral);
        registry.register(openai);
        registry.register(ollama);

        registry
    }

    pub fn register(&mut self, provider: Arc<dyn AiProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
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
    ) -> Result<SummaryResult, AiError> {
        match provider_id {
            "mistral" => {
                SummaryProvider::summarize(self.mistral.as_ref(), api_key, text, options).await
            }
            "openai" => {
                SummaryProvider::summarize(self.openai.as_ref(), api_key, text, options).await
            }
            "ollama" => {
                SummaryProvider::summarize(self.ollama.as_ref(), api_key, text, options).await
            }
            _ => Err(AiError::Other(format!(
                "le fournisseur « {provider_id} » ne prend pas en charge le résumé structuré"
            ))),
        }
    }

    pub async fn transcribe_audio(
        &self,
        provider_id: &str,
        api_key: &str,
        audio_path: &Path,
        options: TranscriptionOptions,
    ) -> Result<TranscriptionResult, AiError> {
        match provider_id {
            "mistral" => {
                TranscriptionProvider::transcribe(
                    self.mistral.as_ref(),
                    api_key,
                    audio_path,
                    options,
                )
                .await
            }
            "openai" => {
                TranscriptionProvider::transcribe(
                    self.openai.as_ref(),
                    api_key,
                    audio_path,
                    options,
                )
                .await
            }
            _ => Err(AiError::Other(format!(
                "le fournisseur « {provider_id} » ne prend pas en charge la transcription audio"
            ))),
        }
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
            )
            .await;
        assert!(matches!(result, Err(AiError::Other(_))));
    }
}
