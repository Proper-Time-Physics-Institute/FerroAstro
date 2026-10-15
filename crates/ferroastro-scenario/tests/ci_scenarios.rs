// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The committed scenarios and schema (CI-002), the switch refusals (REF-011) and the IC hash.
//!
//! WP13 check 1 (Rust half; the Python half is `tools/tests/test_validate_scenarios.py`), check 4
//! and the hash properties of check 7.

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_core::ErrorCode;
use ferroastro_scenario::{IcRecord, Mode, ScenarioSpec, ic_hash, json_schema};
use std::path::PathBuf;

fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ci/scenarios")
}

fn scenario_files() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(scenarios_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    v.sort();
    v
}

/// Every `ci/scenarios/*.toml` parses and validates.
///
/// Verifies: CI-002
#[test]
fn every_committed_scenario_validates() {
    let files = scenario_files();
    assert!(!files.is_empty());
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        match ScenarioSpec::parse(&text) {
            Ok(s) => println!(
                "{}: ok ({}, {} bodies, switches {:?}, {} yr, {})",
                f.file_name().unwrap().to_string_lossy(),
                s.mode.label(),
                s.bodies.len(),
                s.forces.requested(),
                s.span.years,
                s.integrator.precision.label()
            ),
            Err(e) => unreachable!("{}: {e}", f.display()),
        }
    }
}

/// The committed JSON Schema equals the one generated from the Rust types (set
/// `FA_REGENERATE_SCHEMA=1` to rewrite it after a deliberate spec change).
///
/// Verifies: CI-002
#[test]
fn committed_json_schema_is_current() {
    let path = scenarios_dir().join("scenario.schema.json");
    let generated = json_schema();
    if std::env::var_os("FA_REGENERATE_SCHEMA").is_some() {
        std::fs::write(&path, &generated).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap();
    assert!(
        committed == generated,
        "ci/scenarios/scenario.schema.json is stale; regenerate with FA_REGENERATE_SCHEMA=1"
    );
}

fn base() -> String {
    std::fs::read_to_string(scenarios_dir().join("sun-mercury-newton-header440.toml")).unwrap()
}

/// An unknown switch is refused with FA-E1202 naming it.
///
/// Verifies: REF-011, CI-002
#[test]
fn unknown_switch_is_refused_by_name() {
    let text = base().replace("newton = true", "newton = true\nfifth_force = true");
    let e = ScenarioSpec::parse(&text).unwrap_err();
    println!("{e}");
    assert_eq!(e.code(), ErrorCode::ScenarioUnknownSwitch);
    assert!(e.to_string().contains("\"fifth_force\""));
}

/// A switch the runner does not implement is refused with FA-E1201 naming it; nothing is
/// silently dropped.
///
/// Verifies: REF-011
#[test]
fn unsupported_switch_is_refused_by_name() {
    let s = ScenarioSpec::parse(&base().replace("newton = true", "newton = true\ngr_eih = true"))
        .unwrap();
    let e = s
        .refuse_unsupported("test-runner", |item, _| {
            if item == "gr_eih" {
                Err("not in this runner".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    println!("{e}");
    assert_eq!(e.code(), ErrorCode::ScenarioUnsupported);
    assert!(e.to_string().contains("forces.gr_eih"));
    let s = ScenarioSpec::parse(&base().replace("\"ias15\"", "\"whfast\"")).unwrap();
    let e = s
        .refuse_unsupported("test-runner", |item, _| {
            if item.starts_with("integrator.whfast") {
                Err("no WH".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err();
    assert!(e.to_string().contains("integrator.whfast"), "{e}");
}

/// Malformed and inconsistent scenarios give coded errors naming the field.
///
/// Verifies: CI-002, API-002
#[test]
fn malformed_scenarios_are_coded_errors() {
    let cases: [(&str, &str, ErrorCode, &str); 7] = [
        (
            "mode = \"n-body\"",
            "mode = \"nbody\"",
            ErrorCode::ScenarioMalformed,
            "nbody",
        ),
        ("name =", "nam =", ErrorCode::ScenarioMalformed, "nam"),
        (
            "years = 100.0",
            "years = 0.0",
            ErrorCode::ScenarioValue,
            "span.years",
        ),
        (
            "central_body = \"Sun\"",
            "central_body = \"Venus\"",
            ErrorCode::ScenarioValue,
            "central_body",
        ),
        (
            "[\"Sun\", \"Mercury\"]\ncentral",
            "[\"Sun\", \"Mercury\", \"Sun\"]\ncentral",
            ErrorCode::ScenarioValue,
            "twice",
        ),
        (
            "source = \"de440-header\"",
            "source = \"de440-bsp\"",
            ErrorCode::ScenarioValue,
            "epoch_jd_tdb",
        ),
        (
            "newton = true",
            "newton = true\nppn = { beta = 1.0, gamma = 1.0 }",
            ErrorCode::ScenarioValue,
            "forces.ppn",
        ),
    ];
    for (from, to, code, needle) in cases {
        let text = base().replacen(from, to, 1);
        assert_ne!(text, base(), "case {from:?} did not apply");
        let e = ScenarioSpec::parse(&text).unwrap_err();
        println!("{e}");
        assert_eq!(e.code(), code, "{e}");
        assert!(e.to_string().contains(needle), "{e} lacks {needle:?}");
    }
    let s = ScenarioSpec::parse(&base()).unwrap();
    assert_eq!(s.mode, Mode::NBody);
}

fn record(naif: i32, x: f64) -> IcRecord {
    let p = |z: f64| [z, 0.0];
    IcRecord {
        naif,
        gm: p(1.0),
        position: [p(x), p(2.0), p(3.0)],
        velocity: [p(4.0), p(5.0), p(6.0)],
    }
}

/// The IC hash is a pure function of the canonical ICs and the constants version: identical
/// inputs give identical hashes; one changed bit, another constants version, mode or a
/// double-double low part give a different one.
///
/// Verifies: CI-002, NUM-013
#[test]
fn ic_hash_identifies_the_initial_conditions() {
    let epoch = [2_440_400.5, 0.0]; // lint: allow-literal(arbitrary JD for a structural hash test)
    let bodies = [record(10, 1.0), record(1, 7.0)];
    let h = ic_hash("DE440 sha256 x", "n-body", epoch, &bodies);
    assert_eq!(h.len(), 64);
    assert_eq!(h, ic_hash("DE440 sha256 x", "n-body", epoch, &bodies));
    let mut flipped = bodies;
    flipped[1].position[0][0] = f64::from_bits(flipped[1].position[0][0].to_bits() ^ 1);
    let mut dd = bodies;
    dd[1].velocity[2][1] = f64::EPSILON;
    let others = [
        ic_hash("DE440 sha256 x", "n-body", epoch, &flipped),
        ic_hash("DE440 sha256 y", "n-body", epoch, &bodies),
        ic_hash("DE440 sha256 x", "test-particle", epoch, &bodies),
        ic_hash("DE440 sha256 x", "n-body", epoch, &dd),
        ic_hash("DE440 sha256 x", "n-body", epoch, &bodies[..1]),
    ];
    for o in &others {
        assert_ne!(o, &h);
    }
    println!("IC hash {h}; variants {others:?}");
}
