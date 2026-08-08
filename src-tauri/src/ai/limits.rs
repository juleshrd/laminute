use crate::ai::error::AiError;
use crate::audio::import::MAX_IMPORT_BYTES;

/// Taille maximale acceptée par les API cloud (Mistral, OpenAI) pour la transcription.
/// Alignée sur [`MAX_IMPORT_BYTES`] pour éviter d'importer un fichier non transcriptible.
pub const MAX_AUDIO_BYTES: u64 = MAX_IMPORT_BYTES;

/// Texte maximal envoyé au modèle de compte-rendu.
pub const MAX_SUMMARY_INPUT_TEXT_BYTES: usize = 1_000_000;

/// Corps maximal lu depuis une réponse IA réussie avant désérialisation.
pub const MAX_PROVIDER_RESPONSE_BYTES: usize = 2_000_000;

/// Corps maximal lu depuis une réponse d'erreur fournisseur.
pub const MAX_PROVIDER_ERROR_BYTES: usize = 16_384;

/// Message d'erreur maximal propagé vers l'UI.
pub const MAX_ERROR_MESSAGE_CHARS: usize = 2_000;

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

pub fn validate_summary_input_text(text: &str) -> Result<(), AiError> {
    if text.trim().is_empty() {
        return Err(AiError::Other("le texte fourni est vide".to_string()));
    }
    if text.len() > MAX_SUMMARY_INPUT_TEXT_BYTES {
        return Err(AiError::Other(format!(
            "Texte de transcription trop volumineux (max {} Ko).",
            MAX_SUMMARY_INPUT_TEXT_BYTES / 1024
        )));
    }
    Ok(())
}

pub fn validate_provider_response_size(len: usize) -> Result<(), AiError> {
    if len > MAX_PROVIDER_RESPONSE_BYTES {
        return Err(AiError::Other(format!(
            "Réponse fournisseur trop volumineuse (max {} Ko).",
            MAX_PROVIDER_RESPONSE_BYTES / 1024
        )));
    }
    Ok(())
}

pub fn validate_provider_error_size(len: usize) -> Result<(), AiError> {
    if len > MAX_PROVIDER_ERROR_BYTES {
        return Err(AiError::Other(format!(
            "Message d'erreur fournisseur trop volumineux (max {} Ko).",
            MAX_PROVIDER_ERROR_BYTES / 1024
        )));
    }
    Ok(())
}

pub fn truncate_error_message(message: &str) -> String {
    if message.chars().count() <= MAX_ERROR_MESSAGE_CHARS {
        return message.to_string();
    }

    let mut truncated: String = message.chars().take(MAX_ERROR_MESSAGE_CHARS).collect();
    truncated.push('…');
    truncated
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

    #[test]
    fn documents_text_response_and_error_limits() {
        assert_eq!(MAX_AUDIO_BYTES, MAX_IMPORT_BYTES);
        assert!(validate_summary_input_text("Bonjour").is_ok());
        assert!(
            validate_summary_input_text(&"x".repeat(MAX_SUMMARY_INPUT_TEXT_BYTES + 1)).is_err()
        );
        assert!(validate_provider_response_size(MAX_PROVIDER_RESPONSE_BYTES).is_ok());
        assert!(validate_provider_response_size(MAX_PROVIDER_RESPONSE_BYTES + 1).is_err());
        assert!(validate_provider_error_size(MAX_PROVIDER_ERROR_BYTES).is_ok());
        assert!(validate_provider_error_size(MAX_PROVIDER_ERROR_BYTES + 1).is_err());
    }

    #[test]
    fn truncates_error_messages_on_char_boundaries() {
        let message = "é".repeat(MAX_ERROR_MESSAGE_CHARS + 10);
        let truncated = truncate_error_message(&message);
        assert_eq!(truncated.chars().count(), MAX_ERROR_MESSAGE_CHARS + 1);
        assert!(truncated.ends_with('…'));
    }
}
