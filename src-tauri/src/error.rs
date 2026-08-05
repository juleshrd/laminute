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

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

pub type AppResult<T> = Result<T, AppError>;
