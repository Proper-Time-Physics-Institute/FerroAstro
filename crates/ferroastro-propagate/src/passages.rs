// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The IAS15-driven perihelion-passage run of a single-body (test-particle) problem: moved from
//! `ferroastro-precession` to `ferroastro-ladder` in WP6 (estimators contain no propagation), and
//! from `ferroastro-ladder::passage_run` to this crate in WP13, so that scenario-driven
//! test-particle runs and the ladder's direct-API rungs share one code path (bit-identical
//! results, WP13 check 5). `ferroastro_ladder::passage_run` re-exports it unchanged.
//!
//! A passage is a root of `f(t) = r·v` with `f` rising (minimum of `r`). It is bracketed by two
//! accepted integrator steps and refined by Newton iterations on `f`, with `f' = v² + r·a`;
//! each iteration re-integrates from the start of the bracketing step, so the main run is never
//! disturbed. The passages are then handed to the estimator
//! [`ferroastro_precession::passages::fit_per_orbit`].
//!
//! The same run also samples the LRL eccentricity-vector longitude after every accepted step
//! (the NUM-012 cross-check estimator), and, if asked, the state on a uniform time grid (input
//! of the Park2017 estimator, NUM-011/NUM-012).

use crate::PropagateError;
use ferroastro_analytic::vector::{dot, norm};
use ferroastro_integrators::{Ias15, Ias15Config, Ias15Stats, SecondOrderSystem};
use ferroastro_precession::PrecessionError;
use ferroastro_precession::lrl::PerihelionTracker;
use ferroastro_precession::passages::Passage;
use ferroastro_real::Real;

/// Output of [`find_perihelia`].
#[derive(Debug, Clone)]
pub struct PassageRun {
    /// The passages, in time order.
    pub passages: Vec<Passage>,
    /// Integrator counters of the main run (refinements excluded).
    pub stats: Ias15Stats,
    /// Largest |r·v| / (|r||v|) left at a refined passage (refinement quality).
    pub max_residual: f64,
    /// Times (s) of the LRL cross-check samples (after every accepted step).
    pub lrl_times: Vec<f64>,
    /// Unwrapped LRL eccentricity-vector longitudes (rad) in the initial osculating plane.
    pub lrl_longitudes: Vec<f64>,
}

/// Integrate `sys` (3 coordinates: heliocentric position) from `(r0, v0)` at `t = 0` until
/// `passages` perihelion passages are found or `max_time_s` is reached, in backend `R` (the
/// integration and the passage refinement run in `R`; the passages and the LRL samples handed to
/// the f64 estimators are rounded to f64 at the end).
///
/// `mu` is the gravitational parameter of the LRL cross-check (km³/s²).
///
/// # Errors
/// Integrator and force-model errors, `FA-E0600` for a degenerate initial orbit, and
/// `FA-E0601` if fewer than `passages` passages occur before `max_time_s`.
pub fn find_perihelia<R: Real, S: SecondOrderSystem<R> + ?Sized>(
    sys: &S,
    r0: [R; 3],
    v0: [R; 3],
    mu: R,
    passages: usize,
    max_time_s: R,
    cfg: Ias15Config,
) -> Result<PassageRun, PropagateError> {
    let run = perihelia_until(sys, r0, v0, mu, Some(passages), max_time_s, cfg)?;
    if run.passages.len() < passages {
        return Err(PropagateError::from(PrecessionError::Fit(format!(
            "only {} of {passages} perihelion passages before t = {} s",
            run.passages.len(),
            max_time_s.to_f64()
        ))));
    }
    Ok(run)
}

/// As [`find_perihelia`], but stop at `max_time_s` without an error, or earlier once
/// `passages` passages (if given) are found. The step sequence is that of [`find_perihelia`],
/// so both return bit-identical passages for the same inputs.
///
/// # Errors
/// Integrator and force-model errors, `FA-E0600` for a degenerate initial orbit.
pub fn perihelia_until<R: Real, S: SecondOrderSystem<R> + ?Sized>(
    sys: &S,
    r0: [R; 3],
    v0: [R; 3],
    mu: R,
    passages: Option<usize>,
    max_time_s: R,
    cfg: Ias15Config,
) -> Result<PassageRun, PropagateError> {
    let f = |x: [R; 3]| x.map(R::to_f64);
    let mut tracker = PerihelionTracker::new(f(r0), f(v0), mu.to_f64())?;
    let dt0 = norm(r0) / norm(v0) / R::from_f64(16.0);
    let mut ias = Ias15::new(3, R::ZERO, &r0, &v0, dt0, cfg)?;
    let mut out = PassageRun {
        passages: Vec::with_capacity(passages.unwrap_or(0)),
        stats: Ias15Stats::default(),
        max_residual: 0.0,
        lrl_times: vec![0.0],
        lrl_longitudes: vec![tracker.longitude(f(r0), f(v0))],
    };
    let mut prev = (R::ZERO, r0, v0);
    while passages.is_none_or(|n| out.passages.len() < n) {
        let rep = ias.step(sys, max_time_s)?;
        let (r, v) = (to3(ias.positions())?, to3(ias.velocities())?);
        if rep.dt_done != R::ZERO {
            out.lrl_times.push(rep.t.to_f64());
            out.lrl_longitudes.push(tracker.longitude(f(r), f(v)));
            if dot(prev.1, prev.2) < R::ZERO && dot(r, v) >= R::ZERO {
                let (t, pr, pv) = refine(sys, prev, rep.t, cfg)?;
                let resid = (dot(pr, pv).abs() / (norm(pr) * norm(pv))).to_f64();
                out.max_residual = out.max_residual.max(resid);
                out.passages.push(Passage {
                    t: t.to_f64(),
                    r: f(pr),
                    v: f(pv),
                });
            }
            prev = (rep.t, r, v);
        }
        if rep.reached_limit {
            break;
        }
    }
    out.stats = ias.stats();
    Ok(out)
}

fn to3<R: Real>(x: &[R]) -> Result<[R; 3], PropagateError> {
    <[R; 3]>::try_from(x).map_err(|_| {
        PropagateError::from(PrecessionError::Fit(
            "perihelion search needs a 3-coordinate system".into(),
        ))
    })
}

/// States on a uniform time grid: times (s from the start), positions (km), velocities (km/s).
pub type UniformSamples = (Vec<f64>, Vec<[f64; 3]>, Vec<[f64; 3]>);

/// Integrate `sys` from `(r0, v0)` at `t = 0` and record the state every `step_s` seconds,
/// `samples` states in all (the first is the initial state). The input of sampled-data
/// estimators such as Park2017 (NUM-011/NUM-012). Every grid time is hit exactly
/// ([`Ias15::integrate_to`] shortens the step that would overshoot it), so this trajectory
/// differs from [`find_perihelia`]'s at round-off level.
///
/// # Errors
/// Integrator and force-model errors; `FA-E0601` for a non-positive step or fewer than 2 samples.
pub fn sample_uniform<S: SecondOrderSystem + ?Sized>(
    sys: &S,
    r0: [f64; 3],
    v0: [f64; 3],
    step_s: f64,
    samples: usize,
    cfg: Ias15Config,
) -> Result<UniformSamples, PropagateError> {
    if !(step_s.is_finite() && step_s > 0.0) || samples < 2 {
        return Err(PropagateError::from(PrecessionError::Fit(format!(
            "uniform sampling needs step > 0 and >= 2 samples (step {step_s} s, {samples} samples)"
        ))));
    }
    let mut ias = Ias15::new(3, 0.0, &r0, &v0, norm(r0) / norm(v0) / 16.0, cfg)?;
    let (mut t, mut r, mut v) = (
        Vec::with_capacity(samples),
        Vec::with_capacity(samples),
        Vec::with_capacity(samples),
    );
    t.push(0.0);
    r.push(r0);
    v.push(v0);
    for k in 1..samples {
        let tk = step_s * f64::from(u32::try_from(k).unwrap_or(u32::MAX));
        ias.integrate_to(sys, tk, |_, _| Ok(()))?;
        t.push(tk);
        r.push(to3(ias.positions())?);
        v.push(to3(ias.velocities())?);
    }
    Ok((t, r, v))
}

/// Number of samples on the grid `0, step, 2·step, …` within a span of `span_s` seconds (whole
/// steps, with a few ulp of slack for spans that are an exact multiple), the initial one included.
#[must_use]
pub fn grid_samples(span_s: f64, step_s: f64) -> usize {
    let limit = span_s * 4.0f64.mul_add(f64::EPSILON, 1.0);
    let mut intervals: u32 = 0;
    while step_s > 0.0 && f64::from(intervals + 1) * step_s <= limit {
        intervals += 1;
    }
    usize::try_from(intervals).unwrap_or(usize::MAX) + 1
}

/// A point of the trajectory: time (s), position (km), velocity (km/s).
type Point<R> = (R, [R; 3], [R; 3]);

/// Locate the root of `r·v` in `(t0, t1]`, starting from the state at `t0` (in backend `R`).
fn refine<R: Real, S: SecondOrderSystem<R> + ?Sized>(
    sys: &S,
    start: Point<R>,
    t1: R,
    cfg: Ias15Config,
) -> Result<Point<R>, PropagateError> {
    let (t0, r0, v0) = start;
    let state_at = |t: R| -> Result<([R; 3], [R; 3]), PropagateError> {
        if (t - t0).abs() <= R::ZERO {
            return Ok((r0, v0));
        }
        let mut ias = Ias15::new(3, t0, &r0, &v0, t - t0, cfg)?;
        ias.integrate_to(sys, t, |_, _| Ok(()))?;
        Ok((to3(ias.positions())?, to3(ias.velocities())?))
    };
    // Start from the linear interpolation of r·v between the bracket ends.
    let (r1, v1) = state_at(t1)?;
    let (f0, f1) = (dot(r0, v0), dot(r1, v1));
    let mut t = if f1 > f0 {
        t0 + (t1 - t0) * (-f0) / (f1 - f0)
    } else {
        t1
    };
    let mut best = (t1, r1, v1);
    let four_eps = R::from_f64(4.0) * R::EPSILON;
    for _ in 0..12 {
        let (r, v) = state_at(t)?;
        best = (t, r, v);
        let mut a = [R::ZERO; 3];
        sys.acceleration(t, &r, &v, &mut a)?;
        let f = dot(r, v);
        let fp = dot(v, v) + dot(r, a);
        if fp.is_nan() || fp <= R::ZERO {
            break;
        }
        let next = (t - f / fp).max(t0).min(t1);
        if (next - t).abs() <= four_eps * t.abs().max(t1 - t0) {
            break;
        }
        t = next;
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_integrators::IntegratorError;
    use ferroastro_precession::passages::{OrbitPlane, fit_per_orbit};
    use std::f64::consts::TAU;

    /// Kepler problem with μ = 1 in the plane, no precession.
    struct Kepler;
    impl SecondOrderSystem for Kepler {
        fn dimension(&self) -> usize {
            3
        }
        fn acceleration(
            &self,
            _t: f64,
            x: &[f64],
            _v: &[f64],
            a: &mut [f64],
        ) -> Result<(), IntegratorError> {
            let r2: f64 = x.iter().map(|z| z * z).sum();
            let k = -1.0 / (r2 * r2.sqrt());
            for (ai, xi) in a.iter_mut().zip(x) {
                *ai = k * xi;
            }
            Ok(())
        }
    }

    /// Kepler orbit (a = 1, e = 1/2, μ = 1): passages every 2π at angle 0 with r·v = 0.
    ///
    /// Verifies: NUM-012
    #[test]
    fn kepler_passages_have_period_two_pi_and_no_advance() {
        // Start at apoapsis r = 3/2, speed sqrt((1-e)/(1+e)) = sqrt(1/3).
        let r0 = [-1.5, 0.0, 0.0]; // lint: allow-literal(dimensionless apoapsis distance a(1+e))
        let v0 = [0.0, -(1.0_f64 / 3.0).sqrt(), 0.0];
        let run = find_perihelia(&Kepler, r0, v0, 1.0, 11, 1.0e3, Ias15Config::default()).unwrap(); // lint: allow-literal(time limit far beyond 10 orbits)
        assert_eq!(run.passages.len(), 11);
        let first = run.passages[0];
        assert!((first.t - std::f64::consts::PI).abs() < 1e-11); // lint: allow-literal(integration accuracy bound)
        assert!(run.max_residual < 1e-14); // lint: allow-literal(root-refinement bound, ~100 ulp)
        let plane = OrbitPlane::osculating(r0, v0, 1.0).unwrap();
        let fit = fit_per_orbit(&run.passages, &plane).unwrap();
        assert_eq!(fit.orbits, 10);
        assert!((fit.period.seconds() - TAU).abs() < 1e-11); // lint: allow-literal(integration accuracy bound)
        assert!(fit.advance.radians().abs() < 1e-13); // lint: allow-literal(integration accuracy bound)
        assert!(fit_per_orbit(&run.passages[..2], &plane).is_err());
    }
}
