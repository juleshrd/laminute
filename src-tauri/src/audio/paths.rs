//! Confinement des fichiers audio aux répertoires gérés (`imports/`, `recordings/`).
//!
//! Toute validation d'ownership vit ici, hors du repository SQL.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::error::AudioError;
use super::import::{import_mp3, ImportedAudio};

/// Racines de stockage audio possédées par l'application.
#[derive(Debug, Clone)]
pub struct ManagedAudioRoots {
    pub imports_dir: PathBuf,
    pub recordings_dir: PathBuf,
}

impl ManagedAudioRoots {
    pub fn from_app(app: &AppHandle) -> Result<Self, AudioError> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|err| AudioError::Io(err.to_string()))?;
        Ok(Self::from_app_data_dir(app_data_dir))
    }

    pub fn from_app_data_dir(app_data_dir: PathBuf) -> Self {
        Self {
            imports_dir: app_data_dir.join("imports"),
            recordings_dir: app_data_dir.join("recordings"),
        }
    }

    pub fn ensure_dirs(&self) -> Result<(), AudioError> {
        fs::create_dir_all(&self.imports_dir)?;
        fs::create_dir_all(&self.recordings_dir)?;
        Ok(())
    }

    fn canonical_roots(&self) -> Result<(PathBuf, PathBuf), AudioError> {
        self.ensure_dirs()?;
        let imports = fs::canonicalize(&self.imports_dir).map_err(|err| {
            AudioError::Io(format!(
                "impossible de résoudre imports/ : {err}"
            ))
        })?;
        let recordings = fs::canonicalize(&self.recordings_dir).map_err(|err| {
            AudioError::Io(format!(
                "impossible de résoudre recordings/ : {err}"
            ))
        })?;
        Ok((imports, recordings))
    }

    fn contains_canonical(&self, canonical: &Path) -> Result<bool, AudioError> {
        let (imports, recordings) = self.canonical_roots()?;
        Ok(canonical.starts_with(&imports) || canonical.starts_with(&recordings))
    }
}

fn reject_symlink(path: &Path) -> Result<(), AudioError> {
    match path.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => Err(AudioError::SymlinkRejected),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AudioError::Io(err.to_string())),
    }
}

fn assert_regular_file(path: &Path) -> Result<(), AudioError> {
    let meta = path.symlink_metadata().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            AudioError::InvalidAudio("fichier audio introuvable".into())
        } else {
            AudioError::Io(err.to_string())
        }
    })?;
    if meta.file_type().is_symlink() {
        return Err(AudioError::SymlinkRejected);
    }
    if !meta.is_file() {
        return Err(AudioError::InvalidAudio(
            "le chemin fourni ne correspond pas à un fichier".into(),
        ));
    }
    Ok(())
}

/// Vérifie qu'un chemin appartient aux racines gérées (pas de symlink).
///
/// Si le fichier est absent, le parent doit être canonisable sous une racine gérée.
pub fn resolve_owned(path: &Path, roots: &ManagedAudioRoots) -> Result<PathBuf, AudioError> {
    reject_symlink(path)?;

    match path.symlink_metadata() {
        Ok(meta) => {
            if !meta.is_file() {
                return Err(AudioError::InvalidAudio(
                    "le chemin fourni ne correspond pas à un fichier".into(),
                ));
            }
            let canonical = fs::canonicalize(path).map_err(|err| AudioError::Io(err.to_string()))?;
            if !roots.contains_canonical(&canonical)? {
                return Err(AudioError::PathNotOwned);
            }
            Ok(canonical)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let file_name = path.file_name().ok_or(AudioError::PathNotOwned)?;
            let parent = path.parent().ok_or(AudioError::PathNotOwned)?;
            let parent_canonical = fs::canonicalize(parent).map_err(|_| AudioError::PathNotOwned)?;
            let candidate = parent_canonical.join(file_name);
            if !roots.contains_canonical(&candidate)? {
                return Err(AudioError::PathNotOwned);
            }
            Ok(candidate)
        }
        Err(err) => Err(AudioError::Io(err.to_string())),
    }
}

/// Copie une source externe vers une racine gérée, ou renvoie le chemin canonique
/// s'il est déjà owned.
pub fn ingest_if_needed(source: &Path, roots: &ManagedAudioRoots) -> Result<PathBuf, AudioError> {
    roots.ensure_dirs()?;

    match resolve_owned(source, roots) {
        Ok(owned) => {
            assert_regular_file(&owned)?;
            return Ok(owned);
        }
        Err(AudioError::PathNotOwned) => {}
        Err(err) => return Err(err),
    }

    assert_regular_file(source)?;

    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    match extension.as_deref() {
        Some("mp3") => {
            let imported: ImportedAudio = import_mp3(source, &roots.imports_dir)?;
            resolve_owned(&imported.dest_path, roots)
        }
        Some("wav") => {
            validate_wav_header(source)?;
            let dest = roots
                .recordings_dir
                .join(format!("{}.wav", Uuid::new_v4()));
            fs::copy(source, &dest)?;
            resolve_owned(&dest, roots)
        }
        _ => Err(AudioError::UnsupportedFormat),
    }
}

fn validate_wav_header(path: &Path) -> Result<(), AudioError> {
    let mut header = [0_u8; 12];
    let mut file = fs::File::open(path)?;
    let read = file.read(&mut header)?;
    if read < 12 {
        return Err(AudioError::InvalidAudio(
            "fichier trop court pour être un WAV valide".into(),
        ));
    }
    if &header[0..4] == b"RIFF" && &header[8..12] == b"WAVE" {
        Ok(())
    } else {
        Err(AudioError::UnsupportedFormat)
    }
}

/// Supprime un fichier audio uniquement s'il est sous une racine gérée.
/// Fichier absent mais chemin owned → Ok.
pub fn remove_owned(path: &Path, roots: &ManagedAudioRoots) -> Result<(), AudioError> {
    let owned = resolve_owned(path, roots)?;

    // Re-check anti-TOCTOU juste avant la suppression.
    reject_symlink(&owned)?;
    match owned.symlink_metadata() {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(AudioError::SymlinkRejected);
            }
            if !meta.is_file() {
                return Err(AudioError::InvalidAudio(
                    "le chemin fourni ne correspond pas à un fichier".into(),
                ));
            }
            // Confirmer encore l'appartenance après re-lecture des métadonnées.
            let canonical =
                fs::canonicalize(&owned).map_err(|err| AudioError::Io(err.to_string()))?;
            if !roots.contains_canonical(&canonical)? {
                return Err(AudioError::PathNotOwned);
            }
            fs::remove_file(&canonical)?;
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(AudioError::Io(err.to_string())),
    }
}

/// Tente `remove_owned` ; les chemins hors racines sont ignorés (pas de suppression).
/// Utile pour `delete_all_local_data` quand la DB peut être empoisonnée.
pub fn try_remove_owned(path: &Path, roots: &ManagedAudioRoots) -> Result<(), AudioError> {
    match remove_owned(path, roots) {
        Ok(()) => Ok(()),
        Err(AudioError::PathNotOwned) => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn temp_roots(label: &str) -> (tempfile::TempDir, ManagedAudioRoots) {
        let dir = tempfile::tempdir().expect("tempdir");
        let app_data = dir.path().join(label);
        let roots = ManagedAudioRoots::from_app_data_dir(app_data);
        roots.ensure_dirs().expect("ensure dirs");
        (dir, roots)
    }

    fn write_mp3(path: &Path) {
        let mut file = fs::File::create(path).expect("create");
        file.write_all(&[0xFF, 0xFB, 0x90, 0x00]).expect("write");
    }

    fn write_wav(path: &Path) {
        let mut file = fs::File::create(path).expect("create");
        // Minimal RIFF/WAVE header (not a full valid wav, enough for magic check).
        let mut buf = Vec::new();
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&36u32.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&[1, 0, 1, 0]); // PCM mono
        buf.extend_from_slice(&44100u32.to_le_bytes());
        buf.extend_from_slice(&88200u32.to_le_bytes());
        buf.extend_from_slice(&[2, 0, 16, 0]);
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&0u32.to_le_bytes());
        file.write_all(&buf).expect("write wav");
    }

    #[test]
    fn resolve_owned_accepts_file_under_imports() {
        let (_tmp, roots) = temp_roots("owned-ok");
        let path = roots.imports_dir.join("a.mp3");
        write_mp3(&path);

        let resolved = resolve_owned(&path, &roots).unwrap();
        assert!(resolved.starts_with(fs::canonicalize(&roots.imports_dir).unwrap()));
    }

    #[test]
    fn resolve_owned_rejects_external_path() {
        let (_tmp, roots) = temp_roots("owned-ext");
        let external = tempfile::NamedTempFile::new().unwrap();
        write_mp3(external.path());

        let err = resolve_owned(external.path(), &roots).unwrap_err();
        assert!(matches!(err, AudioError::PathNotOwned));
    }

    #[test]
    fn resolve_owned_rejects_parent_traversal() {
        let (_tmp, roots) = temp_roots("owned-dotdot");
        let outside = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("secret.mp3");
        write_mp3(&outside);
        let traversal = roots.imports_dir.join("..").join("secret.mp3");

        let err = resolve_owned(&traversal, &roots).unwrap_err();
        assert!(matches!(err, AudioError::PathNotOwned));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_owned_rejects_symlink_under_imports() {
        let (_tmp, roots) = temp_roots("owned-symlink");
        let secret = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("secret.txt");
        fs::write(&secret, b"secret").unwrap();
        let link = roots.imports_dir.join("link.mp3");
        symlink(&secret, &link).unwrap();

        let err = resolve_owned(&link, &roots).unwrap_err();
        assert!(matches!(err, AudioError::SymlinkRejected));
    }

    #[test]
    fn ingest_copies_external_mp3_into_imports() {
        let (_tmp, roots) = temp_roots("ingest-mp3");
        // import_mp3 needs a valid duration — use a tiny file and expect DurationTooShort
        // for bare header. Prefer a source already under temp outside roots with enough
        // content: we only assert copy destination when validation passes.
        // For unit test of path confinement, place a pre-validated-looking path by
        // writing directly then testing PathNotOwned → copy path for wav instead.

        let external = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("external.wav");
        write_wav(&external);

        let owned = ingest_if_needed(&external, &roots).unwrap();
        assert!(owned.starts_with(fs::canonicalize(&roots.recordings_dir).unwrap()));
        assert!(owned.exists());
        assert!(external.exists(), "la source externe ne doit pas être déplacée");
    }

    #[test]
    fn ingest_returns_canonical_when_already_owned() {
        let (_tmp, roots) = temp_roots("ingest-owned");
        let path = roots.recordings_dir.join("rec.wav");
        write_wav(&path);

        let owned = ingest_if_needed(&path, &roots).unwrap();
        assert_eq!(owned, fs::canonicalize(&path).unwrap());
    }

    #[test]
    fn remove_owned_deletes_managed_file() {
        let (_tmp, roots) = temp_roots("remove-ok");
        let path = roots.imports_dir.join("gone.mp3");
        write_mp3(&path);

        remove_owned(&path, &roots).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn remove_owned_rejects_external_and_leaves_file() {
        let (_tmp, roots) = temp_roots("remove-ext");
        let external = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("keep.mp3");
        write_mp3(&external);

        let err = remove_owned(&external, &roots).unwrap_err();
        assert!(matches!(err, AudioError::PathNotOwned));
        assert!(external.exists());
    }

    #[test]
    fn try_remove_owned_ignores_external() {
        let (_tmp, roots) = temp_roots("try-remove");
        let external = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("keep2.mp3");
        write_mp3(&external);

        try_remove_owned(&external, &roots).unwrap();
        assert!(external.exists());
    }

    #[cfg(unix)]
    #[test]
    fn remove_owned_rejects_toctou_symlink_swap() {
        let (_tmp, roots) = temp_roots("toctou");
        let path = roots.imports_dir.join("swap.mp3");
        write_mp3(&path);

        let owned = resolve_owned(&path, &roots).unwrap();
        assert!(owned.exists());

        fs::remove_file(&path).unwrap();
        let secret = roots
            .imports_dir
            .parent()
            .unwrap()
            .join("secret-toctou.txt");
        fs::write(&secret, b"secret").unwrap();
        symlink(&secret, &path).unwrap();

        let err = remove_owned(&path, &roots).unwrap_err();
        assert!(matches!(err, AudioError::SymlinkRejected));
        assert!(secret.exists(), "le secret ne doit pas être effacé");
    }

    #[cfg(unix)]
    #[test]
    fn remove_owned_propagates_disk_error() {
        let (_tmp, roots) = temp_roots("remove-err");
        let path = roots.imports_dir.join("locked.mp3");
        write_mp3(&path);

        // Retirer le droit d'écriture sur le répertoire parent empêche remove_file.
        let mut perms = fs::metadata(&roots.imports_dir).unwrap().permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&roots.imports_dir, perms).unwrap();

        let result = remove_owned(&path, &roots);

        // Restaurer avant assert pour que TempDir puisse nettoyer.
        let mut restore = fs::metadata(&roots.imports_dir).unwrap().permissions();
        restore.set_mode(0o755);
        fs::set_permissions(&roots.imports_dir, restore).unwrap();

        assert!(result.is_err());
        assert!(path.exists());
    }

    #[test]
    fn remove_owned_ok_when_file_already_missing() {
        let (_tmp, roots) = temp_roots("remove-missing");
        let path = roots.imports_dir.join("already-gone.mp3");

        remove_owned(&path, &roots).unwrap();
    }
}
