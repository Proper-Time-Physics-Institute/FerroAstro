// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The relativistic solar-system barycentre, Park et al. 2021 Eqs. 1–2 (NUM-018):
//!
//! `r_ssbc = (Σ_i μ*_i r_i) / (Σ_i μ*_i)`, `μ*_i = GM_i (1 + v_i²/(2c²) − Σ_{j≠i} GM_j/(2c² r_ij))`,
//!
//! summed over the bodies with finite mass, `v_i` the barycentric speed and `r_ij = |r_i − r_j|`.
//! Under the EIH equations this point moves uniformly up to O(c⁻⁴) (the 1PN centre of mass), while
//! the Newtonian `Σ GM_i r_i / Σ GM_i` oscillates at O(c⁻²).

use ferroastro_core::units::Velocity;
use ferroastro_forces::ForceError;
use ferroastro_real::Real;

/// One body for [`mu_star`] and [`relativistic_barycentre`]: GM (km³/s²), barycentric position
/// (km) and velocity (km/s).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarycentreBody<R: Real = f64> {
    /// GM (km³/s²).
    pub gm: R,
    /// Position (km).
    pub position: [R; 3],
    /// Velocity (km/s).
    pub velocity: [R; 3],
}

fn speed_of_light<R: Real>(c: Velocity<R>) -> Result<R, ForceError> {
    let cc = c.km_per_s();
    if cc.is_finite() && cc > R::ZERO {
        Ok(cc)
    } else {
        Err(ForceError::Invalid {
            name: "c (km/s)",
            value: cc.to_f64(),
            valid: "finite and > 0",
        })
    }
}

fn dot<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// `μ*_i` of every body (Eq. 2), in input order.
///
/// # Errors
/// `FA-E0500` for a non-positive `c`; `FA-E0501` if two massive bodies coincide.
pub fn mu_star<R: Real>(
    bodies: &[BarycentreBody<R>],
    c: Velocity<R>,
) -> Result<Vec<R>, ForceError> {
    let cc = speed_of_light(c)?;
    let two_c2 = R::TWO * cc * cc;
    let mut out = Vec::with_capacity(bodies.len());
    for (i, b) in bodies.iter().enumerate() {
        let mut u = R::ZERO;
        for (j, o) in bodies.iter().enumerate() {
            if j == i || o.gm == R::ZERO {
                continue;
            }
            let d = [
                b.position[0] - o.position[0],
                b.position[1] - o.position[1],
                b.position[2] - o.position[2],
            ];
            let r = dot(d, d).sqrt();
            if r == R::ZERO {
                return Err(ForceError::Singular { i, j, t: f64::NAN });
            }
            u += o.gm / r;
        }
        let v2 = dot(b.velocity, b.velocity);
        out.push(b.gm * (R::ONE + (v2 - u) / two_c2));
    }
    Ok(out)
}

/// The relativistic barycentre (Eq. 1) and the μ*-weighted mean velocity `Σ μ*_i v_i / Σ μ*_i`.
///
/// # Errors
/// As [`mu_star`], and `FA-E0500` if the total μ* is not positive.
pub fn relativistic_barycentre<R: Real>(
    bodies: &[BarycentreBody<R>],
    c: Velocity<R>,
) -> Result<([R; 3], [R; 3]), ForceError> {
    let mu = mu_star(bodies, c)?;
    let total = mu.iter().fold(R::ZERO, |s, &m| s + m);
    if total <= R::ZERO || !total.is_finite() {
        return Err(ForceError::Invalid {
            name: "total mu*",
            value: total.to_f64(),
            valid: "> 0",
        });
    }
    let mut r = [R::ZERO; 3];
    let mut v = [R::ZERO; 3];
    for (b, &m) in bodies.iter().zip(&mu) {
        for (rk, pk) in r.iter_mut().zip(b.position) {
            *rk = m.mul_add(pk, *rk);
        }
        for (vk, wk) in v.iter_mut().zip(b.velocity) {
            *vk = m.mul_add(wk, *vk);
        }
    }
    Ok((r.map(|x| x / total), v.map(|x| x / total)))
}
