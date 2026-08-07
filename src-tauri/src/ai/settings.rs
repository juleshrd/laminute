use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai::model_catalog;
use crate::ai::models::ProviderModelPreferences;
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
    #[serde(default)]
    diarization_enabled: bool,
    #[serde(default)]
    provider_models: HashMap<String, ProviderModelPreferences>,
}

fn default_ollama_base_url() -> String {
    DEFAULT_OLLAMA_BASE.to_string()
}

impl Default for PersistedAiSettings {
    fn default() -> Self {
        Self {
            selected_provider_id: None,
            ollama_base_url: default_ollama_base_url(),
            diarization_enabled: false,
            provider_models: HashMap::new(),
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

    pub fn diarization_enabled(&self) -> bool {
        self.data.diarization_enabled
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

    pub fn set_diarization_enabled(&mut self, enabled: bool) -> Result<(), SettingsError> {
        self.data.diarization_enabled = enabled;
        self.save()
    }

    /// Remet les préférences en mémoire aux valeurs par défaut (fichier déjà supprimé).
    pub fn reset_in_memory(&mut self) {
        self.data = PersistedAiSettings::default();
    }

    pub fn transcription_model_for(&self, provider_id: &str) -> Option<String> {
        self.data
            .provider_models
            .get(provider_id)
            .and_then(|prefs| prefs.transcription_model.clone())
            .or_else(|| model_catalog::default_transcription_model(provider_id).map(str::to_string))
    }

    pub fn summary_model_for(&self, provider_id: &str) -> Option<String> {
        self.data
            .provider_models
            .get(provider_id)
            .and_then(|prefs| prefs.summary_model.clone())
            .or_else(|| model_catalog::default_summary_model(provider_id).map(str::to_string))
    }

    pub fn set_provider_models(
        &mut self,
        provider_id: &str,
        transcription_model: Option<String>,
        summary_model: Option<String>,
    ) -> Result<(), SettingsError> {
        let entry = self
            .data
            .provider_models
            .entry(provider_id.to_string())
            .or_insert_with(|| ProviderModelPreferences {
                transcription_model: None,
                summary_model: None,
            });

        if let Some(model) = transcription_model {
            let trimmed = model.trim().to_string();
            entry.transcription_model = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }

        if let Some(model) = summary_model {
            let trimmed = model.trim().to_string();
            entry.summary_model = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persists_model_preferences_per_provider() {
        let dir = tempdir().expect("tempdir");
        let mut store = SettingsStore::load(dir.path().to_path_buf()).expect("load");
        store
            .set_provider_models(
                "mistral",
                Some("voxtral-mini-latest".into()),
                Some("mistral-medium-latest".into()),
            )
            .expect("save");
        store.set_diarization_enabled(true).expect("diarize");

        let reloaded = SettingsStore::load(dir.path().to_path_buf()).expect("reload");
        assert_eq!(
            reloaded.transcription_model_for("mistral").as_deref(),
            Some("voxtral-mini-latest")
        );
        assert_eq!(
            reloaded.summary_model_for("mistral").as_deref(),
            Some("mistral-medium-latest")
        );
        assert!(reloaded.diarization_enabled());
        assert_eq!(
            reloaded.summary_model_for("openai").as_deref(),
            Some("gpt-4o-mini")
        );
    }
}
