// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Raw 3-vector helpers (km, km/s) and the Laplace–Runge–Lenz (eccentricity) vector.
//!
//! These work on the untyped `[R; 3]` arrays that integrators produce (any backend
//! `R: Real`); typed states live in `ferroastro_core::frames`.

use ferroastro_real::Real;

/// Cross product `a × b` (fused multiply-add form), any backend.
#[must_use]
pub fn cross<R: Real>(a: [R; 3], b: [R; 3]) -> [R; 3] {
    [
        a[1].mul_add(b[2], -a[2] * b[1]),
        a[2].mul_add(b[0], -a[0] * b[2]),
        a[0].mul_add(b[1], -a[1] * b[0]),
    ]
}

/// Dot product `a · b`.
#[must_use]
pub fn dot<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// Euclidean norm `|a|`.
#[must_use]
pub fn norm<R: Real>(a: [R; 3]) -> R {
    dot(a, a).sqrt()
}

/// The eccentricity (LRL) vector `(v × h)/μ − r̂` (dimensionless) of the relative orbit with
/// gravitational parameter `mu` (km³/s²); `r` in km, `v` in km/s. It points to periapsis, has
/// length `e`, and is a constant of Newtonian two-body motion.
#[must_use]
pub fn eccentricity_vector<R: Real>(r: [R; 3], v: [R; 3], mu: R) -> [R; 3] {
    let h = cross(r, v);
    let vxh = cross(v, h);
    let rn = norm(r);
    [
        vxh[0] / mu - r[0] / rn,
        vxh[1] / mu - r[1] / rn,
        vxh[2] / mu - r[2] / rn,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The eccentricity vector of a periapsis state points along r with |e| = e
    /// (GM = 1, a = 1, e = 1/2: periapsis r = 1/2, speed sqrt((1+e)/(1-e)) = sqrt(3)).
    #[test]
    fn eccentricity_vector_at_periapsis() {
        let r = [0.5, 0.0, 0.0];
        let v = [0.0, 3.0_f64.sqrt(), 0.0];
        let e = eccentricity_vector(r, v, 1.0);
        assert!((e[0] - 0.5).abs() < 4.0 * f64::EPSILON);
        assert!(e[1].abs() < 4.0 * f64::EPSILON && e[2].abs() < 4.0 * f64::EPSILON);
        assert!((norm(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])) - 1.0).abs() <= f64::EPSILON);
    }
}
