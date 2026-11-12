// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-002 property tests (PR #13 judge review R13-CODE-7): arbitrary scenario text and arbitrary
//! numeric fields give `FA-E12xx` errors or a valid spec, never a panic; every error message
//! starts with its code. Offline, PR tier, deterministic RNG.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::default_trait_access,
    clippy::single_match_else,
    reason = "test: a failed expect is a test failure; fixtures are spelled plainly"
)]

use ferroastro_core::ErrorCode;
use ferroastro_scenario::{MAX_OUTPUT_SAMPLES, ScenarioError, ScenarioSpec};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use std::path::PathBuf;

fn runner(cases: u32) -> TestRunner {
    TestRunner::new_with_rng(
        Config {
            cases,
            failure_persistence: None,
            ..Config::default()
        },
        TestRng::deterministic_rng(RngAlgorithm::ChaCha),
    )
}

fn base() -> (String, ScenarioSpec) {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/oracle-clite-sun-mercury-tp-newton-252yr.toml");
    let text = std::fs::read_to_string(p).expect("committed scenario");
    let spec = ScenarioSpec::parse(&text).expect("committed scenario parses");
    (text, spec)
}

fn coded(e: &ScenarioError) -> bool {
    let code = e.code();
    matches!(
        code,
        ErrorCode::ScenarioMalformed
            | ErrorCode::ScenarioUnsupported
            | ErrorCode::ScenarioUnknownSwitch
            | ErrorCode::ScenarioValue
    ) && e.to_string().starts_with(code.as_str())
}

/// Any f64, weighted towards the edge cases.
fn wild() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(0.0),
        Just(-0.0),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
        Just(f64::MIN_POSITIVE),
        Just(f64::MAX),
        -1.0e12..1.0e12_f64, // lint: allow-literal(property-test range)
    ]
}

/// Verifies: API-002
#[test]
fn arbitrary_text_and_mutated_scenarios_never_panic() {
    let (text, spec) = base();
    // Arbitrary bytes, and the committed file with one line replaced by arbitrary text.
    runner(512) // lint: allow-literal(proptest case count)
        .run(
            &(any::<String>(), any::<prop::sample::Index>(), ".{0,40}"),
            |(junk, line, repl)| {
                if let Err(e) = ScenarioSpec::parse(&junk) {
                    prop_assert!(coded(&e), "{e}");
                }
                let mut lines: Vec<&str> = text.lines().collect();
                let i = line.index(lines.len());
                lines[i] = &repl;
                if let Err(e) = ScenarioSpec::parse(&lines.join("\n")) {
                    prop_assert!(coded(&e), "{e}");
                }
                Ok(())
            },
        )
        .unwrap();
    // Every numeric field the spec validates, set to anything.
    runner(1024) // lint: allow-literal(proptest case count)
        .run(
            &(wild(), wild(), wild(), wild(), wild(), wild()),
            |(years, cadence, eps, pc, dt0, jd)| {
                let mut s = spec.clone();
                s.span.years = years;
                s.output.cadence_days = Some(cadence);
                s.integrator.epsilon = Some(eps);
                s.integrator.pc_tolerance = Some(pc);
                s.integrator.dt0_days = Some(dt0);
                s.initial_conditions.epoch_jd_tdb = Some(jd);
                match s.validate() {
                    Err(e) => prop_assert!(coded(&e), "{e}"),
                    Ok(()) => {
                        // A valid spec has a finite, non-zero span and a grid within the cap.
                        prop_assert!(years.is_finite() && years != 0.0);
                        let intervals = (years.abs() * ferroastro_scenario::DAYS_PER_JULIAN_YEAR
                            / cadence)
                            .floor();
                        prop_assert!(intervals < f64::from(MAX_OUTPUT_SAMPLES));
                    }
                }
                Ok(())
            },
        )
        .unwrap();
}
