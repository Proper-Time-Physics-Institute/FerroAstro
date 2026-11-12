// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP18 acceptance on two consecutive speed tables (nightly, local): both are valid CI-008 tables
//! from the same machine; scenario (a) lists REBOUND, Swiftest, Mercury6, Orekit and FerroAstro;
//! and every FerroAstro median of the later table differs from the earlier one by less than the
//! run-to-run spread recorded in the earlier table (pre-registered E6).
//!
//! The tables come from `tools/bench/speed_table.sh` (hours of oracle runs, not part of the test):
//! `FA_BENCH_TABLES=<earlier.json>,<later.json>`, or by default the two most recent
//! `target/bench/*/speed-table.json` that pass validation.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failure is a test failure"
)]

use ferroastro_bench::{SpeedTable, Status, stability};
use std::path::PathBuf;

const SCENARIO_A: &str = "bench-a-sun-8planets-newton-1000yr";
const TOOLS_ON_A: [&str; 5] = ["rebound", "swiftest", "mercury6", "orekit", "ferroastro"];

fn read(p: &PathBuf) -> SpeedTable {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn tables() -> (SpeedTable, SpeedTable) {
    if let Ok(v) = std::env::var("FA_BENCH_TABLES") {
        let (a, b) = v
            .split_once(',')
            .expect("FA_BENCH_TABLES=<earlier>,<later>");
        return (read(&PathBuf::from(a)), read(&PathBuf::from(b)));
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
    let later = found
        .pop()
        .expect("no valid speed table; run tools/bench/speed_table.sh")
        .1;
    let earlier = found
        .pop()
        .expect("only one valid speed table; run the harness again")
        .1;
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
