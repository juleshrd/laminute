use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ai::model_catalog;
use crate::ai::models::ProviderModelPreferences;
use crate::ai::ollama_url::{self, OllamaUrlError};
use crate::ai::providers::ollama::DEFAULT_OLLAMA_BASE;

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("impossible de résoudre le répertoire de données : {0}")]
    Path(String),

    #[error("erreur d'accès au fichier de configuration : {0}")]
    Io(#[from] std::io::Error),

    #[error("erreur de sérialisation : {0}")]
    Serde(#[from] serde_json::Error),

    #[error("{0}")]
    Validation(String),
}

impl From<OllamaUrlError> for SettingsError {
    fn from(value: OllamaUrlError) -> Self {
        Self::Validation(value.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAiSettings {
    selected_provider_id: Option<String>,
    #[serde(default = "default_ollama_base_url")]
    ollama_base_url: String,
    #[serde(default)]
    ollama_allow_remote: bool,
    #[serde(default)]
    diarization_enabled: bool,
    #[serde(default)]
    provider_models: HashMap<String, ProviderModelPreferences>,
}

fn default_ollama_base_url() -> String {
    DEFAULT_OLLAMA_BASE.to_string()
}

fn sanitize_ollama_settings(data: &mut PersistedAiSettings) {
    match ollama_url::normalize(&data.ollama_base_url, data.ollama_allow_remote) {
        Ok(normalized) => {
            data.ollama_base_url = normalized.into_string();
        }
        Err(_) => {
            data.ollama_base_url = default_ollama_base_url();
            data.ollama_allow_remote = false;
        }
    }
}

impl Default for PersistedAiSettings {
    fn default() -> Self {
        Self {
            selected_provider_id: None,
            ollama_base_url: default_ollama_base_url(),
            ollama_allow_remote: false,
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
        let mut data = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_json::from_str(&raw)?
        } else {
            PersistedAiSettings::default()
        };
        sanitize_ollama_settings(&mut data);

        Ok(Self { path, data })
    }

    pub fn selected_provider_id(&self) -> Option<&str> {
        self.data.selected_provider_id.as_deref()
    }

    pub fn ollama_base_url(&self) -> &str {
        &self.data.ollama_base_url
    }

    pub fn ollama_allow_remote(&self) -> bool {
        self.data.ollama_allow_remote
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

    /// Valide, normalise et persiste l'URL Ollama. Retourne la valeur normalisée
    /// (identique en mémoire et sur disque).
    pub fn set_ollama_base_url(
        &mut self,
        base_url: String,
        allow_remote: bool,
    ) -> Result<String, SettingsError> {
        let normalized = ollama_url::normalize(&base_url, allow_remote)?;
        self.data.ollama_base_url = normalized.as_str().to_string();
        self.data.ollama_allow_remote = allow_remote;
        self.save()?;
        Ok(self.data.ollama_base_url.clone())
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

    #[test]
    fn set_ollama_base_url_persists_normalized_value() {
        let dir = tempdir().expect("tempdir");
        let mut store = SettingsStore::load(dir.path().to_path_buf()).expect("load");
        let normalized = store
            .set_ollama_base_url(" http://127.0.0.1:11434/ ".into(), false)
            .expect("set url");
        assert_eq!(normalized, "http://127.0.0.1:11434");
        assert_eq!(store.ollama_base_url(), "http://127.0.0.1:11434");

        let reloaded = SettingsStore::load(dir.path().to_path_buf()).expect("reload");
        assert_eq!(reloaded.ollama_base_url(), "http://127.0.0.1:11434");
        assert!(!reloaded.ollama_allow_remote());
    }

    #[test]
    fn load_resets_invalid_or_remote_without_opt_in() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("ai-settings.json");
        fs::write(
            &path,
            r#"{"ollamaBaseUrl":"http://192.168.1.10:11434","ollamaAllowRemote":false}"#,
        )
        .expect("write");
        let store = SettingsStore::load(dir.path().to_path_buf()).expect("load");
        assert_eq!(store.ollama_base_url(), DEFAULT_OLLAMA_BASE);
        assert!(!store.ollama_allow_remote());
    }
}
