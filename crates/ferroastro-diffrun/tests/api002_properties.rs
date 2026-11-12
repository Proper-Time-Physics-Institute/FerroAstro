// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-002 property tests (PR #13 judge review R13-CODE-7): plan construction with arbitrary
//! toggles, labels, pairs and leg spans, and run options with arbitrary segment lengths, give a
//! valid result or `FA-E1400`, never a panic. Offline (no run), PR tier, deterministic RNG.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::default_trait_access,
    clippy::single_match_else,
    reason = "test: a failed expect is a test failure; fixtures are spelled plainly"
)]

use ferroastro_core::ErrorCode;
use ferroastro_diffrun::{BodyToggle, DiffrunError, MAX_SEGMENTS, Plan, RunOptions};
use ferroastro_scenario::{BodyName, ScenarioSpec};
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

fn template() -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/sun-mercury-newton-header440.toml");
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap_or_default())
        .expect("committed scenario parses")
}

fn wild() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(0.0),
        Just(f64::NAN),
        Just(f64::INFINITY),
        Just(f64::MIN_POSITIVE),
        -1.0e9..1.0e9_f64, // lint: allow-literal(property-test range)
    ]
}

fn coded(e: &DiffrunError) -> bool {
    e.code() == ErrorCode::DiffrunPlan && e.to_string().starts_with("FA-E1400")
}

/// Verifies: API-002
#[test]
fn run_options_give_a_bounded_segment_count_or_a_coded_error() {
    let spec = template();
    runner(4096) // lint: allow-literal(proptest case count)
        .run(&(wild(), wild()), |(years, segment_years)| {
            let mut s = spec.clone();
            s.span.years = years;
            let opts = RunOptions {
                segment_years,
                ..RunOptions::default()
            };
            match opts.segments(&s) {
                Ok(n) => prop_assert!(n <= MAX_SEGMENTS),
                Err(e) => prop_assert!(coded(&e), "{}", e),
            }
            Ok(())
        })
        .unwrap();
}

/// Verifies: API-002
#[test]
fn plan_construction_never_panics() {
    use BodyName::{Earth, Jupiter, Mars, Mercury, Moon, Saturn, Sun, Venus};
    let template = template();
    let candidates = [Venus, Earth, Moon, Mars, Jupiter, Saturn, Mercury, Sun];
    let labels = ["A", "B", "C", "A+B", "base", "full"];
    let toggle = (
        prop::sample::select(labels.to_vec()),
        prop::collection::vec(prop::sample::select(candidates.to_vec()), 0..3),
    );
    runner(512) // lint: allow-literal(proptest case count)
        .run(
            &(
                prop::collection::vec(toggle, 0..4),
                prop::collection::vec((0_usize..6, 0_usize..6), 0..4),
                prop::collection::vec(wild(), 0..3),
                any::<bool>(),
            ),
            |(toggles, pairs, years, with_full)| {
                let toggles: Vec<BodyToggle> =
                    toggles.iter().map(|(l, b)| BodyToggle::new(l, b)).collect();
                let full = if with_full {
                    vec![Sun, Mercury, Venus, Earth, Moon, Mars, Jupiter]
                } else {
                    vec![]
                };
                match Plan::bodies(
                    "prop",
                    &template,
                    &[Sun, Mercury],
                    &full,
                    &toggles,
                    &pairs,
                    &years,
                ) {
                    Ok(plan) => prop_assert!(plan.validate().is_ok()),
                    Err(e) => prop_assert!(coded(&e), "{}", e),
                }
                Ok(())
            },
        )
        .unwrap();
}
