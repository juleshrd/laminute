mod audio;

use audio::{AudioError, AudioInputDevice, AudioState, RecordingStatus};
use tauri::Manager;

#[tauri::command]
fn list_audio_input_devices(state: tauri::State<'_, AudioState>) -> Result<Vec<AudioInputDevice>, AudioError> {
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
) -> Result<RecordingStatus, AudioError> {
    state.start_recording()
}

#[tauri::command]
fn stop_microphone_recording(
    state: tauri::State<'_, AudioState>,
) -> Result<RecordingStatus, AudioError> {
    state.stop_recording()
}

#[tauri::command]
fn get_recording_status(state: tauri::State<'_, AudioState>) -> Result<RecordingStatus, AudioError> {
    state.recording_status()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let audio_state = AudioState::initialize(app.handle())?;
            app.manage(audio_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_audio_input_devices,
            get_selected_audio_input_device,
            set_selected_audio_input_device,
            start_microphone_recording,
            stop_microphone_recording,
            get_recording_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
