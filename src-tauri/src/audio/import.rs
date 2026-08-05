use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::error::AudioError;

/// Taille maximale d'un import MP3 (500 Mo).
pub const MAX_IMPORT_BYTES: u64 = 500 * 1024 * 1024;

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

/// Valide, copie et prépare un fichier MP3 pour la transcription.
pub fn import_mp3(source: &Path, imports_dir: &Path) -> Result<ImportedAudio, AudioError> {
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

    fs::create_dir_all(imports_dir)?;

    let dest_path = imports_dir.join(format!("{}.mp3", Uuid::new_v4()));
    fs::copy(source, &dest_path)?;

    Ok(ImportedAudio {
        dest_path,
        duration_ms,
        format: "mp3".into(),
    })
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
        assert_eq!(MAX_IMPORT_BYTES, 500 * 1024 * 1024);
    }

    #[test]
    fn title_from_path_uses_stem() {
        let path = Path::new("/tmp/Comité produit.mp3");
        assert_eq!(title_from_path(path), "Comité produit");
    }
}
