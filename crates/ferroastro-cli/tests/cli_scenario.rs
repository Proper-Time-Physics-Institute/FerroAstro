// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `ferroastro scenario run|resume` in fresh processes (WP13 checks 2a, 2b, 3 and 4).
//!
//! * 2a (PR tier, cached header.440, CI-005): Sun + 8 planets (EMB), Newtonian + solar J2, from
//!   the header.440 ICs: an uninterrupted run against a run stopped at half the span, written to
//!   a checkpoint, and resumed by a **new `ferroastro` process**: every sampled state, every
//!   perihelion passage, the final state and the step counters are bitwise identical. f64 over
//!   100 yr (stop at 50 yr); double-double over 20 yr (stop at 10 yr) to keep the PR tier short.
//! * 2b (nightly, release): the same for the committed 1000-yr scenarios, stopped at 500 yr, in
//!   f64 and double-double (the WP13 brief).
//! * 3 (TST-005): the uninterrupted run made twice in two processes is bitwise identical.
//! * 4 (REF-011): a scenario with `gr_eih` exits with status 2 and a message naming the switch.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_propagate::{Checkpoint, RunOutput};
use std::path::{Path, PathBuf};
use std::process::Command;

fn exe() -> &'static str {
    env!("CARGO_BIN_EXE_ferroastro")
}

fn scenarios() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ci/scenarios")
}

/// The pinned header.440 from the cache (re-verified against its SHA-256), with provenance.
fn header() -> PathBuf {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    println!("de440-header: {}", f.provenance());
    f.path
}

fn workdir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-scenario-{name}"));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The committed scenario `file` with `years` replacing its span, written to `dir`.
fn scenario_with_span(dir: &Path, file: &str, years: &str) -> PathBuf {
    let text = std::fs::read_to_string(scenarios().join(file)).unwrap();
    let text = text.replace("years = 1000.0", &format!("years = {years}"));
    let p = dir.join(file);
    std::fs::write(&p, text).unwrap();
    p
}

fn ferroastro(args: &[&str]) -> String {
    let started = std::time::Instant::now();
    let out = Command::new(exe()).args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    println!(
        "$ ferroastro {} ({:.1} s)\n{stdout}{}",
        args.join(" "),
        started.elapsed().as_secs_f64(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "ferroastro failed: {:?}", out.status);
    stdout
}

fn read_output(p: &Path) -> RunOutput {
    RunOutput::decode(&std::fs::read(p).unwrap()).unwrap()
}

/// Uninterrupted vs stopped-and-resumed (each in its own process), and the uninterrupted run
/// twice; returns nothing, asserts bitwise identity.
fn interrupted_equals_uninterrupted(tag: &str, scenario: &Path, stop_years: &str) {
    let dir = workdir(tag);
    let h = header();
    let (s, h) = (scenario.to_str().unwrap(), h.to_str().unwrap());
    let full = dir.join("full.bin");
    let again = dir.join("again.bin");
    let first = dir.join("first.bin");
    let rest = dir.join("rest.bin");
    let ck = dir.join("checkpoint.bin");
    for p in [&full, &again, &first, &rest, &ck] {
        let _ = std::fs::remove_file(p);
    }
    ferroastro(&[
        "scenario",
        "run",
        s,
        "--header",
        h,
        "--output",
        full.to_str().unwrap(),
    ]);
    ferroastro(&[
        "scenario",
        "run",
        s,
        "--header",
        h,
        "--output",
        again.to_str().unwrap(),
    ]);
    ferroastro(&[
        "scenario",
        "run",
        s,
        "--header",
        h,
        "--output",
        first.to_str().unwrap(),
        "--stop-years",
        stop_years,
        "--checkpoint",
        ck.to_str().unwrap(),
    ]);
    let ck_bytes = std::fs::read(&ck).unwrap();
    let decoded = Checkpoint::decode(&ck_bytes).unwrap();
    ferroastro(&[
        "scenario",
        "resume",
        s,
        "--header",
        h,
        "--checkpoint",
        ck.to_str().unwrap(),
        "--output",
        rest.to_str().unwrap(),
    ]);
    let (full, again) = (read_output(&full), read_output(&again));
    let (first, rest) = (read_output(&first), read_output(&rest));
    let stop_t = first.final_state.as_ref().unwrap().t[0];
    let mut joined = first.clone();
    joined.extend_with(rest).unwrap();
    let resumed_words = full.differing_words(&joined);
    let twice_words = full.differing_words(&again);
    println!(
        "{tag}: IC hash {}; checkpoint {} bytes at t = {stop_t:.6e} s ({} steps; {} states and {} \
         passages before it); full run {} states, {} passages, {} steps; differing 64-bit words: \
         resumed {resumed_words:?}, second run {twice_words:?}; counters equal: {} / {}",
        full.ic_hash,
        ck_bytes.len(),
        decoded.integrator.stats.steps,
        first.states.len(),
        first.passages.len(),
        full.states.len(),
        full.passages.len(),
        full.stats.steps,
        full.stats == joined.stats,
        full.stats == again.stats
    );
    assert!(!first.finished && joined.finished && full.finished);
    assert!(!first.states.is_empty() && !first.passages.is_empty());
    assert_eq!(
        resumed_words,
        Some(0),
        "resumed run differs from the uninterrupted run"
    );
    assert_eq!(full.stats, joined.stats);
    assert_eq!(twice_words, Some(0), "TST-005: two identical runs differ");
    assert_eq!(full.stats, again.stats);
    assert_eq!(full.ic_hash, joined.ic_hash);
}

/// 2a, f64: 100 yr, stopped at 50 yr and resumed in a fresh process.
///
/// Verifies: CI-010, TST-005
#[test]
fn checkpoint_resume_in_a_fresh_process_is_bitwise_f64() {
    let dir = workdir("f64-100yr");
    let s = scenario_with_span(&dir, "sun-8planets-newton-j2-1000yr.toml", "100.0");
    interrupted_equals_uninterrupted("f64-100yr", &s, "50");
}

/// 2a, double-double: 20 yr, stopped at 10 yr and resumed in a fresh process.
///
/// Verifies: CI-010, TST-005, DEP-007
#[test]
fn checkpoint_resume_in_a_fresh_process_is_bitwise_double_double() {
    let dir = workdir("dd-20yr");
    let s = scenario_with_span(&dir, "sun-8planets-newton-j2-1000yr-dd.toml", "20.0");
    interrupted_equals_uninterrupted("dd-20yr", &s, "10");
}

/// 2b, f64: the committed 1000-yr scenario, stopped at 500 yr.
///
/// Verifies: CI-010, TST-005
#[test]
#[ignore = "nightly: 1000-yr N-body runs; run with --profile nightly --cargo-profile release"]
fn nightly_checkpoint_resume_1000yr_f64() {
    let s = scenarios().join("sun-8planets-newton-j2-1000yr.toml");
    interrupted_equals_uninterrupted("f64-1000yr", &s, "500");
}

/// 2b, double-double: the committed 1000-yr scenario, stopped at 500 yr.
///
/// Verifies: CI-010, TST-005, DEP-007
#[test]
#[ignore = "nightly: 1000-yr double-double N-body runs; run with --profile nightly --cargo-profile release"]
fn nightly_checkpoint_resume_1000yr_double_double() {
    let s = scenarios().join("sun-8planets-newton-j2-1000yr-dd.toml");
    interrupted_equals_uninterrupted("dd-1000yr", &s, "500");
}

/// 4: an unsupported switch is a non-zero exit naming it; an unknown one too.
///
/// Verifies: REF-011
#[test]
fn cli_refuses_unsupported_and_unknown_switches_by_name() {
    let dir = workdir("refusal");
    let base =
        std::fs::read_to_string(scenarios().join("sun-mercury-newton-header440.toml")).unwrap();
    for (switch, line, code) in [
        ("forces.gr_eih", "gr_eih = true", "FA-E1201"),
        // asteroids are implemented in N-body mode since WP15; a central-body term is not.
        ("forces.lense_thirring", "lense_thirring = true", "FA-E1201"),
        ("\"fifth_force\"", "fifth_force = true", "FA-E1202"),
    ] {
        let p = dir.join("refused.toml");
        std::fs::write(
            &p,
            base.replace("newton = true", &format!("newton = true\n{line}")),
        )
        .unwrap();
        let out = Command::new(exe())
            .args(["scenario", "validate", p.to_str().unwrap()])
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        println!("{line}: exit {:?}: {err}", out.status.code());
        assert_eq!(out.status.code(), Some(2));
        assert!(err.contains(code) && err.contains(switch), "{err}");
    }
}
