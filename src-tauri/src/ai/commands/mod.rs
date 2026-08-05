use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

use crate::ai::models::{AiSettings, KeyValidationResult, ProviderInfo};
use crate::ai::secrets;
use crate::ai::settings::SettingsStore;
use crate::AiAppState;

pub mod transcription;

pub use transcription::TranscriptionState;

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
    let has_api_key = selected_provider_id
        .as_deref()
        .map(secrets::has_api_key)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(false);

    Ok(AiSettings {
        selected_provider_id,
        has_api_key,
    })
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

    let has_api_key = secrets::has_api_key(&provider_id).map_err(|e| e.to_string())?;

    Ok(AiSettings {
        selected_provider_id: Some(provider_id),
        has_api_key,
    })
}

#[tauri::command]
pub fn save_api_key(
    state: State<'_, AiAppState>,
    provider_id: String,
    api_key: String,
) -> Result<(), String> {
    state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    if api_key.trim().is_empty() {
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
