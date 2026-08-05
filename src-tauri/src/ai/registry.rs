use std::collections::HashMap;
use std::sync::Arc;

use crate::ai::error::AiError;
use crate::ai::models::ProviderInfo;
use crate::ai::provider::AiProvider;
use crate::ai::providers::mistral::MistralProvider;

/// Registre central des fournisseurs IA. Ajouter un fournisseur ici suffit — aucun changement UI requis.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };

        registry.register(Arc::new(MistralProvider::new()));

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
}
