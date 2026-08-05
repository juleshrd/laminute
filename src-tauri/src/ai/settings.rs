use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("impossible de résoudre le répertoire de données : {0}")]
    Path(String),

    #[error("erreur d'accès au fichier de configuration : {0}")]
    Io(#[from] std::io::Error),

    #[error("erreur de sérialisation : {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct PersistedAiSettings {
    selected_provider_id: Option<String>,
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

    pub fn set_selected_provider_id(&mut self, provider_id: Option<String>) -> Result<(), SettingsError> {
        self.data.selected_provider_id = provider_id;
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
