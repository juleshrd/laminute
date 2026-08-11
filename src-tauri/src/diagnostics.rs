//! Diagnostic local, journaux bornés et bundle de support expurgé (JUL-205).

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};
use uuid::Uuid;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::ai::secrets;
use crate::audio::devices;
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::export_write::{issue_grant, write_granted};
use crate::AiAppState;

const LOGS_DIR_NAME: &str = "logs";
const LOG_FILE_NAME: &str = "laminute.log";
const MAX_LOG_FILE_BYTES: u64 = 512 * 1024;
const MAX_LOG_TOTAL_BYTES: u64 = 2 * 1024 * 1024;
const RING_CAPACITY: usize = 64;
const LOG_TAIL_LINES: usize = 200;

static RECENT_EVENTS: OnceLock<Mutex<VecDeque<DiagnosticEvent>>> = OnceLock::new();
static LOG_WRITER: OnceLock<Mutex<RotatingLogWriter>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub code: String,
    pub message: String,
    pub correlation_id: Option<String>,
    pub timestamp: String,
    pub subsystem: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSnapshot {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub app_data_dir: String,
    pub logs_dir: String,
    pub db_path: String,
    pub db_schema_version: Option<i64>,
    pub provider_id: Option<String>,
    pub transcription_model: Option<String>,
    pub summary_model: Option<String>,
    pub keyring_status: String,
    pub microphone_status: String,
    pub updater_status: String,
    pub recent_errors: Vec<DiagnosticEvent>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportBundleFilePreview {
    pub name: String,
    pub size_bytes: u64,
    pub text_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportBundlePreview {
    pub files: Vec<SupportBundleFilePreview>,
    pub preview_text: String,
    pub github_report: String,
}

struct BundleFile {
    name: String,
    bytes: Vec<u8>,
}

struct RotatingLogWriter {
    dir: PathBuf,
    file: File,
    current_size: u64,
}

impl RotatingLogWriter {
    fn open(dir: &Path) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(LOG_FILE_NAME);
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let current_size = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            dir: dir.to_path_buf(),
            file,
            current_size,
        })
    }

    fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        let payload = format!("{line}\n");
        let add = payload.len() as u64;
        if self.current_size > 0 && self.current_size + add > MAX_LOG_FILE_BYTES {
            self.rotate()?;
        }
        self.file.write_all(payload.as_bytes())?;
        self.file.flush()?;
        self.current_size += add;
        self.enforce_total_cap()?;
        Ok(())
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        let current = self.dir.join(LOG_FILE_NAME);
        let archive = self.dir.join(format!(
            "laminute-{}.log",
            Utc::now().format("%Y%m%d-%H%M%S-%3f")
        ));
        // Remplacer le handle avant rename pour libérer le fichier.
        let tmp_name = format!("laminute-rotate-{}.tmp", Uuid::new_v4());
        let tmp_path = std::env::temp_dir().join(tmp_name);
        let placeholder = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tmp_path)?;
        let old = std::mem::replace(&mut self.file, placeholder);
        drop(old);
        let _ = fs::remove_file(&tmp_path);

        if current.exists() {
            fs::rename(&current, &archive)?;
        }
        self.file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&current)?;
        self.current_size = 0;
        self.enforce_total_cap()?;
        Ok(())
    }

    fn enforce_total_cap(&self) -> std::io::Result<()> {
        let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
        let mut total = 0u64;
        if !self.dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let meta = entry.metadata()?;
            let len = meta.len();
            let modified = meta
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            total += len;
            entries.push((path, len, modified));
        }
        if total <= MAX_LOG_TOTAL_BYTES {
            return Ok(());
        }
        entries.sort_by_key(|(_, _, modified)| *modified);
        for (path, len, _) in entries {
            if path.file_name().and_then(|n| n.to_str()) == Some(LOG_FILE_NAME) {
                continue;
            }
            if total <= MAX_LOG_TOTAL_BYTES {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(len);
            }
        }
        Ok(())
    }
}

fn append_log_line(line: &str) {
    let Some(writer) = LOG_WRITER.get() else {
        return;
    };
    let Ok(mut guard) = writer.lock() else {
        return;
    };
    let _ = guard.write_line(line);
}

struct RingBufferLayer;

impl<S> Layer<S> for RingBufferLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        if level > Level::WARN {
            return;
        }
        let mut visitor = MessageVisitor::default();
        event.record(&mut visitor);
        let raw = visitor
            .message
            .unwrap_or_else(|| event.metadata().name().into());
        let redacted = redact_text(&raw);
        let code = if level == Level::ERROR {
            "tracing_error"
        } else {
            "tracing_warn"
        };
        push_event(DiagnosticEvent {
            code: code.into(),
            message: truncate_message(&redacted),
            correlation_id: visitor.correlation_id,
            timestamp: Utc::now().to_rfc3339(),
            subsystem: event.metadata().target().to_string(),
        });
        let line = format!(
            "{} level={} target={} msg={}",
            Utc::now().to_rfc3339(),
            level,
            event.metadata().target(),
            redacted.replace('\n', " ")
        );
        append_log_line(&line);
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: Option<String>,
    correlation_id: Option<String>,
}

impl tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = Some(value.to_string()),
            "correlation_id" => self.correlation_id = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(format!("{value:?}"));
        }
    }
}

fn recent_events() -> &'static Mutex<VecDeque<DiagnosticEvent>> {
    RECENT_EVENTS.get_or_init(|| Mutex::new(VecDeque::with_capacity(RING_CAPACITY)))
}

fn push_event(event: DiagnosticEvent) {
    if let Ok(mut guard) = recent_events().lock() {
        if guard.len() >= RING_CAPACITY {
            guard.pop_front();
        }
        guard.push_back(event);
    }
}

/// Identifiant de corrélation pour une opération / un job.
pub fn new_correlation_id() -> String {
    Uuid::new_v4().to_string()
}

/// Enregistre un événement déjà destiné à l'utilisateur (message expurgé).
pub fn record_event(
    code: &str,
    message: &str,
    subsystem: &str,
    correlation_id: Option<&str>,
) {
    let redacted = redact_text(message);
    let event = DiagnosticEvent {
        code: code.to_string(),
        message: truncate_message(&redacted),
        correlation_id: correlation_id.map(str::to_string),
        timestamp: Utc::now().to_rfc3339(),
        subsystem: subsystem.to_string(),
    };
    let line = format!(
        "{} level=error code={} subsystem={} correlation_id={} msg={}",
        event.timestamp,
        event.code,
        event.subsystem,
        event.correlation_id.as_deref().unwrap_or("-"),
        event.message.replace('\n', " ")
    );
    append_log_line(&line);
    push_event(event);
}

pub fn capture_app_error(err: &AppError, subsystem: &str) -> String {
    let correlation = new_correlation_id();
    record_event(err.code(), &err.to_string(), subsystem, Some(&correlation));
    err.to_string()
}

pub fn logs_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(LOGS_DIR_NAME)
}

/// Initialise le journal structuré local sous `app_data_dir/logs/`.
pub fn init_logging(app_data_dir: &Path) -> AppResult<()> {
    let dir = logs_dir(app_data_dir);
    fs::create_dir_all(&dir)?;
    let writer = RotatingLogWriter::open(&dir)?;
    let _ = LOG_WRITER.set(Mutex::new(writer));

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(RingBufferLayer)
        .try_init();

    tracing::info!(target: "laminute::diagnostics", "journal local initialisé");
    Ok(())
}

fn truncate_message(message: &str) -> String {
    const MAX: usize = 400;
    let trimmed = message.trim();
    if trimmed.chars().count() <= MAX {
        return trimmed.to_string();
    }
    let truncated: String = trimmed.chars().take(MAX).collect();
    format!("{truncated}…")
}

fn is_secret_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn redact_api_key_patterns(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        // Bearer tokens (ASCII keyword).
        if input.is_char_boundary(i + 7) && lower.as_bytes()[i..i + 7] == *b"bearer " {
            out.push_str(&input[i..i + 7]);
            i += 7;
            while i < input.len() && input.as_bytes()[i].is_ascii_whitespace() {
                i += 1;
            }
            while i < input.len() {
                let c = input[i..].chars().next().unwrap_or('\0');
                if !(is_secret_char(c) || c == '.' || c == '=') {
                    break;
                }
                i += c.len_utf8();
            }
            out.push_str("[REDACTED]");
            continue;
        }

        // sk- / sk_
        if input.is_char_boundary(i)
            && i + 3 <= input.len()
            && input.is_char_boundary(i + 3)
        {
            let prefix = &lower[i..i + 3];
            if prefix == "sk-" || prefix == "sk_" {
                let mut j = i + 3;
                while j < input.len() {
                    let c = input[j..].chars().next().unwrap_or('\0');
                    if !is_secret_char(c) {
                        break;
                    }
                    j += c.len_utf8();
                }
                if j - (i + 3) >= 8 {
                    out.push_str("[REDACTED_API_KEY]");
                    i = j;
                    continue;
                }
            }
        }

        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn redact_labeled_content(input: &str) -> String {
    let mut out = String::new();
    for line in input.lines() {
        let upper = line.to_ascii_uppercase();
        let mut redacted_line = line.to_string();
        for label in ["TRANSCRIPTION", "TRANSCRIPTION_CONTENT", "MEETING_BODY"] {
            if let Some(idx) = upper.find(label) {
                let after_label = idx + label.len();
                let rest = &line[after_label..];
                let trimmed = rest.trim_start_matches([' ', '\t', ':', '=']);
                if !trimmed.is_empty() && !trimmed.starts_with("[REDACTED_CONTENT]") {
                    let prefix_end = line.len() - trimmed.len();
                    redacted_line = format!("{}[REDACTED_CONTENT]", &line[..prefix_end]);
                }
            }
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&redacted_line);
    }
    if input.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn redact_home_paths(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        let candidates = ["/home/", "/Users/", "C:\\Users\\", "C:/Users/"];
        let mut best: Option<(usize, &str)> = None;
        for prefix in candidates {
            if let Some(idx) = rest.find(prefix) {
                if best.map(|(b, _)| idx < b).unwrap_or(true) {
                    best = Some((idx, prefix));
                }
            }
        }
        let Some((idx, prefix)) = best else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..idx]);
        out.push_str("[REDACTED_PATH]");
        let after_prefix = &rest[idx + prefix.len()..];
        // skip user segment
        let user_end = after_prefix
            .find(|c: char| c == '/' || c == '\\' || c.is_whitespace())
            .unwrap_or(after_prefix.len());
        let after_user = &after_prefix[user_end..];
        // skip remaining path chars
        let path_end = after_user
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ')')
            .unwrap_or(after_user.len());
        rest = &after_user[path_end..];
    }
    out
}

/// Expurgation des secrets et contenus sensibles avant journalisation / bundle.
pub fn redact_text(input: &str) -> String {
    let step1 = redact_api_key_patterns(input);
    let step2 = redact_labeled_content(&step1);
    redact_home_paths(&step2)
}

fn contains_raw_api_key(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i + 11 <= bytes.len() {
        if bytes[i].eq_ignore_ascii_case(&b's')
            && bytes[i + 1].eq_ignore_ascii_case(&b'k')
            && (bytes[i + 2] == b'-' || bytes[i + 2] == b'_')
        {
            let mut j = i + 3;
            while j < bytes.len() && is_secret_char(bytes[j] as char) {
                j += 1;
            }
            if j - (i + 3) >= 8 {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn db_schema_version(conn: &rusqlite::Connection) -> Option<i64> {
    conn.query_row(
        "SELECT MAX(version) FROM refinery_schema_history",
        [],
        |row| row.get::<_, Option<i64>>(0),
    )
    .ok()
    .flatten()
}

fn keyring_status(provider_id: Option<&str>) -> String {
    let Some(provider_id) = provider_id else {
        return "no_provider".into();
    };
    if provider_id == "ollama" {
        return "not_required".into();
    }
    match secrets::has_api_key(provider_id) {
        Ok(true) => "configured".into(),
        Ok(false) => "no_key".into(),
        Err(_) => "unavailable".into(),
    }
}

fn microphone_status() -> String {
    match devices::list_input_devices() {
        Ok(list) => format!("ok:{} devices", list.len()),
        Err(err) => format!("error:{}", err.code()),
    }
}

fn build_snapshot(
    storage: &crate::storage::StorageState,
    db: &AppState,
    ai: &AiAppState,
) -> AppResult<DiagnosticsSnapshot> {
    let app_data_dir = storage.root();
    let logs = logs_dir(&app_data_dir);
    let db_path = app_data_dir.join("laminute.db");

    let schema = db.with_db(|conn| Ok(db_schema_version(conn)))?;

    let (provider_id, transcription_model, summary_model) = {
        let settings = ai
            .settings
            .lock()
            .map_err(|_| AppError::Message("réglages IA indisponibles".into()))?;
        let provider = settings.selected_provider_id().map(str::to_string);
        let tx = provider
            .as_deref()
            .and_then(|id| settings.transcription_model_for(id));
        let sum = provider
            .as_deref()
            .and_then(|id| settings.summary_model_for(id));
        (provider, tx, sum)
    };

    let recent = recent_events()
        .lock()
        .map(|guard| guard.iter().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    Ok(DiagnosticsSnapshot {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_data_dir: app_data_dir.to_string_lossy().to_string(),
        logs_dir: logs.to_string_lossy().to_string(),
        db_path: db_path.to_string_lossy().to_string(),
        db_schema_version: schema,
        provider_id: provider_id.clone(),
        transcription_model,
        summary_model,
        keyring_status: keyring_status(provider_id.as_deref()),
        microphone_status: microphone_status(),
        updater_status: "plugin_configured".into(),
        recent_errors: recent,
    })
}

fn github_report_from_snapshot(snapshot: &DiagnosticsSnapshot) -> String {
    let mut lines = vec![
        "## Rapport de diagnostic — La Minute".to_string(),
        String::new(),
        format!("- Version : `{}`", snapshot.app_version),
        format!("- OS : `{}` / `{}`", snapshot.os, snapshot.arch),
        format!(
            "- Schéma DB : `{}`",
            snapshot
                .db_schema_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "inconnu".into())
        ),
        format!(
            "- Fournisseur : `{}`",
            snapshot.provider_id.as_deref().unwrap_or("aucun")
        ),
        format!(
            "- Modèle transcription : `{}`",
            snapshot.transcription_model.as_deref().unwrap_or("—")
        ),
        format!(
            "- Modèle compte-rendu : `{}`",
            snapshot.summary_model.as_deref().unwrap_or("—")
        ),
        format!("- Trousseau : `{}`", snapshot.keyring_status),
        format!("- Micro : `{}`", snapshot.microphone_status),
        format!("- Updater : `{}`", snapshot.updater_status),
        String::new(),
        "### Derniers codes d'erreur".to_string(),
    ];
    if snapshot.recent_errors.is_empty() {
        lines.push("- (aucun)".into());
    } else {
        for event in snapshot.recent_errors.iter().rev().take(10) {
            lines.push(format!(
                "- `{}` ({}) — {}",
                event.code, event.subsystem, event.message
            ));
        }
    }
    lines.push(String::new());
    lines.push(
        "_Aucun secret, transcription ni fichier audio n'est inclus dans ce rapport._".into(),
    );
    lines.join("\n")
}

fn read_log_tail(logs_dir: &Path) -> String {
    let path = logs_dir.join(LOG_FILE_NAME);
    if !path.exists() {
        return String::new();
    }
    let Ok(file) = File::open(&path) else {
        return String::new();
    };
    let lines: Vec<String> = BufReader::new(file)
        .lines()
        .filter_map(Result::ok)
        .collect();
    let start = lines.len().saturating_sub(LOG_TAIL_LINES);
    lines[start..]
        .iter()
        .map(|line| redact_text(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_bundle_files(snapshot: &DiagnosticsSnapshot) -> AppResult<Vec<BundleFile>> {
    let github_report = github_report_from_snapshot(snapshot);
    let snapshot_json = serde_json::to_string_pretty(snapshot)
        .map_err(|err| AppError::Message(err.to_string()))?;
    let events_json = serde_json::to_string_pretty(&snapshot.recent_errors)
        .map_err(|err| AppError::Message(err.to_string()))?;
    let log_tail = read_log_tail(Path::new(&snapshot.logs_dir));

    let mut files = vec![
        BundleFile {
            name: "diagnostics.json".into(),
            bytes: snapshot_json.into_bytes(),
        },
        BundleFile {
            name: "recent-errors.json".into(),
            bytes: events_json.into_bytes(),
        },
        BundleFile {
            name: "github-report.md".into(),
            bytes: github_report.into_bytes(),
        },
    ];
    if !log_tail.is_empty() {
        files.push(BundleFile {
            name: "logs-tail.txt".into(),
            bytes: log_tail.into_bytes(),
        });
    }

    for file in &mut files {
        if looks_like_text(&file.name) {
            let text = String::from_utf8_lossy(&file.bytes);
            file.bytes = redact_text(&text).into_bytes();
        }
    }
    Ok(files)
}

fn looks_like_text(name: &str) -> bool {
    name.ends_with(".json")
        || name.ends_with(".md")
        || name.ends_with(".txt")
        || name.ends_with(".log")
}

fn preview_from_files(files: &[BundleFile], github_report: String) -> SupportBundlePreview {
    let mut preview_chunks = Vec::new();
    let previews: Vec<SupportBundleFilePreview> = files
        .iter()
        .map(|file| {
            let text_preview = if looks_like_text(&file.name) {
                let text = String::from_utf8_lossy(&file.bytes).to_string();
                let clipped = if text.chars().count() > 4_000 {
                    let clipped: String = text.chars().take(4_000).collect();
                    format!("{clipped}…")
                } else {
                    text.clone()
                };
                preview_chunks.push(format!("----- {} -----\n{clipped}", file.name));
                Some(clipped)
            } else {
                None
            };
            SupportBundleFilePreview {
                name: file.name.clone(),
                size_bytes: file.bytes.len() as u64,
                text_preview,
            }
        })
        .collect();

    SupportBundlePreview {
        files: previews,
        preview_text: preview_chunks.join("\n\n"),
        github_report,
    }
}

fn zip_bundle(files: &[BundleFile]) -> AppResult<Vec<u8>> {
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for file in files {
            zip.start_file(&file.name, options)
                .map_err(|err| AppError::Message(format!("zip : {err}")))?;
            zip.write_all(&file.bytes)
                .map_err(|err| AppError::Message(format!("zip write : {err}")))?;
        }
        zip.finish()
            .map_err(|err| AppError::Message(format!("zip finish : {err}")))?;
    }
    Ok(cursor.into_inner())
}

fn assert_bundle_is_safe(files: &[BundleFile]) -> AppResult<()> {
    for file in files {
        let text = String::from_utf8_lossy(&file.bytes);
        if contains_raw_api_key(&text) {
            return Err(AppError::Message(
                "bundle refusé : secret potentiel détecté".into(),
            ));
        }
        for line in text.lines() {
            let upper = line.to_ascii_uppercase();
            for label in ["TRANSCRIPTION:", "MEETING_BODY:"] {
                if let Some(idx) = upper.find(label) {
                    let after = line[idx + label.len()..].trim_start();
                    if !after.is_empty() && !after.starts_with("[REDACTED_CONTENT]") {
                        return Err(AppError::Message(
                            "bundle refusé : contenu de réunion potentiel".into(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_diagnostics_snapshot(
    storage: State<'_, crate::storage::StorageState>,
    state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
) -> Result<DiagnosticsSnapshot, String> {
    build_snapshot(&storage, &state, &ai_state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn preview_support_bundle(
    storage: State<'_, crate::storage::StorageState>,
    state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
) -> Result<SupportBundlePreview, String> {
    let snapshot = build_snapshot(&storage, &state, &ai_state).map_err(|e| e.to_string())?;
    let github_report = github_report_from_snapshot(&snapshot);
    let files = build_bundle_files(&snapshot).map_err(|e| e.to_string())?;
    assert_bundle_is_safe(&files).map_err(|e| e.to_string())?;
    Ok(preview_from_files(&files, github_report))
}

#[tauri::command]
pub async fn save_support_bundle(
    app: AppHandle,
    storage: State<'_, crate::storage::StorageState>,
    state: State<'_, AppState>,
    ai_state: State<'_, AiAppState>,
) -> Result<bool, String> {
    let snapshot = build_snapshot(&storage, &state, &ai_state).map_err(|e| e.to_string())?;
    let files = build_bundle_files(&snapshot).map_err(|e| e.to_string())?;
    assert_bundle_is_safe(&files).map_err(|e| e.to_string())?;
    let zip_bytes = zip_bundle(&files).map_err(|e| e.to_string())?;

    let default_name = format!("laminute-support-{}.zip", Utc::now().format("%Y-%m-%d"));

    let file_path = app
        .dialog()
        .file()
        .add_filter("ZIP", &["zip"])
        .set_file_name(&default_name)
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(false);
    };

    let path = file_path
        .into_path()
        .map_err(|err| format!("chemin de bundle invalide : {err}"))?;
    let grant = issue_grant(path);
    write_granted(&grant, &zip_bytes).map_err(|err| err.to_string())?;
    Ok(true)
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDiagnosticInput {
    pub code: String,
    pub message: String,
    pub subsystem: Option<String>,
    pub correlation_id: Option<String>,
}

#[tauri::command]
pub fn report_diagnostic_event(input: ReportDiagnosticInput) -> Result<(), String> {
    let code = input.code.trim();
    if code.is_empty() || code.len() > 64 {
        return Err("code diagnostic invalide".into());
    }
    let subsystem = input
        .subsystem
        .as_deref()
        .unwrap_or("frontend")
        .trim()
        .chars()
        .take(64)
        .collect::<String>();
    record_event(
        code,
        &input.message,
        &subsystem,
        input.correlation_id.as_deref(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    #[test]
    fn redact_strips_api_keys_and_bearer() {
        let raw = "Authorization: Bearer sk-abcDEF1234567890 secret=sk_live_ABCDEFGH1234";
        let out = redact_text(raw);
        assert!(!out.contains("sk-abcDEF1234567890"), "{out}");
        assert!(!out.contains("sk_live_ABCDEFGH1234"), "{out}");
        assert!(out.contains("[REDACTED"), "{out}");
    }

    #[test]
    fn redact_strips_transcription_body() {
        let raw =
            "code=ok\nTRANSCRIPTION: Bonjour ceci est le corps secret de la reunion\nOTHER=1";
        let out = redact_text(raw);
        assert!(!out.contains("Bonjour ceci est le corps"), "{out}");
        assert!(out.contains("[REDACTED_CONTENT]"), "{out}");
    }

    #[test]
    fn redact_strips_absolute_home_paths() {
        let raw = "db=/home/alice/.local/share/app.laminute.desktop/laminute.db";
        let out = redact_text(raw);
        assert!(!out.contains("/home/alice"), "{out}");
        assert!(out.contains("[REDACTED_PATH]"), "{out}");
    }

    #[test]
    fn bundle_never_contains_secrets_or_meeting_bodies() {
        let tmp = tempfile::tempdir().unwrap();
        let logs = tmp.path().join("logs");
        fs::create_dir_all(&logs).unwrap();
        fs::write(
            logs.join(LOG_FILE_NAME),
            "level=error msg=Authorization: Bearer sk-SECRETKEY12345678 TRANSCRIPTION: corps secret reunion\n",
        )
        .unwrap();

        let snapshot = DiagnosticsSnapshot {
            app_version: "0.1.2".into(),
            os: "linux".into(),
            arch: "x86_64".into(),
            app_data_dir: "[REDACTED_PATH]/app".into(),
            logs_dir: logs.to_string_lossy().to_string(),
            db_path: "[REDACTED_PATH]/db".into(),
            db_schema_version: Some(4),
            provider_id: Some("mistral".into()),
            transcription_model: Some("voxtral-mini-latest".into()),
            summary_model: Some("mistral-small-latest".into()),
            keyring_status: "configured".into(),
            microphone_status: "error:no_input_device".into(),
            updater_status: "plugin_configured".into(),
            recent_errors: vec![DiagnosticEvent {
                code: "db_error".into(),
                message: redact_text("échec avec sk-LEAKEDKEY99999999"),
                correlation_id: Some("cid-1".into()),
                timestamp: Utc::now().to_rfc3339(),
                subsystem: "db".into(),
            }],
        };

        let files = build_bundle_files(&snapshot).unwrap();
        assert_bundle_is_safe(&files).unwrap();

        let joined = files
            .iter()
            .map(|f| String::from_utf8_lossy(&f.bytes).to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!joined.contains("sk-SECRETKEY12345678"), "{joined}");
        assert!(!joined.contains("sk-LEAKEDKEY99999999"), "{joined}");
        assert!(!joined.contains("corps secret reunion"), "{joined}");

        let zip_bytes = zip_bundle(&files).unwrap();
        assert!(zip_bytes.len() > 32);
        assert_eq!(&zip_bytes[0..2], b"PK");

        let report = github_report_from_snapshot(&snapshot);
        assert!(report.contains("0.1.2"));
        assert!(report.contains("db_error"));
        assert!(!contains_raw_api_key(&report));
    }

    #[test]
    fn record_event_lands_in_ring_buffer() {
        let _lock = TEST_LOCK.lock().unwrap();
        if let Ok(mut guard) = recent_events().lock() {
            guard.clear();
        }
        record_event(
            "io_error",
            "échec lecture /home/bob/secret.mp3 sk-ABCDEFghijkl",
            "audio",
            Some("corr-42"),
        );
        let guard = recent_events().lock().unwrap();
        let last = guard.back().expect("event");
        assert_eq!(last.code, "io_error");
        assert_eq!(last.correlation_id.as_deref(), Some("corr-42"));
        assert!(!last.message.contains("sk-ABCDEF"), "{}", last.message);
        assert!(!last.message.contains("/home/bob"), "{}", last.message);
    }

    #[test]
    fn schema_version_reads_refinery_history() {
        let conn = crate::db::open_in_memory().unwrap();
        let version = db_schema_version(&conn);
        assert_eq!(version, Some(4));
    }

    #[test]
    fn app_error_codes_are_stable() {
        assert_eq!(AppError::Message("x".into()).code(), "message");
        assert_eq!(
            AppError::MeetingNotFound { id: "1".into() }.code(),
            "meeting_not_found"
        );
    }
}
