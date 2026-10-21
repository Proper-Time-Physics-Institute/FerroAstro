// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-002 property tests (PR #13 judge review R13-CODE-7): the output-grid count and `run` on
//! scenarios with arbitrary span, cadence and step settings return a coded error or a bounded
//! result, never a panic or an endless count. Offline: every case `run` reaches is refused by
//! validation before any data is read. Deterministic RNG.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::default_trait_access,
    clippy::single_match_else,
    reason = "test: a failed expect is a test failure; fixtures are spelled plainly"
)]

use ferroastro_core::ErrorCode;
use ferroastro_propagate::passages::grid_samples;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::{MAX_OUTPUT_SAMPLES, ScenarioSpec};
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

fn wild() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(0.0),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::MIN_POSITIVE),
        Just(f64::MAX),
        -1.0e15..1.0e15_f64,  // lint: allow-literal(property-test range)
        1.0e-300..1.0e-3_f64, // lint: allow-literal(property-test range of tiny steps)
    ]
}

/// Verifies: API-002
#[test]
fn grid_count_is_bounded_or_a_coded_error() {
    let t0 = std::time::Instant::now();
    runner(4096) // lint: allow-literal(proptest case count)
        .run(&(wild(), wild()), |(span, step)| {
            match grid_samples("p", span, step) {
                Ok(n) => {
                    prop_assert!(n >= 1);
                    prop_assert!(n <= usize::try_from(MAX_OUTPUT_SAMPLES).unwrap_or(usize::MAX));
                }
                Err(e) => {
                    prop_assert_eq!(e.code(), ErrorCode::PropagateSetup);
                    prop_assert!(e.to_string().starts_with("FA-E1300"), "{}", e);
                }
            }
            Ok(())
        })
        .unwrap();
    assert!(t0.elapsed().as_secs_f64() < 10.0); // lint: allow-literal(wall-clock bound: closed-form count)
}

/// Verifies: API-002
#[test]
fn run_refuses_invalid_numeric_settings_without_panicking() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/oracle-clite-sun-mercury-tp-newton-252yr.toml");
    let spec = ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap_or_default())
        .expect("committed scenario parses");
    let data = DataSources::none();
    runner(1024) // lint: allow-literal(proptest case count)
        .run(&(wild(), wild(), wild()), |(years, cadence, eps)| {
            let mut s = spec.clone();
            s.span.years = years;
            s.output.cadence_days = Some(cadence);
            s.integrator.epsilon = Some(eps);
            if let Err(v) = s.validate() {
                let e = run(&s, &data).expect_err("an invalid spec must not run");
                prop_assert_eq!(e.code(), v.code());
                prop_assert!(e.to_string().starts_with(v.code().as_str()), "{}", e);
            }
            Ok(())
        })
        .unwrap();
}
