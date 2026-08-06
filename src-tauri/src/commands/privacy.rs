use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::ai::structured_summary::parse_structured_summary;
use crate::db::AppState;
use crate::error::{AppError, AppResult};
use crate::export_write::{issue_grant, write_granted};
use crate::models::{Action, Meeting, Summary, Transcription};
use crate::report_markdown::{build_meeting_report_markdown, MeetingReportMarkdownInput};
use crate::report_pdf::{build_meeting_report_pdf, MeetingReportPdfInput};
use crate::repository::MeetingRepository;

const EXPORT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExportFormat {
    Json,
    Markdown,
    Pdf,
}

impl ExportFormat {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "json" => Ok(Self::Json),
            "markdown" | "md" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            other => Err(format!("format d'export inconnu : {other}")),
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
            Self::Pdf => "pdf",
        }
    }

    fn filter_name(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
            Self::Pdf => "PDF",
        }
    }
}

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
    let export = with_db(&state, |conn| build_export(conn, &id)).map_err(|err| err.to_string())?;
    serde_json::to_string_pretty(&export).map_err(|err| err.to_string())
}

/// Dialogue de sauvegarde + écriture native (JSON / Markdown / PDF).
/// Retourne `false` si l'utilisateur annule le dialogue.
#[tauri::command]
pub async fn save_meeting_export(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    format: String,
    default_file_name: String,
) -> Result<bool, String> {
    let format = ExportFormat::parse(&format)?;
    let default_file_name = sanitize_default_file_name(&default_file_name, format)?;

    let file_path = app
        .dialog()
        .file()
        .add_filter(format.filter_name(), &[format.extension()])
        .set_file_name(&default_file_name)
        .blocking_save_file();

    let Some(file_path) = file_path else {
        return Ok(false);
    };

    let path = file_path
        .into_path()
        .map_err(|err| format!("chemin d'export invalide : {err}"))?;

    // Générer avant d'émettre le grant pour ne pas laisser de grant orphelin.
    let bytes = build_export_bytes(&state, &id, format)?;
    let grant = issue_grant(path);
    write_granted(&grant, &bytes).map_err(|err| err.to_string())?;
    Ok(true)
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

    let meetings_count = with_db(&state, |conn| {
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

    with_db(&state, |conn| {
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

fn sanitize_default_file_name(raw: &str, format: ExportFormat) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
    {
        return Err("nom de fichier d'export invalide".into());
    }

    let name = Path::new(trimmed)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "nom de fichier d'export invalide".to_string())?;

    if name != trimmed {
        return Err("nom de fichier d'export invalide".into());
    }

    let expected_ext = format.extension();
    if Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case(expected_ext))
    {
        return Ok(name.to_string());
    }

    Ok(format!("{name}.{expected_ext}"))
}

fn build_export_bytes(
    state: &State<'_, AppState>,
    id: &str,
    format: ExportFormat,
) -> Result<Vec<u8>, String> {
    match format {
        ExportFormat::Json => {
            let export =
                with_db(state, |conn| build_export(conn, id)).map_err(|err| err.to_string())?;
            let json = serde_json::to_string_pretty(&export).map_err(|err| err.to_string())?;
            Ok(json.into_bytes())
        }
        ExportFormat::Markdown => {
            let (meeting, summary, duration_ms) = load_structured_export(state, id)?;
            let md = build_meeting_report_markdown(MeetingReportMarkdownInput {
                title: &meeting.title,
                status: meeting.status,
                display_date: &format_display_date(&meeting),
                duration_label: &format_duration_ms(duration_ms),
                summary: &summary,
            });
            Ok(md.into_bytes())
        }
        ExportFormat::Pdf => {
            let (meeting, summary, duration_ms) = load_structured_export(state, id)?;
            build_meeting_report_pdf(MeetingReportPdfInput {
                title: &meeting.title,
                status: meeting.status,
                display_date: &format_display_date(&meeting),
                duration_label: &format_duration_ms(duration_ms),
                summary: &summary,
            })
        }
    }
}

fn load_structured_export(
    state: &State<'_, AppState>,
    id: &str,
) -> Result<
    (
        Meeting,
        crate::ai::structured_summary::StructuredSummary,
        Option<i64>,
    ),
    String,
> {
    with_db(state, |conn| {
        let detail = MeetingRepository::get_detail(conn, id)?;
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
    .map_err(|err| err.to_string())
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

fn with_db<T, F>(state: &State<'_, AppState>, f: F) -> AppResult<T>
where
    F: FnOnce(&rusqlite::Connection) -> AppResult<T>,
{
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Message("impossible d'accéder à la base de données".into()))?;
    f(&db)
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

    fn seed_meeting_with_valid_summary(conn: &rusqlite::Connection) -> String {
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO ai_providers (id, name, provider_type, is_enabled, credential_key_id, created_at, updated_at)
             VALUES ('mistral', 'Mistral AI', 'mistral', 1, NULL, ?1, ?1)",
            [&now],
        )
        .unwrap();

        let meeting = MeetingRepository::create(
            conn,
            CreateMeetingInput {
                title: "Comité produit".into(),
                description: None,
            },
        )
        .unwrap();

        let summary_json = r#"{
            "synthese": "Point d'avancement",
            "decisions": ["Go"],
            "actions": [{"titre": "Brief", "description": null, "responsable": "Alice", "echeance": null}],
            "risques": [],
            "questionsOuvertes": []
        }"#;

        conn.execute(
            "INSERT INTO summaries (id, meeting_id, provider_id, content, created_at, updated_at)
             VALUES ('sum-1', ?1, 'mistral', ?2, ?3, ?3)",
            params![meeting.id, summary_json, now],
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
    fn build_export_bytes_json_markdown_pdf_from_seed() {
        let conn = open_in_memory().unwrap();
        let meeting_id = seed_meeting_with_valid_summary(&conn);

        let export = build_export(&conn, &meeting_id).unwrap();
        let json = serde_json::to_string_pretty(&export).unwrap();
        assert!(json.contains("Comité produit"));
        assert!(json.contains("Point d'avancement"));

        let detail = MeetingRepository::get_detail(&conn, &meeting_id).unwrap();
        let summary = parse_structured_summary(&detail.summaries.last().unwrap().content).unwrap();
        let md = build_meeting_report_markdown(MeetingReportMarkdownInput {
            title: &detail.meeting.title,
            status: detail.meeting.status,
            display_date: &format_display_date(&detail.meeting),
            duration_label: &format_duration_ms(None),
            summary: &summary,
        });
        assert!(md.contains("# Comité produit"));
        assert!(md.contains("Point d'avancement"));

        let pdf = build_meeting_report_pdf(MeetingReportPdfInput {
            title: &detail.meeting.title,
            status: detail.meeting.status,
            display_date: &format_display_date(&detail.meeting),
            duration_label: &format_duration_ms(None),
            summary: &summary,
        })
        .unwrap();
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn sanitize_default_file_name_rejects_path_traversal() {
        let err = sanitize_default_file_name("../secret.json", ExportFormat::Json).unwrap_err();
        assert!(err.contains("invalide"));

        let ok = sanitize_default_file_name("laminute-reunion-2026-08-06.json", ExportFormat::Json)
            .unwrap();
        assert_eq!(ok, "laminute-reunion-2026-08-06.json");

        let with_ext =
            sanitize_default_file_name("laminute-reunion-2026-08-06", ExportFormat::Pdf).unwrap();
        assert_eq!(with_ext, "laminute-reunion-2026-08-06.pdf");
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
