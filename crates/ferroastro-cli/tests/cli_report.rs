// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! End-to-end check of `ferroastro ladder --rungs single-body --details` (ROADMAP M2 acceptance:
//! the first CLI report). Short runs: this checks the report, not the physics (the rung tests in
//! ferroastro-ladder do that at 1000 orbits).

use ferroastro_cli::{Options, run};

/// Verifies: PHY-015, PHY-008, NUM-016
#[test]
fn single_body_report_lists_every_rung_with_gauge_and_metadata() {
    let opts = Options {
        rungs: "single-body".into(),
        orbits: Some(20),
        details: true,
        check: true,
    };
    let mut out = Vec::new();
    let all_ok = run(&opts, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    println!("{text}");
    for id in [
        "SR-a",
        "SR-b/harmonic",
        "SR-b/areal",
        "GR-SCHW",
        "J2",
        "J2-DE440",
        "J2-INCL",
        "LT",
    ] {
        assert!(text.lines().any(|l| l.starts_with(id)), "missing rung {id}");
    }
    for needle in [
        "PPN isotropic/harmonic",
        "Schwarzschild areal",
        "special relativity",
        "frame     :",
        "epoch     :",
        "fit model :",
        "constants : DE440 (header.440 sha256",
        "not an observable",
        "H = gamma m c^2 - mu m/r",
    ] {
        assert!(text.contains(needle), "report lacks {needle:?}");
    }
    assert!(all_ok, "a rung failed its oracle in the CLI report");
}

/// The report is evidence, so the binary refuses an `FA_VECTORS_DIR` override (FA-E0106) with a
/// NON-CANONICAL banner instead of checking rungs against substituted oracles (WP6 review).
///
/// Verifies: TST-002
#[test]
fn report_refuses_a_vector_directory_override() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_ferroastro"))
        .args(["ladder", "--rungs", "single-body", "--orbits", "3"])
        .env("FA_VECTORS_DIR", env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("{stderr}");
    assert!(!out.status.success(), "override was accepted");
    assert!(stderr.contains("FA-E0106"), "no FA-E0106 in {stderr:?}");
    assert!(stderr.contains("NON-CANONICAL VECTORS"), "no banner");
}
