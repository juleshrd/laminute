use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("base de données: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("migration: {0}")]
    Migration(#[from] refinery::Error),
    #[error("fichier: {0}")]
    Io(#[from] std::io::Error),
    #[error("réunion introuvable: {id}")]
    MeetingNotFound { id: String },
    #[error("{0}")]
    Message(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Database(_) => "db_error",
            Self::Migration(_) => "migration_error",
            Self::Io(_) => "io_error",
            Self::MeetingNotFound { .. } => "meeting_not_found",
            Self::Message(_) => "message",
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct AppErrorPayload {
    pub code: String,
    pub message: String,
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        AppErrorPayload {
            code: self.code().to_string(),
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type AppResult<T> = Result<T, AppError>;
