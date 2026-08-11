use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    Draft,
    Recording,
    Processing,
    Completed,
}

impl MeetingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Recording => "recording",
            Self::Processing => "processing",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "recording" => Some(Self::Recording),
            "processing" => Some(Self::Processing),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: MeetingStatus,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Labels techniques de diarisation → noms confirmés (ex. SPEAKER_00 → Marie).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_map: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummary {
    pub id: String,
    pub title: String,
    pub status: MeetingStatus,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSearchFilters {
    pub query: Option<String>,
    pub status: Option<MeetingStatus>,
    pub provider_id: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingListItem {
    pub id: String,
    pub title: String,
    pub status: MeetingStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub updated_at: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSearchPage {
    pub items: Vec<MeetingListItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingDetail {
    #[serde(flatten)]
    pub meeting: Meeting,
    pub audio_files: Vec<super::AudioFile>,
    pub transcriptions: Vec<super::TranscriptionMetadata>,
    pub summaries: Vec<super::SummaryMetadata>,
    pub actions: Vec<super::Action>,
}

/// Full database representation used by exports and server-side workflows.
/// It must never be returned by the meeting detail IPC command.
#[derive(Debug, Clone)]
pub struct MeetingFullDetail {
    pub meeting: Meeting,
    pub audio_files: Vec<super::AudioFile>,
    pub transcriptions: Vec<super::Transcription>,
    pub summaries: Vec<super::Summary>,
    pub actions: Vec<super::Action>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMeetingInput {
    pub title: String,
    pub description: Option<String>,
}
