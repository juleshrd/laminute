use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("aucun périphérique d'entrée audio détecté")]
    NoInputDevice,

    #[error("périphérique introuvable : {0}")]
    DeviceNotFound(String),

    #[error("permission microphone refusée — autorisez l'accès au micro dans les réglages système")]
    PermissionDenied,

    #[error("un enregistrement est déjà en cours")]
    AlreadyRecording,

    #[error("aucun enregistrement en cours")]
    NotRecording,

    #[error("échec de l'enregistrement : {0}")]
    Io(String),

    #[error("erreur audio : {0}")]
    Internal(String),

    #[error("format non supporté — seuls les fichiers MP3 sont acceptés pour l'import")]
    UnsupportedFormat,

    #[error("fichier trop volumineux (maximum {max_mb} Mo)")]
    FileTooLarge { max_mb: u64 },

    #[error("durée audio trop courte (minimum {min_secs} s)")]
    DurationTooShort { min_secs: i64 },

    #[error("durée audio trop longue (maximum {max_hours} h)")]
    DurationTooLong { max_hours: i64 },

    #[error("fichier audio invalide : {0}")]
    InvalidAudio(String),
}

impl AudioError {
    fn code(&self) -> &'static str {
        match self {
            Self::NoInputDevice => "no_input_device",
            Self::DeviceNotFound(_) => "device_not_found",
            Self::PermissionDenied => "permission_denied",
            Self::AlreadyRecording => "already_recording",
            Self::NotRecording => "not_recording",
            Self::Io(_) => "io_error",
            Self::Internal(_) => "internal_error",
            Self::UnsupportedFormat => "unsupported_format",
            Self::FileTooLarge { .. } => "file_too_large",
            Self::DurationTooShort { .. } => "duration_too_short",
            Self::DurationTooLong { .. } => "duration_too_long",
            Self::InvalidAudio(_) => "invalid_audio",
        }
    }

    pub fn from_cpal(err: cpal::BuildStreamError) -> Self {
        let message = err.to_string().to_lowercase();
        if message.contains("permission")
            || message.contains("denied")
            || message.contains("not allowed")
            || message.contains("access")
        {
            return Self::PermissionDenied;
        }
        Self::Internal(err.to_string())
    }

    pub fn from_host(err: cpal::DevicesError) -> Self {
        let message = err.to_string().to_lowercase();
        if message.contains("permission")
            || message.contains("denied")
            || message.contains("not allowed")
        {
            return Self::PermissionDenied;
        }
        Self::Internal(err.to_string())
    }
}

#[derive(Debug, Serialize)]
pub struct AudioErrorPayload {
    pub code: String,
    pub message: String,
}

impl Serialize for AudioError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AudioErrorPayload {
            code: self.code().to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

impl From<std::io::Error> for AudioError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<hound::Error> for AudioError {
    fn from(value: hound::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for AudioError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.to_string())
    }
}
