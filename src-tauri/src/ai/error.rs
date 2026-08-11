use thiserror::Error;

use crate::ai::secrets;
use crate::ai::settings;

#[derive(Debug, Error)]
pub enum AiError {
    #[error("fournisseur inconnu : {0}")]
    UnknownProvider(String),

    #[error("fonctionnalité non implémentée : {0}")]
    NotImplemented(String),

    #[error("erreur réseau : {0}")]
    Network(#[from] reqwest::Error),

    #[error("erreur fournisseur ({provider}) : {message}")]
    Provider { provider: String, message: String },

    #[error("erreur de stockage sécurisé : {0}")]
    Secret(#[from] secrets::SecretError),

    #[error("erreur de configuration : {0}")]
    Settings(#[from] settings::SettingsError),

    #[error("traitement IA annulé")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl AiError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnknownProvider(_) => "unknown_provider",
            Self::NotImplemented(_) => "not_implemented",
            Self::Network(_) => "network_error",
            Self::Provider { .. } => "provider_error",
            Self::Secret(_) => "secret_error",
            Self::Settings(_) => "settings_error",
            Self::Cancelled => "cancelled",
            Self::Other(_) => "other",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_error_codes_are_stable() {
        assert_eq!(AiError::Cancelled.code(), "cancelled");
        assert_eq!(AiError::Other("x".into()).code(), "other");
        assert_eq!(
            AiError::UnknownProvider("x".into()).code(),
            "unknown_provider"
        );
    }
}
