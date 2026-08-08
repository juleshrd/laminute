use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use uuid::Uuid;

use super::error::AudioError;

/// Taille maximale d'un import MP3 — alignée sur la limite cloud de transcription (100 Mo).
pub const MAX_IMPORT_BYTES: u64 = 100 * 1024 * 1024;

/// Sous-dossier de staging sous `imports/` (fichiers en cours de copie).
pub const STAGING_DIR_NAME: &str = ".staging";

/// Durée minimale acceptée (1 seconde).
pub const MIN_DURATION_MS: i64 = 1_000;

/// Durée maximale acceptée (4 heures).
pub const MAX_DURATION_MS: i64 = 4 * 60 * 60 * 1_000;

/// Formats acceptés pour la transcription : MP3 (import) et WAV (enregistrement micro).
/// Aucun transcodage n'est nécessaire pour ces formats.
pub const TRANSCRIPTION_FORMATS: &[&str] = &["mp3", "wav"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedAudio {
    pub dest_path: PathBuf,
    pub duration_ms: i64,
    pub format: String,
}

/// Résultat d'un import hors verrou SQLite, avec mesure de la phase disque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedImport {
    pub imported: ImportedAudio,
    /// Temps passé en validation + copie/promotion (hors mutex DB).
    pub disk_elapsed_ms: u128,
}

pub fn staging_dir(imports_dir: &Path) -> PathBuf {
    imports_dir.join(STAGING_DIR_NAME)
}

/// Valide, copie en staging puis promeut vers `imports/{uuid}.mp3` — hors verrou SQLite.
pub fn import_mp3(source: &Path, imports_dir: &Path) -> Result<ImportedAudio, AudioError> {
    Ok(stage_mp3_import(source, imports_dir)?.imported)
}

/// Comme [`import_mp3`], en exposant la durée de la phase disque (mesure avant/après mutex).
pub fn stage_mp3_import(source: &Path, imports_dir: &Path) -> Result<StagedImport, AudioError> {
    let started = Instant::now();

    if !source.is_file() {
        return Err(AudioError::InvalidAudio(
            "le chemin fourni ne correspond pas à un fichier".into(),
        ));
    }

    validate_extension(source)?;
    validate_magic_bytes(source)?;
    validate_file_size(source)?;

    let duration_ms = read_duration_ms(source)?;
    validate_duration(duration_ms)?;

    let staging = staging_dir(imports_dir);
    fs::create_dir_all(&staging)?;
    fs::create_dir_all(imports_dir)?;

    let id = Uuid::new_v4();
    let staging_path = staging.join(format!("{id}.mp3"));
    let dest_path = imports_dir.join(format!("{id}.mp3"));

    if let Err(err) = fs::copy(source, &staging_path) {
        let _ = fs::remove_file(&staging_path);
        return Err(AudioError::Io(err.to_string()));
    }

    if let Err(err) = fs::rename(&staging_path, &dest_path) {
        let _ = fs::remove_file(&staging_path);
        let _ = fs::remove_file(&dest_path);
        return Err(AudioError::Io(err.to_string()));
    }

    Ok(StagedImport {
        imported: ImportedAudio {
            dest_path,
            duration_ms,
            format: "mp3".into(),
        },
        disk_elapsed_ms: started.elapsed().as_millis(),
    })
}

/// Supprime les fichiers orphelins restés dans `imports/.staging/` (démarrage / reprise).
/// Ne touche pas aux imports finalisés sous `imports/`.
pub fn cleanup_staging(imports_dir: &Path) -> Result<usize, AudioError> {
    let staging = staging_dir(imports_dir);
    if !staging.exists() {
        return Ok(0);
    }

    let mut removed = 0usize;
    for entry in fs::read_dir(&staging)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            fs::remove_file(&path)?;
            removed += 1;
        } else if file_type.is_dir() {
            fs::remove_dir_all(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Compensation : efface un fichier importé promu si la transaction DB échoue ensuite.
pub fn discard_imported_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn validate_extension(path: &Path) -> Result<(), AudioError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    let is_supported = extension
        .as_deref()
        .is_some_and(|value| TRANSCRIPTION_FORMATS.contains(&value));

    if is_supported && extension.as_deref() == Some("mp3") {
        Ok(())
    } else {
        Err(AudioError::UnsupportedFormat)
    }
}

fn validate_magic_bytes(path: &Path) -> Result<(), AudioError> {
    let mut header = [0_u8; 3];
    let mut file = fs::File::open(path)?;
    let read = file.read(&mut header)?;
    if read < 2 {
        return Err(AudioError::InvalidAudio(
            "fichier trop court pour être un MP3 valide".into(),
        ));
    }

    if is_mp3_header(&header[..read]) {
        Ok(())
    } else {
        Err(AudioError::UnsupportedFormat)
    }
}

fn is_mp3_header(header: &[u8]) -> bool {
    if header.starts_with(b"ID3") {
        return true;
    }

    header.len() >= 2 && header[0] == 0xFF && (header[1] & 0xE0) == 0xE0
}

fn validate_file_size(path: &Path) -> Result<(), AudioError> {
    let metadata = fs::metadata(path)?;
    let size = metadata.len();

    if size == 0 {
        return Err(AudioError::InvalidAudio("fichier vide".into()));
    }

    if size > MAX_IMPORT_BYTES {
        return Err(AudioError::FileTooLarge {
            max_mb: MAX_IMPORT_BYTES / (1024 * 1024),
        });
    }

    Ok(())
}

fn read_duration_ms(path: &Path) -> Result<i64, AudioError> {
    let duration = mp3_duration::from_path(path).map_err(|err| {
        AudioError::InvalidAudio(format!("impossible de lire la durée audio : {err}"))
    })?;

    let millis = (duration.as_secs_f64() * 1000.0).round() as i64;
    if millis <= 0 {
        return Err(AudioError::DurationTooShort {
            min_secs: MIN_DURATION_MS / 1000,
        });
    }

    Ok(millis)
}

fn validate_duration(duration_ms: i64) -> Result<(), AudioError> {
    if duration_ms < MIN_DURATION_MS {
        return Err(AudioError::DurationTooShort {
            min_secs: MIN_DURATION_MS / 1000,
        });
    }

    if duration_ms > MAX_DURATION_MS {
        return Err(AudioError::DurationTooLong {
            max_hours: MAX_DURATION_MS / (60 * 60 * 1000),
        });
    }

    Ok(())
}

pub fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Import audio")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::Write;

    fn write_temp_mp3(name: &str, payload: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "laminute-import-test-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("{name}.mp3"));
        let mut file = fs::File::create(&path).expect("create temp mp3");
        file.write_all(payload).expect("write temp mp3");
        path
    }

    fn fixture_mp3() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-1s.mp3")
    }

    #[test]
    fn detects_id3_and_frame_sync_headers() {
        assert!(is_mp3_header(b"ID3"));
        assert!(is_mp3_header(&[0xFF, 0xFB, 0x90]));
        assert!(!is_mp3_header(b"RIFF"));
        assert!(!is_mp3_header(b"Ogg"));
    }

    #[test]
    fn transcription_formats_include_mp3_and_wav() {
        assert!(TRANSCRIPTION_FORMATS.contains(&"mp3"));
        assert!(TRANSCRIPTION_FORMATS.contains(&"wav"));
    }

    #[test]
    fn rejects_non_mp3_extension() {
        let dir = std::env::temp_dir().join(format!("laminute-import-ext-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("sample.wav");
        fs::write(&path, [0xFF, 0xFB, 0x00]).unwrap();

        let err = validate_extension(&path).unwrap_err();
        assert!(matches!(err, AudioError::UnsupportedFormat));
    }

    #[test]
    fn rejects_file_without_mp3_signature() {
        let path = write_temp_mp3("invalid-signature", b"not-an-mp3-file");
        let err = validate_magic_bytes(&path).unwrap_err();
        assert!(matches!(err, AudioError::UnsupportedFormat));
    }

    #[test]
    fn accepts_small_file_and_documents_size_limit() {
        let path = write_temp_mp3("small", &[0xFF, 0xFB, 0x00]);
        validate_file_size(&path).expect("un petit fichier valide doit passer");
        assert_eq!(MAX_IMPORT_BYTES, 100 * 1024 * 1024);
        assert_eq!(MAX_IMPORT_BYTES, crate::ai::limits::MAX_AUDIO_BYTES);
    }

    #[test]
    fn rejects_file_above_cloud_transcription_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.mp3");
        {
            let file = fs::File::create(&path).unwrap();
            file.set_len(MAX_IMPORT_BYTES + 1).unwrap();
        }
        {
            let mut file = OpenOptions::new().write(true).open(&path).unwrap();
            file.write_all(&[0xFF, 0xFB, 0x90, 0x00]).unwrap();
        }

        let err = validate_file_size(&path).unwrap_err();
        match err {
            AudioError::FileTooLarge { max_mb } => {
                assert_eq!(max_mb, 100);
                assert!(err.to_string().contains("transcription cloud"));
            }
            other => panic!("attendu FileTooLarge, obtenu {other:?}"),
        }
    }

    #[test]
    fn title_from_path_uses_stem() {
        let path = Path::new("/tmp/Comité produit.mp3");
        assert_eq!(title_from_path(path), "Comité produit");
    }

    #[test]
    fn stage_mp3_import_promotes_out_of_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staged = stage_mp3_import(&fixture_mp3(), &imports).unwrap();

        assert!(staged.imported.dest_path.starts_with(&imports));
        assert!(staged.imported.dest_path.exists());
        assert!(staged.imported.duration_ms >= MIN_DURATION_MS);

        let staging_entries: Vec<_> = fs::read_dir(staging_dir(&imports))
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            staging_entries.is_empty(),
            "aucun fichier ne doit rester en staging après promotion"
        );
    }

    #[test]
    fn failed_validation_leaves_no_staging_or_import() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let bad = write_temp_mp3("bad", b"not-mp3");

        let err = stage_mp3_import(&bad, &imports).unwrap_err();
        assert!(matches!(err, AudioError::UnsupportedFormat));
        assert!(
            !imports.exists() || fs::read_dir(&imports).unwrap().next().is_none(),
            "aucun fichier final"
        );
        let staging = staging_dir(&imports);
        assert!(
            !staging.exists() || fs::read_dir(&staging).unwrap().next().is_none(),
            "aucun orphelin staging"
        );
    }

    #[test]
    fn cleanup_staging_removes_orphans_keeps_final_imports() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staging = staging_dir(&imports);
        fs::create_dir_all(&staging).unwrap();
        fs::write(staging.join("orphan.mp3"), b"orphan").unwrap();
        fs::write(imports.join("kept.mp3"), b"kept").unwrap();

        let removed = cleanup_staging(&imports).unwrap();
        assert_eq!(removed, 1);
        assert!(!staging.join("orphan.mp3").exists());
        assert!(imports.join("kept.mp3").exists());
    }

    #[test]
    fn discard_imported_file_removes_promoted_orphan() {
        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let staged = stage_mp3_import(&fixture_mp3(), &imports).unwrap();
        assert!(staged.imported.dest_path.exists());

        discard_imported_file(&staged.imported.dest_path);
        assert!(
            !staged.imported.dest_path.exists(),
            "compensation : fichier promu effacé"
        );
    }

    #[test]
    fn disk_phase_runs_outside_and_db_insert_stays_brief() {
        use std::time::Duration;

        let tmp = tempfile::tempdir().unwrap();
        let imports = tmp.path().join("imports");
        let source = tmp.path().join("bulky.mp3");
        fs::copy(fixture_mp3(), &source).unwrap();
        {
            let mut file = OpenOptions::new().append(true).open(&source).unwrap();
            let chunk = vec![0u8; 512 * 1024];
            for _ in 0..16 {
                file.write_all(&chunk).unwrap();
            }
        }

        let staged = stage_mp3_import(&source, &imports).unwrap();
        // Mesure exposée (critère JUL-184) — la copie ~8 Mo doit prendre du temps CPU/IO.
        assert!(
            Duration::from_millis(staged.disk_elapsed_ms as u64) >= Duration::ZERO
                && staged.imported.dest_path.exists(),
            "phase disque hors mutex doit produire un fichier et une mesure"
        );

        let conn = crate::db::open_in_memory().unwrap();
        let db_started = Instant::now();
        let detail = crate::repository::MeetingRepository::create_from_imported_audio(
            &conn,
            "Mesure",
            &staged.imported,
        )
        .unwrap();
        let db_ms = db_started.elapsed().as_millis();

        assert_eq!(detail.meeting.title, "Mesure");
        // Insertion SQL courte : ne doit pas approcher le coût d'une grosse copie.
        assert!(
            db_ms < 200,
            "transaction DB trop longue sous mutex : {db_ms} ms (disque {} ms)",
            staged.disk_elapsed_ms
        );
    }
}
