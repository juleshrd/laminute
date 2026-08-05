use async_trait::async_trait;

use crate::ai::capabilities::ProviderCapabilities;
use crate::ai::error::AiError;
use crate::ai::models::{KeyValidationResult, ModelInfo, ProviderInfo};

/// Contrat commun à tous les fournisseurs IA.
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn id(&self) -> &str;

    fn display_name(&self) -> &str;

    fn capabilities(&self) -> ProviderCapabilities;

    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: self.id().to_string(),
            display_name: self.display_name().to_string(),
            capabilities: self.capabilities(),
        }
    }

    /// Vérifie qu'une clé API est utilisable auprès du fournisseur.
    async fn validate_key(&self, api_key: &str) -> Result<KeyValidationResult, AiError>;

    /// Liste les modèles disponibles pour une clé donnée.
    async fn list_models(&self, api_key: &str) -> Result<Vec<ModelInfo>, AiError>;
}
