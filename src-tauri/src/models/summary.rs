use serde::{Deserialize, Serialize};

use crate::ai::structured_summary::SummaryValidationState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub meeting_id: String,
    pub provider_id: Option<String>,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub validation_state: SummaryValidationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryMetadata {
    pub id: String,
    pub meeting_id: String,
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub validation_state: SummaryValidationState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRevision {
    pub id: String,
    pub summary_id: String,
    pub meeting_id: String,
    pub content: String,
    pub validation_state: SummaryValidationState,
    pub model: Option<String>,
    pub provider_id: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
}
