use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiJobKind {
    Transcription,
    Summary,
}

impl AiJobKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Transcription => "transcription",
            Self::Summary => "summary",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "transcription" => Some(Self::Transcription),
            "summary" => Some(Self::Summary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiJobStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl AiJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAiJobOutput {
    pub job_id: String,
    pub cancelled: bool,
}

#[derive(Debug)]
pub enum BeginAiJobError {
    Duplicate { job_id: String },
    LockUnavailable,
}

impl std::fmt::Display for BeginAiJobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate { job_id } => write!(f, "traitement IA déjà en cours ({job_id})"),
            Self::LockUnavailable => write!(f, "verrou des traitements IA indisponible"),
        }
    }
}

struct AiJobRecord {
    job_id: String,
    key: String,
    cancelled: bool,
    status: AiJobStatus,
    cancel_token: CancellationToken,
}

#[derive(Default)]
struct AiJobRegistry {
    jobs: HashMap<String, AiJobRecord>,
    active_by_key: HashMap<String, String>,
}

pub struct AiJobState {
    registry: Mutex<AiJobRegistry>,
}

impl AiJobState {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(AiJobRegistry::default()),
        }
    }

    pub fn new_job_id(kind: AiJobKind) -> String {
        let prefix = match kind {
            AiJobKind::Transcription => "transcription",
            AiJobKind::Summary => "summary",
        };
        format!("{prefix}-{}", Uuid::new_v4())
    }

    pub fn begin(
        &self,
        job_id: String,
        _kind: AiJobKind,
        key: String,
    ) -> Result<AiJobGuard<'_>, BeginAiJobError> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| BeginAiJobError::LockUnavailable)?;

        if let Some(existing_job_id) = registry.active_by_key.get(&key) {
            return Err(BeginAiJobError::Duplicate {
                job_id: existing_job_id.clone(),
            });
        }

        if let Some(existing) = registry.jobs.get(&job_id) {
            if existing.status == AiJobStatus::Running {
                return Err(BeginAiJobError::Duplicate {
                    job_id: existing.job_id.clone(),
                });
            }
        }

        registry.active_by_key.insert(key.clone(), job_id.clone());
        registry.jobs.insert(
            job_id.clone(),
            AiJobRecord {
                job_id: job_id.clone(),
                key,
                cancelled: false,
                status: AiJobStatus::Running,
                cancel_token: CancellationToken::new(),
            },
        );

        Ok(AiJobGuard {
            state: self,
            job_id,
            finished: false,
        })
    }

    pub fn cancel(&self, job_id: &str) -> Result<CancelAiJobOutput, String> {
        let mut registry = self
            .registry
            .lock()
            .map_err(|_| "verrou des traitements IA indisponible".to_string())?;
        let Some(record) = registry.jobs.get_mut(job_id) else {
            return Ok(CancelAiJobOutput {
                job_id: job_id.to_string(),
                cancelled: false,
            });
        };

        if record.status == AiJobStatus::Running {
            record.cancelled = true;
            record.status = AiJobStatus::Cancelled;
            record.cancel_token.cancel();
            return Ok(CancelAiJobOutput {
                job_id: job_id.to_string(),
                cancelled: true,
            });
        }

        Ok(CancelAiJobOutput {
            job_id: job_id.to_string(),
            cancelled: record.status == AiJobStatus::Cancelled,
        })
    }

    pub fn ensure_not_cancelled(&self, job_id: &str) -> Result<(), String> {
        let registry = self
            .registry
            .lock()
            .map_err(|_| "verrou des traitements IA indisponible".to_string())?;
        let Some(record) = registry.jobs.get(job_id) else {
            return Err("traitement IA introuvable".to_string());
        };
        if record.cancelled
            || record.status == AiJobStatus::Cancelled
            || record.cancel_token.is_cancelled()
        {
            return Err("traitement IA annulé".to_string());
        }
        Ok(())
    }

    pub fn cancellation_token(&self, job_id: &str) -> Option<CancellationToken> {
        let registry = self.registry.lock().ok()?;
        registry
            .jobs
            .get(job_id)
            .map(|record| record.cancel_token.clone())
    }

    fn finish(&self, job_id: &str, status: AiJobStatus) {
        if let Ok(mut registry) = self.registry.lock() {
            let key = registry.jobs.get(job_id).map(|record| record.key.clone());
            if let Some(record) = registry.jobs.get_mut(job_id) {
                if record.status != AiJobStatus::Cancelled {
                    record.status = status;
                }
            }
            if let Some(key) = key {
                registry.active_by_key.remove(&key);
            }
        }
    }
}

impl Default for AiJobState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AiJobGuard<'a> {
    state: &'a AiJobState,
    job_id: String,
    finished: bool,
}

impl AiJobGuard<'_> {
    pub fn job_id(&self) -> &str {
        &self.job_id
    }

    pub fn cancellation_token(&self) -> CancellationToken {
        self.state
            .cancellation_token(&self.job_id)
            .unwrap_or_default()
    }

    pub fn finish_completed(mut self) {
        self.state.finish(&self.job_id, AiJobStatus::Completed);
        self.finished = true;
    }

    pub fn finish_cancelled(mut self) {
        self.state.finish(&self.job_id, AiJobStatus::Cancelled);
        self.finished = true;
    }
}

impl Drop for AiJobGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.state.finish(&self.job_id, AiJobStatus::Failed);
        }
    }
}

pub fn meeting_job_key(meeting_id: &str) -> String {
    format!("meeting:{meeting_id}")
}

pub fn transcription_fallback_key(file_path: &str) -> String {
    format!("transcription:file:{file_path}")
}

pub fn summary_fallback_key(text: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("summary:text:{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_concurrent_incompatible_jobs_for_same_meeting() {
        let state = AiJobState::new();
        let first = state
            .begin(
                "job-1".into(),
                AiJobKind::Transcription,
                meeting_job_key("meeting-1"),
            )
            .expect("first job");

        let duplicate = match state.begin(
            "job-2".into(),
            AiJobKind::Summary,
            meeting_job_key("meeting-1"),
        ) {
            Ok(_) => panic!("same meeting must be locked"),
            Err(err) => err,
        };
        assert!(matches!(duplicate, BeginAiJobError::Duplicate { job_id } if job_id == "job-1"));

        first.finish_completed();
        assert!(state
            .begin(
                "job-3".into(),
                AiJobKind::Summary,
                meeting_job_key("meeting-1")
            )
            .is_ok());
    }

    #[test]
    fn duplicate_click_reuses_active_lock_without_second_job() {
        let state = AiJobState::new();
        let _first = state
            .begin(
                "job-1".into(),
                AiJobKind::Transcription,
                transcription_fallback_key("/tmp/audio.mp3"),
            )
            .expect("first job");

        let second = match state.begin(
            "job-2".into(),
            AiJobKind::Transcription,
            transcription_fallback_key("/tmp/audio.mp3"),
        ) {
            Ok(_) => panic!("duplicate"),
            Err(err) => err,
        };
        assert!(matches!(second, BeginAiJobError::Duplicate { job_id } if job_id == "job-1"));
    }

    #[test]
    fn cancellation_is_indexed_by_job_id() {
        let state = AiJobState::new();
        let job = state
            .begin(
                "job-1".into(),
                AiJobKind::Summary,
                meeting_job_key("meeting-1"),
            )
            .expect("job");
        let token = job.cancellation_token();
        let output = state.cancel("job-1").expect("cancel");
        assert!(output.cancelled);
        assert_eq!(output.job_id, "job-1");
        assert!(state.ensure_not_cancelled("job-1").is_err());
        assert!(token.is_cancelled());
        assert!(!state.cancel("missing").expect("missing").cancelled);
    }
}
