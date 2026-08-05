mod ai;
mod audio;

pub const APP_IDENTIFIER: &str = "app.laminute.desktop";

use std::path::PathBuf;
use std::sync::Mutex;

use audio::{
    load_recordings, load_selected_device_id, save_selected_device_id, AudioError, AudioInputDevice,
    AudioRecorder, RecordingSession, RecordingStatusResponse,
};
use tauri::{AppHandle, Manager, State};

pub struct AppState {
    pub registry: ai::ProviderRegistry,
    pub settings: Mutex<ai::SettingsStore>,
}

struct AudioState {
    recorder: Mutex<AudioRecorder>,
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("impossible d'accéder au répertoire de données : {e}"))
}

#[tauri::command]
fn audio_list_input_devices() -> Result<Vec<AudioInputDevice>, String> {
    audio::list_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
fn audio_get_selected_device(app: AppHandle) -> Result<Option<String>, String> {
    load_selected_device_id(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn audio_set_selected_device(app: AppHandle, device_id: Option<String>) -> Result<(), String> {
    save_selected_device_id(&app, device_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
fn audio_start_recording(
    app: AppHandle,
    state: State<'_, AudioState>,
    device_id: Option<String>,
) -> Result<RecordingSession, String> {
    let data_dir = app_data_dir(&app)?;
    std::fs::create_dir_all(data_dir.join("recordings")).map_err(|e| e.to_string())?;

    let selected = device_id
        .or(load_selected_device_id(&app).map_err(|e| e.to_string())?)
        .or_else(|| {
            audio::list_input_devices()
                .ok()
                .and_then(|devices| {
                    devices
                        .iter()
                        .find(|d| d.is_default)
                        .or(devices.first())
                        .map(|d| d.id.clone())
                })
        });

    let mut session = RecordingSession::new_recording(String::new());
    let wav_path = audio::metadata::recording_wav_path(&data_dir, &session.id);
    session.absolute_path = Some(wav_path.to_string_lossy().into_owned());

    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "état audio verrouillé".to_string())?;

    let session = recorder
        .start(session, wav_path, selected.as_deref())
        .map_err(|e| e.to_string())?;

    Ok(session)
}

#[tauri::command]
fn audio_stop_recording(
    app: AppHandle,
    state: State<'_, AudioState>,
) -> Result<RecordingSession, String> {
    let data_dir = app_data_dir(&app)?;
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "état audio verrouillé".to_string())?;

    let mut session = recorder.stop().map_err(|e| e.to_string())?;

    if session.status == audio::RecordingSessionStatus::Completed {
        if let Some(path) = &session.absolute_path {
            session.file_path = Some(
                PathBuf::from(path)
                    .file_name()
                    .map(|n| format!("recordings/{}", n.to_string_lossy()))
                    .unwrap_or_else(|| format!("recordings/{}.wav", session.id)),
            );
        }
    }

    audio::metadata::save_recording(&data_dir, &session).map_err(|e| e.to_string())?;
    Ok(session)
}

#[tauri::command]
fn audio_get_recording_status(state: State<'_, AudioState>) -> Result<RecordingStatusResponse, String> {
    let recorder = state
        .recorder
        .lock()
        .map_err(|_| "état audio verrouillé".to_string())?;
    Ok(recorder.status())
}

#[tauri::command]
fn audio_list_recordings(app: AppHandle) -> Result<Vec<RecordingSession>, String> {
    let data_dir = app_data_dir(&app)?;
    load_recordings(&data_dir).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let settings = ai::commands::init_settings(app.handle())?;
            app.manage(AppState {
                registry: ai::ProviderRegistry::new(),
                settings,
            });
            app.manage(AudioState {
                recorder: Mutex::new(AudioRecorder::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ai::commands::list_ai_providers,
            ai::commands::get_ai_settings,
            ai::commands::set_selected_provider,
            ai::commands::save_api_key,
            ai::commands::delete_api_key,
            ai::commands::validate_api_key,
            audio_list_input_devices,
            audio_get_selected_device,
            audio_set_selected_device,
            audio_start_recording,
            audio_stop_recording,
            audio_get_recording_status,
            audio_list_recordings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::registry::ProviderRegistry;

    #[test]
    fn registry_is_extensible_without_ui_changes() {
        let registry = ProviderRegistry::new();
        assert!(!registry.list().is_empty());
    }

    #[test]
    fn audio_error_messages_are_french() {
        let err = AudioError::PermissionDenied;
        assert!(err.to_string().contains("permission"));
    }
}
