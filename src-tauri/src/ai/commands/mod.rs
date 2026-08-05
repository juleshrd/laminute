use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::ai::models::{AiSettings, KeyValidationResult, ProviderInfo};
use crate::ai::secrets;
use crate::ai::settings::SettingsStore;
use crate::AiAppState;

pub mod transcription;

pub use transcription::TranscriptionState;

fn provider_has_api_key(provider_id: &str, has_stored_key: bool) -> bool {
    if provider_id == "ollama" {
        return true;
    }
    has_stored_key
}

fn build_ai_settings(
    settings: &SettingsStore,
    selected_provider_id: Option<String>,
) -> Result<AiSettings, String> {
    let has_api_key = selected_provider_id
        .as_deref()
        .map(|id| {
            secrets::has_api_key(id)
                .map(|stored| provider_has_api_key(id, stored))
                .map_err(|e| e.to_string())
        })
        .transpose()?
        .unwrap_or(false);

    Ok(AiSettings {
        selected_provider_id,
        has_api_key,
        ollama_base_url: Some(settings.ollama_base_url().to_string()),
    })
}

#[tauri::command]
pub fn list_ai_providers(state: State<'_, AiAppState>) -> Result<Vec<ProviderInfo>, String> {
    Ok(state.registry.list())
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AiAppState>) -> Result<AiSettings, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "verrou des réglages indisponible".to_string())?;

    let selected_provider_id = settings.selected_provider_id().map(str::to_string);
    build_ai_settings(&settings, selected_provider_id)
}

#[tauri::command]
pub fn set_selected_provider(
    state: State<'_, AiAppState>,
    provider_id: String,
) -> Result<AiSettings, String> {
    state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "verrou des réglages indisponible".to_string())?;
    settings
        .set_selected_provider_id(Some(provider_id.clone()))
        .map_err(|e| e.to_string())?;

    build_ai_settings(&settings, Some(provider_id))
}

#[tauri::command]
pub fn set_ollama_base_url(
    state: State<'_, AiAppState>,
    base_url: String,
) -> Result<AiSettings, String> {
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "verrou des réglages indisponible".to_string())?;
    settings
        .set_ollama_base_url(base_url.clone())
        .map_err(|e| e.to_string())?;

    state.registry.ollama().set_base_url(base_url);

    let selected_provider_id = settings.selected_provider_id().map(str::to_string);
    build_ai_settings(&settings, selected_provider_id)
}

#[tauri::command]
pub fn save_api_key(
    state: State<'_, AiAppState>,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    let provider = state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    if api_key.trim().is_empty() {
        if provider.capabilities().local {
            secrets::delete_api_key(&provider_id).map_err(|e| e.to_string())?;
            return Ok(());
        }
        return Err("La clé API ne peut pas être vide.".to_string());
    }

    secrets::store_api_key(&provider_id, api_key.trim())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_api_key(state: State<'_, AiAppState>, provider_id: String) -> Result<(), String> {
    state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    secrets::delete_api_key(&provider_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_api_key(
    state: State<'_, AiAppState>,
    provider_id: String,
    api_key: Option<String>,
) -> Result<KeyValidationResult, String> {
    let provider = state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    let key = match api_key {
        Some(key) if !key.trim().is_empty() => key,
        _ if provider.capabilities().local => String::new(),
        _ => secrets::get_api_key(&provider_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Aucune clé API fournie ni enregistrée.".to_string())?,
    };

    provider
        .validate_key(&key)
        .await
        .map_err(|e| e.to_string())
}

pub fn init_settings(app: &AppHandle) -> Result<Mutex<SettingsStore>, crate::ai::error::AiError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| {
            crate::ai::error::AiError::Settings(crate::ai::settings::SettingsError::Path(
                e.to_string(),
            ))
        })?;

    let store = SettingsStore::load(app_data_dir)?;
    Ok(Mutex::new(store))
}

pub fn sync_ollama_base_url(state: &AiAppState) {
    if let Ok(settings) = state.settings.lock() {
        state
            .registry
            .ollama()
            .set_base_url(settings.ollama_base_url().to_string());
    }
}
