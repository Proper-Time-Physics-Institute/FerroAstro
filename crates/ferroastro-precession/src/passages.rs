// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The per-orbit advance fitted from perihelion passages of a single-body (heliocentric,
//! 3-coordinate) run.
//!
//! A passage is a root of `f(t) = r·v` with `f` rising (minimum of `r`). This module only
//! **consumes** passages: locating them in a propagated run (Newton refinement on `f` with
//! `f' = v² + r·a`, re-integrating from the bracketing step) is done by the run driver in
//! `ferroastro-ladder` (CRATE-PLAN: estimators contain no propagation). The **perihelion longitude** is the angle of `r` at the passage in a fixed
//! [`OrbitPlane`] (the initial osculating plane, zero at the initial periapsis). This is the
//! geometric part of the Park et al. 2017 observable (§3.1 of Journal #4) without the mean-orbit
//! frame and periodic fit, which a single-body run does not need.
//!
//! The per-orbit advance is the ordinary-least-squares slope of the longitude against the
//! passage index; the anomalistic period is the slope of the passage time against the index.

use crate::PrecessionError;
use crate::fit::fit_line;
use ferroastro_analytic::vector::{cross, dot, eccentricity_vector, norm};
use ferroastro_core::units::{Angle, AngularRate, TimeSpan};
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

    /// Passages on a Kepler ellipse (a = 1, e = 1/2, μ = 1) that advance by a fixed angle per
    /// orbit: the fit returns that advance and the period exactly, and refuses two passages.
    ///
    /// Verifies: NUM-012
    #[test]
    fn fit_recovers_a_prescribed_advance() {
        // Periapsis r = 1/2, speed sqrt(3) (dimensionless test orbit).
        let (r0, v0) = ([0.5, 0.0, 0.0], [0.0, 3.0_f64.sqrt(), 0.0]);
        let plane = OrbitPlane::osculating(r0, v0, 1.0).unwrap();
        let step = 1.0e-3; // lint: allow-literal(prescribed synthetic advance, rad/orbit)
        let passages: Vec<Passage> = (0..11_u32)
            .map(|k| {
                let (s, c) = (step * f64::from(k)).sin_cos();
                Passage {
                    t: TAU * f64::from(k),
                    r: [0.5 * c, 0.5 * s, 0.0],
                    v: [-(3.0_f64.sqrt()) * s, 3.0_f64.sqrt() * c, 0.0],
                }
            })
            .collect();
        let fit = fit_per_orbit(&passages, &plane).unwrap();
        assert_eq!(fit.orbits, 10);
        assert!((fit.advance.radians() - step).abs() < 1e-15); // lint: allow-literal(round-off)
        assert!((fit.period.seconds() - TAU).abs() < 1e-12); // lint: allow-literal(round-off)
        assert!(fit_per_orbit(&passages[..2], &plane).is_err());
        assert!(OrbitPlane::osculating(r0, v0, 0.0).is_err());
    }
}
