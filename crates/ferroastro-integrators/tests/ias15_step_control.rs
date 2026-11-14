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
//! (c) The step-rejection rule kept by the principal on issue #20 (2026-11-14):
//! reject when `dt_required < safety·dt_trial`. A step in the band where this rule and R&S 2015's
//! `dt_required < dt_trial` disagree is accepted as tried.
//! The Kepler problem is dimensionless (GM = 1, a = 1).

#![expect(
    clippy::indexing_slicing,
    reason = "the test system indexes fixed-length state slices of its own dimension"
)]

use ferroastro_integrators::{Ias15, Ias15Config, Ias15Stats, IntegratorError, SecondOrderSystem};
use ferroastro_vectors::VectorSet;

/// Eccentricity of the test orbit (R&S 2015 Kepler test orbit); start at pericentre.
const E: f64 = 0.5;
/// Arbitrary first trial step from which the controller finds its own step `h`.
const DT_START: f64 = 0.01; // lint: allow-literal(dimensionless first trial step)
/// log₂ of the ε ratio of test (a): 2⁻⁷, so the proposal ratio 1/2 stays inside the 0.25–4 acceptance band.
const EPS_FACTOR_LOG2: i32 = -7;

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
#[expect(
    clippy::unwrap_used,
    reason = "test helper: an integrator error is a test failure"
)]
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
    let p2 = proposal(h, eps * 2.0_f64.powi(EPS_FACTOR_LOG2));
    let k = (p2 / p1).log2() / f64::from(EPS_FACTOR_LOG2);
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

/// One step from pericentre with first trial step `dt0`: the report, the counters, and the step
/// the controller proposes next.
#[expect(
    clippy::unwrap_used,
    reason = "test helper: an integrator error is a test failure"
)]
fn first_step(dt0: f64) -> (f64, Ias15Stats, f64) {
    let x0 = [1.0 - E, 0.0];
    let v0 = [0.0, ((1.0 + E) / (1.0 - E)).sqrt()];
    let mut ias = Ias15::new(2, 0.0, &x0, &v0, dt0, Ias15Config::default()).unwrap();
    let report = ias.step(&Kepler, f64::INFINITY).unwrap();
    (report.dt_done, ias.stats(), ias.next_dt())
}

/// Issue #20 (principal decision of 2026-11-14): the rejection rule is
/// `|dt_required| < safety·|dt_trial|`, not R&S 2015's stated `dt_required < dt_trial`.
///
/// The two rules disagree exactly when `safety·dt_trial ≤ dt_required < dt_trial`. Eq. 11 is
/// dead-beat (`step_proposal_is_dead_beat`), so a trial step of `(1 + 1/safety)/2` times the
/// controller's own step `h` from the same state gets `dt_required ≈ h`, in the middle of that
/// band. The test checks that the step lands in the band, then that it is accepted as tried:
/// no rejection, `dt_done == dt_trial` bit for bit. Under `dt_required < dt_trial` it would be
/// rejected and redone with a shorter step.
///
/// Verifies: NUM-001
#[test]
fn step_between_safety_and_full_trial_step_is_accepted() {
    let safety = Ias15Config::default().safety;
    let h = proposal(DT_START, Ias15Config::default().epsilon);
    let trial = h * (1.0 + 1.0 / safety) / 2.0;
    let (dt_done, stats, dt_required) = first_step(trial);
    println!(
        "IAS15 rejection band (#20): h = {h:e}, dt_trial = {trial:e}, dt_required = \
         {dt_required:e} (ratio {:.4}; band [{safety}, 1)), dt_done = {dt_done:e}, rejected = {}",
        dt_required / trial,
        stats.rejected
    );
    assert!(
        dt_required >= safety * trial && dt_required < trial,
        "test set-up: dt_required/dt_trial = {} is outside [{safety}, 1), where the rules differ",
        dt_required / trial
    );
    assert_eq!(stats.rejected, 0, "step in the band was rejected");
    assert_eq!(stats.steps, 1);
    assert!(
        (dt_done - trial).abs() <= 0.0,
        "step in the band was not taken as tried: {dt_done} vs {trial}"
    );
}

/// Issue #20 companion: both rules reject a step whose `dt_required` is below `safety·dt_trial`
/// (here a trial step of `2h/safety`), so the band test above is not passed by a controller that
/// never rejects.
///
/// Verifies: NUM-001
#[test]
fn step_below_safety_times_trial_step_is_rejected() {
    let safety = Ias15Config::default().safety;
    let h = proposal(DT_START, Ias15Config::default().epsilon);
    let trial = 2.0 * h / safety;
    let (dt_done, stats, _) = first_step(trial);
    println!(
        "IAS15 rejection (#20): h = {h:e}, dt_trial = {trial:e}, dt_done = {dt_done:e}, \
         rejected = {}",
        stats.rejected
    );
    assert!(stats.rejected >= 1, "oversized step was not rejected");
    assert_eq!(stats.steps, 1);
    assert!(
        dt_done < safety * trial,
        "accepted step {dt_done} is not below safety·dt_trial = {}",
        safety * trial
    );
}
