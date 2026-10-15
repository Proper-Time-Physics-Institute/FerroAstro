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
use ferroastro_real::Real;

/// Solar (central-body) J2 with a configurable pole, in backend `R` (default `f64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarJ2<F: Frame, R: Real = f64> {
    mu: R,
    j2: R,
    radius: R,
    pole: Pole<F, R>,
}

impl<F: Frame, R: Real> SolarJ2<F, R> {
    /// Build from the central `μ`, the unnormalised `J₂`, the reference radius and the pole.
    ///
    /// # Errors
    /// `FA-E0500` if `μ` or the radius is not positive, or `J₂` is not finite.
    pub fn new(
        mu: GravParam<R>,
        j2: R,
        radius: Length<R>,
        pole: Pole<F, R>,
    ) -> Result<Self, ForceError> {
        let check = |name: &'static str, v: R, ok: bool| {
            if ok {
                Ok(v)
            } else {
                Err(ForceError::Invalid {
                    name,
                    value: v.to_f64(),
                    valid: "finite (and > 0 for GM and radius)",
                })
            }
        };
        let m = mu.km3_per_s2();
        let r = radius.km();
        Ok(Self {
            mu: check("GM (km^3/s^2)", m, m.is_finite() && m > R::ZERO)?,
            j2: check("J2", j2, j2.is_finite())?,
            radius: check("reference radius (km)", r, r.is_finite() && r > R::ZERO)?,
            pole,
        })
    }

    /// The pole.
    #[must_use]
    pub const fn pole(&self) -> Pole<F, R> {
        self.pole
    }
}

impl<F: Frame, R: Real> CentralTerm<F, R> for SolarJ2<F, R> {
    fn add_acceleration(&self, r: [R; 3], _v: [R; 3], a: &mut [R; 3]) {
        let k_hat = self.pole.unit();
        let r2 = dot(r, r);
        let z = dot(r, k_hat);
        let r5 = r2 * r2 * r2.sqrt();
        let k = R::from_f64(-1.5) * self.mu * self.j2 * self.radius * self.radius / r5;
        let radial = R::ONE - R::from_f64(5.0) * z * z / r2;
        for ((ai, ri), ki) in a.iter_mut().zip(r).zip(k_hat) {
            *ai += k * radial.mul_add(ri, R::TWO * z * ki);
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn describe(&self) -> String {
        let p = self.pole.unit().map(Real::to_f64);
        format!(
            "central-body J2 (Journal #4 R1), J2 = {:e}, R = {} km, pole [{}, {}, {}]",
            self.j2.to_f64(),
            self.radius.to_f64(),
            p[0],
            p[1],
            p[2]
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
