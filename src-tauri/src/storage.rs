use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::Command;
use std::sync::{Mutex, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::ai::SettingsStore;
use crate::audio::{AudioState, ManagedAudioRoots};
use crate::db::{open_and_migrate, AppState};
use crate::local_activity::LocalActivityGate;
use crate::AiAppState;

const CONFIG_FILE: &str = "storage-config.json";
const CONFIG_VERSION: u32 = 1;
const MARKER_FILE: &str = ".laminute-storage.json";
const DB_FILE: &str = "laminute.db";
const MIN_FREE_AFTER_MIGRATION: u64 = 10 * 1024 * 1024;
const MANAGED_FILES: &[&str] = &[
    DB_FILE,
    "laminute.db-wal",
    "laminute.db-shm",
    "ai-settings.json",
    "audio-settings.json",
];
const MANAGED_DIRS: &[&str] = &["imports", "recordings"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageConfig {
    version: u32,
    data_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageMarker {
    version: u32,
    application: String,
}

#[derive(Debug)]
pub struct StorageState {
    root: RwLock<PathBuf>,
    default_root: PathBuf,
    config_path: PathBuf,
}

impl StorageState {
    pub fn load(default_root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&default_root).map_err(|err| {
            format!(
                "Impossible de préparer le dossier de données par défaut « {} » : {err}",
                default_root.display()
            )
        })?;
        let default_root = canonical_existing_dir(&default_root)?;
        let config_path = default_root.join(CONFIG_FILE);
        let configured_root = resolve_configured_root(&default_root, &config_path)?;
        validate_active_root(&configured_root, &default_root)?;
        let root = canonical_existing_dir(&configured_root)?;
        Ok(Self {
            root: RwLock::new(root),
            default_root,
            config_path,
        })
    }

    pub fn root(&self) -> PathBuf {
        self.root
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn default_root(&self) -> &Path {
        &self.default_root
    }

    pub fn is_custom(&self) -> bool {
        self.root() != self.default_root
    }

    pub fn ensure_accessible(&self) -> Result<(), String> {
        validate_active_root(&self.root(), &self.default_root)
    }

    fn activate(&self, root: PathBuf) {
        *self
            .root
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = root;
    }

    fn persist_destination(&self, destination: &Path) -> Result<(), String> {
        if destination == self.default_root {
            return match fs::remove_file(&self.config_path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "Impossible de rétablir le dossier de stockage par défaut : {err}"
                )),
            };
        }

        let config = StorageConfig {
            version: CONFIG_VERSION,
            data_root: destination.to_path_buf(),
        };
        write_json_atomically(&self.config_path, &config).map_err(|err| {
            format!("Impossible d’enregistrer le nouvel emplacement de stockage : {err}")
        })
    }
}

#[derive(Debug, Clone)]
struct PreparedStorageChange {
    source: PathBuf,
    destination: PathBuf,
}

#[derive(Default)]
pub struct StorageSelectionState {
    prepared: Mutex<HashMap<String, PreparedStorageChange>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageChangePreview {
    pub token: String,
    pub current_path: String,
    pub destination_path: String,
    pub data_bytes: u64,
    pub available_bytes: u64,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageChangeResult {
    pub root_dir: String,
    pub moved_bytes: u64,
    pub source_cleanup_warning: Option<String>,
}

#[tauri::command]
pub async fn choose_local_storage_parent(app: AppHandle) -> Result<Option<String>, String> {
    let selected = app
        .dialog()
        .file()
        .set_title("Choisir l’emplacement du dossier La Minute")
        .blocking_pick_folder();
    selected
        .map(|path| {
            path.into_path()
                .map(|path| path.to_string_lossy().to_string())
                .map_err(|err| format!("Le dossier sélectionné est invalide : {err}"))
        })
        .transpose()
}

#[tauri::command]
pub fn prepare_local_storage_change(
    storage: State<'_, StorageState>,
    selections: State<'_, StorageSelectionState>,
    selected_parent: Option<String>,
    use_default: bool,
) -> Result<StorageChangePreview, String> {
    let source = canonical_existing_dir(&storage.root())?;
    let destination = if use_default {
        storage.default_root().to_path_buf()
    } else {
        let parent = selected_parent
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| "Aucun dossier parent n’a été sélectionné.".to_string())?;
        canonical_existing_dir(Path::new(&parent))?.join("La Minute")
    };

    validate_destination(&source, &destination, storage.default_root())?;
    let write_test_root = if destination.exists() {
        destination.as_path()
    } else {
        destination
            .parent()
            .ok_or_else(|| "Le dossier sélectionné n’a pas de parent accessible.".to_string())?
    };
    verify_write_access(write_test_root)?;

    let data_bytes = managed_size(&source)?;
    let available_bytes = available_space(write_test_root)?;
    let required = data_bytes.saturating_add(MIN_FREE_AFTER_MIGRATION);
    if available_bytes < required {
        return Err(format!(
            "Espace insuffisant : {} octets sont disponibles, {} octets sont nécessaires en incluant la marge de sécurité.",
            available_bytes, required
        ));
    }

    let token = Uuid::new_v4().to_string();
    selections
        .prepared
        .lock()
        .map_err(|_| "Impossible de mémoriser le dossier sélectionné.".to_string())?
        .insert(
            token.clone(),
            PreparedStorageChange {
                source: source.clone(),
                destination: destination.clone(),
            },
        );

    Ok(StorageChangePreview {
        token,
        current_path: source.to_string_lossy().to_string(),
        destination_path: destination.to_string_lossy().to_string(),
        data_bytes,
        available_bytes,
        is_default: destination == storage.default_root(),
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn apply_local_storage_change(
    app: AppHandle,
    storage: State<'_, StorageState>,
    selections: State<'_, StorageSelectionState>,
    db_state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
    audio_state: State<'_, AudioState>,
    gate: State<'_, LocalActivityGate>,
    token: String,
) -> Result<StorageChangeResult, String> {
    let prepared = selections
        .prepared
        .lock()
        .map_err(|_| "Impossible d’accéder au dossier sélectionné.".to_string())?
        .remove(&token)
        .ok_or_else(|| {
            "Cette sélection de dossier a expiré. Choisissez de nouveau le dossier.".to_string()
        })?;

    let current = canonical_existing_dir(&storage.root())?;
    if current != prepared.source {
        return Err("Le dossier de stockage a changé depuis la sélection. Recommencez.".into());
    }
    validate_destination(&current, &prepared.destination, storage.default_root())?;

    let write_test_root = if prepared.destination.exists() {
        prepared.destination.as_path()
    } else {
        prepared
            .destination
            .parent()
            .ok_or_else(|| "Le dossier sélectionné n’a pas de parent accessible.".to_string())?
    };
    verify_write_access(write_test_root)?;
    let moved_bytes = managed_size(&current)?;
    let available_bytes = available_space(write_test_root)?;
    if available_bytes < moved_bytes.saturating_add(MIN_FREE_AFTER_MIGRATION) {
        return Err(
            "L’espace disponible a changé depuis la confirmation. La migration est annulée.".into(),
        );
    }

    let _migration_guard = gate.begin_purge().map_err(|err| err.to_string())?;
    audio_state
        .stop_recording_if_active()
        .map_err(|err| format!("Impossible d’arrêter l’enregistrement avant migration : {err}"))?;

    let mut db = db_state.db.lock().map_err(|_| {
        "Impossible de verrouiller la base de données pour la migration.".to_string()
    })?;
    let mut ai_settings = ai_state
        .settings
        .lock()
        .map_err(|_| "Impossible de verrouiller les réglages IA pour la migration.".to_string())?;

    let migration = prepare_destination_data(&current, &prepared.destination, &db);
    let (destination_db, destination_ai_settings) = match migration {
        Ok(value) => value,
        Err(err) => {
            cleanup_partial_destination(&prepared.destination, storage.default_root());
            return Err(err);
        }
    };
    if let Err(err) = audio_state.persist_to_root(&prepared.destination) {
        cleanup_partial_destination(&prepared.destination, storage.default_root());
        return Err(format!(
            "Impossible de vérifier les réglages audio migrés : {err}"
        ));
    }

    let roots = ManagedAudioRoots::from_app_data_dir(prepared.destination.clone());
    if let Err(err) = roots.ensure_dirs() {
        cleanup_partial_destination(&prepared.destination, storage.default_root());
        return Err(err.to_string());
    }
    if let Err(err) = app
        .asset_protocol_scope()
        .allow_directory(&roots.imports_dir, true)
        .and_then(|_| {
            app.asset_protocol_scope()
                .allow_directory(&roots.recordings_dir, true)
        })
    {
        cleanup_partial_destination(&prepared.destination, storage.default_root());
        return Err(format!(
            "Impossible d’autoriser la lecture des audios migrés : {err}"
        ));
    }

    if let Err(err) = storage.persist_destination(&prepared.destination) {
        cleanup_partial_destination(&prepared.destination, storage.default_root());
        return Err(err);
    }

    *db = destination_db;
    *ai_settings = destination_ai_settings;
    audio_state.relocate(prepared.destination.clone());
    storage.activate(prepared.destination.clone());

    let source_cleanup_warning = cleanup_source_data(&current, storage.default_root()).err();

    Ok(StorageChangeResult {
        root_dir: prepared.destination.to_string_lossy().to_string(),
        moved_bytes,
        source_cleanup_warning,
    })
}

fn resolve_configured_root(default_root: &Path, config_path: &Path) -> Result<PathBuf, String> {
    if !config_path.exists() {
        return Ok(default_root.to_path_buf());
    }
    let raw = fs::read_to_string(config_path).map_err(|err| {
        format!(
            "Impossible de lire la configuration du stockage « {} » : {err}",
            config_path.display()
        )
    })?;
    let config: StorageConfig = serde_json::from_str(&raw)
        .map_err(|err| format!("La configuration du stockage est invalide : {err}"))?;
    if config.version != CONFIG_VERSION || !config.data_root.is_absolute() {
        return Err("La configuration du stockage local est invalide ou obsolète.".into());
    }
    Ok(config.data_root)
}

fn validate_active_root(root: &Path, default_root: &Path) -> Result<(), String> {
    if root != default_root {
        let canonical = canonical_existing_dir(root).map_err(|_| {
            format!(
                "Le dossier de stockage choisi « {} » est inaccessible ou indisponible. Reconnectez le disque ou restaurez l’accès avant de relancer La Minute.",
                root.display()
            )
        })?;
        validate_marker(&canonical)?;
    }
    verify_write_access(root).map_err(|_| {
        format!(
            "Le dossier de stockage « {} » n’est pas accessible en lecture et écriture.",
            root.display()
        )
    })
}

fn canonical_existing_dir(path: &Path) -> Result<PathBuf, String> {
    let meta = fs::symlink_metadata(path).map_err(|err| {
        format!(
            "Le dossier « {} » est inaccessible ou indisponible : {err}",
            path.display()
        )
    })?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(format!(
            "Le chemin « {} » doit être un dossier réel, pas un lien symbolique.",
            path.display()
        ));
    }
    fs::canonicalize(path).map_err(|err| {
        format!(
            "Impossible de résoudre le dossier « {} » : {err}",
            path.display()
        )
    })
}

fn validate_destination(
    source: &Path,
    destination: &Path,
    default_root: &Path,
) -> Result<(), String> {
    let normalized_source = canonical_existing_dir(source)?;
    let normalized_destination = if destination.exists() {
        canonical_existing_dir(destination)?
    } else {
        destination.to_path_buf()
    };
    if normalized_source == normalized_destination {
        return Err("Ce dossier est déjà utilisé pour le stockage local.".into());
    }
    if normalized_destination.starts_with(&normalized_source)
        || normalized_source.starts_with(&normalized_destination)
    {
        return Err(
            "Le nouveau dossier ne peut pas contenir l’ancien dossier de stockage, ni l’inverse."
                .into(),
        );
    }

    if normalized_destination.exists() {
        for entry in fs::read_dir(&normalized_destination).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            let name = entry.file_name();
            let ignored_default_config =
                normalized_destination == default_root && name.to_string_lossy() == CONFIG_FILE;
            if !ignored_default_config {
                return Err(format!(
                    "Le dossier cible « {} » n’est pas vide. Choisissez un autre emplacement pour éviter tout écrasement.",
                    normalized_destination.display()
                ));
            }
        }
    }
    Ok(())
}

fn verify_write_access(path: &Path) -> Result<(), String> {
    let probe = path.join(format!(".laminute-write-test-{}", Uuid::new_v4()));
    let result = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .and_then(|mut file| file.write_all(b"La Minute"));
    match result {
        Ok(()) => fs::remove_file(&probe).map_err(|err| {
            format!(
                "Le test d’écriture a réussi mais son fichier temporaire ne peut pas être supprimé : {err}"
            )
        }),
        Err(err) => {
            let _ = fs::remove_file(&probe);
            Err(format!(
                "Le dossier « {} » n’est pas accessible en écriture : {err}",
                path.display()
            ))
        }
    }
}

fn managed_size(root: &Path) -> Result<u64, String> {
    let mut total = 0u64;
    for name in MANAGED_FILES {
        total = total.saturating_add(path_size(&root.join(name))?);
    }
    for name in MANAGED_DIRS {
        total = total.saturating_add(path_size(&root.join(name))?);
    }
    Ok(total)
}

fn path_size(path: &Path) -> Result<u64, String> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(err.to_string()),
    };
    if meta.file_type().is_symlink() {
        return Err(format!(
            "La migration refuse le lien symbolique « {} ».",
            path.display()
        ));
    }
    if meta.is_file() {
        return Ok(meta.len());
    }
    if !meta.is_dir() {
        return Err(format!(
            "Type de fichier non pris en charge : {}",
            path.display()
        ));
    }
    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|err| err.to_string())? {
        total = total.saturating_add(path_size(&entry.map_err(|err| err.to_string())?.path())?);
    }
    Ok(total)
}

fn prepare_destination_data(
    source: &Path,
    destination: &Path,
    source_db: &rusqlite::Connection,
) -> Result<(rusqlite::Connection, SettingsStore), String> {
    fs::create_dir_all(destination).map_err(|err| {
        format!(
            "Impossible de créer le dossier cible « {} » : {err}",
            destination.display()
        )
    })?;

    for name in MANAGED_DIRS {
        copy_directory_if_present(&source.join(name), &destination.join(name))?;
    }
    for name in ["ai-settings.json", "audio-settings.json"] {
        copy_file_if_present(&source.join(name), &destination.join(name))?;
    }

    source_db
        .execute_batch("PRAGMA wal_checkpoint(FULL);")
        .map_err(|err| format!("Impossible de stabiliser la base SQLite : {err}"))?;
    let source_db_path = source.join(DB_FILE);
    let destination_db_path = destination.join(DB_FILE);
    if source_db_path.exists() {
        copy_file_if_present(&source_db_path, &destination_db_path)?;
    }
    let destination_db = open_and_migrate(&destination_db_path)
        .map_err(|err| format!("Impossible d’ouvrir la base migrée : {err}"))?;
    rewrite_audio_paths(&destination_db, source, destination)?;
    let integrity: String = destination_db
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|err| format!("Impossible de vérifier la base migrée : {err}"))?;
    if integrity != "ok" {
        return Err(format!(
            "La vérification de la base migrée a échoué : {integrity}"
        ));
    }

    write_marker(destination)?;
    let settings = SettingsStore::load(destination.to_path_buf())
        .map_err(|err| format!("Impossible de vérifier les réglages IA migrés : {err}"))?;
    Ok((destination_db, settings))
}

fn rewrite_audio_paths(
    db: &rusqlite::Connection,
    source: &Path,
    destination: &Path,
) -> Result<(), String> {
    let mut stmt = db
        .prepare("SELECT id, file_path FROM audio_files")
        .map_err(|err| err.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|err| err.to_string())?;
    let mut updates = Vec::new();
    for row in rows {
        let (id, raw_path) = row.map_err(|err| err.to_string())?;
        let path = PathBuf::from(&raw_path);
        if let Ok(relative) = path.strip_prefix(source) {
            updates.push((id, destination.join(relative).to_string_lossy().to_string()));
        }
    }
    drop(stmt);
    for (id, path) in updates {
        db.execute(
            "UPDATE audio_files SET file_path = ?1 WHERE id = ?2",
            rusqlite::params![path, id],
        )
        .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn copy_file_if_present(source: &Path, destination: &Path) -> Result<(), String> {
    match fs::symlink_metadata(source) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "La migration refuse le lien symbolique « {} ».",
            source.display()
        )),
        Ok(meta) if meta.is_file() => {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            fs::copy(source, destination).map_err(|err| {
                format!(
                    "Impossible de copier « {} » vers « {} » : {err}",
                    source.display(),
                    destination.display()
                )
            })?;
            Ok(())
        }
        Ok(_) => Err(format!(
            "Le chemin géré « {} » n’est pas un fichier régulier.",
            source.display()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn copy_directory_if_present(source: &Path, destination: &Path) -> Result<(), String> {
    match fs::symlink_metadata(source) {
        Ok(meta) if meta.file_type().is_symlink() => Err(format!(
            "La migration refuse le lien symbolique « {} ».",
            source.display()
        )),
        Ok(meta) if meta.is_dir() => {
            fs::create_dir_all(destination).map_err(|err| err.to_string())?;
            for entry in fs::read_dir(source).map_err(|err| err.to_string())? {
                let entry = entry.map_err(|err| err.to_string())?;
                let from = entry.path();
                let to = destination.join(entry.file_name());
                let entry_meta = entry.file_type().map_err(|err| err.to_string())?;
                if entry_meta.is_symlink() {
                    return Err(format!(
                        "La migration refuse le lien symbolique « {} ».",
                        from.display()
                    ));
                }
                if entry_meta.is_dir() {
                    copy_directory_if_present(&from, &to)?;
                } else if entry_meta.is_file() {
                    copy_file_if_present(&from, &to)?;
                } else {
                    return Err(format!(
                        "Type de fichier non pris en charge pendant la migration : {}",
                        from.display()
                    ));
                }
            }
            Ok(())
        }
        Ok(_) => Err(format!(
            "Le chemin géré « {} » n’est pas un dossier.",
            source.display()
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

fn write_marker(destination: &Path) -> Result<(), String> {
    write_json_atomically(
        &destination.join(MARKER_FILE),
        &StorageMarker {
            version: CONFIG_VERSION,
            application: crate::APP_IDENTIFIER.to_string(),
        },
    )
    .map_err(|err| format!("Impossible de marquer le dossier de stockage : {err}"))
}

fn validate_marker(root: &Path) -> Result<(), String> {
    let raw = fs::read_to_string(root.join(MARKER_FILE)).map_err(|err| {
        format!(
            "Le dossier « {} » n’est pas reconnu comme un stockage La Minute : {err}",
            root.display()
        )
    })?;
    let marker: StorageMarker = serde_json::from_str(&raw)
        .map_err(|err| format!("Le marqueur du dossier de stockage est invalide : {err}"))?;
    if marker.version != CONFIG_VERSION || marker.application != crate::APP_IDENTIFIER {
        return Err(
            "Le dossier choisi appartient à une version ou une application incompatible.".into(),
        );
    }
    Ok(())
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let payload = serde_json::to_vec_pretty(value).map_err(std::io::Error::other)?;
    {
        let mut file = File::create(&temp)?;
        file.write_all(&payload)?;
        file.sync_all()?;
    }
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp, path)
}

fn cleanup_partial_destination(destination: &Path, default_root: &Path) {
    for name in MANAGED_FILES {
        let _ = fs::remove_file(destination.join(name));
    }
    for name in MANAGED_DIRS {
        let _ = fs::remove_dir_all(destination.join(name));
    }
    let _ = fs::remove_file(destination.join(MARKER_FILE));
    if destination != default_root {
        let _ = fs::remove_dir(destination);
    }
}

fn cleanup_source_data(source: &Path, default_root: &Path) -> Result<(), String> {
    let mut failures = Vec::new();
    for name in MANAGED_FILES {
        if let Err(err) = remove_file_if_present(&source.join(name)) {
            failures.push(err);
        }
    }
    for name in MANAGED_DIRS {
        if let Err(err) = remove_dir_if_present(&source.join(name)) {
            failures.push(err);
        }
    }
    if source != default_root {
        if let Err(err) = remove_file_if_present(&source.join(MARKER_FILE)) {
            failures.push(err);
        }
        if let Err(err) = fs::remove_dir(source) {
            if err.kind() != std::io::ErrorKind::NotFound
                && err.kind() != std::io::ErrorKind::DirectoryNotEmpty
            {
                failures.push(format!("{} : {err}", source.display()));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Les données sont actives dans le nouveau dossier, mais certains éléments de l’ancien emplacement n’ont pas pu être supprimés : {}",
            failures.join(" ; ")
        ))
    }
}

fn remove_file_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("{} : {err}", path.display())),
    }
}

fn remove_dir_if_present(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("{} : {err}", path.display())),
    }
}

#[cfg(unix)]
pub(crate) fn available_space(path: &Path) -> Result<u64, String> {
    let output = Command::new("df")
        .arg("-Pk")
        .arg(path)
        .output()
        .map_err(|err| format!("Impossible de vérifier l’espace disponible : {err}"))?;
    if !output.status.success() {
        return Err("Impossible de vérifier l’espace disponible sur le disque sélectionné.".into());
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "La réponse du système sur l’espace disque est invalide.".to_string())?;
    let line = stdout
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .ok_or_else(|| "Aucune information d’espace disque n’est disponible.".to_string())?;
    let fields: Vec<&str> = line.split_whitespace().collect();
    let available_kib = fields
        .get(3)
        .ok_or_else(|| "Le format de l’espace disque est inattendu.".to_string())?
        .parse::<u64>()
        .map_err(|_| "L’espace disque disponible est illisible.".to_string())?;
    Ok(available_kib.saturating_mul(1024))
}

#[cfg(windows)]
pub(crate) fn available_space(path: &Path) -> Result<u64, String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            directory_name: *const u16,
            free_bytes_available: *mut u64,
            total_number_of_bytes: *mut u64,
            total_number_of_free_bytes: *mut u64,
        ) -> i32;
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0u64;
    let result = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(format!(
            "Impossible de vérifier l’espace disponible : {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(available)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_and_migrate;
    use rusqlite::params;

    #[test]
    fn configured_custom_root_requires_valid_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let default = tmp.path().join("default");
        let custom = tmp.path().join("custom");
        fs::create_dir_all(&default).unwrap();
        fs::create_dir_all(&custom).unwrap();
        write_json_atomically(
            &default.join(CONFIG_FILE),
            &StorageConfig {
                version: CONFIG_VERSION,
                data_root: custom.clone(),
            },
        )
        .unwrap();

        let error = StorageState::load(default.clone()).unwrap_err();
        assert!(error.contains("pas reconnu"));

        write_marker(&custom).unwrap();
        let state = StorageState::load(default).unwrap();
        assert_eq!(state.root(), fs::canonicalize(custom).unwrap());
        assert!(state.is_custom());
    }

    #[test]
    fn migration_copies_all_managed_data_and_rewrites_audio_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let destination = tmp.path().join("destination");
        fs::create_dir_all(source.join("imports")).unwrap();
        fs::create_dir_all(source.join("recordings")).unwrap();
        fs::write(source.join("imports/audio.mp3"), b"audio").unwrap();
        let connection = open_and_migrate(&source.join(DB_FILE)).unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO meetings (id, title, status, created_at, updated_at) VALUES ('m1', 'Test', 'draft', ?1, ?1)",
                [&now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO audio_files (id, meeting_id, file_path, created_at) VALUES ('a1', 'm1', ?1, ?2)",
                params![source.join("imports/audio.mp3").to_string_lossy(), now],
            )
            .unwrap();

        let (migrated, _) = prepare_destination_data(&source, &destination, &connection).unwrap();
        assert_eq!(
            fs::read(destination.join("imports/audio.mp3")).unwrap(),
            b"audio"
        );
        let path: String = migrated
            .query_row(
                "SELECT file_path FROM audio_files WHERE id = 'a1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            path,
            destination.join("imports/audio.mp3").to_string_lossy()
        );
        assert!(destination.join(MARKER_FILE).exists());
    }

    #[test]
    fn destination_must_be_empty_and_not_nested() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let occupied = tmp.path().join("occupied");
        fs::create_dir_all(source.join("child")).unwrap();
        fs::create_dir_all(&occupied).unwrap();
        fs::write(occupied.join("unrelated.txt"), b"keep").unwrap();
        assert!(validate_destination(&source, &source.join("child"), tmp.path()).is_err());
        assert!(validate_destination(&source, &occupied, tmp.path()).is_err());
    }
}
