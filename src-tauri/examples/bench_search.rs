//! Bench de la recherche historique FTS5 (JUL-172).

use std::time::Instant;
use chrono::Utc;
use laminute_lib::{MeetingRepository, MeetingSearchFilters, open_in_memory};
use rusqlite::params;
use uuid::Uuid;

const MEETING_COUNT: usize = 1_000;
const SEARCH_ITERATIONS: usize = 50;

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() { return 0; }
    let idx = ((sorted.len() as f64 - 1.0) * p / 100.0).round() as usize;
    sorted[idx]
}

fn seed(conn: &rusqlite::Connection) {
    let now = Utc::now().to_rfc3339();
    for i in 0..MEETING_COUNT {
        let id = Uuid::new_v4().to_string();
        let title = format!("Réunion {i} — comité produit");
        let status = if i % 4 == 0 { "completed" } else { "draft" };
        conn.execute("INSERT INTO meetings (id, title, description, status, created_at, updated_at) VALUES (?1, ?2, NULL, ?3, ?4, ?4)", params![id, title, status, now]).unwrap();
        if i % 3 == 0 {
            conn.execute("INSERT INTO transcriptions (id, meeting_id, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)", params![Uuid::new_v4().to_string(), id, format!("Transcription {i} : discussion avec le client Dufour."), now]).unwrap();
        }
    }
}

fn main() {
    let conn = open_in_memory().expect("open db");
    println!("Seeding {MEETING_COUNT} meetings…");
    seed(&conn);
    let filters = MeetingSearchFilters { query: Some("Dufour".into()), status: None, provider_id: None, date_from: None, date_to: None };
    let mut durations = Vec::with_capacity(SEARCH_ITERATIONS);
    for _ in 0..SEARCH_ITERATIONS {
        let start = Instant::now();
        let results = MeetingRepository::search(&conn, &filters).expect("search");
        durations.push(start.elapsed().as_micros());
        assert!(!results.is_empty());
    }
    durations.sort_unstable();
    println!("p50: {} µs", percentile(&durations, 50.0));
    println!("p95: {} µs", percentile(&durations, 95.0));
}
