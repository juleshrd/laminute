use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ai::jobs::{
    meeting_job_key, transcription_fallback_key, AiJobKind, AiJobState, CancelAiJobOutput,
};
use crate::ai::limits::validate_transcription_audio_size;
use crate::ai::model_catalog;
use crate::ai::models::TranscriptionOptions;
use crate::ai::secrets;
use crate::audio::paths::{ingest_if_needed, resolve_owned, ManagedAudioRoots};
use crate::db::AppState;
use crate::local_activity::LocalActivityGate;
use crate::models::{CreateMeetingInput, MeetingStatus, Transcription};
use crate::repository::MeetingRepository;
use crate::retention;
use crate::AiAppState;

const DEFAULT_PROVIDER_ID: &str = "mistral";
const TRANSCRIPTION_PROGRESS_EVENT: &str = "transcription-progress";
const DEFAULT_FINISHED_LRU_CAP: usize = 1000;
// 10 minutes. Compatible avec l'objectif : garder un historique court côté RAM.
const DEFAULT_FINISHED_TTL_MS: u64 = 10 * 60 * 1000;

fn system_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionPhase {
    Idle,
    Preparing,
    Uploading,
    Transcribing,
    Saving,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionProgress {
    pub job_id: String,
    pub phase: TranscriptionPhase,
    pub message: String,
    pub meeting_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeAudioInput {
    pub job_id: Option<String>,
    pub file_path: String,
    pub meeting_id: Option<String>,
    pub meeting_title: Option<String>,
    pub language: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeAudioOutput {
    pub job_id: String,
    pub transcription: Transcription,
}

pub struct TranscriptionState {
    inner: Mutex<TranscriptionStateInner>,
}

struct TranscriptionProgressRecord {
    progress: TranscriptionProgress,
    finished_at_ms: Option<u64>,
}

struct TranscriptionStateInner {
    progress_by_job: HashMap<String, TranscriptionProgressRecord>,
    latest_job_id: Option<String>,
    terminal_queue: VecDeque<String>,
    terminal_count: usize,
    finished_lru_cap: usize,
    finished_ttl_ms: u64,
    now_ms_fn: fn() -> u64,

    #[cfg(test)]
    evicted_lru_count: usize,
    #[cfg(test)]
    evicted_ttl_count: usize,
}

impl TranscriptionState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(TranscriptionStateInner {
                progress_by_job: HashMap::new(),
                latest_job_id: None,
                terminal_queue: VecDeque::new(),
                terminal_count: 0,
                finished_lru_cap: DEFAULT_FINISHED_LRU_CAP,
                finished_ttl_ms: DEFAULT_FINISHED_TTL_MS,
                now_ms_fn: system_now_ms,
                #[cfg(test)]
                evicted_lru_count: 0,
                #[cfg(test)]
                evicted_ttl_count: 0,
            }),
        }
    }

    #[cfg(test)]
    pub fn new_with_limits(
        now_ms_fn: fn() -> u64,
        finished_ttl_ms: u64,
        finished_lru_cap: usize,
    ) -> Self {
        Self {
            inner: Mutex::new(TranscriptionStateInner {
                progress_by_job: HashMap::new(),
                latest_job_id: None,
                terminal_queue: VecDeque::new(),
                terminal_count: 0,
                finished_lru_cap,
                finished_ttl_ms,
                now_ms_fn,
                #[cfg(test)]
                evicted_lru_count: 0,
                #[cfg(test)]
                evicted_ttl_count: 0,
            }),
        }
    }

    pub fn reset(&self) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "verrou transcription indisponible".to_string())?;
        inner.progress_by_job.clear();
        inner.latest_job_id = None;
        inner.terminal_queue.clear();
        inner.terminal_count = 0;
        Ok(())
    }

    fn is_terminal_phase(phase: &TranscriptionPhase) -> bool {
        matches!(phase, TranscriptionPhase::Completed | TranscriptionPhase::Failed)
    }

    fn store_progress(&self, progress: TranscriptionProgress) {
        if let Ok(mut inner) = self.inner.lock() {
            let job_id = progress.job_id.clone();
            let now = (inner.now_ms_fn)();

            if let Some(existing) = inner.progress_by_job.get_mut(&job_id) {
                existing.progress = progress.clone();
                if Self::is_terminal_phase(&progress.phase) && existing.finished_at_ms.is_none() {
                    existing.finished_at_ms = Some(now);
                    inner.terminal_queue.push_back(job_id.clone());
                    inner.terminal_count += 1;
                }
            } else {
                let finished_at_ms = if Self::is_terminal_phase(&progress.phase) {
                    Some(now)
                } else {
                    None
                };
                inner.progress_by_job.insert(
                    job_id.clone(),
                    TranscriptionProgressRecord {
                        progress: progress.clone(),
                        finished_at_ms,
                    },
                );
                if finished_at_ms.is_some() {
                    inner.terminal_queue.push_back(job_id.clone());
                    inner.terminal_count += 1;
                }
            }

            inner.latest_job_id = Some(job_id);

            if Self::is_terminal_phase(&progress.phase) {
                inner.evict_if_needed_locked();
            }
        }
    }
}

impl Default for TranscriptionState {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_progress(
    app: &AppHandle,
    transcription_state: &TranscriptionState,
    progress: TranscriptionProgress,
) {
    transcription_state.store_progress(progress.clone());
    let _ = app.emit(TRANSCRIPTION_PROGRESS_EVENT, &progress);
}

#[tauri::command]
pub fn get_transcription_progress(
    state: State<'_, TranscriptionState>,
    job_id: Option<String>,
) -> Result<Option<TranscriptionProgress>, String> {
    get_transcription_progress_for_job(&state, job_id)
}

#[tauri::command]
pub fn cancel_ai_job(
    jobs: State<'_, AiJobState>,
    job_id: String,
) -> Result<CancelAiJobOutput, String> {
    jobs.cancel(&job_id)
}

fn get_transcription_progress_for_job(
    state: &TranscriptionState,
    job_id: Option<String>,
) -> Result<Option<TranscriptionProgress>, String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "verrou transcription indisponible".to_string())?;

    if let Some(job_id) = job_id {
        return Ok(inner
            .progress_by_job
            .get(&job_id)
            .map(|r| r.progress.clone()));
    }

    Ok(inner
        .latest_job_id
        .as_deref()
        .and_then(|id| inner.progress_by_job.get(id))
        .map(|r| r.progress.clone()))
}

impl TranscriptionStateInner {
    fn evict_if_needed_locked(&mut self) {
        let now = (self.now_ms_fn)();

        // 1) TTL : suppression des entrées terminales trop anciennes.
        while let Some(front_job_id) = self.terminal_queue.front().cloned() {
            let finished_at_ms = self
                .progress_by_job
                .get(&front_job_id)
                .and_then(|r| r.finished_at_ms);

            match finished_at_ms {
                None => {
                    self.terminal_queue.pop_front();
                }
                Some(at_ms) => {
                    if now.saturating_sub(at_ms) > self.finished_ttl_ms {
                        self.terminal_queue.pop_front();
                        if self.progress_by_job.remove(&front_job_id).is_some() {
                            self.terminal_count = self.terminal_count.saturating_sub(1);
                            #[cfg(test)]
                            {
                                self.evicted_ttl_count += 1;
                            }
                        }
                        if self.latest_job_id.as_deref() == Some(&front_job_id) {
                            self.latest_job_id = None;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        // 2) LRU plafonné : suppression des plus anciens terminés.
        while self.terminal_count > self.finished_lru_cap {
            let Some(front_job_id) = self.terminal_queue.pop_front() else {
                break;
            };
            let removed = self.progress_by_job.remove(&front_job_id);
            if removed.is_some() {
                self.terminal_count = self.terminal_count.saturating_sub(1);
                #[cfg(test)]
                {
                    self.evicted_lru_count += 1;
                }
                if self.latest_job_id.as_deref() == Some(&front_job_id) {
                    self.latest_job_id = None;
                }
            }
        }
    }
}

#[tauri::command]
pub async fn transcribe_audio_file(
    app: AppHandle,
    ai_state: State<'_, AiAppState>,
    db_state: State<'_, AppState>,
    transcription_state: State<'_, TranscriptionState>,
    jobs: State<'_, AiJobState>,
    gate: State<'_, LocalActivityGate>,
    input: TranscribeAudioInput,
) -> Result<TranscribeAudioOutput, String> {
    let job_id = input
        .job_id
        .clone()
        .unwrap_or_else(|| AiJobState::new_job_id(AiJobKind::Transcription));
    let job_key = input
        .meeting_id
        .as_deref()
        .map(meeting_job_key)
        .unwrap_or_else(|| transcription_fallback_key(&input.file_path));
    let job = jobs
        .begin(job_id.clone(), AiJobKind::Transcription, job_key)
        .map_err(|err| err.to_string())?;

    let activity = gate.begin_operation().map_err(|e| e.to_string())?;

    let (provider_id, transcription_model, diarize) = {
        let settings = ai_state
            .settings
            .lock()
            .map_err(|_| "verrou des réglages indisponible".to_string())?;
        let provider_id = settings
            .selected_provider_id()
            .map(str::to_string)
            .unwrap_or_else(|| DEFAULT_PROVIDER_ID.to_string());
        let transcription_model = settings.transcription_model_for(&provider_id);
        let diarize = settings.diarization_enabled();
        let transcription_model =
            model_catalog::validate_transcription_model(&provider_id, transcription_model)
                .map_err(|e| e.to_string())?;
        (provider_id, transcription_model, diarize)
    };

    let provider = ai_state
        .registry
        .require(&provider_id)
        .map_err(|e| e.to_string())?;

    if !provider.capabilities().transcription {
        return Err(format!(
            "Le fournisseur « {} » ne prend pas en charge la transcription audio. Choisissez OpenAI ou Mistral dans les réglages IA.",
            provider.display_name()
        ));
    }

    let api_key = secrets::get_api_key(&provider_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "Aucune clé API enregistrée — configurez {} dans les réglages.",
                provider.display_name()
            )
        })?;

    let provider_display_name = provider.display_name().to_string();
    let diarize = diarize && provider.capabilities().diarization;

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            job_id: job.job_id().to_string(),
            phase: TranscriptionPhase::Preparing,
            message: "Préparation de la transcription…".to_string(),
            meeting_id: input.meeting_id.clone(),
        },
    );

    jobs.ensure_not_cancelled(job.job_id())?;
    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let roots = ManagedAudioRoots::from_app(&app).map_err(|err| {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    // Copie éventuelle hors verrou SQLite ; seul un chemin owned sera uploadé.
    let owned_path = ingest_if_needed(Path::new(&input.file_path), &roots).map_err(|err| {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    let metadata = std::fs::metadata(&owned_path).map_err(|err| {
        let message = format!("Impossible de lire le fichier audio : {err}");
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        message
    })?;

    if let Err(err) = validate_transcription_audio_size(metadata.len()) {
        let message = err.to_string();
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: input.meeting_id.clone(),
            },
        );
        return Err(message);
    }

    let file_name = owned_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let format = owned_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_string);
    let owned_path_str = owned_path.to_string_lossy().to_string();

    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let meeting_id = db_state
        .with_db(|conn| {
            if let Some(existing_id) = &input.meeting_id {
                MeetingRepository::get_by_id(conn, existing_id)?;
                Ok(existing_id.clone())
            } else {
                let title = input.meeting_title.clone().unwrap_or_else(|| {
                    Local::now()
                        .format("Enregistrement %d/%m/%Y %H:%M")
                        .to_string()
                });
                let meeting = MeetingRepository::create(
                    conn,
                    CreateMeetingInput {
                        title,
                        description: None,
                    },
                )?;
                Ok(meeting.id)
            }
        })
        .map_err(|e| e.to_string())?;

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Preparing,
            message: "Enregistrement de l'audio…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let audio_file = db_state
        .with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Processing)?;
            MeetingRepository::attach_audio_file(
                conn,
                &meeting_id,
                &owned_path_str,
                input.duration_ms,
                format.as_deref(),
            )
        })
        .map_err(|e| e.to_string())?;

    // Re-vérification anti-TOCTOU juste avant l'upload cloud.
    let upload_path = resolve_owned(&owned_path, &roots).map_err(|err| {
        let message = err.to_string();
        let _ = db_state.with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
        });
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        message
    })?;

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Uploading,
            message: format!("Envoi de l'audio à {provider_display_name}…"),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    jobs.ensure_not_cancelled(job.job_id())?;
    gate.ensure_generation(activity)
        .map_err(|e| e.to_string())?;

    let result = ai_state
        .registry
        .transcribe_audio(
            &provider_id,
            &api_key,
            &upload_path,
            TranscriptionOptions {
                model: transcription_model,
                language: input.language.clone(),
                file_name,
                diarize,
            },
        )
        .await
        .map_err(|err| {
            let message = err.to_string();
            let _ = db_state.with_db(|conn| {
                MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
            });
            emit_progress(
                &app,
                &transcription_state,
                TranscriptionProgress {
                    job_id: job_id.clone(),
                    phase: TranscriptionPhase::Failed,
                    message: message.clone(),
                    meeting_id: Some(meeting_id.clone()),
                },
            );
            message
        })?;

    if let Err(message) = jobs.ensure_not_cancelled(job.job_id()) {
        let _ = db_state.with_db(|conn| {
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Draft)
        });
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: message.clone(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        return Err(message);
    }

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Saving,
            message: "Enregistrement de la transcription…".to_string(),
            meeting_id: Some(meeting_id.clone()),
        },
    );

    gate.ensure_generation(activity).map_err(|e| {
        emit_progress(
            &app,
            &transcription_state,
            TranscriptionProgress {
                job_id: job_id.clone(),
                phase: TranscriptionPhase::Failed,
                message: e.to_string(),
                meeting_id: Some(meeting_id.clone()),
            },
        );
        e.to_string()
    })?;

    let transcription = db_state
        .with_db(|conn| {
            let saved = MeetingRepository::create_transcription(
                conn,
                &meeting_id,
                Some(&audio_file.id),
                &provider_id,
                &provider_display_name,
                &result.text,
                result.language.as_deref(),
            )?;
            MeetingRepository::update_status(conn, &meeting_id, MeetingStatus::Completed)?;
            Ok(saved)
        })
        .map_err(|e| e.to_string())?;

    retention::maybe_purge_audio_files(&app, &db_state, &meeting_id).map_err(|e| e.to_string())?;

    emit_progress(
        &app,
        &transcription_state,
        TranscriptionProgress {
            job_id: job_id.clone(),
            phase: TranscriptionPhase::Completed,
            message: "Transcription terminée.".to_string(),
            meeting_id: Some(meeting_id),
        },
    );

    job.finish_completed();

    Ok(TranscribeAudioOutput {
        job_id,
        transcription,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_NOW_MS: AtomicU64 = AtomicU64::new(0);

    fn test_now_ms() -> u64 {
        TEST_NOW_MS.load(Ordering::SeqCst)
    }

    #[test]
    fn default_progress_is_idle() {
        let state = TranscriptionState::new();
        let progress = get_transcription_progress_for_job(&state, None).expect("progress");
        assert!(progress.is_none());
    }

    #[test]
    fn repository_persists_transcription_for_meeting() {
        let conn = open_in_memory().expect("db");
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Test".into(),
                description: None,
            },
        )
        .expect("meeting");

        let audio = MeetingRepository::attach_audio_file(
            &conn,
            &meeting.id,
            "/tmp/test.wav",
            Some(1000),
            Some("wav"),
        )
        .expect("audio");

        let transcription = MeetingRepository::create_transcription(
            &conn,
            &meeting.id,
            Some(&audio.id),
            "mistral",
            "Mistral AI",
            "Bonjour",
            Some("fr"),
        )
        .expect("transcription");

        assert_eq!(transcription.content, "Bonjour");
        let detail = MeetingRepository::get_detail(&conn, &meeting.id).expect("detail");
        assert_eq!(detail.transcriptions.len(), 1);
        assert_eq!(detail.audio_files.len(), 1);
    }

    #[test]
    fn evicts_terminal_transcription_entries_lru_bounded() {
        TEST_NOW_MS.store(0, Ordering::SeqCst);
        let state = TranscriptionState::new_with_limits(test_now_ms, u64::MAX / 2, 2);

        state.store_progress(TranscriptionProgress {
            job_id: "t1".into(),
            phase: TranscriptionPhase::Completed,
            message: "ok".into(),
            meeting_id: Some("m1".into()),
        });
        state.store_progress(TranscriptionProgress {
            job_id: "t2".into(),
            phase: TranscriptionPhase::Completed,
            message: "ok".into(),
            meeting_id: Some("m2".into()),
        });
        state.store_progress(TranscriptionProgress {
            job_id: "t3".into(),
            phase: TranscriptionPhase::Completed,
            message: "ok".into(),
            meeting_id: Some("m3".into()),
        });

        let inner = state.inner.lock().expect("lock");
        assert!(inner.progress_by_job.contains_key("t2"));
        assert!(inner.progress_by_job.contains_key("t3"));
        assert!(!inner.progress_by_job.contains_key("t1"));
        assert_eq!(inner.latest_job_id.as_deref(), Some("t3"));
        assert!(inner.terminal_count <= 2);
    }

    #[test]
    fn evicts_terminal_transcription_entries_ttl_expired() {
        TEST_NOW_MS.store(0, Ordering::SeqCst);
        let state = TranscriptionState::new_with_limits(test_now_ms, 1000, 1000);

        state.store_progress(TranscriptionProgress {
            job_id: "u1".into(),
            phase: TranscriptionPhase::Failed,
            message: "failed".into(),
            meeting_id: None,
        });
        state.store_progress(TranscriptionProgress {
            job_id: "u2".into(),
            phase: TranscriptionPhase::Failed,
            message: "failed".into(),
            meeting_id: None,
        });

        TEST_NOW_MS.store(2000, Ordering::SeqCst);
        state.store_progress(TranscriptionProgress {
            job_id: "u3".into(),
            phase: TranscriptionPhase::Completed,
            message: "ok".into(),
            meeting_id: None,
        });

        let inner = state.inner.lock().expect("lock");
        assert!(inner.progress_by_job.contains_key("u3"));
        assert!(!inner.progress_by_job.contains_key("u1"));
        assert!(!inner.progress_by_job.contains_key("u2"));
        assert_eq!(inner.terminal_count, 1);
        assert_eq!(inner.latest_job_id.as_deref(), Some("u3"));
    }

    #[test]
    fn evicts_terminal_transcription_entries_under_synthetic_load_10k() {
        TEST_NOW_MS.store(0, Ordering::SeqCst);
        let cap = 1000;
        let state = TranscriptionState::new_with_limits(test_now_ms, u64::MAX / 2, cap);

        for i in 0..10_000 {
            state.store_progress(TranscriptionProgress {
                job_id: format!("t-{i}"),
                phase: TranscriptionPhase::Completed,
                message: "ok".into(),
                meeting_id: Some(format!("m-{i}")),
            });
        }

        let inner = state.inner.lock().expect("lock");
        assert!(
            inner.terminal_count <= cap,
            "terminal_count = {}",
            inner.terminal_count
        );
        assert!(
            inner.progress_by_job.len() <= cap,
            "progress_by_job len = {}",
            inner.progress_by_job.len()
        );
        assert_eq!(inner.latest_job_id.as_deref(), Some("t-9999"));
    }
}
