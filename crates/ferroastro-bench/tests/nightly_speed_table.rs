// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP18 acceptance on two consecutive speed tables (nightly, local): both are valid CI-008 tables
//! from the same machine; scenario (a) lists REBOUND, Swiftest, Mercury6, Orekit and FerroAstro;
//! and every FerroAstro median of the later table differs from the earlier one by less than the
//! run-to-run spread recorded in the earlier table (pre-registered E6).
//!
//! The tables come from `tools/bench/speed_table.sh` (hours of oracle runs, not part of the test):
//! `FA_BENCH_TABLES=<earlier.json>,<later.json>`, or by default the two most recent
//! `$FA_BENCH_DIR/*/speed-table.json` (the harness's output directory, default `target/bench`).
//! Every table found there must parse and validate: an unreadable, unparseable or invalid table
//! fails the test and is named, and a missing output directory fails with the harness command
//! (issue #39). A run directory without `speed-table.json` (a harness run still going, or one
//! that stopped before assembling) is listed and passed over.
//!
//! The PR-tier tests below check that discovery on scratch directories, against the committed
//! `docs/validation/speed-tables/run1-*.json` table.

#![expect(clippy::unwrap_used, reason = "test: a failure is a test failure")]

use ferroastro_bench::{SpeedTable, Status, stability};
use std::path::{Path, PathBuf};

const SCENARIO_A: &str = "bench-a-sun-8planets-newton-1000yr";
const TOOLS_ON_A: [&str; 5] = ["rebound", "swiftest", "mercury6", "orekit", "ferroastro"];
/// The harness's output directory variable (`tools/bench/speed_table.sh`).
const BENCH_DIR_ENV: &str = "FA_BENCH_DIR";
const HARNESS_HINT: &str =
    "run tools/bench/speed_table.sh (twice), or set FA_BENCH_TABLES=<earlier.json>,<later.json>";

/// Read, parse and validate one table; the error names the file.
fn read(p: &Path) -> Result<SpeedTable, String> {
    let text =
        std::fs::read_to_string(p).map_err(|e| format!("{}: cannot read: {e}", p.display()))?;
    let t: SpeedTable = serde_json::from_str(&text)
        .map_err(|e| format!("{}: not a speed table: {e}", p.display()))?;
    t.validate()
        .map_err(|e| format!("{}: invalid speed table: {e}", p.display()))?;
    Ok(t)
}

/// Every `<base>/<run>/speed-table.json`, sorted by `generated_utc` (oldest first). Fails if
/// `base` cannot be listed or if any table found is unreadable, unparseable or invalid.
fn discover(base: &Path) -> Result<Vec<SpeedTable>, String> {
    let entries = std::fs::read_dir(base).map_err(|e| {
        format!(
            "no speed-table directory {} ({e}): {HARNESS_HINT}",
            base.display()
        )
    })?;
    let mut found = Vec::new();
    let mut invalid = Vec::new();
    for entry in entries {
        let dir = entry
            .map_err(|e| format!("cannot list {}: {e}", base.display()))?
            .path();
        if !dir.is_dir() {
            continue; // harness logs next to the run directories
        }
        let p = dir.join("speed-table.json");
        if !p.exists() {
            println!(
                "passing over {}: no speed-table.json (run unfinished or stopped before assembly)",
                dir.display()
            );
            continue;
        }
        match read(&p) {
            Ok(t) => found.push(t),
            Err(e) => invalid.push(e),
        }
    }
    if !invalid.is_empty() {
        invalid.sort();
        return Err(format!(
            "{} invalid speed table(s) under {} (fix, move or delete them, or set FA_BENCH_TABLES):\n  {}",
            invalid.len(),
            base.display(),
            invalid.join("\n  ")
        ));
    }
    found.sort_by(|a, b| a.generated_utc.cmp(&b.generated_utc));
    Ok(found)
}

/// The earlier and the later table under test.
fn tables() -> Result<(SpeedTable, SpeedTable), String> {
    if let Ok(v) = std::env::var("FA_BENCH_TABLES") {
        let (a, b) = v
            .split_once(',')
            .ok_or_else(|| format!("FA_BENCH_TABLES={v:?}: want <earlier.json>,<later.json>"))?;
        return Ok((read(Path::new(a))?, read(Path::new(b))?));
    }
    let base = std::env::var_os(BENCH_DIR_ENV).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/bench"),
        PathBuf::from,
    );
    let mut found = discover(&base)?;
    let later = found
        .pop()
        .ok_or_else(|| format!("no speed table under {}: {HARNESS_HINT}", base.display()))?;
    let earlier = found.pop().ok_or_else(|| {
        format!(
            "only one speed table under {} ({}): run tools/bench/speed_table.sh again",
            base.display(),
            later.run_id
        )
    })?;
    Ok((earlier, later))
}

/// Verifies: CI-008, DEP-014, NUM-009
#[test]
#[ignore = "nightly: needs two speed tables from tools/bench/speed_table.sh (local, hours); cargo nextest run -p ferroastro-bench --run-ignored only"]
fn nightly_two_consecutive_speed_tables_are_stable_and_complete() {
    let (prev, next) = tables().unwrap_or_else(|e| panic!("{e}"));
    for t in [&prev, &next] {
        t.validate().unwrap();
        println!(
            "table {}: {} UTC, {} ({} logical CPUs), flags {:?}, AVX-512 {}, runner class {}, commit {}, load {:.1} -> {:.1}",
            t.run_id,
            t.generated_utc,
            t.host.cpu_model,
            t.host.logical_cpus,
            t.host.cpu_flags,
            t.host.avx512,
            t.host.runner_class,
            t.host.git_commit,
            t.load_avg_run[0],
            t.load_avg_run[1]
        );
        for tool in TOOLS_ON_A {
            let e = t
                .entry(SCENARIO_A, tool)
                .unwrap_or_else(|| panic!("{}: {tool} missing on (a)", t.run_id));
            println!(
                "  (a) {tool:<10} {:?} median {} s {}",
                e.status,
                e.timing
                    .as_ref()
                    .map_or_else(|| "-".to_owned(), |x| format!("{:.3}", x.median_s)),
                e.message.lines().next().unwrap_or("")
            );
            assert_ne!(
                e.status,
                Status::Failed,
                "{}: {tool} failed on (a)",
                t.run_id
            );
        }
    }
    let rows = stability(&prev, &next, "ferroastro").unwrap();
    let mut failed = Vec::new();
    for r in &rows {
        println!(
            "FerroAstro {}: median {:.3} -> {:.3} s, |delta| {:.3} s, range {:.3} / {:.3} s: {} (reverse {})",
            r.scenario,
            r.median_prev_s,
            r.median_next_s,
            r.delta_s(),
            r.range_prev_s,
            r.range_next_s,
            if r.pass { "pass" } else { "FAIL" },
            if r.pass_reverse { "pass" } else { "fail" }
        );
        if !r.pass {
            failed.push(r.scenario.clone());
        }
    }
    assert!(
        failed.is_empty(),
        "medians moved by more than the recorded spread: {failed:?}"
    );
}

// PR tier: table discovery on scratch directories (issue #39).

/// The committed run-1 table (WP18), a valid CI-008 table.
fn committed_table_json() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/validation/speed-tables/run1-20261020T031916Z.json");
    std::fs::read_to_string(&p).unwrap()
}

/// A fresh scratch directory (the caller removes it).
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "fa-bench-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `<base>/<run>/speed-table.json` with `text`.
fn put(base: &Path, run: &str, text: &str) {
    std::fs::create_dir_all(base.join(run)).unwrap();
    std::fs::write(base.join(run).join("speed-table.json"), text).unwrap();
}

/// Verifies: CI-008
#[test]
fn speed_table_discovery_fails_clearly_on_a_missing_directory() {
    let parent = scratch("missing");
    let base = parent.join("no-such-bench-dir");
    let err = discover(&base).unwrap_err();
    assert!(
        err.contains("no-such-bench-dir") && err.contains("tools/bench/speed_table.sh"),
        "{err}"
    );
    std::fs::remove_dir_all(&parent).unwrap();
}

/// Verifies: CI-008
#[test]
fn speed_table_discovery_fails_on_an_unparseable_or_invalid_table() {
    let valid = committed_table_json();
    assert!(
        read_str(&valid).is_ok(),
        "the committed run-1 table must be valid"
    );
    // Valid JSON of the right shape, but out of schema: validate() refuses it.
    let mut v: serde_json::Value = serde_json::from_str(&valid).unwrap();
    v["schema_version"] = serde_json::json!(999);
    let wrong_schema = serde_json::to_string(&v).unwrap();
    let half = (0..=valid.len() / 2)
        .rev()
        .find(|&i| valid.is_char_boundary(i))
        .unwrap();
    for (tag, bad) in [
        ("truncated", &valid[..half]),
        ("schema", wrong_schema.as_str()),
    ] {
        let base = scratch(tag);
        put(&base, "run-a", &valid);
        put(&base, "run-b", &valid);
        put(&base, "run-c-bad", bad);
        let err = discover(&base).unwrap_err();
        assert!(
            err.contains("run-c-bad") && err.starts_with("1 invalid speed table"),
            "{tag}: {err}"
        );
        std::fs::remove_dir_all(&base).unwrap();
    }
}

/// Verifies: CI-008
#[test]
fn speed_table_discovery_reads_valid_tables_and_passes_over_unfinished_runs() {
    let valid = committed_table_json();
    let base = scratch("valid");
    put(&base, "run-a", &valid);
    std::fs::create_dir_all(base.join("run-unfinished")).unwrap();
    std::fs::write(base.join("run.log"), "harness log, not a run directory\n").unwrap();
    let found = discover(&base).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].run_id, "run1-20261020T031916Z");
    std::fs::remove_dir_all(&base).unwrap();
}

/// Parse and validate a table held in memory.
fn read_str(text: &str) -> Result<SpeedTable, String> {
    let t: SpeedTable = serde_json::from_str(text).map_err(|e| e.to_string())?;
    t.validate().map_err(|e| e.to_string())?;
    Ok(t)
}
