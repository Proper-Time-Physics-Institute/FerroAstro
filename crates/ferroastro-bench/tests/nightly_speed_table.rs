// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP18 acceptance on two consecutive speed tables (nightly, local): both are valid CI-008 tables
//! from the same machine; scenario (a) lists REBOUND, Swiftest, Mercury6, Orekit and FerroAstro;
//! and every FerroAstro median of the later table differs from the earlier one by less than the
//! run-to-run spread recorded in the earlier table (pre-registered E6).
//!
//! The tables come from `tools/bench/speed_table.sh` (hours of oracle runs, not part of the test):
//! `FA_BENCH_TABLES=<earlier.json>,<later.json>`, or by default the two most recent
//! `target/bench/*/speed-table.json` that pass validation **and are complete**: every E6 scenario
//! (the four WP18 scenarios (a)–(d)) timed for FerroAstro, and every tool listed on (a). Partial
//! tables (e.g. an `--only` run such as WP21's row (e)) are listed by name and skipped (progress
//! review V3 after WP21). Fewer than two complete tables, or a named table that is partial, is a
//! named precondition failure, reported before E6 is evaluated.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failure is a test failure"
)]

use ferroastro_bench::{SpeedTable, Status, stability};
use std::path::PathBuf;

const SCENARIO_A: &str = "bench-a-sun-8planets-newton-1000yr";
const TOOLS_ON_A: [&str; 5] = ["rebound", "swiftest", "mercury6", "orekit", "ferroastro"];
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

/// Panic with a named precondition failure if `t` is partial.
fn require_complete(t: &SpeedTable, origin: &str) {
    let m = missing(t);
    assert!(
        m.is_empty(),
        "PRECONDITION FAILED (not E6): speed table {} ({origin}) is partial: {}",
        t.run_id,
        m.join("; ")
    );
}

fn read(p: &PathBuf) -> SpeedTable {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn tables() -> (SpeedTable, SpeedTable) {
    if let Ok(v) = std::env::var("FA_BENCH_TABLES") {
        let (a, b) = v
            .split_once(',')
            .expect("FA_BENCH_TABLES=<earlier>,<later>");
        let (a, b) = (read(&PathBuf::from(a)), read(&PathBuf::from(b)));
        require_complete(&a, "FA_BENCH_TABLES");
        require_complete(&b, "FA_BENCH_TABLES");
        return (a, b);
    }
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/bench");
    let mut found: Vec<(String, SpeedTable)> = std::fs::read_dir(&base)
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|d| {
            let p = d.path().join("speed-table.json");
            let t: SpeedTable = serde_json::from_str(&std::fs::read_to_string(&p).ok()?).ok()?;
            t.validate().ok()?;
            Some((t.generated_utc.clone(), t))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let (complete, partial): (Vec<_>, Vec<_>) =
        found.into_iter().partition(|(_, t)| missing(t).is_empty());
    for (_, t) in &partial {
        println!(
            "skipped partial table {} ({} UTC): {}",
            t.run_id,
            t.generated_utc,
            missing(t).join("; ")
        );
    }
    let mut complete: Vec<SpeedTable> = complete.into_iter().map(|x| x.1).collect();
    assert!(
        complete.len() >= 2,
        "PRECONDITION FAILED (not E6): {} complete speed table(s) under {} (need 2 with every E6 \
         scenario {:?} timed for FerroAstro and every tool on (a)); {} partial table(s) skipped; \
         run tools/bench/speed_table.sh without --only",
        complete.len(),
        base.display(),
        E6_SCENARIOS,
        partial.len()
    );
    let later = complete.pop().expect("checked above");
    let earlier = complete.pop().expect("checked above");
    println!(
        "selected the two most recent complete tables: {} and {}",
        earlier.run_id, later.run_id
    );
    (earlier, later)
}

/// Verifies: CI-008, DEP-014, NUM-009
#[test]
#[ignore = "nightly: needs two speed tables from tools/bench/speed_table.sh (local, hours); cargo nextest run -p ferroastro-bench --run-ignored only"]
fn nightly_two_consecutive_speed_tables_are_stable_and_complete() {
    let (prev, next) = tables();
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
        return read(&PathBuf::from(p));
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
    read(&p)
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
