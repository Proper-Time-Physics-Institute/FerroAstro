// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP18 acceptance on two consecutive speed tables (nightly, local): both are valid CI-008 tables
//! from the same machine; scenario (a) lists REBOUND, Swiftest, Mercury6, Orekit and FerroAstro;
//! and every FerroAstro median of the later table differs from the earlier one by less than the
//! run-to-run spread recorded in the earlier table (pre-registered E6).
//!
//! The tables come from `tools/bench/speed_table.sh` (hours of oracle runs, not part of the test):
//! `FA_BENCH_TABLES=<earlier.json>,<later.json>`, or by default the two most recent
//! `$FA_BENCH_DIR/*/speed-table.json` (the harness's output directory, default `target/bench`)
//! that are **complete**: every E6 scenario (the four WP18 scenarios (a)–(d)) timed for
//! FerroAstro, and every tool listed on (a).
//! Every table found there must parse and validate: an unreadable, unparseable or invalid table
//! fails the test and is named, and a missing output directory fails with the harness command
//! (issue #39). A run directory without `speed-table.json` (a harness run still going, or one
//! that stopped before assembling) is listed and passed over.
//!
//! The PR-tier tests below check that discovery on scratch directories, against the committed
//! `docs/validation/speed-tables/run1-*.json` table.
//!
//! Partial tables (e.g. an `--only` run such as WP21's row (e)) are listed by name and skipped
//! (progress review V3 after WP21). Fewer than two complete tables, or a named table that is
//! partial, is a named precondition failure, reported before E6 is evaluated.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failure is a test failure"
)]

use ferroastro_bench::{SpeedTable, Status, stability};
use std::path::{Path, PathBuf};

const SCENARIO_A: &str = "bench-a-sun-8planets-newton-1000yr";
const TOOLS_ON_A: [&str; 5] = ["rebound", "swiftest", "mercury6", "orekit", "ferroastro"];
/// The harness's output directory variable (`tools/bench/speed_table.sh`).
const BENCH_DIR_ENV: &str = "FA_BENCH_DIR";
const HARNESS_HINT: &str =
    "run tools/bench/speed_table.sh (twice), or set FA_BENCH_TABLES=<earlier.json>,<later.json>";
/// The scenarios E6 was pre-registered on (WP18 rows (a)-(d)).
const E6_SCENARIOS: [&str; 4] = [
    SCENARIO_A,
    "bench-b-sun-mercury-tp-gr1pn-1000yr",
    "bench-c-full-newton-minor343-100yr",
    "oracle-stage-b-mercury-de440-field-100yr",
];

/// What makes a table partial for E6/E7: the missing (scenario, tool) entries, empty if complete.
fn missing(t: &SpeedTable) -> Vec<String> {
    let mut out: Vec<String> = E6_SCENARIOS
        .iter()
        .filter(|s| {
            t.entry(s, "ferroastro")
                .and_then(|e| e.timing.as_ref())
                .is_none()
        })
        .map(|s| format!("{s} / ferroastro not timed"))
        .collect();
    out.extend(
        TOOLS_ON_A
            .iter()
            .filter(|tool| t.entry(SCENARIO_A, tool).is_none())
            .map(|tool| format!("{SCENARIO_A} / {tool} missing")),
    );
    out
}

/// A named precondition failure if `t` is partial.
fn require_complete(t: &SpeedTable, origin: &str) -> Result<(), String> {
    let m = missing(t);
    if m.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "PRECONDITION FAILED (not E6): speed table {} ({origin}) is partial: {}",
            t.run_id,
            m.join("; ")
        ))
    }
}

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

/// The earlier and the later table under test: the two most recent complete tables.
fn tables() -> Result<(SpeedTable, SpeedTable), String> {
    if let Ok(v) = std::env::var("FA_BENCH_TABLES") {
        let (a, b) = v
            .split_once(',')
            .ok_or_else(|| format!("FA_BENCH_TABLES={v:?}: want <earlier.json>,<later.json>"))?;
        let (a, b) = (read(Path::new(a))?, read(Path::new(b))?);
        require_complete(&a, "FA_BENCH_TABLES")?;
        require_complete(&b, "FA_BENCH_TABLES")?;
        return Ok((a, b));
    }
    let base = std::env::var_os(BENCH_DIR_ENV).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/bench"),
        PathBuf::from,
    );
    let found = discover(&base)?;
    let (mut complete, partial): (Vec<_>, Vec<_>) =
        found.into_iter().partition(|t| missing(t).is_empty());
    for t in &partial {
        println!(
            "skipped partial table {} ({} UTC): {}",
            t.run_id,
            t.generated_utc,
            missing(t).join("; ")
        );
    }
    if complete.len() < 2 {
        return Err(format!(
            "PRECONDITION FAILED (not E6): {} complete speed table(s) under {} (need 2 with every \
             E6 scenario {:?} timed for FerroAstro and every tool on (a)); {} partial table(s) \
             skipped: {HARNESS_HINT} (without --only)",
            complete.len(),
            base.display(),
            E6_SCENARIOS,
            partial.len()
        ));
    }
    let later = complete.pop().expect("checked above");
    let earlier = complete.pop().expect("checked above");
    println!(
        "selected the two most recent complete tables: {} and {}",
        earlier.run_id, later.run_id
    );
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

/// WP28 (NUM-024) rows of the speed table.
const WP28_SCENARIOS: [&str; 5] = [
    "bench-f-sun-8planets-eih-100yr",
    "bench-g-sun-8planets-eih-100yr-abm",
    "bench-h-num024-epm16-f64-40yr",
    "ephemeris-mixed-epm16-40yr",
    "bench-j-de440class-mixed-40yr",
];
/// The rows that carry the round-off and de440.bsp columns with values (FerroAstro).
const WP28_ACCURACY: [&str; 3] = [
    "bench-h-num024-epm16-f64-40yr",
    "ephemeris-mixed-epm16-40yr",
    "bench-j-de440class-mixed-40yr",
];
const SUBBOTIN_IDS: [&str; 3] = [
    "SUBBOTIN23-T1-WALL-F64-S",
    "SUBBOTIN23-T1-WALL-MIXED-S",
    "SUBBOTIN23-T1-WALL-DD-S",
];

/// The NUM-024 table: `FA_BENCH_NUM024_TABLE`, or the most recent committed
/// `docs/validation/speed-tables/*wp28*.json`.
fn num024_table() -> SpeedTable {
    if let Ok(p) = std::env::var("FA_BENCH_NUM024_TABLE") {
        return read(Path::new(&p)).unwrap();
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/validation/speed-tables");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|d| d.path())
        .filter(|p| {
            p.extension().is_some_and(|e| e == "json")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().contains("wp28"))
        })
        .collect();
    found.sort();
    let p = found
        .pop()
        .expect("PRECONDITION FAILED: no committed *wp28*.json speed table");
    println!("NUM-024 table: {}", p.display());
    read(&p).unwrap()
}

/// N1 (WP28 plan): the published NUM-024 table is complete: rows (f)-(j) timed for FerroAstro
/// with >= 5 runs, a scenario SHA-256, IC hash and step count; both Subbotin et al. 2023 rows (and
/// the DD row) equal to their vectors; the round-off and de440.bsp columns with values for
/// (h)-(j); the mixed/f64 cost ratio; and the single-core reference score.
///
/// Verifies: NUM-024, CI-008
#[test]
#[ignore = "nightly: reads the WP28 speed table from tools/bench/speed_table.sh --suite wp28 (local, ~1.5 h)"]
fn nightly_num024_speed_table_is_complete() {
    let vs = ferroastro_vectors::VectorSet::load_canonical().unwrap();
    let t = num024_table();
    t.validate().unwrap();
    assert!(t.schema_version >= 2, "schema 2 table expected");
    let mut problems = Vec::new();
    for sc in WP28_SCENARIOS {
        let Some(info) = t.scenarios.iter().find(|s| s.name == sc) else {
            problems.push(format!("{sc}: scenario not listed"));
            continue;
        };
        if info.sha256.len() != 64 {
            problems.push(format!("{sc}: scenario sha256 missing"));
        }
        match t.entry(sc, "ferroastro") {
            Some(e)
                if e.timing
                    .as_ref()
                    .is_some_and(ferroastro_bench::Timing::meets_minimum) =>
            {
                if e.ic_hash.as_deref().is_none_or(str::is_empty) || e.steps.is_none() {
                    problems.push(format!("{sc}: IC hash or step count missing"));
                }
                let t = e.timing.as_ref().expect("checked");
                println!(
                    "{sc}: median {:.3} s, range {:.3} s, {} steps, {:.4} ms/step, IC hash {}",
                    t.median_s,
                    t.range_s(),
                    e.steps.unwrap_or(0),
                    e.ms_per_step().unwrap_or(f64::NAN),
                    e.ic_hash.as_deref().unwrap_or("-")
                );
                if WP28_ACCURACY.contains(&sc) {
                    for (col, m) in [
                        ("round-off", e.accuracy.roundoff_vs_dd_km.as_ref()),
                        ("de440", e.accuracy.mercury_de440_km.as_ref()),
                    ] {
                        match m.and_then(|m| m.value) {
                            Some(v) => println!("  {col}: {v:.6e} km"),
                            None => problems.push(format!("{sc}: no {col} value")),
                        }
                    }
                }
            }
            _ => problems.push(format!("{sc}: FerroAstro not timed with >= 5 runs")),
        }
    }
    for id in SUBBOTIN_IDS {
        let want = vs.get(id).unwrap().value().unwrap();
        match t.published.iter().find(|p| p.vector_id == id) {
            Some(p) if p.wall_s.to_bits() == want.to_bits() => {
                println!("{id}: {} s, {:.4} ms/step", p.wall_s, p.ms_per_step());
            }
            Some(p) => problems.push(format!("{id}: {} s != vector {want} s", p.wall_s)),
            None => problems.push(format!("{id}: published row missing")),
        }
    }
    let ratios = t.cost_ratios();
    for r in &ratios {
        println!("{r}");
    }
    if !ratios.iter().any(|r| {
        r.contains("mixed DD / Subbotin 2023 f64: FerroAstro ") && !r.contains("FerroAstro –")
    }) {
        problems.push("the mixed/f64 cost ratio is missing".into());
    }
    match &t.host.reference_score {
        Some(r) if !r.ratings.is_empty() => println!(
            "single-core reference score: {:?} {} ({})",
            r.ratings, r.unit, r.command
        ),
        _ => problems.push("single-core reference score missing".into()),
    }
    assert!(
        problems.is_empty(),
        "NUM-024 table incomplete: {problems:?}"
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
