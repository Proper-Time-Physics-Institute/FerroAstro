// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Laplace–Runge–Lenz (eccentricity-vector) perihelion longitude for single-planet runs.
//!
//! For the relative orbit with gravitational parameter `μ`, the eccentricity vector
//! `e = (v × h)/μ − r/|r|` points to periapsis and is exactly conserved by Newtonian two-body
//! motion. [`PerihelionTracker`] measures its direction in a fixed plane: the **initial osculating
//! orbit plane** (axes fixed in the integration frame), with the initial periapsis direction as
//! zero. Its secular drift is the perihelion precession of the run; for a two-body Newtonian run
//! any drift is integrator error (PHY-001).
//!
//! Units: `r` in km, `v` in km/s, `μ` in km³/s²; the longitude is returned in radians.

use crate::PrecessionError;
pub use ferroastro_analytic::vector::eccentricity_vector;
use ferroastro_analytic::vector::{cross, dot, norm};

/// Tracks the longitude of the eccentricity vector, unwrapped, in the initial orbit plane.
#[derive(Debug, Clone)]
pub struct PerihelionTracker {
    mu: f64,
    p_hat: [f64; 3],
    q_hat: [f64; 3],
    last: f64,
}

impl PerihelionTracker {
    /// Fix the reference plane and zero direction from the initial relative state.
    ///
    /// # Errors
    /// `FA-E0600` if `μ ≤ 0`, the angular momentum is zero or the orbit is circular to f64
    /// precision (periapsis undefined).
    pub fn new(r0: [f64; 3], v0: [f64; 3], mu: f64) -> Result<Self, PrecessionError> {
        if mu.is_nan() || mu <= 0.0 {
            return Err(PrecessionError::Domain {
                what: "gravitational parameter (km^3/s^2)",
                value: mu,
            });
        }
        let h = cross(r0, v0);
        let hn = norm(h);
        let e0 = eccentricity_vector(r0, v0, mu);
        let en = norm(e0);
        if hn.is_nan() || en.is_nan() || hn <= 0.0 || en <= f64::EPSILON.sqrt() {
            return Err(PrecessionError::Domain {
                what: "eccentricity (periapsis undefined)",
                value: en,
            });
        }
        let p_hat = [e0[0] / en, e0[1] / en, e0[2] / en];
        let q_hat = cross([h[0] / hn, h[1] / hn, h[2] / hn], p_hat);
        Ok(Self {
            mu,
            p_hat,
            q_hat,
            last: 0.0,
        })
    }

    /// Longitude (rad) of the eccentricity vector of `(r, v)` from the initial periapsis,
    /// measured in the initial orbit plane and unwrapped against the previous call.
    pub fn longitude(&mut self, r: [f64; 3], v: [f64; 3]) -> f64 {
        let e = eccentricity_vector(r, v, self.mu);
        let raw = dot(e, self.q_hat).atan2(dot(e, self.p_hat));
        let turns = ((self.last - raw) / std::f64::consts::TAU).round();
        let unwrapped = turns.mul_add(std::f64::consts::TAU, raw);
        self.last = unwrapped;
        unwrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tracker reads zero at the initial periapsis and refuses `mu = 0`.
    ///
    /// Verifies: NUM-012
    #[test]
    fn tracker_starts_at_zero_longitude() {
        // GM = 1, a = 1, e = 1/2: periapsis r = 1/2, speed sqrt((1+e)/(1-e)) = sqrt(3).
        let r = [0.5, 0.0, 0.0];
        let v = [0.0, 3.0_f64.sqrt(), 0.0];
        let mut t = PerihelionTracker::new(r, v, 1.0).unwrap();
        assert!(t.longitude(r, v).abs() < 4.0 * f64::EPSILON);
        assert!(PerihelionTracker::new(r, v, 0.0).is_err());
    }
}
