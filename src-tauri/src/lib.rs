mod ai;
mod commands;
mod db;
mod error;
mod models;
mod repository;

pub const APP_IDENTIFIER: &str = "app.laminute.desktop";

use std::sync::Mutex;

use tauri::Manager;

use commands::{create_meeting, delete_meeting, get_meeting, list_meetings};
use db::open_and_migrate;

/// État IA (providers BYOK) — distinct de l'état SQLite.
pub struct AiAppState {
    pub registry: ai::ProviderRegistry,
    pub settings: Mutex<ai::SettingsStore>,
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {name}! You've been greeted from Rust!")
}

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

            app.manage(db::AppState {
                db: Mutex::new(conn),
            });

            let settings = ai::commands::init_settings(app.handle())?;
            app.manage(AiAppState {
                registry: ai::ProviderRegistry::new(),
                settings,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            create_meeting,
            get_meeting,
            list_meetings,
            delete_meeting,
            ai::commands::list_ai_providers,
            ai::commands::get_ai_settings,
            ai::commands::set_selected_provider,
            ai::commands::save_api_key,
            ai::commands::delete_api_key,
            ai::commands::validate_api_key,
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
