use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai::providers::ollama::DEFAULT_OLLAMA_BASE;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("impossible de résoudre le répertoire de données : {0}")]
    Path(String),

    #[error("erreur d'accès au fichier de configuration : {0}")]
    Io(#[from] std::io::Error),

    #[error("erreur de sérialisation : {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAiSettings {
    selected_provider_id: Option<String>,
    #[serde(default = "default_ollama_base_url")]
    ollama_base_url: String,
}

fn default_ollama_base_url() -> String {
    DEFAULT_OLLAMA_BASE.to_string()
}

impl Default for PersistedAiSettings {
    fn default() -> Self {
        Self {
            selected_provider_id: None,
            ollama_base_url: default_ollama_base_url(),
        }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    data: PersistedAiSettings,
}

impl SettingsStore {
    pub fn load(app_data_dir: PathBuf) -> Result<Self, SettingsError> {
        let path = app_data_dir.join("ai-settings.json");
        let data = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw)?
        } else {
            PersistedAiSettings::default()
        };

        Ok(Self { path, data })
    }

    pub fn selected_provider_id(&self) -> Option<&str> {
        self.data.selected_provider_id.as_deref()
    }

    pub fn ollama_base_url(&self) -> &str {
        &self.data.ollama_base_url
    }

    pub fn set_selected_provider_id(
        &mut self,
        provider_id: Option<String>,
    ) -> Result<(), SettingsError> {
        self.data.selected_provider_id = provider_id;
        self.save()
    }

    pub fn set_ollama_base_url(&mut self, base_url: String) -> Result<(), SettingsError> {
        let trimmed = base_url.trim();
        self.data.ollama_base_url = if trimmed.is_empty() {
            default_ollama_base_url()
        } else {
            trimmed.to_string()
        };
        self.save()
    }

    fn save(&self) -> Result<(), SettingsError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = serde_json::to_string_pretty(&self.data)?;
        fs::write(&self.path, raw)?;
        Ok(())
    }
}
