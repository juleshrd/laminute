use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use crate::audio::AudioError;
use crate::db::AppState;
use crate::error::AppError;
use crate::local_activity::LocalActivityGate;
use crate::models::MeetingDetail;
use crate::repository::MeetingRepository;

#[tauri::command]
pub fn import_mp3_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    gate: State<'_, LocalActivityGate>,
    source_path: String,
) -> Result<MeetingDetail, AudioError> {
    gate.ensure_not_purging()
        .map_err(|err| AudioError::Internal(err.to_string()))?;

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AudioError::Io(err.to_string()))?;
    let imports_dir = app_data_dir.join("imports");
    let source = PathBuf::from(source_path);

    state
        .with_db(|conn| MeetingRepository::create_from_mp3_import(conn, &source, &imports_dir))
        .map_err(map_app_error)
}

fn map_app_error(error: AppError) -> AudioError {
    match error {
        AppError::Database(err) => AudioError::Internal(format!("base de données : {err}")),
        AppError::Migration(err) => AudioError::Internal(format!("migration : {err}")),
        AppError::Io(err) => AudioError::Io(err.to_string()),
        AppError::MeetingNotFound { id } => {
            AudioError::Internal(format!("réunion introuvable après import : {id}"))
        }
        AppError::Message(message) => AudioError::Internal(message),
    }
}
