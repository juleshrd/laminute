use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiJobKind {
    Transcription,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AiJobStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
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
    finished_at_ms: Option<u64>,
}

const DEFAULT_FINISHED_LRU_CAP: usize = 1000;
// 10 minutes. Objectif : borner la croissance sans casser l'UI.
const DEFAULT_FINISHED_TTL_MS: u64 = 10 * 60 * 1000;

fn system_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct AiJobRegistry {
    jobs: HashMap<String, AiJobRecord>,
    active_by_key: HashMap<String, String>,
    terminal_queue: VecDeque<String>,
    finished_lru_cap: usize,
    finished_ttl_ms: u64,
    terminal_count: usize,
    now_ms_fn: fn() -> u64,

    #[cfg(test)]
    evicted_lru_count: usize,
    #[cfg(test)]
    evicted_ttl_count: usize,
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

    #[cfg(test)]
    pub fn new_with_limits(now_ms_fn: fn() -> u64, finished_ttl_ms: u64, finished_lru_cap: usize) -> Self {
        Self {
            registry: Mutex::new(AiJobRegistry {
                jobs: HashMap::new(),
                active_by_key: HashMap::new(),
                terminal_queue: VecDeque::new(),
                finished_lru_cap,
                finished_ttl_ms,
                terminal_count: 0,
                now_ms_fn,
                #[cfg(test)]
                evicted_lru_count: 0,
                #[cfg(test)]
                evicted_ttl_count: 0,
            }),
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
                finished_at_ms: None,
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
        if record.cancelled || record.status == AiJobStatus::Cancelled {
            return Err("traitement IA annulé".to_string());
        }
        Ok(())
    }

    fn finish(&self, job_id: &str, status: AiJobStatus) {
        if let Ok(mut registry) = self.registry.lock() {
            let key = registry.jobs.get(job_id).map(|record| record.key.clone());
            let finished_at_ms = (registry.now_ms_fn)();
            if let Some(record) = registry.jobs.get_mut(job_id) {
                if record.status != AiJobStatus::Cancelled {
                    record.status = status;
                }

                // Le job est terminal : on le compte et on l'enfile pour purge TTL/LRU.
                if record.finished_at_ms.is_none() {
                    record.finished_at_ms = Some(finished_at_ms);
                    registry.terminal_queue.push_back(job_id.to_string());
                    registry.terminal_count += 1;
                }
            }
            if let Some(key) = key {
                registry.active_by_key.remove(&key);
            }

            registry.evict_if_needed_locked();
        }
    }
}

impl Default for AiJobRegistry {
    fn default() -> Self {
        Self {
            jobs: HashMap::new(),
            active_by_key: HashMap::new(),
            terminal_queue: VecDeque::new(),
            finished_lru_cap: DEFAULT_FINISHED_LRU_CAP,
            finished_ttl_ms: DEFAULT_FINISHED_TTL_MS,
            terminal_count: 0,
            now_ms_fn: system_now_ms,
            #[cfg(test)]
            evicted_lru_count: 0,
            #[cfg(test)]
            evicted_ttl_count: 0,
        }
    }
}

impl Default for AiJobState {
    fn default() -> Self {
        Self::new()
    }
}

impl AiJobRegistry {
    fn is_terminal_status(status: AiJobStatus) -> bool {
        status != AiJobStatus::Running
    }

    fn evict_if_needed_locked(&mut self) {
        let now = (self.now_ms_fn)();

        // 1) TTL : purge des jobs terminés trop anciens.
        while let Some(front_job_id) = self.terminal_queue.front().cloned() {
            let finished_at_ms = self
                .jobs
                .get(&front_job_id)
                .and_then(|r| r.finished_at_ms);

            match finished_at_ms {
                None => {
                    // Incohérence ou entrée déjà évincée : nettoyer la queue.
                    self.terminal_queue.pop_front();
                }
                Some(at_ms) => {
                    if now.saturating_sub(at_ms) > self.finished_ttl_ms {
                        self.terminal_queue.pop_front();
                        if let Some(removed) = self.jobs.remove(&front_job_id) {
                            if Self::is_terminal_status(removed.status) {
                                self.terminal_count = self.terminal_count.saturating_sub(1);
                                #[cfg(test)]
                                {
                                    self.evicted_ttl_count += 1;
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        // 2) LRU (plafond) : on évince les plus anciens terminés.
        while self.terminal_count > self.finished_lru_cap {
            let Some(front_job_id) = self.terminal_queue.pop_front() else {
                break;
            };
            let Some(removed) = self.jobs.remove(&front_job_id) else {
                continue;
            };
            if Self::is_terminal_status(removed.status) {
                self.terminal_count = self.terminal_count.saturating_sub(1);
                #[cfg(test)]
                {
                    self.evicted_lru_count += 1;
                }
            }
        }
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

    pub fn finish_completed(mut self) {
        self.state.finish(&self.job_id, AiJobStatus::Completed);
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_NOW_MS: AtomicU64 = AtomicU64::new(0);

    fn test_now_ms() -> u64 {
        TEST_NOW_MS.load(Ordering::SeqCst)
    }

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
        let _job = state
            .begin(
                "job-1".into(),
                AiJobKind::Summary,
                meeting_job_key("meeting-1"),
            )
            .expect("job");
        let output = state.cancel("job-1").expect("cancel");
        assert!(output.cancelled);
        assert_eq!(output.job_id, "job-1");
        assert!(state.ensure_not_cancelled("job-1").is_err());
        assert!(!state.cancel("missing").expect("missing").cancelled);
    }

    #[test]
    fn evicts_finished_jobs_lru_bounded() {
        let state = AiJobState::new_with_limits(test_now_ms, u64::MAX / 2, 5);
        TEST_NOW_MS.store(0, Ordering::SeqCst);

        for i in 0..10 {
            let job_id = format!("job-{i}");
            let guard = state
                .begin(
                    job_id.clone(),
                    AiJobKind::Summary,
                    meeting_job_key(&format!("meeting-{i}")),
                )
                .expect("begin");
            guard.finish_completed();
        }

        let registry = state.registry.lock().expect("lock");
        assert!(
            registry.terminal_count <= 5,
            "terminal_count = {}",
            registry.terminal_count
        );
        assert!(
            registry.jobs.len() <= 5,
            "jobs len = {}",
            registry.jobs.len()
        );
    }

    #[test]
    fn evicts_finished_jobs_ttl_expired() {
        TEST_NOW_MS.store(0, Ordering::SeqCst);
        let state = AiJobState::new_with_limits(test_now_ms, 1000, 1000);

        for i in 0..3 {
            let job_id = format!("job-ttl-{i}");
            let guard = state
                .begin(
                    job_id.clone(),
                    AiJobKind::Summary,
                    meeting_job_key(&format!("meeting-ttl-{i}")),
                )
                .expect("begin");
            guard.finish_completed();
        }

        // +2s => tout doit expirer, puis on déclenche l'éviction en terminant un job récent.
        TEST_NOW_MS.store(2000, Ordering::SeqCst);
        let guard = state
            .begin(
                "job-ttl-recent".into(),
                AiJobKind::Summary,
                meeting_job_key("meeting-ttl-recent"),
            )
            .expect("begin recent");
        guard.finish_completed();

        let registry = state.registry.lock().expect("lock");
        assert_eq!(
            registry.terminal_count, 1,
            "terminal_count = {}",
            registry.terminal_count
        );
        assert!(registry.jobs.contains_key("job-ttl-recent"));
        assert!(!registry.jobs.contains_key("job-ttl-0"));
    }

    #[test]
    fn does_not_evict_running_jobs_under_load() {
        TEST_NOW_MS.store(0, Ordering::SeqCst);
        let state = AiJobState::new_with_limits(test_now_ms, u64::MAX / 2, 5);

        let running_job_id = "running-job";
        let running_guard = state
            .begin(
                running_job_id.into(),
                AiJobKind::Summary,
                meeting_job_key("running-meeting"),
            )
            .expect("begin running");

        // Génère un grand nombre de jobs terminés pour dépasser le cap terminal.
        for i in 0..50 {
            let job_id = format!("job-load-{i}");
            let guard = state
                .begin(
                    job_id.clone(),
                    AiJobKind::Summary,
                    meeting_job_key(&format!("meeting-load-{i}")),
                )
                .expect("begin");
            guard.finish_completed();
        }

        state
            .ensure_not_cancelled(running_job_id)
            .expect("running job should remain");

        drop(running_guard);
    }
}
