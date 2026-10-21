// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP29 checks K1 and K2: the universal-variable Kepler drift (the Wisdom–Holman drift, NUM-004)
//! against the element-based two-body solution, and its reversibility (elliptic and hyperbolic).
//! Orbit: SW92 J2000 mean elements of Mercury with the DE440 au and GMs.

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_analytic::drift::kepler_drift;
use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::{EclipticJ2000, Heliocentric, State};
use ferroastro_core::units::{Angle, GravParam, Length, Velocity};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

fn value(vs: &VectorSet, id: &str) -> f64 {
    vs.get(id).unwrap().value().unwrap()
}

fn deg(vs: &VectorSet, id: &str) -> Angle {
    Angle::from_radians(value(vs, id).to_radians()).unwrap()
}

fn sw92(vs: &VectorSet, de: &De440) -> Elements {
    let a = Length::from_km(value(vs, "SW92-MERCURY-A") * de.au().km()).unwrap();
    Elements::from_longitudes(
        a,
        value(vs, "SW92-MERCURY-E"),
        deg(vs, "SW92-MERCURY-I"),
        deg(vs, "SW92-MERCURY-NODE"),
        deg(vs, "SW92-MERCURY-VARPI"),
        deg(vs, "SW92-MERCURY-L"),
    )
}

fn raw(s: &State<EclipticJ2000, Heliocentric>) -> ([f64; 3], [f64; 3]) {
    (
        s.position().components().map(Length::km),
        s.velocity().map(Velocity::km_per_s),
    )
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

fn diff(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
}

/// Drifts in units of the orbital period (WP29 plan K1: 12 drifts from −3 to +7 periods).
#[rustfmt::skip]
const PERIODS: [f64; 12] = [-3.0, -1.5, -0.37, -0.01, 0.01, 0.25, 0.5, 0.999, 1.3, 2.7, 5.1, 7.0]; // lint: allow-literal(test design: drift lengths in periods)

/// Verifies: NUM-004
#[test]
fn kepler_drift_matches_the_element_solution() {
    let vs = VectorSet::load_workspace().unwrap();
    let de = De440::load().unwrap();
    let el0 = sw92(&vs, &de);
    let mu = de.gm_sun() + de.gm(Body::Mercury);
    let mu_km = mu.km3_per_s2();
    let a = el0.a.km();
    let n = (mu_km / (a * a * a)).sqrt();
    let period = std::f64::consts::TAU / n;
    let (r0, v0) = raw(&el0.to_state::<EclipticJ2000, Heliocentric>(mu).unwrap());
    let mut worst: f64 = 0.0;
    for &k in &PERIODS {
        let dt = k * period;
        let (mut r, mut v) = (r0, v0);
        kepler_drift(mu_km, &mut r, &mut v, dt).unwrap();
        let mut el1 = el0;
        el1.mean_anomaly = Angle::from_radians(n.mul_add(dt, el0.mean_anomaly.radians())).unwrap();
        let (re, ve) = raw(&el1.to_state::<EclipticJ2000, Heliocentric>(mu).unwrap());
        let dr = diff(r, re) / a;
        let dv = diff(v, ve) / norm(ve);
        println!("K1 dt = {k:+.3} P: |dr|/a = {dr:.3e}, |dv|/v = {dv:.3e}");
        worst = worst.max(dr).max(dv);
    }
    let bound = value(&vs, "KEPLER-DRIFT-VS-ELEMENTS-REL");
    println!(
        "K1: max relative difference {worst:.3e} (bound {bound:e}); P = {:.6} d, a = {a} km, \
         mu = {mu_km} km^3/s^2 (DE440 header sha256 {})",
        period / ferroastro_core::time::SECONDS_PER_DAY,
        de.table().source().sha256
    );
    assert!(worst <= bound, "K1: {worst:e} > {bound:e}");
}

/// Hyperbolic test eccentricity of check K2 (WP29 plan).
const E_HYPERBOLIC: f64 = 1.5; // lint: allow-literal(test design: hyperbolic eccentricity, dimensionless)
/// Elliptic drift length of check K2 in periods.
const K2_PERIODS: f64 = 1.3; // lint: allow-literal(test design: drift length in periods)

/// Verifies: NUM-004
#[test]
fn kepler_drift_is_reversible_for_elliptic_and_hyperbolic_orbits() {
    let vs = VectorSet::load_workspace().unwrap();
    let de = De440::load().unwrap();
    let el0 = sw92(&vs, &de);
    let mu: GravParam = de.gm_sun() + de.gm(Body::Mercury);
    let mu_km = mu.km3_per_s2();
    let a = el0.a.km();
    let period = std::f64::consts::TAU * (a * a * a / mu_km).sqrt();
    let (r0, v0) = raw(&el0.to_state::<EclipticJ2000, Heliocentric>(mu).unwrap());
    // A hyperbolic orbit through Mercury's perihelion distance.
    let q = a * (1.0 - value(&vs, "SW92-MERCURY-E"));
    let rh = [q, 0.0, 0.0];
    let vh = [0.0, (mu_km * (1.0 + E_HYPERBOLIC) / q).sqrt(), 0.0];
    let bound = value(&vs, "KEPLER-DRIFT-REVERSIBILITY-REL");
    for (label, r_start, v_start, dt) in [
        ("elliptic", r0, v0, K2_PERIODS * period),
        ("hyperbolic", rh, vh, K2_PERIODS * period),
    ] {
        let (mut r, mut v) = (r_start, v_start);
        kepler_drift(mu_km, &mut r, &mut v, dt).unwrap();
        let moved = diff(r, r_start) / norm(r_start);
        kepler_drift(mu_km, &mut r, &mut v, -dt).unwrap();
        let dr = diff(r, r_start) / norm(r_start);
        let dv = diff(v, v_start) / norm(v_start);
        println!(
            "K2 {label}: moved {moved:.3} (relative), back |dr|/r = {dr:.3e}, |dv|/v = {dv:.3e}"
        );
        assert!(moved > 0.0);
        assert!(
            dr <= bound && dv <= bound,
            "K2 {label}: {dr:e}, {dv:e} > {bound:e}"
        );
    }
    // The same drift in double-double agrees with f64 to f64 round-off (generic backend).
    let dd = |z: f64| DoubleDouble::from_f64(z);
    let (mut r, mut v) = (r0.map(dd), v0.map(dd));
    kepler_drift(dd(mu_km), &mut r, &mut v, dd(K2_PERIODS * period)).unwrap();
    let (mut rf, mut vf) = (r0, v0); // the velocities are drifted too; positions are compared
    kepler_drift(mu_km, &mut rf, &mut vf, K2_PERIODS * period).unwrap();
    let ddiff = diff(r.map(Real::to_f64), rf) / a;
    println!("K2 double-double vs f64: |dr|/a = {ddiff:.3e}");
    assert!(ddiff <= value(&vs, "KEPLER-DRIFT-VS-ELEMENTS-REL"));
}
