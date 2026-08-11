use std::collections::HashMap;

use tauri::{AppHandle, State};

use crate::audio::paths::ManagedAudioRoots;
use crate::db::AppState;
use crate::models::{
    CreateMeetingInput, Meeting, MeetingDetail, MeetingSearchFilters, MeetingSearchPage,
    MeetingSummary, Summary, Transcription, TranscriptionMetadata,
};
use crate::repository::MeetingRepository;
use crate::retention;

#[tauri::command]
pub fn create_meeting(
    state: State<'_, AppState>,
    input: CreateMeetingInput,
) -> Result<Meeting, String> {
    state
        .with_db(|conn| MeetingRepository::create(conn, input))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_meeting(state: State<'_, AppState>, id: String) -> Result<MeetingDetail, String> {
    state
        .with_db(|conn| MeetingRepository::get_detail(conn, &id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_latest_summary(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Summary>, String> {
    state
        .with_db(|conn| MeetingRepository::latest_summary(conn, &id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_latest_transcription(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Transcription>, String> {
    state
        .with_db(|conn| MeetingRepository::latest_transcription(conn, &id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_transcription_versions(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<TranscriptionMetadata>, String> {
    state
        .with_db(|conn| MeetingRepository::list_transcription_versions(conn, &id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_meetings(state: State<'_, AppState>) -> Result<Vec<MeetingSummary>, String> {
    state
        .with_db(MeetingRepository::list)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_meetings(
    state: State<'_, AppState>,
    filters: MeetingSearchFilters,
) -> Result<MeetingSearchPage, String> {
    state
        .with_db(|conn| MeetingRepository::search(conn, &filters))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let roots = ManagedAudioRoots::from_app(&app).map_err(|e| e.to_string())?;
    state
        .with_db(|conn| {
            retention::purge_meeting_audio(&roots, conn, &id)?;
            MeetingRepository::delete(conn, &id)
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<Meeting, String> {
    state
        .with_db(|conn| MeetingRepository::update_title(conn, &id, &title))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_meeting_speaker_map(
    state: State<'_, AppState>,
    id: String,
    speaker_map: HashMap<String, String>,
) -> Result<Meeting, String> {
    state
        .with_db(|conn| MeetingRepository::update_speaker_map(conn, &id, &speaker_map))
        .map_err(|e| e.to_string())
}
