use serde::{Deserialize, Serialize};

use crate::ai::models::TranscriptionSegment;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transcription {
    pub id: String,
    pub meeting_id: String,
    pub audio_file_id: Option<String>,
    pub provider_id: Option<String>,
    pub content: String,
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<TranscriptionSegment>>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionMetadata {
    pub id: String,
    pub meeting_id: String,
    pub audio_file_id: Option<String>,
    pub provider_id: Option<String>,
    pub language: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
