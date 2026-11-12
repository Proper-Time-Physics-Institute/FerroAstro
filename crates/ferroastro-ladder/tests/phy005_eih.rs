// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rungs GR-SCHW and PPN through the EIH N-body equations (WP19 checks C3, C3b, C4; PR tier:
//! header.440 from the pinned JPL cache, CI-005). Sun + Mercury as two bodies with their DE440
//! GMs, from the DE440 initial conditions, 1001 perihelion passages, per-orbit fit:
//!
//! * C3: `gr_eih` gives 42.980 ± 0.001 ″/cy (PHY-005) and matches the R3 test-particle run of the
//!   same header state within 1e-4 ″/cy (NUM-013(a));
//! * C3b: the Sun-only rung `gr_1pn_central` in N-body mode matches the R3 run likewise;
//! * C4: rate(β, γ)/rate(1, 1) = (2 + 2γ − β)/3 within 1e-6 at PHY-006's range edges, from
//!   identical-IC differential runs (PHY-006, REF-010).

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

mod common;

use ferroastro_analytic::advance::{ppn_scaling, rate_arcsec_per_century};
use ferroastro_constants::header440::Header;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_precession::passages::{OrbitPlane, Passage, fit_per_orbit};
use ferroastro_propagate::{DataSources, RunOutput, run};
use ferroastro_scenario::{Ppn, ScenarioSpec};
use std::path::PathBuf;

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Mercury − Sun at JDEPOC from the header (km, km/s): the plane of the fit.
fn plane(h: &Header, mu: f64) -> OrbitPlane {
    let v = |n: &str| h.value(n).unwrap();
    let au = v("AU");
    let r0 = ["X", "Y", "Z"].map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au);
    let v0 = ["XD", "YD", "ZD"]
        .map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au / SECONDS_PER_DAY);
    OrbitPlane::osculating(r0, v0, mu).unwrap()
}

/// The per-orbit rate (″/cy) of a run's Mercury passages relative to the Sun.
fn rate(out: &RunOutput, plane: &OrbitPlane) -> f64 {
    let ps: Vec<Passage> = out
        .passages
        .iter()
        .map(|p| Passage {
            t: p.t[0],
            r: p.r.map(|z| z[0]),
            v: p.v.map(|z| z[0]),
        })
        .collect();
    let fit = fit_per_orbit(&ps, plane).unwrap();
    rate_arcsec_per_century(fit.rate())
}

/// C3 + C3b.
///
/// Verifies: PHY-005, NUM-013, NUM-017
#[test]
fn eih_and_nbody_central_rung_reproduce_gr_schw_and_the_r3_run() {
    let c = common::ctx();
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let pl = plane(&h, c.de.gm_sun().km3_per_s2());
    let r3 = run(&spec("sun-mercury-gr1pn-header440.toml"), &data).unwrap();
    let eih_spec = spec("sun-mercury-eih-header440.toml");
    let eih = run(&eih_spec, &data).unwrap();
    let mut central_spec = eih_spec.clone();
    central_spec.forces.gr_eih = false;
    central_spec.forces.gr_1pn_central = true;
    let central = run(&central_spec, &data).unwrap();
    let (r_r3, r_eih, r_central) = (rate(&r3, &pl), rate(&eih, &pl), rate(&central, &pl));
    for (label, out, r) in [
        ("R3 test particle", &r3, r_r3),
        ("EIH n-body", &eih, r_eih),
        ("gr_1pn_central n-body", &central, r_central),
    ] {
        println!(
            "C3 {label}: rate {r:.9} ″/cy, {} passages, {} IAS15 steps, IC hash {}\n  model: {}",
            out.passages.len(),
            out.stats.steps,
            out.ic_hash,
            out.model
        );
    }
    // NUM-017: the EIH model declares its gauge and time argument.
    assert!(
        eih.model.contains("PPN isotropic/harmonic"),
        "{}",
        eih.model
    );
    assert!(eih.model.contains("time argument TDB"), "{}", eih.model);
    assert!(eih.model.contains("eih-1pn"), "{}", eih.model);
    assert_eq!(eih.passages.len(), r3.passages.len());
    c.check("PHY005-GR-NUM-RATE", r_eih);
    println!(
        "C3: EIH - R3 = {:+.3e} ″/cy; gr_1pn_central (n-body) - R3 = {:+.3e} ″/cy",
        r_eih - r_r3,
        r_central - r_r3
    );
    c.check("EIH-VS-R3-RATE-DIFF", r_eih - r_r3);
    c.check("EIH-VS-R3-RATE-DIFF", r_central - r_r3);
}

/// C4: the PPN scaling through EIH, identical initial conditions.
///
/// Verifies: PHY-006, REF-010
#[test]
fn eih_rate_scales_as_two_plus_two_gamma_minus_beta_over_three() {
    let c = common::ctx();
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let pl = plane(&h, c.de.gm_sun().km3_per_s2());
    let base_spec = spec("sun-mercury-eih-header440.toml");
    let at = |beta: f64, gamma: f64| {
        let mut s = base_spec.clone();
        s.forces.ppn = Some(Ppn { beta, gamma });
        let out = run(&s, &data).unwrap();
        (rate(&out, &pl), out.ic_hash)
    };
    let (base, hash) = at(1.0, 1.0);
    println!("C4 base rate {base:.12} ″/cy, IC hash {hash}");
    // PHY-006's range edges |beta - 1|, |gamma - 1| <= 1e-3, as the WP4 test-particle rung.
    let d = 1.0e-3; // lint: allow-literal(PHY-006 range bound, from the requirement text)
    for (beta, gamma) in [
        (1.0 + d, 1.0),
        (1.0 - d, 1.0),
        (1.0, 1.0 + d),
        (1.0, 1.0 - d),
        (1.0 + d, 1.0 - d),
    ] {
        let (r, h2) = at(beta, gamma);
        assert_eq!(
            h2, hash,
            "PHY-006 runs must share identical initial conditions"
        );
        let ratio = r / base;
        let want = ppn_scaling(beta, gamma);
        println!(
            "C4 beta = {beta}, gamma = {gamma}: ratio {ratio:.12}, (2+2gamma-beta)/3 = {want:.12}"
        );
        c.check("PHY006-RATIO-REL-DEV", ratio / want - 1.0);
    }
}
