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
mod storage;

pub use db::open_in_memory;
pub use models::MeetingSearchFilters;
pub use repository::MeetingRepository;

pub const APP_IDENTIFIER: &str = "app.laminute.desktop";

use std::sync::Mutex;

use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use audio::{AudioError, AudioInputDevice, AudioInputSetup, AudioState, RecordingStatus};
use commands::{
    create_meeting, delete_all_local_data, delete_meeting, export_meeting,
    generate_structured_summary, get_latest_summary, get_latest_transcription,
    get_local_storage_info, get_meeting, import_mp3_meeting, list_meetings,
    list_transcription_versions, save_meeting_export, search_meetings, update_meeting_title,
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
fn set_selected_audio_input_device(
    state: tauri::State<'_, AudioState>,
    device_id: String,
) -> Result<AudioInputDevice, AudioError> {
    state.set_selected_device(device_id)
}

#[tauri::command]
fn prepare_audio_input(state: tauri::State<'_, AudioState>) -> Result<AudioInputSetup, AudioError> {
    state.prepare_input()
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

fn show_startup_storage_error(app: &tauri::AppHandle, message: &str) {
    let app = app.clone();
    let message = message.to_string();
    let _ = std::thread::spawn(move || {
        app.dialog()
            .message(message)
            .title("Stockage local inaccessible")
            .kind(MessageDialogKind::Error)
            .blocking_show();
    })
    .join();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let default_app_data_dir = app
                .path()
                .app_data_dir()
                .expect("répertoire de données applicatives introuvable");
            let storage = match storage::StorageState::load(default_app_data_dir) {
                Ok(storage) => storage,
                Err(message) => {
                    show_startup_storage_error(app.handle(), &message);
                    return Err(std::io::Error::other(message).into());
                }
            };
            let app_data_dir = storage.root();
            let roots = audio::ManagedAudioRoots::from_app_data_dir(app_data_dir.clone());
            roots.ensure_dirs()?;
            app.asset_protocol_scope()
                .allow_directory(&roots.imports_dir, true)?;
            app.asset_protocol_scope()
                .allow_directory(&roots.recordings_dir, true)?;

            let db_path = app_data_dir.join("laminute.db");
            let conn = open_and_migrate(&db_path).expect("initialisation SQLite");
            ai::reconcile::reconcile_ai_jobs(&conn).expect("réconciliation jobs IA");

            app.manage(storage);
            app.manage(storage::StorageSelectionState::default());
            app.manage(db::AppState {
                db: Mutex::new(conn),
            });

            let settings = Mutex::new(ai::SettingsStore::load(app_data_dir.clone())?);
            let ai_state = AiAppState {
                registry: ai::ProviderRegistry::new(),
                settings,
            };
            ai::commands::sync_ollama_base_url(&ai_state);
            app.manage(ai_state);
            app.manage(ai::TranscriptionState::new());
            app.manage(ai::jobs::AiJobState::new());
            app.manage(LocalActivityGate::new());

            let audio_state = AudioState::initialize(app_data_dir.clone())?;
            app.manage(audio_state);

            // Nettoyage best-effort des imports interrompus (JUL-184).
            if let Err(err) = audio::import::cleanup_staging(&roots.imports_dir) {
                eprintln!("[startup] nettoyage staging imports : {err}");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            create_meeting,
            get_meeting,
            get_latest_summary,
            get_latest_transcription,
            list_transcription_versions,
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
            ai::commands::transcription::cancel_ai_job,
            ai::commands::recovery::get_ai_recovery_actions,
            ai::commands::recovery::resume_transcription_for_meeting,
            ai::commands::recovery::resume_summary_for_meeting,
            set_selected_audio_input_device,
            prepare_audio_input,
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
            storage::choose_local_storage_parent,
            storage::prepare_local_storage_change,
            storage::apply_local_storage_change,
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
