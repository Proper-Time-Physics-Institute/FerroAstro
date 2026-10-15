// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-010: dense output and event location for r·v = 0 (heliocentric perihelion), in f64 and in
//! double-double (DEP-007: the integrator passes its tests with both backends).
//!
//! Orbit: a Keplerian Mercury-like ellipse, μ = DE440 GM☉, a and e from SW92 (vectors), started at
//! aphelion so that perihelion falls at `t = P/2 + kP`. Internal units km, km/s, s.

#![expect(
    clippy::indexing_slicing,
    reason = "the test system indexes fixed-length state slices of its own dimension"
)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_integrators::{Ias15, Ias15Config, IntegratorError, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

/// Planar Kepler problem `ẍ = −μx/|x|³` in backend `R`.
struct Kepler<R: Real> {
    mu: R,
}

impl<R: Real> SecondOrderSystem<R> for Kepler<R> {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(&self, _t: R, x: &[R], _v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let r2 = x[0] * x[0] + x[1] * x[1];
        let k = -self.mu / (r2 * r2.sqrt());
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

/// `r·v`, rising through zero at perihelion.
fn rdotv<R: Real>(_t: R, x: &[R], v: &[R]) -> R {
    x[0] * v[0] + x[1] * v[1]
}

/// Metres per kilometre (SI prefix, exact).
const M_PER_KM: f64 = 1000.0; // lint: allow-literal(SI prefix)
/// First trial step as a fraction of the period; the controller adapts it.
const FIRST_STEP_PER_PERIOD: f64 = 1.0 / 64.0; // lint: allow-literal(first trial step fraction)
/// First trial step of the dimensionless dense-output checks.
const DT_UNIT: f64 = 0.1; // lint: allow-literal(dimensionless first step)

struct Orbit<R: Real> {
    mu: R,
    x0: [R; 2],
    v0: [R; 2],
    period: R,
}

fn orbit<R: Real>(vs: &VectorSet) -> Orbit<R> {
    let val = |id: &str| R::from_f64(vs.get(id).unwrap().value().unwrap());
    let mu = val("DE440-GMS-KM3S2");
    let au = val("CONST-AU-IAU2012") / R::from_f64(M_PER_KM);
    let a = val("SW92-MERCURY-A") * au;
    let e = val("SW92-MERCURY-E");
    let ra = a * (R::ONE + e);
    let va = (mu * (R::ONE - e) / ra).sqrt();
    Orbit {
        mu,
        x0: [-ra, R::ZERO],
        v0: [R::ZERO, -va],
        period: R::TAU * (a * a * a / mu).sqrt(),
    }
}

/// Integrate `orbits` periods with dense output on; return every rising r·v root.
fn perihelia<R: Real>(o: &Orbit<R>, orbits: i32) -> (Vec<R>, u64) {
    let cfg = Ias15Config {
        dense_output: true,
        ..Ias15Config::default()
    };
    let sys = Kepler { mu: o.mu };
    let dt0 = o.period.scale(FIRST_STEP_PER_PERIOD);
    let mut ias = Ias15::new(2, R::ZERO, &o.x0, &o.v0, dt0, cfg).unwrap();
    let mut events = Vec::new();
    let mut prev = rdotv(R::ZERO, &o.x0, &o.v0);
    let t_end = o.period * R::from_i32(orbits) + o.period.scale(0.25);
    ias.integrate_to(&sys, t_end, |s, _| {
        let now = rdotv(s.time(), s.positions(), s.velocities());
        if prev < R::ZERO && now >= R::ZERO {
            if let Some(t) = s.locate_event(rdotv)? {
                events.push(t);
            }
        }
        prev = now;
        Ok(())
    })
    .unwrap();
    (events, ias.stats().steps)
}

/// The integrated trajectory's own r·v root near `t`: integrate exactly to `t` and return
/// `|r·v| / |d(r·v)/dt|` with `d(r·v)/dt = v² + r·a`.
fn root_offset(o: &Orbit<f64>, t: f64) -> f64 {
    let sys = Kepler { mu: o.mu };
    let mut ias = Ias15::new(
        2,
        0.0,
        &o.x0,
        &o.v0,
        o.period * FIRST_STEP_PER_PERIOD,
        Ias15Config::default(),
    )
    .unwrap();
    ias.integrate_to(&sys, t, |_, _| Ok(())).unwrap();
    let (x, v) = (ias.positions(), ias.velocities());
    let mut a = [0.0; 2];
    sys.acceleration(t, x, v, &mut a).unwrap();
    let g = rdotv(t, x, v);
    let dg = v[0].mul_add(v[0], v[1] * v[1]) + x[0].mul_add(a[0], x[1] * a[1]);
    (g / dg).abs()
}

const ORBITS: i32 = 100; // lint: allow-literal(number of orbits)
/// Every 10th event is re-integrated for the location check.
const RECHECK_EVERY: usize = 10; // lint: allow-literal(sampling of the expensive check)

/// Verifies: NUM-010, NUM-001
#[test]
fn perihelion_events_are_located_on_the_dense_output() {
    let vs = VectorSet::load_canonical().unwrap();
    let o = orbit::<f64>(&vs);
    let (events, steps) = perihelia(&o, ORBITS);
    assert_eq!(events.len(), usize::try_from(ORBITS).unwrap());
    let mut worst = 0.0_f64;
    for t in events.iter().step_by(RECHECK_EVERY) {
        worst = worst.max(root_offset(&o, *t));
    }
    let analytic = |k: usize| o.period * (0.5 + f64::from(u32::try_from(k).unwrap()));
    let drift = events
        .iter()
        .enumerate()
        .map(|(k, t)| (t - analytic(k)).abs())
        .fold(0.0, f64::max);
    println!(
        "f64: {} events in {steps} steps; location error (dense root vs integrated trajectory) max {worst:.3e} s; \
         event - analytic max {drift:.3e} s (round-off, reported)",
        events.len()
    );
    vs.get("NUM010-EVENT-LOCATION")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// Verifies: NUM-010, DEP-007, NUM-006
#[test]
fn kepler_perihelion_times_match_analytic_in_double_double() {
    let vs = VectorSet::load_canonical().unwrap();
    let o = orbit::<DoubleDouble>(&vs);
    let (events, steps) = perihelia(&o, ORBITS);
    assert_eq!(events.len(), usize::try_from(ORBITS).unwrap());
    let half = DoubleDouble::HALF;
    let worst = events
        .iter()
        .enumerate()
        .map(|(k, &t)| {
            let kk = DoubleDouble::from_i32(i32::try_from(k).unwrap());
            (t - o.period * (kk + half)).abs().to_f64()
        })
        .fold(0.0, f64::max);
    println!(
        "double-double: {} events in {steps} steps; event - analytic max {worst:.3e} s",
        events.len()
    );
    vs.get("NUM010-KEPLER-PERIHELION-TIMES")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// Dense output is opt-in and refuses times outside the last step.
///
/// Verifies: NUM-010, API-002
#[test]
fn dense_output_is_checked() {
    use ferroastro_core::ErrorCode;
    let sys = Kepler { mu: 1.0 };
    let mut off = Ias15::new(
        2,
        0.0,
        &[1.0, 0.0],
        &[0.0, 1.0],
        DT_UNIT,
        Ias15Config::default(),
    )
    .unwrap();
    off.step(&sys, f64::INFINITY).unwrap();
    let (mut x, mut v) = ([0.0; 2], [0.0; 2]);
    let e = off.dense_state(0.05, &mut x, &mut v).unwrap_err(); // lint: allow-literal(inside the first step)
    assert_eq!(e.code(), ErrorCode::IntegratorConfig);
    let cfg = Ias15Config {
        dense_output: true,
        ..Ias15Config::default()
    };
    let mut on = Ias15::new(2, 0.0, &[1.0, 0.0], &[0.0, 1.0], DT_UNIT, cfg).unwrap();
    on.step(&sys, f64::INFINITY).unwrap();
    let (t0, t1) = on.last_step_span().unwrap();
    on.dense_state(t1, &mut x, &mut v).unwrap();
    assert!((x[0] - on.positions()[0]).abs() <= 4.0 * f64::EPSILON);
    assert!(on.dense_state(t1 + (t1 - t0), &mut x, &mut v).is_err());
    on.dense_state(t0, &mut x, &mut v).unwrap();
    assert!((x[0] - 1.0).abs() <= f64::EPSILON && x[1].abs() <= f64::EPSILON);
}
