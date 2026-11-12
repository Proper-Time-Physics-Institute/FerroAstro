// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-002 property tests (PR #13 judge review R13-CODE-7): arbitrary golden text and arbitrary
//! FerroAstro states give `FA-E15xx` errors or a finite comparison, never a panic. Offline, PR
//! tier, deterministic RNG.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::default_trait_access,
    clippy::single_match_else,
    reason = "test: a failed expect is a test failure; fixtures are spelled plainly"
)]

use ferroastro_core::ErrorCode;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_oracle::{GoldenStates, OracleError, STATE_COLUMNS, compare_states};
use ferroastro_propagate::{RunOutput, StateRecord};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

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

fn coded(e: &OracleError) -> bool {
    let code = e.code();
    matches!(
        code,
        ErrorCode::OracleGolden | ErrorCode::OracleComparison | ErrorCode::OracleManifest
    ) && e.to_string().starts_with(code.as_str())
}

fn wild() -> impl Strategy<Value = f64> {
    prop_oneof![
        any::<f64>(),
        Just(f64::NAN),
        Just(f64::INFINITY),
        -1.0e9..1.0e9_f64, // lint: allow-literal(property-test range)
    ]
}

/// Verifies: API-002
#[test]
fn arbitrary_golden_text_never_panics() {
    let good = format!("# epoch_jd_tdb: 2451545\n{STATE_COLUMNS}\n0,199,1,2,3,4,5,6\n");
    runner(1024) // lint: allow-literal(proptest case count)
        .run(
            &(any::<String>(), any::<prop::sample::Index>(), ".{0,30}"),
            |(junk, at, insert)| {
                if let Err(e) = GoldenStates::parse("p.csv", &junk) {
                    prop_assert!(coded(&e), "{}", e);
                }
                let mut s = good.clone();
                let i = at.index(s.len() + 1);
                if s.is_char_boundary(i) {
                    s.insert_str(i, &insert);
                }
                match GoldenStates::parse("p.csv", &s) {
                    Ok(g) => {
                        let _ = g.meta_f64("epoch_jd_tdb");
                        let _ = g.center_naif();
                    }
                    Err(e) => prop_assert!(coded(&e), "{}", e),
                }
                Ok(())
            },
        )
        .unwrap();
}

/// Verifies: API-002
#[test]
fn compare_states_gives_a_finite_result_or_a_coded_error() {
    let golden = GoldenStates::parse(
        "p.csv",
        &format!("# epoch_jd_tdb: 2451545\n{STATE_COLUMNS}\n0,199,1,2,3,4,5,6\n"),
    )
    .expect("fixture parses");
    let base = RunOutput {
        scenario: "p".into(),
        scenario_digest: String::new(),
        ic_hash: String::new(),
        precision: "f64".into(),
        constants: String::new(),
        ic_source: String::new(),
        epoch_jd_tdb: [2_451_545.0, 0.0], // lint: allow-literal(the fixture's epoch, J2000 JD)
        model: String::new(),
        states: Vec::new(),
        passages: Vec::new(),
        final_state: None,
        stats: Default::default(),
        finished: true,
    };
    runner(2048) // lint: allow-literal(proptest case count)
        .run(
            &(
                prop::array::uniform3(wild()),
                prop::array::uniform3(wild()),
                wild(),
                prop_oneof![Just(199), any::<i32>()],
            ),
            |(p, v, t_days, naif)| {
                let mut run = base.clone();
                run.states = vec![StateRecord {
                    t: [t_days * SECONDS_PER_DAY, 0.0],
                    bodies: vec![naif],
                    positions: vec![p.map(|c| [c, 0.0])],
                    velocities: vec![v.map(|c| [c, 0.0])],
                }];
                match compare_states(&golden, &run) {
                    Ok(c) => prop_assert!(c.max_dr_km().is_finite()),
                    Err(e) => prop_assert!(coded(&e), "{}", e),
                }
                Ok(())
            },
        )
        .unwrap();
}
