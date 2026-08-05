use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub meeting_id: String,
    pub provider_id: Option<String>,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}
