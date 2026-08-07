//! Inventaire et purge exhaustive des données locales (JUL-176).

use std::fs;
#[cfg(test)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::ai::secrets::{self, SecretError};
use crate::ai::settings::SettingsStore;
use crate::audio::paths::{try_remove_owned, ManagedAudioRoots};
use crate::db::{open_and_migrate, AppState};
use crate::error::{AppError, AppResult};
use crate::local_activity::LocalActivityGate;

pub const AI_SETTINGS_FILE: &str = "ai-settings.json";
pub const AUDIO_SETTINGS_FILE: &str = "audio-settings.json";
pub const DB_FILE: &str = "laminute.db";

/// Chemins de l'inventaire local à purger.
#[derive(Debug, Clone)]
pub struct LocalDataPaths {
    pub app_data_dir: PathBuf,
}

impl LocalDataPaths {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    pub fn db_path(&self) -> PathBuf {
        self.app_data_dir.join(DB_FILE)
    }

    pub fn ai_settings_path(&self) -> PathBuf {
        self.app_data_dir.join(AI_SETTINGS_FILE)
    }

    pub fn audio_settings_path(&self) -> PathBuf {
        self.app_data_dir.join(AUDIO_SETTINGS_FILE)
    }

    pub fn roots(&self) -> ManagedAudioRoots {
        ManagedAudioRoots::from_app_data_dir(self.app_data_dir.clone())
    }
}

/// Paramètres de la purge (testable hors AppHandle Tauri).
pub struct PurgeRequest<'a> {
    pub paths: &'a LocalDataPaths,
    pub db: &'a AppState,
    pub gate: &'a LocalActivityGate,
    pub ai_settings: Option<&'a Mutex<SettingsStore>>,
    pub provider_ids: &'a [String],
    pub stop_recording: Option<&'a dyn Fn() -> AppResult<()>>,
    pub reset_audio_memory: Option<&'a dyn Fn() -> AppResult<()>>,
    pub reset_transcription: Option<&'a dyn Fn() -> AppResult<()>>,
    /// Si faux, ignore le trousseau (tests unitaires hors Secret Service).
    pub clear_secrets: bool,
}

/// Exécute la purge exhaustive. Toute erreur disque/keyring est remontée.
pub fn purge_all_local_data(req: PurgeRequest<'_>) -> AppResult<()> {
    let _purge_guard = req.gate.begin_purge();

    if let Some(stop) = req.stop_recording {
        stop()?;
    }

    let roots = req.paths.roots();
    roots
        .ensure_dirs()
        .map_err(|err| AppError::Message(err.to_string()))?;

    remove_tracked_audio_files(req.db, &roots)?;
    clear_directory_contents(&roots.imports_dir)?;
    clear_directory_contents(&roots.recordings_dir)?;
    recreate_sqlite_database(req.db, &req.paths.db_path())?;

    remove_if_exists(&req.paths.ai_settings_path())?;
    remove_if_exists(&req.paths.audio_settings_path())?;

    if let Some(settings) = req.ai_settings {
        let mut store = settings.lock().map_err(|_| {
            AppError::Message("impossible d'accéder aux réglages IA en mémoire".into())
        })?;
        store.reset_in_memory();
    }

    if let Some(reset) = req.reset_audio_memory {
        reset()?;
    }

    if req.clear_secrets {
        clear_provider_secrets(req.provider_ids)?;
    }

    if let Some(reset) = req.reset_transcription {
        reset()?;
    }

    Ok(())
}

/// Supprime les secrets des fournisseurs connus ; les absences sont ignorées.
pub fn clear_provider_secrets(provider_ids: &[String]) -> AppResult<()> {
    for provider_id in provider_ids {
        secrets::delete_api_key(provider_id).map_err(map_secret_error)?;
    }
    Ok(())
}

fn map_secret_error(err: SecretError) -> AppError {
    AppError::Message(format!("trousseau : {err}"))
}

fn remove_tracked_audio_files(db: &AppState, roots: &ManagedAudioRoots) -> AppResult<()> {
    let paths = db.with_db(|conn| {
        let mut stmt = conn.prepare("SELECT file_path FROM audio_files")?;
        let paths = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paths)
    })?;

    for path in paths {
        try_remove_owned(Path::new(&path), roots)
            .map_err(|err| AppError::Message(err.to_string()))?;
    }
    Ok(())
}

fn recreate_sqlite_database(state: &AppState, db_path: &Path) -> AppResult<()> {
    {
        let mut db = state
            .db
            .lock()
            .map_err(|_| AppError::Message("impossible d'accéder à la base de données".into()))?;
        // Libérer le fichier avant suppression (pages libres / sentinelle).
        let replacement = Connection::open_in_memory()?;
        let old = std::mem::replace(&mut *db, replacement);
        drop(old);
    }

    remove_sqlite_sidecar_files(db_path)?;

    {
        let mut db = state
            .db
            .lock()
            .map_err(|_| AppError::Message("impossible d'accéder à la base de données".into()))?;
        *db = open_and_migrate(db_path)?;
    }
    Ok(())
}

fn remove_sqlite_sidecar_files(db_path: &Path) -> AppResult<()> {
    let path_str = db_path.to_string_lossy();
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let candidate = if suffix.is_empty() {
            db_path.to_path_buf()
        } else {
            PathBuf::from(format!("{path_str}{suffix}"))
        };
        remove_if_exists(&candidate)?;
    }
    Ok(())
}

fn clear_directory_contents(dir: &Path) -> AppResult<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn remove_if_exists(path: &Path) -> AppResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AppError::Io(err)),
    }
}

/// Parcourt récursivement un répertoire à la recherche d'une séquence d'octets.
#[cfg(test)]
pub fn directory_contains_bytes(root: &Path, needle: &[u8]) -> std::io::Result<bool> {
    if !root.exists() {
        return Ok(false);
    }
    if root.is_file() {
        return file_contains_bytes(root, needle);
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            if directory_contains_bytes(&path, needle)? {
                return Ok(true);
            }
        } else if file_contains_bytes(&path, needle)? {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
fn file_contains_bytes(path: &Path, needle: &[u8]) -> std::io::Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(buf.windows(needle.len()).any(|w| w == needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::settings::SettingsStore;
    use crate::db::open_and_migrate;
    use crate::models::CreateMeetingInput;
    use crate::repository::MeetingRepository;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use tempfile::tempdir;

    const SENTINEL: &str = "JUL176-SENTINEL-SECRET-XYZ";

    fn seed_local_data(paths: &LocalDataPaths) -> AppState {
        fs::create_dir_all(&paths.app_data_dir).unwrap();
        let roots = paths.roots();
        roots.ensure_dirs().unwrap();

        let audio_path = roots.imports_dir.join("sentinel.mp3");
        fs::write(&audio_path, format!("ID3{SENTINEL}audio")).unwrap();

        let ai_raw = serde_json::to_string_pretty(&serde_json::json!({
            "selectedProviderId": "mistral",
            "ollamaBaseUrl": "http://localhost:11434",
            "diarizationEnabled": true,
            "providerModels": {
                "mistral": {
                    "transcriptionModel": SENTINEL,
                    "summaryModel": "mistral-medium-latest"
                }
            }
        }))
        .unwrap();
        fs::write(paths.ai_settings_path(), ai_raw).unwrap();
        // Fichier parasite hors inventaire nommé — doit aussi disparaître via scan sentinelle
        // uniquement s'il est sous imports/recordings ; on place la sentinelle dans les fichiers connus.

        fs::write(
            paths.audio_settings_path(),
            format!(r#"{{"selectedDeviceId":"{SENTINEL}","keepAudioFiles":false}}"#),
        )
        .unwrap();

        let conn = open_and_migrate(&paths.db_path()).unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: SENTINEL.into(),
                description: Some(format!("desc {SENTINEL}")),
            },
        )
        .unwrap();
        MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            &audio_path.to_string_lossy(),
            Some(1000),
            Some("mp3"),
        )
        .unwrap();

        AppState {
            db: Mutex::new(conn),
        }
    }

    #[test]
    fn purge_all_local_data_removes_sentinel_everywhere() {
        let tmp = tempdir().unwrap();
        let paths = LocalDataPaths::new(tmp.path().join("appdata"));
        let db = seed_local_data(&paths);
        let gate = LocalActivityGate::new();
        let settings = Mutex::new(SettingsStore::load(paths.app_data_dir.clone()).unwrap());

        assert!(
            directory_contains_bytes(&paths.app_data_dir, SENTINEL.as_bytes()).unwrap(),
            "précondition : sentinelle présente"
        );

        purge_all_local_data(PurgeRequest {
            paths: &paths,
            db: &db,
            gate: &gate,
            ai_settings: Some(&settings),
            provider_ids: &[],
            stop_recording: None,
            reset_audio_memory: None,
            reset_transcription: None,
            clear_secrets: false,
        })
        .expect("purge");

        let count: i64 = db
            .with_db(|conn| {
                conn.query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(count, 0);

        assert!(!paths.ai_settings_path().exists());
        assert!(!paths.audio_settings_path().exists());
        assert!(paths
            .roots()
            .imports_dir
            .read_dir()
            .unwrap()
            .next()
            .is_none());
        assert!(paths
            .roots()
            .recordings_dir
            .read_dir()
            .unwrap()
            .next()
            .is_none());

        let settings_guard = settings.lock().unwrap();
        assert!(settings_guard.selected_provider_id().is_none());
        assert!(!settings_guard.diarization_enabled());
        drop(settings_guard);

        assert!(
            !directory_contains_bytes(&paths.app_data_dir, SENTINEL.as_bytes()).unwrap(),
            "sentinelle encore récupérable après purge"
        );

        // Redémarrage simulé : recharger DB + réglages depuis le disque.
        let reopened = open_and_migrate(&paths.db_path()).unwrap();
        let restarted_count: i64 = reopened
            .query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(restarted_count, 0);
        let reloaded = SettingsStore::load(paths.app_data_dir.clone()).unwrap();
        assert!(reloaded.selected_provider_id().is_none());
    }

    #[test]
    fn purge_propagates_directory_errors() {
        let tmp = tempdir().unwrap();
        let paths = LocalDataPaths::new(tmp.path().join("appdata"));
        fs::create_dir_all(&paths.app_data_dir).unwrap();
        let roots = paths.roots();
        roots.ensure_dirs().unwrap();

        let conn = open_and_migrate(&paths.db_path()).unwrap();
        let db = AppState {
            db: Mutex::new(conn),
        };
        let gate = LocalActivityGate::new();

        // Remplacer imports/ par un fichier pour forcer une erreur de lecture.
        fs::remove_dir_all(&roots.imports_dir).unwrap();
        fs::write(&roots.imports_dir, b"not-a-dir").unwrap();

        let err = purge_all_local_data(PurgeRequest {
            paths: &paths,
            db: &db,
            gate: &gate,
            ai_settings: None,
            provider_ids: &[],
            stop_recording: None,
            reset_audio_memory: None,
            reset_transcription: None,
            clear_secrets: false,
        })
        .expect_err("doit échouer");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn race_purge_vs_in_flight_db_write() {
        let tmp = tempdir().unwrap();
        let paths = LocalDataPaths::new(tmp.path().join("appdata"));
        let db = Arc::new(seed_local_data(&paths));
        let gate = Arc::new(LocalActivityGate::new());

        let token = gate.begin_operation().unwrap();

        let db_purge = Arc::clone(&db);
        let gate_purge = Arc::clone(&gate);
        let paths_purge = paths.clone();
        let purge_thread = thread::spawn(move || {
            purge_all_local_data(PurgeRequest {
                paths: &paths_purge,
                db: &db_purge,
                gate: &gate_purge,
                ai_settings: None,
                provider_ids: &[],
                stop_recording: None,
                reset_audio_memory: None,
                reset_transcription: None,
                clear_secrets: false,
            })
        });

        // Simuler une transcription qui revalide avant d'écrire.
        while !gate.is_purging() {
            thread::yield_now();
        }
        let aborted = gate.ensure_generation(token);
        assert!(aborted.is_err(), "l'écriture doit être refusée après purge");

        purge_thread.join().unwrap().expect("purge ok");

        let count: i64 = db
            .with_db(|conn| {
                conn.query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
                    .map_err(AppError::from)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn race_purge_calls_stop_recording_hook() {
        let tmp = tempdir().unwrap();
        let paths = LocalDataPaths::new(tmp.path().join("appdata"));
        let db = seed_local_data(&paths);
        let gate = LocalActivityGate::new();
        let stopped = Mutex::new(false);

        purge_all_local_data(PurgeRequest {
            paths: &paths,
            db: &db,
            gate: &gate,
            ai_settings: None,
            provider_ids: &[],
            stop_recording: Some(&|| {
                *stopped.lock().unwrap() = true;
                Ok(())
            }),
            reset_audio_memory: None,
            reset_transcription: None,
            clear_secrets: false,
        })
        .unwrap();

        assert!(*stopped.lock().unwrap());
    }

    #[test]
    fn clear_directory_removes_nested_orphan() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("imports");
        fs::create_dir_all(dir.join("nested")).unwrap();
        let mut f = fs::File::create(dir.join("nested").join("orphan.bin")).unwrap();
        write!(f, "{SENTINEL}").unwrap();

        clear_directory_contents(&dir).unwrap();
        assert!(dir.read_dir().unwrap().next().is_none());
        assert!(!directory_contains_bytes(tmp.path(), SENTINEL.as_bytes()).unwrap());
    }

    #[test]
    #[ignore = "nécessite un trousseau système accessible (Keychain / Secret Service)"]
    fn clear_provider_secrets_removes_keyring_entries() {
        let provider = "jul176-purge-test-provider";
        let _ = secrets::delete_api_key(provider);
        secrets::store_api_key(provider, "sk-sentinel-jul176").expect("store");
        clear_provider_secrets(&[provider.to_string()]).expect("clear");
        assert!(secrets::get_api_key(provider).expect("get").is_none());
    }
}
