use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFile {
    pub id: String,
    pub meeting_id: String,
    pub file_path: String,
    pub duration_ms: Option<i64>,
    pub format: Option<String>,
    pub created_at: String,
}
