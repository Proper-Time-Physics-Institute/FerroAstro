// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung J2: the central body's quadrupole (Journal #4 R1; DE440 Eq. 29 form).
//!
//! `a = −(3μJ₂R²)/(2r⁵) [(1 − 5z²/r²) r + 2z k̂]`, with `z = r·k̂` and `k̂` the body's pole. In
//! the test-particle limit the reaction on the central body (`−(m/M) a`) is not applied.

use crate::central::{CentralTerm, Pole, dot};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_core::units::{GravParam, Length};

/// Solar (central-body) J2 with a configurable pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarJ2<F: Frame> {
    mu: f64,
    j2: f64,
    radius: f64,
    pole: Pole<F>,
}

impl<F: Frame> SolarJ2<F> {
    /// Build from the central `μ`, the unnormalised `J₂`, the reference radius and the pole.
    ///
    /// # Errors
    /// `FA-E0500` if `μ` or the radius is not positive, or `J₂` is not finite.
    pub fn new(mu: GravParam, j2: f64, radius: Length, pole: Pole<F>) -> Result<Self, ForceError> {
        let check = |name: &'static str, v: f64, ok: bool| {
            if ok {
                Ok(v)
            } else {
                Err(ForceError::Invalid {
                    name,
                    value: v,
                    valid: "finite (and > 0 for GM and radius)",
                })
            }
        };
        let m = mu.km3_per_s2();
        let r = radius.km();
        Ok(Self {
            mu: check("GM (km^3/s^2)", m, m.is_finite() && m > 0.0)?,
            j2: check("J2", j2, j2.is_finite())?,
            radius: check("reference radius (km)", r, r.is_finite() && r > 0.0)?,
            pole,
        })
    }

    /// The pole.
    #[must_use]
    pub const fn pole(&self) -> Pole<F> {
        self.pole
    }
}

impl<F: Frame> CentralTerm<F> for SolarJ2<F> {
    fn add_acceleration(&self, r: [f64; 3], _v: [f64; 3], a: &mut [f64; 3]) {
        let k_hat = self.pole.unit();
        let r2 = dot(r, r);
        let z = dot(r, k_hat);
        let r5 = r2 * r2 * r2.sqrt();
        let k = -1.5 * self.mu * self.j2 * self.radius * self.radius / r5;
        let radial = 1.0 - 5.0 * z * z / r2;
        for ((ai, ri), ki) in a.iter_mut().zip(r).zip(k_hat) {
            *ai += k * radial.mul_add(ri, 2.0 * z * ki);
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn describe(&self) -> String {
        let p = self.pole.unit();
        format!(
            "central-body J2 (Journal #4 R1), J2 = {:e}, R = {} km, pole [{}, {}, {}]",
            self.j2, self.radius, p[0], p[1], p[2]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::frames::Icrf;

    /// The J2 field is the gradient of `−μJ₂R²/(2r³)(3 sin²φ − 1)`: on the equator the
    /// radial acceleration is `−(3/2)μJ₂R²/r⁴`, at the pole `+3μJ₂R²/r⁴` along the axis.
    ///
    /// Verifies: PHY-010
    #[test]
    fn j2_acceleration_on_equator_and_pole() {
        let gm = GravParam::from_km3_per_s2(1.0).unwrap();
        let pole = Pole::<Icrf>::from_components([0.0, 0.0, 1.0]).unwrap();
        let j2 = SolarJ2::new(gm, 1.0, Length::from_km(1.0).unwrap(), pole).unwrap();
        let mut a = [0.0; 3];
        j2.add_acceleration([2.0, 0.0, 0.0], [0.0; 3], &mut a);
        assert!((a[0] + 1.5 / 16.0).abs() < 4.0 * f64::EPSILON); // lint: allow-literal(r = 2: r^4 = 16)
        let mut b = [0.0; 3];
        j2.add_acceleration([0.0, 0.0, 2.0], [0.0; 3], &mut b);
        assert!((b[2] - 3.0 / 16.0).abs() < 4.0 * f64::EPSILON); // lint: allow-literal(r = 2: r^4 = 16)
        assert!(SolarJ2::new(gm, f64::NAN, Length::from_km(1.0).unwrap(), pole).is_err());
    }
}
