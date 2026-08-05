mod commands;
mod db;
mod error;
mod models;
mod repository;

use std::sync::Mutex;

use tauri::Manager;

use commands::{create_meeting, delete_meeting, get_meeting, list_meetings};
use db::{open_and_migrate, AppState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("répertoire de données applicatives introuvable");
            let db_path = app_data_dir.join("laminute.db");
            let conn = open_and_migrate(&db_path).expect("initialisation SQLite");

            app.manage(AppState {
                db: Mutex::new(conn),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_meeting,
            get_meeting,
            list_meetings,
            delete_meeting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
