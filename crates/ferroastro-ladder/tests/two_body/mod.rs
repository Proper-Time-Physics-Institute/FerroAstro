// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared set-up for the Sun + Mercury rungs: constants from the DE440 table, SW92 elements from
//! the test vectors, barycentric initial conditions, and an LRL-tracked IAS15 run.

#![allow(
    dead_code,
    reason = "each test binary uses a different subset of the helpers"
)]
#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test-only helpers: a failed unwrap or index is a test failure with a clear message"
)]

use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::{Barycentric, EclipticJ2000, Heliocentric, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_core::units::{Angle, GravParam, Length, Velocity};
use ferroastro_forces::NewtonianPointMasses;
use ferroastro_integrators::{Ias15, Ias15Config, Ias15Stats};
use ferroastro_precession::lrl::PerihelionTracker;
use ferroastro_vectors::VectorSet;

/// The Newtonian Sun + Mercury model in the J2000 ecliptic frame about the barycentre.
pub(crate) type Model = NewtonianPointMasses<EclipticJ2000, Barycentric>;

/// One Julian year in seconds.
pub(crate) fn julian_year_s() -> f64 {
    DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY
}

/// Load a vector value by id.
pub(crate) fn value(vs: &VectorSet, id: &str) -> f64 {
    vs.get(id).unwrap().value().unwrap()
}

fn deg(vs: &VectorSet, id: &str) -> Angle {
    Angle::from_radians(value(vs, id).to_radians()).unwrap()
}

/// SW92 J2000 mean elements of Mercury (a converted with the DE440 au).
pub(crate) fn sw92_elements(vs: &VectorSet, de: &De440) -> Elements {
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

/// `(GM_sun, GM_mercury)` from DE440.
pub(crate) fn gms(de: &De440) -> (GravParam, GravParam) {
    (de.gm_sun(), de.gm(Body::Mercury))
}

/// Split a heliocentric relative state into barycentric Sun and Mercury coordinates, flattened
/// as the force model expects (Sun first).
pub(crate) fn barycentric(
    rel: &State<EclipticJ2000, Heliocentric>,
    gm_sun: GravParam,
    gm_me: GravParam,
) -> (Vec<f64>, Vec<f64>) {
    let total = gm_sun.km3_per_s2() + gm_me.km3_per_s2();
    let (fs, fm) = (gm_me.km3_per_s2() / total, gm_sun.km3_per_s2() / total);
    let r = rel.position().components().map(Length::km);
    let v = rel.velocity().map(Velocity::km_per_s);
    let x: Vec<f64> = r
        .iter()
        .map(|z| -fs * z)
        .chain(r.iter().map(|z| fm * z))
        .collect();
    let w: Vec<f64> = v
        .iter()
        .map(|z| -fs * z)
        .chain(v.iter().map(|z| fm * z))
        .collect();
    (x, w)
}

/// Relative (Mercury − Sun) position and velocity from a flat two-body state.
pub(crate) fn relative(x: &[f64], v: &[f64]) -> ([f64; 3], [f64; 3]) {
    let r = std::array::from_fn(|k| x[3 + k] - x[k]);
    let w = std::array::from_fn(|k| v[3 + k] - v[k]);
    (r, w)
}

/// Output of an LRL-tracked run.
pub(crate) struct LrlRun {
    /// Sample times (s from the start).
    pub(crate) times: Vec<f64>,
    /// Unwrapped eccentricity-vector longitudes (rad).
    pub(crate) longitudes: Vec<f64>,
    /// Integrator counters.
    pub(crate) stats: Ias15Stats,
    /// Final flat state.
    pub(crate) x: Vec<f64>,
    /// Final flat velocities.
    pub(crate) v: Vec<f64>,
}

/// Integrate the two-body model for `span_s` seconds with IAS15 and track the LRL longitude of
/// the relative orbit after every accepted step.
pub(crate) fn run_lrl(
    model: &Model,
    x0: &[f64],
    v0: &[f64],
    mu: f64,
    span_s: f64,
    cfg: Ias15Config,
) -> LrlRun {
    let (r0, w0) = relative(x0, v0);
    let mut tracker = PerihelionTracker::new(r0, w0, mu).unwrap();
    let mut times = vec![0.0];
    let mut longitudes = vec![tracker.longitude(r0, w0)];
    let mut ias = Ias15::new(6, 0.0, x0, v0, SECONDS_PER_DAY, cfg).unwrap();
    ias.integrate_to(model, span_s, |s, _| {
        let (r, w) = relative(s.positions(), s.velocities());
        times.push(s.time());
        longitudes.push(tracker.longitude(r, w));
        Ok(())
    })
    .unwrap();
    LrlRun {
        times,
        longitudes,
        stats: ias.stats(),
        x: ias.positions().to_vec(),
        v: ias.velocities().to_vec(),
    }
}
