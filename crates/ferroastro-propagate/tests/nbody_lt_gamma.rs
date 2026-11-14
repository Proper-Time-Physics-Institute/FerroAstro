// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Progress review after WP21, V1 (applied in WP24): the N-body solar Lense–Thirring term takes
//! PPN γ from the scenario. Sun + Mercury N-body from header.440 (pinned cache), 1 Julian year,
//! `lense_thirring` on (with `ppn = {beta = 1, gamma = 0.9}` and `{1, 1}`) and off; without
//! `gr_eih`, γ enters only the LT term, whose acceleration is ∝ (1 + γ)/2 (DE440 Eqs. 37–39).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, RunOutput, run};
use ferroastro_scenario::{Ppn, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

/// The γ of the test (PPN parameter, a model setting; the vector's quantity names it).
const GAMMA: f64 = 0.9; // lint: allow-literal(PPN gamma of the V1 test, see NBODY-LT-GAMMA-WIRING)
/// NAIF id of the Mercury barycentre (the N-body Mercury slot).
const MERCURY: i32 = 1;

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn final_mercury(out: &RunOutput) -> [f64; 3] {
    let f = out.final_state.as_ref().unwrap();
    let i = f.bodies.iter().position(|&b| b == MERCURY).unwrap();
    f.positions[i].map(|p| p[0] + p[1])
}

/// V1: the LT displacement scales as (1 + γ)/2 with the scenario's γ, and the model records it.
///
/// Verifies: NUM-019, PHY-011
#[test]
fn nbody_lense_thirring_uses_the_scenario_gamma() {
    let tol = VectorSet::load_canonical()
        .unwrap()
        .get("NBODY-LT-GAMMA-WIRING")
        .unwrap()
        .clone();
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/sun-mercury-newton-header440.toml");
    let base = ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
    let mut displacement = Vec::new();
    for gamma in [GAMMA, 1.0] {
        // The LT-off run carries no PPN block (it has no relativistic term).
        let mut off = base.clone();
        off.span.years = 1.0;
        let mut on = off.clone();
        on.forces.lense_thirring = true;
        on.forces.ppn = Some(Ppn { beta: 1.0, gamma });
        let (a, b) = (run(&on, &data).unwrap(), run(&off, &data).unwrap());
        assert_eq!(a.ic_hash, b.ic_hash);
        let line = a
            .model
            .lines()
            .find(|l| l.contains("solar-lense-thirring"))
            .unwrap();
        println!("gamma = {gamma}: {line}");
        assert!(
            line.contains(&format!("PPN gamma = {gamma} 1;")),
            "the LT term must record the scenario's gamma: {line}"
        );
        let (ra, rb) = (final_mercury(&a), final_mercury(&b));
        let d = [ra[0] - rb[0], ra[1] - rb[1], ra[2] - rb[2]];
        displacement.push((d[0].hypot(d[1]).hypot(d[2]), a.ic_hash.clone()));
    }
    assert_eq!(displacement[0].1, displacement[1].1, "same IC hash");
    let ratio = displacement[0].0 / displacement[1].0;
    let want = f64::midpoint(1.0, GAMMA);
    println!(
        "V1: LT displacement of Mercury after 1 yr: gamma = {GAMMA}: {:.6e} km, gamma = 1: {:.6e} \
         km; ratio {ratio:.9} (expected (1 + gamma)/2 = {want})",
        displacement[0].0, displacement[1].0
    );
    tol.check(ratio - want).unwrap();
}
