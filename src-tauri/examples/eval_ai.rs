//! Banc d'évaluation offline/live transcription + compte-rendu (JUL-199).
//!
//! Usage (depuis `src-tauri/`) :
//!   cargo run --example eval_ai -- --mode offline --corpus-dir ../eval --out-dir ../reports/eval

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use laminute_lib::eval::{report_to_markdown, run_offline_eval};

fn print_usage() {
    eprintln!(
        "Usage: eval_ai --mode <offline|live> --corpus-dir <dir> --out-dir <dir> [--write-baseline]"
    );
}

fn parse_args() -> Result<(String, PathBuf, PathBuf, bool), String> {
    let args: Vec<String> = env::args().collect();
    let mut mode = None;
    let mut corpus_dir = None;
    let mut out_dir = None;
    let mut write_baseline = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                i += 1;
                mode = Some(args.get(i).ok_or("--mode requiert une valeur")?.clone());
            }
            "--corpus-dir" => {
                i += 1;
                corpus_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--corpus-dir requiert une valeur")?,
                ));
            }
            "--out-dir" => {
                i += 1;
                out_dir = Some(PathBuf::from(
                    args.get(i).ok_or("--out-dir requiert une valeur")?,
                ));
            }
            "--write-baseline" => {
                write_baseline = true;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            flag => return Err(format!("argument inconnu : {flag}")),
        }
        i += 1;
    }

    let mode = mode.ok_or("--mode est obligatoire (offline|live)")?;
    let corpus_dir = corpus_dir.ok_or("--corpus-dir est obligatoire")?;
    let out_dir = out_dir.ok_or("--out-dir est obligatoire")?;
    Ok((mode, corpus_dir, out_dir, write_baseline))
}

fn main() -> ExitCode {
    if let Err(message) = run() {
        eprintln!("eval_ai: {message}");
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run() -> Result<(), String> {
    let (mode, corpus_dir, out_dir, write_baseline) = parse_args()?;

    let live_requested =
        mode == "live" || env::var("LAMINUTE_EVAL_LIVE").is_ok_and(|v| v == "1" || v == "true");

    if live_requested && mode != "offline" {
        return Err(
            "mode live non implémenté pour le MVP — utilisez --mode offline (CI et baseline)"
                .into(),
        );
    }

    if mode != "offline" {
        return Err(format!("mode inconnu : {mode} (attendu : offline)"));
    }

    let report = run_offline_eval(&corpus_dir)?;

    fs::create_dir_all(&out_dir)
        .map_err(|e| format!("création {} : {e}", out_dir.display()))?;

    let json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("sérialisation rapport : {e}"))?;
    let md = report_to_markdown(&report);

    let latest_json = out_dir.join("latest.json");
    let latest_md = out_dir.join("latest.md");
    fs::write(&latest_json, &json)
        .map_err(|e| format!("écriture {} : {e}", latest_json.display()))?;
    fs::write(&latest_md, &md)
        .map_err(|e| format!("écriture {} : {e}", latest_md.display()))?;

    if write_baseline {
        let baseline_json = out_dir.join("baseline.json");
        let baseline_md = out_dir.join("baseline.md");
        fs::write(&baseline_json, &json)
            .map_err(|e| format!("écriture {} : {e}", baseline_json.display()))?;
        fs::write(&baseline_md, &md)
            .map_err(|e| format!("écriture {} : {e}", baseline_md.display()))?;
    }

    println!("{}", md);

    if report.aggregate.thresholds_met {
        println!("\n✅ Seuils atteints — PASS");
        Ok(())
    } else {
        eprintln!("\n❌ Seuils non atteints — FAIL");
        for t in &report.aggregate.failed_thresholds {
            eprintln!("  - {t}");
        }
        Err("seuils non atteints".into())
    }
}
