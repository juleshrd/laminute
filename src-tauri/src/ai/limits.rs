use crate::ai::error::AiError;
use crate::audio::import::MAX_IMPORT_BYTES;

/// Taille maximale acceptée par les API cloud (Mistral, OpenAI) pour la transcription.
/// Alignée sur [`MAX_IMPORT_BYTES`] pour éviter d'importer un fichier non transcriptible.
pub const MAX_AUDIO_BYTES: u64 = MAX_IMPORT_BYTES;

/// Vérifie la taille d'un fichier audio avant lecture du contenu (metadata uniquement).
pub fn validate_transcription_audio_size(len: u64) -> Result<(), AiError> {
    if len == 0 {
        return Err(AiError::Other("Le fichier audio est vide.".to_string()));
    }
    if len > MAX_AUDIO_BYTES {
        return Err(AiError::Other(format!(
            "Fichier audio trop volumineux (max {} Mo).",
            MAX_AUDIO_BYTES / 1024 / 1024
        )));
    }
    Ok(())
}

// Mesure RSS (manuelle) : comparer VmRSS dans `/proc/self/status` avant/après
// l'upload d'un fichier ~90 Mo avec `Part::file` (pas de pic ~2× la taille fichier).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_oversized_audio_from_metadata_len() {
        assert!(validate_transcription_audio_size(0).is_err());
        assert!(validate_transcription_audio_size(MAX_AUDIO_BYTES + 1).is_err());
        assert!(validate_transcription_audio_size(1).is_ok());
        assert!(validate_transcription_audio_size(MAX_AUDIO_BYTES).is_ok());
    }

    #[test]
    fn oversized_error_message_is_in_french() {
        let err = validate_transcription_audio_size(MAX_AUDIO_BYTES + 1).unwrap_err();
        assert!(err.to_string().contains("trop volumineux"));
        assert!(err.to_string().contains("100 Mo"));
    }
}
