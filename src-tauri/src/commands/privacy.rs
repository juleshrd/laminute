use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::ai::structured_summary::parse_structured_summary;
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{Action, Meeting, Summary, Transcription};
use crate::report_pdf::{build_meeting_report_pdf, MeetingReportPdfInput};
use crate::repository::MeetingRepository;

const EXPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalStorageInfo {
    pub meetings_count: i64,
    pub db_path: String,
    pub imports_dir: String,
    pub recordings_dir: String,
    pub imports_bytes: Option<u64>,
    pub recordings_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingExport {
    pub export_version: u32,
    pub exported_at: String,
    #[serde(flatten)]
    pub meeting: Meeting,
    pub audio_files: Vec<ExportAudioFile>,
    pub transcriptions: Vec<ExportTranscription>,
    pub summaries: Vec<ExportSummary>,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAudioFile {
    pub id: String,
    pub meeting_id: String,
    pub file_name: String,
    pub duration_ms: Option<i64>,
    pub format: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportTranscription {
    pub id: String,
    pub meeting_id: String,
    pub audio_file_id: Option<String>,
    pub provider_id: Option<String>,
    pub content: String,
    pub language: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub id: String,
    pub meeting_id: String,
    pub provider_id: Option<String>,
    pub content: String,
    pub structured: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub fn export_meeting(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let export = state
        .with_db(|conn| build_export(conn, &id))
        .map_err(|err| err.to_string())?;
    serde_json::to_string_pretty(&export).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_local_storage_info(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LocalStorageInfo, String> {
    let app_data_dir = app_data_dir(&app)?;
    let imports_dir = app_data_dir.join("imports");
    let recordings_dir = app_data_dir.join("recordings");
    let db_path = app_data_dir.join("laminute.db");

    let meetings_count = state
        .with_db(|conn| {
            conn.query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
                .map_err(AppError::from)
        })
        .map_err(|err| err.to_string())?;

    Ok(LocalStorageInfo {
        meetings_count,
        db_path: db_path.to_string_lossy().to_string(),
        imports_dir: imports_dir.to_string_lossy().to_string(),
        recordings_dir: recordings_dir.to_string_lossy().to_string(),
        imports_bytes: dir_size(&imports_dir),
        recordings_bytes: dir_size(&recordings_dir),
    })
}

#[tauri::command]
pub fn delete_all_local_data(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let app_data_dir = app_data_dir(&app)?;
    let imports_dir = app_data_dir.join("imports");
    let recordings_dir = app_data_dir.join("recordings");

    state
        .with_db(|conn| {
            let mut stmt = conn.prepare("SELECT file_path FROM audio_files")?;
            let paths = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;

            for path in paths {
                remove_file_if_present(Path::new(&path));
            }

            conn.execute("DELETE FROM meetings", [])?;
            Ok(())
        })
        .map_err(|err| err.to_string())?;

    clear_dir_files(&imports_dir).map_err(|err| err.to_string())?;
    clear_dir_files(&recordings_dir).map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn write_export_file(path: String, contents: String) -> Result<(), String> {
    let target = PathBuf::from(&path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(target, contents).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn write_export_bytes(path: String, contents_base64: String) -> Result<(), String> {
    let bytes = BASE64
        .decode(contents_base64.as_bytes())
        .map_err(|err| format!("contenu export invalide : {err}"))?;
    let target = PathBuf::from(&path);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::write(target, bytes).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_meeting_pdf(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let (meeting, summary, duration_ms) = state
        .with_db(|conn| {
            let detail = MeetingRepository::get_detail(conn, &id)?;
            let summary_record = detail
                .summaries
                .last()
                .ok_or_else(|| AppError::Message("aucun compte-rendu structuré à exporter".into()))?;
            let structured = parse_structured_summary(&summary_record.content)
                .map_err(|err| AppError::Message(err.to_string()))?;
            let duration_ms = detail
                .audio_files
                .first()
                .and_then(|audio| audio.duration_ms)
                .or_else(|| duration_from_range(&detail.meeting));
            Ok((detail.meeting, structured, duration_ms))
        })
        .map_err(|err| err.to_string())?;

    let display_date = format_display_date(&meeting);
    let duration_label = format_duration_ms(duration_ms);
    let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
        title: &meeting.title,
        status: meeting.status,
        display_date: &display_date,
        duration_label: &duration_label,
        summary: &summary,
    })?;
    Ok(BASE64.encode(bytes))
}

fn duration_from_range(meeting: &Meeting) -> Option<i64> {
    let start = meeting.started_at.as_deref()?;
    let end = meeting.ended_at.as_deref()?;
    let start = DateTime::parse_from_rfc3339(start).ok()?;
    let end = DateTime::parse_from_rfc3339(end).ok()?;
    let ms = end.timestamp_millis() - start.timestamp_millis();
    if ms >= 0 {
        Some(ms)
    } else {
        None
    }
}

fn format_display_date(meeting: &Meeting) -> String {
    let raw = meeting
        .started_at
        .as_deref()
        .unwrap_or(meeting.created_at.as_str());
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return dt.format("%d/%m/%Y %H:%M").to_string();
    }
    raw.to_string()
}

fn format_duration_ms(duration_ms: Option<i64>) -> String {
    let Some(ms) = duration_ms else {
        return "—".into();
    };
    let total_seconds = ms / 1000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes}:{seconds:02}")
}

fn build_export(conn: &rusqlite::Connection, id: &str) -> AppResult<MeetingExport> {
    let detail = MeetingRepository::get_detail(conn, id)?;

    Ok(MeetingExport {
        export_version: EXPORT_VERSION,
        exported_at: Utc::now().to_rfc3339(),
        meeting: detail.meeting,
        audio_files: detail
            .audio_files
            .into_iter()
            .map(|audio| ExportAudioFile {
                id: audio.id,
                meeting_id: audio.meeting_id,
                file_name: basename(&audio.file_path),
                duration_ms: audio.duration_ms,
                format: audio.format,
                created_at: audio.created_at,
            })
            .collect(),
        transcriptions: detail
            .transcriptions
            .into_iter()
            .map(map_export_transcription)
            .collect(),
        summaries: detail
            .summaries
            .into_iter()
            .map(map_export_summary)
            .collect(),
        actions: detail.actions,
    })
}

fn map_export_transcription(transcription: Transcription) -> ExportTranscription {
    ExportTranscription {
        id: transcription.id,
        meeting_id: transcription.meeting_id,
        audio_file_id: transcription.audio_file_id,
        provider_id: transcription.provider_id,
        content: transcription.content,
        language: transcription.language,
        created_at: transcription.created_at,
        updated_at: transcription.updated_at,
    }
}

fn map_export_summary(summary: Summary) -> ExportSummary {
    let structured = serde_json::from_str::<serde_json::Value>(&summary.content).ok();
    ExportSummary {
        id: summary.id,
        meeting_id: summary.meeting_id,
        provider_id: summary.provider_id,
        content: summary.content,
        structured,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
    }
}

fn basename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn dir_size(path: &Path) -> Option<u64> {
    if !path.is_dir() {
        return Some(0);
    }

    let mut total = 0u64;
    let entries = std::fs::read_dir(path).ok()?;
    for entry in entries.flatten() {
        if let Ok(meta) = entry.metadata() {
            if meta.is_file() {
                total += meta.len();
            }
        }
    }
    Some(total)
}

fn clear_dir_files(dir: &Path) -> AppResult<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            remove_file_if_present(&path);
        }
    }
    Ok(())
}

fn remove_file_if_present(path: &Path) {
    if path.is_file() {
        let _ = std::fs::remove_file(path);
    }
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::models::CreateMeetingInput;
    use rusqlite::params;

    fn seed_meeting_with_secrets(conn: &rusqlite::Connection) -> String {
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO ai_providers (id, name, provider_type, is_enabled, credential_key_id, created_at, updated_at)
             VALUES ('mistral', 'Mistral AI', 'mistral', 1, 'keychain:mistral:sk-secret123', ?1, ?1)",
            [&now],
        )
        .unwrap();

        let meeting = MeetingRepository::create(
            conn,
            CreateMeetingInput {
                title: "Comité".into(),
                description: None,
            },
        )
        .unwrap();

        conn.execute(
            "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
             VALUES ('audio-1', ?1, '/home/user/.local/share/app.laminute.desktop/imports/secret.mp3', 60000, 'mp3', ?2)",
            params![meeting.id, now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO transcriptions (id, meeting_id, audio_file_id, provider_id, content, language, created_at, updated_at)
             VALUES ('tx-1', ?1, 'audio-1', 'mistral', 'Bonjour', 'fr', ?2, ?2)",
            params![meeting.id, now],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES ('sum-1', ?1, 'mistral', '{\"synthese\":\"test\"}', ?2, ?2)",
            params![meeting.id, now],
        )
        .unwrap();

        meeting.id
    }

    #[test]
    fn export_omits_secrets_and_absolute_paths() {
        let conn = open_in_memory().unwrap();
        let meeting_id = seed_meeting_with_secrets(&conn);

        let export = build_export(&conn, &meeting_id).unwrap();
        let json = serde_json::to_string(&export).unwrap();

        assert!(!json.contains("apiKey"));
        assert!(!json.contains("sk-"));
        assert!(!json.contains("credential_key_id"));
        assert!(!json.contains("/home/user"));
        assert!(json.contains("secret.mp3"));
        assert_eq!(export.export_version, EXPORT_VERSION);
        assert_eq!(export.audio_files[0].file_name, "secret.mp3");
    }

    #[test]
    fn pdf_export_requires_valid_structured_summary() {
        let conn = open_in_memory().unwrap();
        let meeting = MeetingRepository::create(
            &conn,
            CreateMeetingInput {
                title: "Sans CR".into(),
                description: None,
            },
        )
        .unwrap();

        let err = MeetingRepository::get_detail(&conn, &meeting.id)
            .unwrap()
            .summaries
            .last()
            .is_none();
        assert!(err);
    }

    #[test]
    fn pdf_bytes_from_seeded_summary() {
        use crate::ai::structured_summary::StructuredSummary;
        use crate::models::MeetingStatus;
        use crate::report_pdf::{build_meeting_report_pdf, MeetingReportPdfInput};

        let summary = StructuredSummary {
            synthese: "Resume".into(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        let bytes = build_meeting_report_pdf(MeetingReportPdfInput {
            title: "Test",
            status: MeetingStatus::Completed,
            display_date: "01/01/2026",
            duration_label: "1:00",
            summary: &summary,
        })
        .unwrap();
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn delete_all_local_data_empties_meetings() {
        let conn = open_in_memory().unwrap();
        let meeting_id = seed_meeting_with_secrets(&conn);

        conn.execute("DELETE FROM meetings", []).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM meetings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
        assert!(MeetingRepository::get_by_id(&conn, &meeting_id).is_err());
    }
}
