//! Écriture sécurisée des exports : grant one-shot, taille plafonnée,
//! pas de suivi de symlink, remplacement atomique.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use uuid::Uuid;

/// Taille maximale d'un export écrit sur disque (32 MiB).
pub const MAX_EXPORT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GrantToken(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExportWriteError {
    #[error("destination d'export non accordée ou déjà utilisée")]
    OutsideGrant,
    #[error("export trop volumineux (maximum {max} octets)")]
    Oversized { max: usize },
    #[error("la destination est un lien symbolique")]
    SymlinkDestination,
    #[error("répertoire parent introuvable")]
    MissingParent,
    #[error("écriture impossible : {0}")]
    Io(String),
}

fn grant_registry() -> &'static Mutex<HashMap<GrantToken, PathBuf>> {
    static REGISTRY: OnceLock<Mutex<HashMap<GrantToken, PathBuf>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Enregistre un chemin accordé (typiquement issu du dialogue de sauvegarde).
pub fn issue_grant(path: PathBuf) -> GrantToken {
    let token = GrantToken(Uuid::new_v4().to_string());
    let mut registry = grant_registry()
        .lock()
        .expect("registre de grants export empoisonné");
    registry.insert(token.clone(), path);
    token
}

/// Helper de test : émet un grant pour un chemin arbitraire.
#[cfg(test)]
pub fn issue_grant_for_test(path: PathBuf) -> GrantToken {
    issue_grant(path)
}

/// Consomme le grant et écrit `bytes` de façon atomique sans suivre les symlinks.
pub fn write_granted(token: &GrantToken, bytes: &[u8]) -> Result<(), ExportWriteError> {
    let path = {
        let mut registry = grant_registry()
            .lock()
            .map_err(|_| ExportWriteError::Io("registre de grants indisponible".into()))?;
        registry
            .remove(token)
            .ok_or(ExportWriteError::OutsideGrant)?
    };

    if bytes.len() > MAX_EXPORT_BYTES {
        return Err(ExportWriteError::Oversized {
            max: MAX_EXPORT_BYTES,
        });
    }

    write_atomic_nofollow(&path, bytes)
}

fn write_atomic_nofollow(path: &Path, bytes: &[u8]) -> Result<(), ExportWriteError> {
    if path
        .symlink_metadata()
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(ExportWriteError::SymlinkDestination);
    }

    let parent = path.parent().ok_or(ExportWriteError::MissingParent)?;
    if !parent.is_dir() {
        return Err(ExportWriteError::MissingParent);
    }

    let tmp_name = format!(".laminute-export-{}.tmp", Uuid::new_v4());
    let tmp_path = parent.join(tmp_name);

    if let Err(err) = fs::write(&tmp_path, bytes) {
        let _ = fs::remove_file(&tmp_path);
        return Err(ExportWriteError::Io(err.to_string()));
    }

    if let Err(err) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(ExportWriteError::Io(err.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_grant() {
        let token = GrantToken("missing-token".into());
        let err = write_granted(&token, b"hello").unwrap_err();
        assert_eq!(err, ExportWriteError::OutsideGrant);
    }

    #[test]
    fn rejects_already_consumed_grant() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        let token = issue_grant_for_test(path.clone());

        write_granted(&token, b"first").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");

        let err = write_granted(&token, b"second").unwrap_err();
        assert_eq!(err, ExportWriteError::OutsideGrant);
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_destination() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.txt");
        fs::write(&target, b"keep-me").unwrap();
        let link = dir.path().join("link.txt");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let token = issue_grant_for_test(link);
        let err = write_granted(&token, b"overwrite").unwrap_err();
        assert_eq!(err, ExportWriteError::SymlinkDestination);
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep-me");
    }

    #[test]
    fn overwrites_regular_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("report.md");
        fs::write(&path, b"old").unwrap();

        let token = issue_grant_for_test(path.clone());
        write_granted(&token, b"new-content").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new-content");
    }

    #[test]
    fn rejects_oversized_payload_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        fs::write(&path, b"untouched").unwrap();

        let token = issue_grant_for_test(path.clone());
        let oversized = vec![0u8; MAX_EXPORT_BYTES + 1];
        let err = write_granted(&token, &oversized).unwrap_err();
        assert_eq!(
            err,
            ExportWriteError::Oversized {
                max: MAX_EXPORT_BYTES
            }
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "untouched");
        // Grant one-shot : même après refus taille, le token n'est plus réutilisable.
        assert_eq!(
            write_granted(&token, b"ok").unwrap_err(),
            ExportWriteError::OutsideGrant
        );
    }

    #[test]
    fn rejects_missing_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("out.txt");
        let token = issue_grant_for_test(path);
        let err = write_granted(&token, b"x").unwrap_err();
        assert_eq!(err, ExportWriteError::MissingParent);
    }

    #[test]
    fn creates_new_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fresh.json");
        let token = issue_grant_for_test(path.clone());
        write_granted(&token, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn max_export_bytes_constant() {
        assert_eq!(MAX_EXPORT_BYTES, 32 * 1024 * 1024);
    }
}
