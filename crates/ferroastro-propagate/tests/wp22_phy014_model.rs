// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 check E11 (PR tier): the PHY-014 scenario (`ci/scenarios/phy014-eih-full-header440.toml`)
//! builds through the scenario API and runs for 30 days. The model description must show the EIH
//! and Lense–Thirring terms acting on the Major and the Minor bodies (decision D1, OI-35); the
//! run's step cost is printed for the WP23 nightly estimate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::cast_precision_loss,
    clippy::panic,
    reason = "test: a failed unwrap is a test failure; step counts are small"
)]

use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use std::path::PathBuf;

const SMOKE_DAYS: f64 = 30.0; // lint: allow-literal(run length, PR-tier cost, WP22 plan E11)

/// Verifies: PHY-014, NUM-009, NUM-019
#[test]
fn phy014_model_builds_and_runs_with_eih_and_lt_on_minor_bodies() {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let small = ferroastro_system::SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    let mut spec = ScenarioSpec::parse(
        &std::fs::read_to_string(root.join("ci/scenarios/phy014-eih-full-header440.toml")).unwrap(),
    )
    .unwrap();
    spec.span.years = SMOKE_DAYS / ferroastro_scenario::DAYS_PER_JULIAN_YEAR;
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&small);
    let t = std::time::Instant::now();
    let out = run(&spec, &data).unwrap();
    let wall = t.elapsed().as_secs_f64();
    println!("model: {}", out.model);
    println!("IC source: {}", out.ic_source);
    println!(
        "E11: {SMOKE_DAYS} d, {} IAS15 steps, {} force evaluations, {wall:.2} s wall ({:.2} ms/step)",
        out.stats.steps,
        out.stats.force_evaluations,
        1e3 * wall / out.stats.steps as f64 // lint: allow-literal(seconds to milliseconds)
    );
    assert!(out.finished);
    assert!(
        out.model.contains("393 Minor") || out.model.contains("346 Minor"),
        "{}",
        out.model
    );
    for term in ["eih-1pn", "solar-lense-thirring-with-reaction"] {
        let line = out
            .model
            .split(term)
            .nth(1)
            .unwrap_or_else(|| panic!("{term} missing from the model"));
        let tiers = line.split(':').next().unwrap_or_default();
        assert!(tiers.contains("Major, Minor"), "{term}: {tiers}");
    }
}
