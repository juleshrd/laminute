use tauri::{AppHandle, State};

use crate::audio::paths::ManagedAudioRoots;
use crate::db::AppState;
use crate::models::{
    CreateMeetingInput, Meeting, MeetingDetail, MeetingSearchFilters, MeetingSearchPage,
    MeetingSummary,
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
