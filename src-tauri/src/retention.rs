use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use crate::audio::{paths::remove_owned, AudioState, ManagedAudioRoots};
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::repository::MeetingRepository;

/// Supprime les fichiers audio disque (racines gérées uniquement) puis les lignes DB.
/// En cas d'erreur disque, les lignes `audio_files` sont conservées.
pub fn purge_meeting_audio(
    roots: &ManagedAudioRoots,
    conn: &Connection,
    meeting_id: &str,
) -> AppResult<()> {
    let audio_files = MeetingRepository::list_audio_files(conn, meeting_id)?;
    for audio in &audio_files {
        remove_owned(std::path::Path::new(&audio.file_path), roots)
            .map_err(|e| AppError::Message(e.to_string()))?;
    }
    MeetingRepository::delete_audio_file_rows(conn, meeting_id)?;
    Ok(())
}

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
    let roots = ManagedAudioRoots::from_app(app).map_err(|e| AppError::Message(e.to_string()))?;
    db_state.with_db(|conn| purge_meeting_audio(&roots, conn, meeting_id))
}
