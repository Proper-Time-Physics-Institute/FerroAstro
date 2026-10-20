// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP13 check 5 (data tier, real data): rung GR-SCHW driven by the committed scenario
//! `ci/scenarios/sun-mercury-gr1pn-header440.toml` (Sun + Mercury test particle,
//! `gr_1pn_central`, DE440 header.440 initial conditions, 1001 perihelion passages) through
//! `ferroastro-propagate`, against
//!
//! * PHY-005: 42.980 ± 0.001 ″/cy (vector `PHY005-GR-NUM-RATE`), and
//! * the direct-API run of the same rung on the same header (WP7's D5:
//!   `SingleBody::from_state` with Mercury − Sun from GROUP 1041): the initial state, every
//!   passage and the fitted rate must be **bitwise** identical, in f64 and in double-double.
//!
//! header.440 is downloaded fresh (or taken from the cache after re-verifying its pinned
//! SHA-256) and its URL, retrieval date and SHA-256 are printed.

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

mod common;

use ferroastro_analytic::advance::rate_arcsec_per_century;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Frame, Icrf};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_ladder::single_body::SingleBody;
use ferroastro_precession::passages::{OrbitPlane, Passage, fit_per_orbit};
use ferroastro_propagate::{DataSources, RunOutput, run};
use ferroastro_scenario::{Precision, ScenarioSpec};
use ferroastro_testdata::fetch_product;
use std::path::PathBuf;

fn scenario() -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/sun-mercury-gr1pn-header440.toml");
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn passages(out: &RunOutput) -> Vec<Passage> {
    out.passages
        .iter()
        .map(|p| Passage {
            t: p.t[0],
            r: p.r.map(|z| z[0]),
            v: p.v.map(|z| z[0]),
        })
        .collect()
}

/// Verifies: PHY-005, CI-002, TST-005
#[test]
#[ignore = "data: downloads JPL header.440 (network); run with --profile data"]
fn data_gr_schw_scenario_equals_direct_api_bitwise_from_header440() {
    let c = common::ctx();
    let fetched = fetch_product("de440-header").unwrap();
    let prov = fetched.provenance();
    println!("de440-header {prov}");
    let h = Header::parse(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    // The direct-API run of WP7 D5 (crates/ferroastro-ladder/tests/data_single_body.rs).
    let v = |n: &str| h.value(n).unwrap();
    let au = v("AU");
    let r0 = ["X", "Y", "Z"].map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au);
    let v0 = ["XD", "YD", "ZD"]
        .map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au / SECONDS_PER_DAY);
    let setup = SingleBody::<Icrf>::from_state(
        r0,
        v0,
        &c.de,
        c.scn.orbits,
        format!(
            "initial osculating orbit plane (axes fixed in {})",
            Icrf::NAME
        ),
        format!("DE440; initial state: header.440 X1..ZD1 minus XS..ZDS ({prov})"),
    )
    .unwrap();
    let rung = c.rung("GR-SCHW");
    let direct_passages = setup.perihelion_passages(&rung).unwrap();
    let mu = c.de.gm_sun().km3_per_s2();
    let plane = OrbitPlane::osculating(r0, v0, mu).unwrap();
    let data = DataSources::none().with_header(&h, &prov);
    let mut spec = scenario();
    for precision in [Precision::F64, Precision::Dd] {
        spec.integrator.precision = precision;
        let started = std::time::Instant::now();
        let out = run(&spec, &data).unwrap();
        let wall = started.elapsed().as_secs_f64();
        let direct = match precision {
            Precision::F64 => setup.run(&rung).unwrap(),
            Precision::Dd => setup
                .run_in::<ferroastro_real::DoubleDouble>(&rung)
                .unwrap(),
            Precision::Mixed => unreachable!("test-particle runs refuse the mixed mode"),
        };
        let ps = passages(&out);
        let fit = fit_per_orbit(&ps, &plane).unwrap();
        let rate = rate_arcsec_per_century(fit.rate());
        let direct_rate = direct.numerical_arcsec_per_century();
        let init = &out.states[0];
        let same_ic = init.positions[0].map(|z| z[0].to_bits()) == r0.map(f64::to_bits)
            && init.velocities[0].map(|z| z[0].to_bits()) == v0.map(f64::to_bits);
        println!(
            "WP13-5 {}: scenario {} passages, {} steps ({wall:.2} s); rate {rate:.12} ″/cy; \
             direct API (D5) {direct_rate:.12} ″/cy, {} steps; rate bits equal: {}; same ICs: \
             {same_ic}; IC hash {}; model: {}",
            precision.label(),
            ps.len(),
            out.stats.steps,
            direct.stats.steps,
            rate.to_bits() == direct_rate.to_bits(),
            out.ic_hash,
            out.model
        );
        assert!(same_ic);
        assert_eq!(out.stats, direct.stats);
        assert_eq!(rate.to_bits(), direct_rate.to_bits());
        if precision == Precision::F64 {
            assert_eq!(ps.len(), direct_passages.passages.len());
            let differing = ps
                .iter()
                .zip(&direct_passages.passages)
                .filter(|(a, b)| a != b)
                .count();
            println!("WP13-5 f64: passages differing from the direct API: {differing}");
            assert_eq!(differing, 0);
        }
        c.check("PHY005-GR-NUM-RATE", rate);
    }
}
