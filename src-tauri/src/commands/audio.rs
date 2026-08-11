use std::path::PathBuf;
use std::time::Instant;

use tauri::{AppHandle, State};

use crate::audio::import::{
    discard_imported_file, stage_mp3_import, title_from_path, ImportedAudio,
};
use crate::audio::paths::ManagedAudioRoots;
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
    let activity = gate
        .begin_operation()
        .map_err(|err| AudioError::Internal(err.to_string()))?;

    let roots = ManagedAudioRoots::from_app(&app)?;
    roots.ensure_dirs()?;

    let source = PathBuf::from(source_path);
    let title = title_from_path(&source);

    // Validation + copie hors du verrou SQLite (fichiers jusqu'à 100 Mo).
    let staged = stage_mp3_import(&source, &roots.imports_dir)?;

    if let Err(err) = gate.ensure_generation(activity) {
        discard_imported_file(&staged.imported.dest_path);
        return Err(AudioError::Internal(err.to_string()));
    }

    finalize_mp3_import(&state, &title, &staged.imported).inspect_err(|_| {
        discard_imported_file(&staged.imported.dest_path);
    })
}

/// Insertion DB courte sous mutex ; l'appelant doit compenser (effacer le fichier) en cas d'erreur.
pub fn finalize_mp3_import(
    state: &AppState,
    title: &str,
    imported: &ImportedAudio,
) -> Result<MeetingDetail, AudioError> {
    let started = Instant::now();
    let detail = state
        .with_db(|conn| MeetingRepository::create_from_imported_audio(conn, title, imported))
        .map_err(map_app_error)?;
    let _db_elapsed_ms = started.elapsed().as_millis();
    // Garde la mesure disponible en debug (critère JUL-184 avant/après mutex).
    #[cfg(debug_assertions)]
    {
        eprintln!(
            "[import_mp3] mutex SQLite tenu {_db_elapsed_ms} ms (hors validation/copie disque)"
        );
    }
    Ok(detail)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    use crate::audio::import::{cleanup_staging, staging_dir};
    use crate::db::open_in_memory;
    use crate::models::MeetingStatus;

    fn fixture_mp3() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-1s.mp3")
    }

    #[test]
    fn finalize_compensates_by_caller_when_title_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staged = stage_mp3_import(&fixture_mp3(), &imports).unwrap();
        let state = AppState {
            db: Mutex::new(open_in_memory().unwrap()),
        };

        let err = finalize_mp3_import(&state, "   ", &staged.imported).unwrap_err();
        assert!(err.to_string().contains("titre"));
        discard_imported_file(&staged.imported.dest_path);
        assert!(!staged.imported.dest_path.exists());
        assert_eq!(cleanup_staging(&imports).unwrap(), 0);
        assert!(
            !staging_dir(&imports).exists()
                || fs::read_dir(staging_dir(&imports))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn happy_path_creates_meeting_under_brief_mutex() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staged = stage_mp3_import(&fixture_mp3(), &imports).unwrap();
        let state = AppState {
            db: Mutex::new(open_in_memory().unwrap()),
        };

        let detail = finalize_mp3_import(&state, "Comité", &staged.imported).unwrap();
        assert_eq!(detail.meeting.title, "Comité");
        assert_eq!(detail.meeting.status, MeetingStatus::Processing);
        assert!(staged.imported.dest_path.exists());
    }

    #[test]
    fn history_readable_while_disk_phase_runs_outside_mutex() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        use crate::models::CreateMeetingInput;
        use crate::repository::MeetingRepository;

        let state = Arc::new(AppState {
            db: Mutex::new(open_in_memory().unwrap()),
        });
        state
            .with_db(|conn| {
                MeetingRepository::create(
                    conn,
                    CreateMeetingInput {
                        title: "Déjà là".into(),
                        description: None,
                    },
                )
            })
            .unwrap();

        let reading = Arc::new(AtomicBool::new(false));
        let reading_flag = reading.clone();
        let state_reader = state.clone();
        let reader = thread::spawn(move || {
            // Attendre que la « copie » démarre hors mutex.
            thread::sleep(Duration::from_millis(20));
            let listed = state_reader
                .with_db(MeetingRepository::list)
                .expect("list pendant import disque");
            assert!(listed.iter().any(|m| m.title == "Déjà là"));
            reading_flag.store(true, Ordering::SeqCst);
        });

        // Simule une phase disque longue hors with_db.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && !reading.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            reading.load(Ordering::SeqCst),
            "la liste historique doit aboutir pendant la phase disque hors mutex"
        );

        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staged = stage_mp3_import(&fixture_mp3(), &imports).unwrap();
        finalize_mp3_import(state.as_ref(), "Nouveau", &staged.imported).unwrap();
        reader.join().unwrap();
    }
}
