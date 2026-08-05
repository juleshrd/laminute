use serde::{Deserialize, Serialize};

/// Configuration d'un fournisseur IA.
/// Les secrets sont stockés dans le trousseau OS via `credential_key_id`, jamais en base.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: Option<String>,
    pub model_default: Option<String>,
    pub is_enabled: bool,
    pub credential_key_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
