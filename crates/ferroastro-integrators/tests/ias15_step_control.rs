// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! IAS15 step-size control against Rein & Spiegel 2015 Eq. 11,
//! `dt_next = dt · (ε / ε̃)^(1/7)` with `ε̃ = max|b₇| / max|a|`.
//!
//! The steady-state step count does **not** pin the exponent: any exponent `k` has the same fixed
//! point `(ε/C)^(1/7)` when `b₇ ∝ dt⁷` (WP6 measured step-count ratios 10.66 for `k = 1/7` and
//! 10.56 for `k = 1/6` between ε = 1e-3 and 1e-10). These tests therefore look at **one**
//! proposal: (a) from the same first step, the proposal scales as `ε^k`; (b) the proposal does
//! not depend on the length of the step just taken (`1 − 7k = 0`, a dead-beat controller).
//! The Kepler problem is dimensionless (GM = 1, a = 1).

#![expect(
    clippy::indexing_slicing,
    reason = "the test system indexes fixed-length state slices of its own dimension"
)]

use ferroastro_integrators::{Ias15, Ias15Config, IntegratorError, SecondOrderSystem};
use ferroastro_vectors::VectorSet;

/// Eccentricity of the test orbit (R&S 2015 Kepler test orbit); start at pericentre.
const E: f64 = 0.5;
/// Arbitrary first trial step from which the controller finds its own step `h`.
const DT_START: f64 = 0.01; // lint: allow-literal(dimensionless first trial step)
/// ε ratio of test (a): 2⁻⁷, so the proposal ratio 1/2 stays inside the 0.25–4 acceptance band.
const EPS_FACTOR: f64 = 1.0 / 128.0;

/// Planar Kepler problem with GM = 1.
struct Kepler;

impl SecondOrderSystem for Kepler {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(
        &self,
        _t: f64,
        x: &[f64],
        _v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        let r2 = x[0].mul_add(x[0], x[1] * x[1]);
        let k = -1.0 / (r2 * r2.sqrt());
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

/// Length of the second accepted step after a first step of exactly `dt0` (i.e. the
/// controller's proposal after that first step).
fn proposal(dt0: f64, epsilon: f64) -> f64 {
    let x0 = [1.0 - E, 0.0];
    let v0 = [0.0, ((1.0 + E) / (1.0 - E)).sqrt()];
    let cfg = Ias15Config {
        epsilon,
        ..Ias15Config::default()
    };
    let mut ias = Ias15::new(2, 0.0, &x0, &v0, dt0, cfg).unwrap();
    let first = ias.step(&Kepler, f64::INFINITY).unwrap();
    assert!(
        (first.dt_done - dt0).abs() <= 0.0,
        "first step was rejected or clamped: {} vs {dt0}",
        first.dt_done
    );
    ias.step(&Kepler, f64::INFINITY).unwrap().dt_done
}

/// R&S 2015 Eq. 11: from an identical first step, the proposal scales as ε^(1/7).
///
/// Verifies: NUM-001
#[test]
fn step_proposal_scales_as_epsilon_to_one_seventh() {
    let vs = VectorSet::load_workspace().unwrap();
    let eps = Ias15Config::default().epsilon;
    let h = proposal(DT_START, eps);
    let p1 = proposal(h, eps);
    let p2 = proposal(h, eps * EPS_FACTOR);
    let k = (p2 / p1).ln() / EPS_FACTOR.ln();
    let v = vs.get("IAS15-EQ11-EPS-EXPONENT").unwrap();
    println!(
        "IAS15 Eq. 11 exponent: measured {k:.17} (expected {}; tolerance {:e}); h = {h:e}, \
         p(eps) = {p1:e}, p(eps/128) = {p2:e}",
        v.value().unwrap(),
        v.tolerance.allowed(v.value().unwrap())
    );
    v.check(k).unwrap();
}

/// R&S 2015 Eq. 11 with b₇ ∝ dt⁷: the proposal does not depend on the step just taken.
///
/// Verifies: NUM-001
#[test]
fn step_proposal_is_dead_beat() {
    let vs = VectorSet::load_workspace().unwrap();
    let eps = Ias15Config::default().epsilon;
    let h = proposal(DT_START, eps);
    let p1 = proposal(h, eps);
    let p2 = proposal(2.0 * h, eps);
    let slope = (p2 / p1).log2();
    let v = vs.get("IAS15-EQ11-DEADBEAT").unwrap();
    println!(
        "IAS15 dead-beat check: d ln p / d ln dt = {slope:+.5} (expected 0 +/- {:e}; \
         1 - 7k would be -0.167 for k = 1/6); h = {h:e}, p(h) = {p1:e}, p(2h) = {p2:e}",
        v.tolerance.allowed(0.0)
    );
    v.check(slope).unwrap();
}
