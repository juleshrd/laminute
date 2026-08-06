use tauri::State;

use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateMeetingInput, Meeting, MeetingDetail, MeetingListItem, MeetingSearchFilters,
    MeetingSummary, Summary, Transcription,
};
use crate::repository::MeetingRepository;

#[tauri::command]
pub fn create_meeting(
    state: State<'_, AppState>,
    input: CreateMeetingInput,
) -> Result<Meeting, String> {
    with_db(&state, |conn| MeetingRepository::create(conn, input)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_meeting(state: State<'_, AppState>, id: String) -> Result<MeetingDetail, String> {
    with_db(&state, |conn| MeetingRepository::get_detail(conn, &id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_transcription(state: State<'_, AppState>, id: String) -> Result<Transcription, String> {
    with_db(&state, |conn| MeetingRepository::get_transcription(conn, &id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_summary(state: State<'_, AppState>, id: String) -> Result<Summary, String> {
    with_db(&state, |conn| MeetingRepository::get_summary(conn, &id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_meetings(state: State<'_, AppState>) -> Result<Vec<MeetingSummary>, String> {
    with_db(&state, MeetingRepository::list).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_meetings(
    state: State<'_, AppState>,
    filters: MeetingSearchFilters,
) -> Result<Vec<MeetingListItem>, String> {
    with_db(&state, |conn| MeetingRepository::search(conn, &filters)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_meeting(state: State<'_, AppState>, id: String) -> Result<(), String> {
    with_db(&state, |conn| MeetingRepository::delete(conn, &id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_meeting_title(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> Result<Meeting, String> {
    with_db(&state, |conn| {
        MeetingRepository::update_title(conn, &id, &title)
    })
    .map_err(|e| e.to_string())
}

fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> AppResult<T>
where
    F: FnOnce(&rusqlite::Connection) -> AppResult<T>,
{
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Message("impossible d'accéder à la base de données".into()))?;
    f(&db)
}
