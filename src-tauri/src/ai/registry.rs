use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::error::AiError;
use crate::ai::models::{ProviderInfo, SummaryOptions, SummaryResult};
use crate::ai::provider::AiProvider;
use crate::ai::providers::mistral::MistralProvider;
use crate::ai::summary::SummaryProvider;

/// Registre central des fournisseurs IA. Ajouter un fournisseur ici suffit — aucun changement UI requis.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
    mistral: Arc<MistralProvider>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mistral = Arc::new(MistralProvider::new());
        let mut registry = Self {
            providers: HashMap::new(),
            mistral: mistral.clone(),
        };

        registry.register(mistral);

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

    pub async fn summarize_text(
        &self,
        provider_id: &str,
        api_key: &str,
        text: &str,
        options: SummaryOptions,
    ) -> Result<SummaryResult, AiError> {
        if provider_id == "mistral" {
            return SummaryProvider::summarize(self.mistral.as_ref(), api_key, text, options).await;
        }

        Err(AiError::Other(format!(
            "le fournisseur « {provider_id} » ne prend pas en charge le résumé structuré"
        )))
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
    fn registry_lists_mistral_provider() {
        let registry = ProviderRegistry::new();
        let providers = registry.list();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "mistral");
        assert!(providers[0].capabilities.transcription);
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
}
