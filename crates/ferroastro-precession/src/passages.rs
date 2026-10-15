// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Perihelion passages of a single-body (heliocentric, 3-coordinate) run and the per-orbit
//! advance fitted from them (ARCHITECTURE `ferroastro-precession`: event location `r·v = 0`).
//!
//! A passage is a root of `f(t) = r·v` with `f` rising (minimum of `r`). It is bracketed by two
//! accepted integrator steps and refined by Newton iterations on `f`, with `f' = v² + r·a`;
//! each iteration re-integrates from the start of the bracketing step, so the main run is never
//! disturbed. The **perihelion longitude** is the angle of `r` at the passage in a fixed
//! [`OrbitPlane`] (the initial osculating plane, zero at the initial periapsis). This is the
//! geometric part of the Park et al. 2017 observable (§3.1 of Journal #4) without the mean-orbit
//! frame and periodic fit, which a single-body run does not need.
//!
//! The per-orbit advance is the ordinary-least-squares slope of the longitude against the
//! passage index; the anomalistic period is the slope of the passage time against the index.

use crate::PrecessionError;
use crate::kepler::{cross, dot, norm};
use crate::lrl::{PerihelionTracker, eccentricity_vector};
use ferroastro_core::units::{Angle, AngularRate, TimeSpan};
use ferroastro_integrators::diagnostics::fit_line;
use ferroastro_integrators::{Ias15, Ias15Config, Ias15Stats, SecondOrderSystem};
use std::f64::consts::TAU;

/// The definition string of this estimator, for reports (NUM-012).
pub const ESTIMATOR: &str = "perihelion passages (r.v = 0, r.v rising; Newton-refined); longitude of r \
     at each passage in the initial osculating orbit plane, zero at the initial periapsis";

/// The fit model of [`fit_per_orbit`], for reports (PHY-015).
pub const FIT_MODEL: &str = "OLS: longitude_k = L0 + (advance per orbit) * k and t_k = T0 + \
     (anomalistic period) * k over consecutive passages k = 0..N; rate = advance / period";

/// One perihelion passage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Passage {
    /// Time (s from the start of the run).
    pub t: f64,
    /// Heliocentric position (km).
    pub r: [f64; 3],
    /// Heliocentric velocity (km/s).
    pub v: [f64; 3],
}

/// A fixed reference plane with an in-plane zero direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitPlane {
    p_hat: [f64; 3],
    q_hat: [f64; 3],
}

impl OrbitPlane {
    /// The osculating orbit plane of `(r, v)` with gravitational parameter `μ` (km³/s²), zero
    /// direction at the osculating periapsis.
    ///
    /// # Errors
    /// `FA-E0600` for `μ ≤ 0`, zero angular momentum or a circular orbit.
    pub fn osculating(r: [f64; 3], v: [f64; 3], mu: f64) -> Result<Self, PrecessionError> {
        if mu.is_nan() || mu <= 0.0 {
            return Err(PrecessionError::Domain {
                what: "gravitational parameter (km^3/s^2)",
                value: mu,
            });
        }
        let h = cross(r, v);
        let hn = norm(h);
        let e = eccentricity_vector(r, v, mu);
        let en = norm(e);
        if hn.is_nan() || en.is_nan() || hn <= 0.0 || en <= f64::EPSILON.sqrt() {
            return Err(PrecessionError::Domain {
                what: "eccentricity (periapsis undefined)",
                value: en,
            });
        }
        let p_hat = e.map(|c| c / en);
        let q_hat = cross(h.map(|c| c / hn), p_hat);
        Ok(Self { p_hat, q_hat })
    }

    /// The angle (rad, in `(−π, π]`) of the projection of `x` onto the plane.
    #[must_use]
    pub fn angle(&self, x: [f64; 3]) -> f64 {
        dot(x, self.q_hat).atan2(dot(x, self.p_hat))
    }
}

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
/// `passages` perihelion passages are found or `max_time_s` is reached.
///
/// `mu` is the gravitational parameter of the LRL cross-check (km³/s²).
///
/// # Errors
/// Integrator and force-model errors (as `FA-E0601`), `FA-E0600` for a degenerate initial
/// orbit, and `FA-E0601` if fewer than `passages` passages occur before `max_time_s`.
pub fn find_perihelia<S: SecondOrderSystem + ?Sized>(
    sys: &S,
    r0: [f64; 3],
    v0: [f64; 3],
    mu: f64,
    passages: usize,
    max_time_s: f64,
    cfg: Ias15Config,
) -> Result<PassageRun, PrecessionError> {
    let mut tracker = PerihelionTracker::new(r0, v0, mu)?;
    let dt0 = norm(r0) / norm(v0) / 16.0;
    let mut ias = Ias15::new(3, 0.0, &r0, &v0, dt0, cfg)?;
    let mut out = PassageRun {
        passages: Vec::with_capacity(passages),
        stats: Ias15Stats::default(),
        max_residual: 0.0,
        lrl_times: vec![0.0],
        lrl_longitudes: vec![tracker.longitude(r0, v0)],
    };
    let mut prev = (0.0, r0, v0);
    while out.passages.len() < passages {
        let rep = ias.step(sys, max_time_s)?;
        let (r, v) = (to3(ias.positions())?, to3(ias.velocities())?);
        if rep.dt_done != 0.0 {
            out.lrl_times.push(rep.t);
            out.lrl_longitudes.push(tracker.longitude(r, v));
            if dot(prev.1, prev.2) < 0.0 && dot(r, v) >= 0.0 {
                let p = refine(sys, prev, rep.t, cfg)?;
                let resid = dot(p.r, p.v).abs() / (norm(p.r) * norm(p.v));
                out.max_residual = out.max_residual.max(resid);
                out.passages.push(p);
            }
            prev = (rep.t, r, v);
        }
        if rep.reached_limit && out.passages.len() < passages {
            return Err(PrecessionError::Fit(format!(
                "only {} of {passages} perihelion passages before t = {max_time_s} s",
                out.passages.len()
            )));
        }
    }
    out.stats = ias.stats();
    Ok(out)
}

fn to3(x: &[f64]) -> Result<[f64; 3], PrecessionError> {
    <[f64; 3]>::try_from(x)
        .map_err(|_| PrecessionError::Fit("perihelion search needs a 3-coordinate system".into()))
}

/// Locate the root of `r·v` in `(t0, t1]`, starting from the state at `t0`.
fn refine<S: SecondOrderSystem + ?Sized>(
    sys: &S,
    start: (f64, [f64; 3], [f64; 3]),
    t1: f64,
    cfg: Ias15Config,
) -> Result<Passage, PrecessionError> {
    let (t0, r0, v0) = start;
    let state_at = |t: f64| -> Result<([f64; 3], [f64; 3]), PrecessionError> {
        if (t - t0).abs() <= 0.0 {
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
    let mut best = Passage {
        t: t1,
        r: r1,
        v: v1,
    };
    for _ in 0..12 {
        let (r, v) = state_at(t)?;
        best = Passage { t, r, v };
        let mut a = [0.0; 3];
        sys.acceleration(t, &r, &v, &mut a)?;
        let f = dot(r, v);
        let fp = dot(v, v) + dot(r, a);
        if fp.is_nan() || fp <= 0.0 {
            break;
        }
        let next = (t - f / fp).clamp(t0, t1);
        if (next - t).abs() <= 4.0 * f64::EPSILON * t.abs().max(t1 - t0) {
            break;
        }
        t = next;
    }
    Ok(best)
}

/// The per-orbit advance fitted from perihelion passages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerOrbitFit {
    /// Advance of the perihelion longitude per anomalistic orbit.
    pub advance: Angle,
    /// Formal 1σ error of [`PerOrbitFit::advance`] (understates correlated round-off).
    pub advance_stderr: Angle,
    /// Anomalistic period (mean time between passages).
    pub period: TimeSpan,
    /// Number of orbits spanned (passages − 1).
    pub orbits: usize,
    /// RMS residual of the longitude fit.
    pub rms_residual: Angle,
    /// Time span from the first to the last passage.
    pub span: TimeSpan,
}

impl PerOrbitFit {
    /// The secular rate, advance / period.
    #[must_use]
    pub fn rate(&self) -> AngularRate {
        self.advance / self.period
    }
}

/// Fit the per-orbit advance and the anomalistic period to `passages` measured in `plane`.
///
/// # Errors
/// `FA-E0601` for fewer than 3 passages or a degenerate fit.
pub fn fit_per_orbit(
    passages: &[Passage],
    plane: &OrbitPlane,
) -> Result<PerOrbitFit, PrecessionError> {
    if passages.len() < 3 {
        return Err(PrecessionError::Fit(format!(
            "{} perihelion passages; at least 3 are needed",
            passages.len()
        )));
    }
    let mut index = Vec::with_capacity(passages.len());
    let mut lon = Vec::with_capacity(passages.len());
    let mut times = Vec::with_capacity(passages.len());
    let mut last = 0.0;
    for (k, p) in passages.iter().enumerate() {
        let raw = plane.angle(p.r);
        let unwrapped = ((last - raw) / TAU).round().mul_add(TAU, raw);
        last = unwrapped;
        index.push(f64::from(u32::try_from(k).unwrap_or(u32::MAX)));
        lon.push(unwrapped);
        times.push(p.t);
    }
    let adv = fit_line(&index, &lon)?;
    let per = fit_line(&index, &times)?;
    let dom = |what, value| PrecessionError::Domain { what, value };
    let (first, last_t) = match (passages.first(), passages.last()) {
        (Some(a), Some(b)) => (a.t, b.t),
        _ => return Err(PrecessionError::Fit("no passages".into())),
    };
    Ok(PerOrbitFit {
        advance: Angle::from_radians(adv.slope).map_err(|_| dom("advance", adv.slope))?,
        advance_stderr: Angle::from_radians(adv.slope_stderr)
            .map_err(|_| dom("advance stderr", adv.slope_stderr))?,
        period: TimeSpan::from_seconds(per.slope).map_err(|_| dom("period", per.slope))?,
        orbits: passages.len() - 1,
        rms_residual: Angle::from_radians(adv.rms_residual)
            .map_err(|_| dom("residual", adv.rms_residual))?,
        span: TimeSpan::from_seconds(last_t - first).map_err(|_| dom("span", last_t - first))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_integrators::IntegratorError;

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
    /// Verifies: PHY-001, NUM-012
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
