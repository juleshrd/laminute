use serde::{Deserialize, Serialize};

/// Capacités exposées par un fournisseur IA (flags consommables par le frontend).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    pub transcription: bool,
    pub summary: bool,
    pub local: bool,
    pub streaming: bool,
}

impl ProviderCapabilities {
    pub const fn none() -> Self {
        Self {
            transcription: false,
            summary: false,
            local: false,
            streaming: false,
        }
    }

    pub const fn mistral() -> Self {
        Self {
            transcription: true,
            summary: true,
            local: false,
            streaming: true,
        }
    }

    pub const fn openai() -> Self {
        Self {
            transcription: true,
            summary: true,
            local: false,
            streaming: false,
        }
    }

    pub const fn ollama() -> Self {
        Self {
            transcription: false,
            summary: true,
            local: true,
            streaming: false,
        }
    }
}
