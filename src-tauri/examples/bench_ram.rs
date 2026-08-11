//! Harnais de budgets RAM (JUL-194).
//!
//! Mesure le RSS du processus natif (`/proc/self/status` VmRSS) sur des scénarios
//! 100 % mockés (SQLite mémoire, jobs IA en mémoire). Aucune clé API, aucun WebView.
//!
//! Usage :
//! ```bash
//! cargo run --manifest-path src-tauri/Cargo.toml --example bench_ram -- --check
//! cargo run --manifest-path src-tauri/Cargo.toml --example bench_ram -- --write-baseline
//! ```

use chrono::Utc;
use laminute_lib::{open_in_memory, MeetingRepository, MeetingSearchFilters};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const DEFAULT_BASELINE: &str = "reports/perf/baseline.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioResult {
    name: String,
    rss_kib: u64,
    notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Baseline {
    version: u32,
    measured_at: String,
    platform: String,
    process: String,
    warm_up_ms: u64,
    regression_margin_pct: f64,
    absolute_ceilings_kib: BTreeMap<String, u64>,
    scenarios: BTreeMap<String, u64>,
}

fn read_rss_kib() -> u64 {
    let status = fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
            return digits.parse().unwrap_or(0);
        }
    }
    0
}

fn warm_up(ms: u64) {
    thread::sleep(Duration::from_millis(ms));
}

fn seed_meetings(conn: &rusqlite::Connection, count: usize) {
    let now = Utc::now().to_rfc3339();
    for i in 0..count {
        let id = Uuid::new_v4().to_string();
        let title = format!("Réunion {i}");
        conn.execute(
            "INSERT INTO meetings (id, title, description, status, created_at, updated_at)
             VALUES (?1, ?2, NULL, 'completed', ?3, ?3)",
            params![id, title, now],
        )
        .unwrap();
    }
}

fn measure_idle() -> ScenarioResult {
    warm_up(200);
    ScenarioResult {
        name: "idle_after_warmup".into(),
        rss_kib: read_rss_kib(),
        notes: "processus après warm-up, avant charge".into(),
    }
}

fn measure_search_pages(meeting_count: usize, page_walks: usize) -> ScenarioResult {
    let conn = open_in_memory().expect("db");
    seed_meetings(&conn, meeting_count);
    let mut cursor = None;
    let mut pages = 0usize;
    loop {
        let page = MeetingRepository::search(
            &conn,
            &MeetingSearchFilters {
                query: None,
                status: None,
                provider_id: None,
                date_from: None,
                date_to: None,
                cursor: cursor.clone(),
            },
        )
        .expect("search");
        pages += 1;
        if page.next_cursor.is_none() || pages >= page_walks {
            break;
        }
        cursor = page.next_cursor;
    }
    ScenarioResult {
        name: "history_search_pages".into(),
        rss_kib: read_rss_kib(),
        notes: format!("{meeting_count} réunions, {pages} pages parcourues"),
    }
}

fn measure_jobs_cycle(job_count: usize) -> ScenarioResult {
    // Simulation bornée : payloads terminaux sans graphe provider / clé API.
    let mut terminal: Vec<String> = Vec::with_capacity(job_count.min(1_000));
    for i in 0..job_count {
        terminal.push(format!("job-{i}-completed"));
        if terminal.len() > 500 {
            terminal.drain(0..250);
        }
    }
    drop(terminal);
    ScenarioResult {
        name: "ai_jobs_completed_cycles".into(),
        rss_kib: read_rss_kib(),
        notes: format!("{job_count} cycles terminés avec eviction bornée"),
    }
}

fn measure_import_buffer(bytes: usize) -> ScenarioResult {
    let buffer = vec![0_u8; bytes];
    let checksum = buffer.iter().map(|&b| b as u64).sum::<u64>();
    assert_eq!(checksum, 0);
    let rss = read_rss_kib();
    drop(buffer);
    warm_up(50);
    ScenarioResult {
        name: "import_near_limit_buffer".into(),
        rss_kib: rss,
        notes: format!("buffer synthétique {bytes} octets (pas d'upload réseau)"),
    }
}

fn measure_recording_soak(cycles: usize) -> ScenarioResult {
    let mut frames: Vec<Vec<i16>> = Vec::new();
    for _ in 0..cycles {
        frames.push(vec![0_i16; 4_800]); // ~100 ms @ 48 kHz mono
        if frames.len() > 20 {
            frames.remove(0);
        }
    }
    let rss = read_rss_kib();
    drop(frames);
    ScenarioResult {
        name: "recording_writer_soak".into(),
        rss_kib: rss,
        notes: format!("{cycles} frames synthétiques avec fenêtre bornée"),
    }
}

fn measure_return_baseline(idle_kib: u64) -> ScenarioResult {
    warm_up(300);
    let rss = read_rss_kib();
    ScenarioResult {
        name: "return_near_baseline".into(),
        rss_kib: rss,
        notes: format!("idle de référence {idle_kib} KiB"),
    }
}

fn default_ci_scale() -> (usize, usize, usize, usize, usize) {
    // meetings, page walks, jobs, import bytes, recording cycles
    if env::var("LAMINUTE_RAM_FULL").ok().as_deref() == Some("1") {
        (2_000, 1_000, 10_000, 8 * 1024 * 1024, 200)
    } else {
        // CI rapide : charges réduites, seuils absolus dans baseline.
        (400, 40, 2_000, 2 * 1024 * 1024, 60)
    }
}

fn build_baseline(results: &[ScenarioResult], warm_up_ms: u64, margin: f64) -> Baseline {
    let mut scenarios = BTreeMap::new();
    let mut ceilings = BTreeMap::new();
    for result in results {
        scenarios.insert(result.name.clone(), result.rss_kib);
        // plafond absolu = mesure * (1 + marge) + 8 MiB de slack runner
        let ceiling = ((result.rss_kib as f64) * (1.0 + margin / 100.0)) as u64 + 8_192;
        ceilings.insert(result.name.clone(), ceiling);
    }
    Baseline {
        version: 1,
        measured_at: Utc::now().to_rfc3339(),
        platform: "linux".into(),
        process: "native-example".into(),
        warm_up_ms,
        regression_margin_pct: margin,
        absolute_ceilings_kib: ceilings,
        scenarios,
    }
}

fn check_against_baseline(results: &[ScenarioResult], baseline: &Baseline) -> Result<(), String> {
    let mut failures = Vec::new();
    for result in results {
        let Some(ceiling) = baseline.absolute_ceilings_kib.get(&result.name) else {
            failures.push(format!("{}: scénario absent de la baseline", result.name));
            continue;
        };
        if result.rss_kib > *ceiling {
            failures.push(format!(
                "{}: RSS {} KiB > plafond {} KiB",
                result.name, result.rss_kib, ceiling
            ));
        }
        if let Some(prev) = baseline.scenarios.get(&result.name) {
            let limit = (*prev as f64) * (1.0 + baseline.regression_margin_pct / 100.0) + 4_096.0;
            if (result.rss_kib as f64) > limit {
                failures.push(format!(
                    "{}: régression RSS {} KiB vs baseline {} KiB (marge {}%)",
                    result.name, result.rss_kib, prev, baseline.regression_margin_pct
                ));
            }
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let write_baseline = args.iter().any(|a| a == "--write-baseline");
    let check = args.iter().any(|a| a == "--check") || !write_baseline;
    let baseline_path = args
        .windows(2)
        .find(|w| w[0] == "--baseline")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_BASELINE));

    let warm_up_ms = 200;
    let margin = 35.0;
    let (meetings, pages, jobs, import_bytes, rec_cycles) = default_ci_scale();

    println!("JUL-194 bench_ram — process natif Linux (pas de WebView)");
    println!("échelle: meetings={meetings} pages={pages} jobs={jobs} import={import_bytes}B rec={rec_cycles}");

    let idle = measure_idle();
    let import = measure_import_buffer(import_bytes);
    let recording = measure_recording_soak(rec_cycles);
    let search = measure_search_pages(meetings, pages);
    let jobs_result = measure_jobs_cycle(jobs);
    let returned = measure_return_baseline(idle.rss_kib);

    let results = vec![idle, import, recording, search, jobs_result, returned];
    for result in &results {
        println!(
            "  {:<28} {:>8} KiB  — {}",
            result.name, result.rss_kib, result.notes
        );
    }

    let out_dir = PathBuf::from("reports/perf");
    fs::create_dir_all(&out_dir).expect("mkdir reports/perf");
    let latest_path = out_dir.join("latest.json");
    let payload = serde_json::json!({
        "measuredAt": Utc::now().to_rfc3339(),
        "scenarios": results,
    });
    fs::write(&latest_path, serde_json::to_string_pretty(&payload).unwrap()).expect("write latest");

    if write_baseline {
        let baseline = build_baseline(&results, warm_up_ms, margin);
        fs::write(
            &baseline_path,
            serde_json::to_string_pretty(&baseline).unwrap(),
        )
        .expect("write baseline");
        println!("baseline écrite → {}", baseline_path.display());
        return;
    }

    if check {
        let raw = fs::read_to_string(&baseline_path).unwrap_or_else(|_| {
            panic!(
                "baseline manquante: {} (lancez --write-baseline)",
                baseline_path.display()
            )
        });
        let baseline: Baseline = serde_json::from_str(&raw).expect("baseline JSON");
        match check_against_baseline(&results, &baseline) {
            Ok(()) => println!("OK — budgets RAM respectés ({})", baseline_path.display()),
            Err(err) => {
                eprintln!("ÉCHEC budgets RAM:\n{err}");
                std::process::exit(1);
            }
        }
    }
}
