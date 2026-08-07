mod ai;
mod audio;
mod commands;
mod db;
mod error;
mod export_write;
mod local_activity;
mod models;
mod purge;
mod report_markdown;
mod report_pdf;
mod repository;
mod retention;

pub use db::open_in_memory;
pub use models::MeetingSearchFilters;
pub use repository::MeetingRepository;

pub const APP_IDENTIFIER: &str = "app.laminute.desktop";

use std::sync::Mutex;

use tauri::Manager;

use audio::{AudioError, AudioInputDevice, AudioState, RecordingStatus};
use commands::{
    create_meeting, delete_all_local_data, delete_meeting, export_meeting,
    generate_structured_summary, get_local_storage_info, get_meeting, import_mp3_meeting,
    list_meetings, save_meeting_export, search_meetings, update_meeting_title,
};
use db::open_and_migrate;
use local_activity::LocalActivityGate;

/// État IA (providers BYOK) — distinct de l'état SQLite et audio.
pub struct AiAppState {
    pub registry: ai::ProviderRegistry,
    pub settings: Mutex<ai::SettingsStore>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

#[tauri::command]
fn list_audio_input_devices(
    state: tauri::State<'_, AudioState>,
) -> Result<Vec<AudioInputDevice>, AudioError> {
    state.list_devices()
}

#[tauri::command]
fn get_selected_audio_input_device(
    state: tauri::State<'_, AudioState>,
) -> Result<Option<AudioInputDevice>, AudioError> {
    state.get_selected_device()
}

#[tauri::command]
fn set_selected_audio_input_device(
    state: tauri::State<'_, AudioState>,
    device_id: String,
) -> Result<AudioInputDevice, AudioError> {
    state.set_selected_device(device_id)
}

#[tauri::command]
fn start_microphone_recording(
    state: tauri::State<'_, AudioState>,
    gate: tauri::State<'_, LocalActivityGate>,
) -> Result<RecordingStatus, AudioError> {
    gate.ensure_not_purging()
        .map_err(|err| AudioError::Internal(err.to_string()))?;
    state.start_recording()
}

#[tauri::command]
fn stop_microphone_recording(
    state: tauri::State<'_, AudioState>,
) -> Result<RecordingStatus, AudioError> {
    state.stop_recording()
}

#[tauri::command]
fn get_recording_status(
    state: tauri::State<'_, AudioState>,
) -> Result<RecordingStatus, AudioError> {
    state.recording_status()
}

#[tauri::command]
fn get_keep_audio_files(state: tauri::State<'_, AudioState>) -> Result<bool, AudioError> {
    state.keep_audio_files()
}

#[tauri::command]
fn set_keep_audio_files(
    state: tauri::State<'_, AudioState>,
    keep: bool,
) -> Result<bool, AudioError> {
    state.set_keep_audio_files(keep)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("répertoire de données applicatives introuvable");
            let db_path = app_data_dir.join("laminute.db");
            let conn = open_and_migrate(&db_path).expect("initialisation SQLite");

            app.manage(db::AppState {
                db: Mutex::new(conn),
            });

            let settings = ai::commands::init_settings(app.handle())?;
            let ai_state = AiAppState {
                registry: ai::ProviderRegistry::new(),
                settings,
            };
            ai::commands::sync_ollama_base_url(&ai_state);
            app.manage(ai_state);
            app.manage(ai::TranscriptionState::new());
            app.manage(LocalActivityGate::new());

            let audio_state = AudioState::initialize(app.handle())?;
            app.manage(audio_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            create_meeting,
            get_meeting,
            list_meetings,
            search_meetings,
            delete_meeting,
            update_meeting_title,
            ai::commands::list_ai_providers,
            ai::commands::get_ai_settings,
            ai::commands::set_selected_provider,
            ai::commands::set_ollama_base_url,
            ai::commands::set_model_preferences,
            ai::commands::save_api_key,
            ai::commands::delete_api_key,
            ai::commands::validate_api_key,
            ai::commands::transcription::transcribe_audio_file,
            ai::commands::transcription::get_transcription_progress,
            list_audio_input_devices,
            get_selected_audio_input_device,
            set_selected_audio_input_device,
            start_microphone_recording,
            stop_microphone_recording,
            get_recording_status,
            get_keep_audio_files,
            set_keep_audio_files,
            import_mp3_meeting,
            generate_structured_summary,
            export_meeting,
            save_meeting_export,
            get_local_storage_info,
            delete_all_local_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::registry::ProviderRegistry;

    #[test]
    fn greet_formats_message() {
        assert_eq!(
            greet("La Minute"),
            "Hello, La Minute! You've been greeted from Rust!"
        );
    }

    #[test]
    fn registry_is_extensible_without_ui_changes() {
        let registry = ProviderRegistry::new();
        assert!(!registry.list().is_empty());
    }
}
