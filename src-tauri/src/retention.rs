use tauri::{AppHandle, Manager};

use crate::audio::AudioState;
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::repository::MeetingRepository;

pub fn maybe_purge_audio_files(
    app: &AppHandle,
    db_state: &AppState,
    meeting_id: &str,
) -> AppResult<()> {
    let Some(audio_state) = app.try_state::<AudioState>() else {
        return Ok(());
    };
    let keep = audio_state
        .keep_audio_files()
        .map_err(|e| AppError::Message(e.to_string()))?;
    if keep {
        return Ok(());
    }
    db_state.with_db(|conn| MeetingRepository::delete_audio_files_for_meeting(conn, meeting_id))
}
