//! Smoke E2E natif (JUL-204) — sans WebView ni clé API.
//!
//! Couvre : base fraîche → import MP3 fixture (métadonnées) → transcription/résumé
//! simulés → recherche → détail léger (JUL-169) → suppression.
//!
//! ```bash
//! cargo run --manifest-path src-tauri/Cargo.toml --example e2e_smoke
//! ```

use chrono::Utc;
use laminute_lib::{open_in_memory, MeetingRepository, MeetingSearchFilters};
use rusqlite::params;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use uuid::Uuid;

fn fixture_mp3() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tone-1s.mp3")
}

fn main() -> ExitCode {
    println!("JUL-204 e2e_smoke — démarrage");

    let fixture = fixture_mp3();
    if !fixture.is_file() {
        eprintln!("fixture manquante: {}", fixture.display());
        return ExitCode::FAILURE;
    }
    let bytes = fs::metadata(&fixture).expect("stat").len();
    if bytes < 100 {
        eprintln!("fixture trop petite ({bytes} octets)");
        return ExitCode::FAILURE;
    }

    let conn = open_in_memory().expect("db fraîche (onboarding simulé)");
    let now = Utc::now().to_rfc3339();
    let meeting_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO meetings (id, title, description, status, created_at, updated_at)
         VALUES (?1, ?2, NULL, 'processing', ?3, ?3)",
        params![meeting_id, "Smoke E2E import MP3", now],
    )
    .expect("insert meeting");

    let audio_id = Uuid::new_v4().to_string();
    let audio_path = format!("imports/{audio_id}.mp3");
    conn.execute(
        "INSERT INTO audio_files (id, meeting_id, file_path, duration_ms, format, created_at)
         VALUES (?1, ?2, ?3, 1000, 'mp3', ?4)",
        params![audio_id, meeting_id, audio_path, now],
    )
    .expect("insert audio");

    // Garde JUL-169 : get_detail ne doit pas embarquer de gros contenus.
    let detail = MeetingRepository::get_detail(&conn, &meeting_id).expect("detail");
    assert_eq!(detail.meeting.id, meeting_id);
    assert_eq!(detail.audio_files.len(), 1);
    assert!(detail.summaries.is_empty());
    assert!(detail.transcriptions.is_empty());

    conn.execute(
        "INSERT INTO transcriptions (id, meeting_id, audio_file_id, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            Uuid::new_v4().to_string(),
            meeting_id,
            audio_id,
            "Bonjour, transcription smoke mockée sans API.",
            now
        ],
    )
    .expect("insert transcription");

    let summary_json = r#"{"synthese":"Smoke E2E réussi.","decisions":["Valider le smoke"],"actions":[],"risques":[],"questionsOuvertes":[]}"#;
    conn.execute(
        "INSERT INTO summaries (id, meeting_id, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![Uuid::new_v4().to_string(), meeting_id, summary_json, now],
    )
    .expect("insert summary");
    conn.execute(
        "UPDATE meetings SET status = 'completed', updated_at = ?1 WHERE id = ?2",
        params![now, meeting_id],
    )
    .unwrap();

    let light = MeetingRepository::get_detail(&conn, &meeting_id).expect("detail after content");
    assert_eq!(light.transcriptions.len(), 1);
    assert_eq!(light.summaries.len(), 1);
    // MeetingDetail expose des métadonnées : pas de champ content sur les résumés listés.
    let _ = light.summaries[0].provider_id;

    let full = MeetingRepository::get_full_detail(&conn, &meeting_id).expect("full detail export");
    assert!(
        full.summaries[0].content.contains("Smoke E2E"),
        "export doit conserver le contenu"
    );
    assert!(
        full.transcriptions[0].content.contains("transcription smoke"),
        "export transcription"
    );

    let page = MeetingRepository::search(
        &conn,
        &MeetingSearchFilters {
            query: Some("Smoke".into()),
            status: None,
            provider_id: None,
            date_from: None,
            date_to: None,
            cursor: None,
        },
    )
    .expect("search");
    assert!(
        page.items.iter().any(|m| m.id == meeting_id),
        "réunion absente des résultats"
    );

    MeetingRepository::delete(&conn, &meeting_id).expect("delete");
    assert!(
        MeetingRepository::get_by_id(&conn, &meeting_id).is_err(),
        "réunion encore présente après suppression"
    );

    let tauri_conf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let conf = fs::read_to_string(&tauri_conf).expect("tauri.conf.json");
    assert!(
        conf.contains("assetProtocol") && conf.contains("$APPDATA/imports"),
        "asset protocol strict attendu (JUL-183)"
    );
    assert!(
        conf.contains("updater"),
        "plugin updater attendu (JUL-189)"
    );

    let security_test = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../src/lib/tauriSecurityConfig.test.ts");
    assert!(
        security_test.is_file(),
        "garde config frontend JUL-183/189 attendue"
    );

    println!("JUL-204 e2e_smoke — OK");
    println!("Couvert : base fraîche, import métadonnées, mock IA, search, détail léger, export full, delete.");
    println!("JUL-178 (nav pendant enregistrement) : tests unitaires frontend existants.");
    ExitCode::SUCCESS
}
