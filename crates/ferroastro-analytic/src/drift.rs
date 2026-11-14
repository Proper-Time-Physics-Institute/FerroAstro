// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The exact two-body (Kepler) flow of a relative state over a time step, in universal variables:
//! the Kepler drift of the Wisdom–Holman map (NUM-004, WP29).
//!
//! Universal anomaly `s` with Stumpff's c-functions `c_k(z)`, `z = β s²`, `β = 2μ/r₀ − v₀²`, and
//! the G-functions `G_k = s^k c_k(β s²)` (Danby 1988, *Fundamentals of Celestial Mechanics*, 2nd
//! ed., §6.9; Stumpff 1959). Kepler's equation in these variables is
//! `F(s) = r₀ G₁ + η G₂ + μ G₃ − Δt = 0` with `η = r₀·v₀`, `F'(s) = r(s) = r₀ G₀ + η G₁ + μ G₂`,
//! `F''(s) = η G₀ + (μ − β r₀) G₁`. It is solved by the Laguerre–Conway iteration (Conway 1986,
//! Celest. Mech. 39, 199), and the state follows from the Lagrange coefficients
//! `f = 1 − μG₂/r₀`, `g = r₀G₁ + ηG₂`, `ḟ = −μG₁/(r r₀)`, `ġ = 1 − μG₂/r`, applied as increments
//! (`r₁ = r₀ + ((f−1) r₀ + g v₀)`) so that small steps lose no low-order bits.
//!
//! The c-functions are evaluated by their power series after reducing `z` by factors of 4, then
//! restored with the quadrupling formulae `c₀(4z) = 2c₀² − 1`, `c₁(4z) = c₀c₁`, `c₂(4z) = c₁²/2`,
//! `c₃(4z) = (c₂ + c₀c₃)/4` (each verified from `c₀ = cos x`, `c₁ = sin x / x`,
//! `c₂ = (1 − cos x)/x²`, `c₃ = (x − sin x)/x³`, `z = x²`). Only `+ − × ÷` and `√` are used, so
//! the drift runs in any backend `R: Real`, including double-double.
//!
//! For a bound orbit the step is first reduced modulo the period (`F` grows by exactly one period
//! when `s` grows by `2π/√β`, while `G₀ … G₂`, hence `f, g, ḟ, ġ`, are periodic in `s`).
//!
//! Clean-room (DEP-008): written from Danby's and Conway's published formulae; no REBOUND,
//! `WHFast` or Swiftest source was consulted.

use crate::AnalyticError;
use ferroastro_real::Real;

fn domain<R: Real>(what: &'static str, value: R) -> AnalyticError {
    AnalyticError::Domain {
        what,
        value: value.to_f64(),
    }
}

/// Stumpff's `c₀ … c₃` at `z` (any sign).
#[must_use]
pub fn stumpff<R: Real>(z: R) -> [R; 4] {
    let tenth = R::from_f64(0.1);
    let mut reduced = z;
    let mut quarterings = 0_u32;
    while reduced.abs() > tenth && quarterings < 200 {
        reduced = reduced.scale(0.25);
        quarterings += 1;
    }
    // Series c₂ = Σ (−z)^k / (2k+2)!, c₃ = Σ (−z)^k / (2k+3)!, nested from the highest term.
    // |z| ≤ 0.1: 7 terms leave < 1e-20 (binary64), 12 terms < 1e-35 (double-double).
    let terms: i32 = if R::EPSILON.to_f64() < 1e-20 { 12 } else { 7 };
    let mut c2 = R::ONE;
    let mut c3 = R::ONE;
    for k in (1..=terms).rev() {
        // c2: factor z / ((2k+1)(2k+2)); c3: z / ((2k+2)(2k+3)).
        let d2 = R::from_i32((2 * k + 1) * (2 * k + 2));
        let d3 = R::from_i32((2 * k + 2) * (2 * k + 3));
        c2 = R::ONE - reduced * c2 / d2;
        c3 = R::ONE - reduced * c3 / d3;
    }
    c2 *= R::HALF;
    c3 /= R::from_i32(6);
    let mut c1 = R::ONE - reduced * c3;
    let mut c0 = R::ONE - reduced * c2;
    for _ in 0..quarterings {
        let n3 = (c2 + c0 * c3).scale(0.25);
        let n2 = c1 * c1 * R::HALF;
        let n1 = c0 * c1;
        let n0 = R::TWO * c0 * c0 - R::ONE;
        c0 = n0;
        c1 = n1;
        c2 = n2;
        c3 = n3;
    }
    [c0, c1, c2, c3]
}

/// `[G₀, G₁, G₂, G₃]` at universal anomaly `s` for `β`.
fn g_functions<R: Real>(beta: R, s: R) -> [R; 4] {
    let [c0, c1, c2, c3] = stumpff(beta * s * s);
    let s2 = s * s;
    [c0, s * c1, s2 * c2, s2 * s * c3]
}

#[inline]
fn dot3<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// Advance the relative two-body state `(r, v)` (any consistent units, e.g. km and km/s) by
/// `dt` under `μ` (the sum of the two GMs): the exact Kepler flow, for elliptic, parabolic and
/// hyperbolic orbits, forwards or backwards.
///
/// # Errors
/// `FA-E0600` if `μ ≤ 0`, the state is not finite or zero, or the universal anomaly does not
/// converge in 100 Laguerre–Conway iterations.
pub fn kepler_drift<R: Real>(
    mu: R,
    r: &mut [R; 3],
    v: &mut [R; 3],
    dt: R,
) -> Result<(), AnalyticError> {
    if mu.is_nan() || mu <= R::ZERO {
        return Err(domain("gravitational parameter of the Kepler drift", mu));
    }
    if dt == R::ZERO {
        return Ok(());
    }
    let r0v = *r;
    let v0v = *v;
    let r0 = dot3(r0v, r0v).sqrt();
    if !(r0.is_finite() && r0 > R::ZERO && dt.is_finite()) {
        return Err(domain("Kepler drift radius", r0));
    }
    let eta = dot3(r0v, v0v);
    let v2 = dot3(v0v, v0v);
    let beta = R::TWO * mu / r0 - v2;
    if !beta.is_finite() || !eta.is_finite() {
        return Err(domain("Kepler drift velocity", v2));
    }
    // Reduce a bound orbit's step modulo the period P = 2π μ / β^{3/2}.
    let mut tau = dt;
    if beta > R::ZERO {
        let period = R::TAU * mu / (beta * beta.sqrt());
        let k = (dt / period).round();
        if k != R::ZERO {
            tau = (-k).mul_add(period, dt);
        }
    }
    // Initial guess: the two-body anomaly change at the current radius, capped for bound orbits
    // at half an orbit in s (2π/√β per orbit).
    let mut s = tau / r0;
    if beta > R::ZERO {
        let half = R::PI / beta.sqrt();
        s = s.max(-half).min(half);
    }
    let five = R::from_i32(5);
    let tol = R::EPSILON.scale(4.0);
    let mut converged = false;
    // Round-off floor of the iteration: F is a sum of terms of size ~|τ|, so near the root a
    // step can stay at a few ulp of s; stagnation below this floor also counts as converged.
    let floor = R::EPSILON.scale(64.0);
    let mut ds_prev: Option<R> = None;
    for _ in 0..100 {
        let [g0, g1, g2, g3] = g_functions(beta, s);
        let f = r0.mul_add(g1, eta.mul_add(g2, mu * g3)) - tau;
        let fp = r0.mul_add(g0, eta.mul_add(g1, mu * g2));
        let fpp = eta.mul_add(g0, (mu - beta * r0) * g1);
        // Laguerre–Conway with n = 5.
        let disc = (R::from_i32(16) * fp * fp - R::from_i32(20) * f * fpp).abs();
        let root = disc.sqrt();
        let denom = if fp >= R::ZERO { fp + root } else { fp - root };
        if denom == R::ZERO || !denom.is_finite() {
            return Err(domain("Kepler drift iteration denominator", denom));
        }
        let ds = five * f / denom;
        s -= ds;
        let size = ds.abs();
        let scale = s.abs().max(tol);
        if size <= tol * scale
            || (size <= floor * scale && ds_prev.is_some_and(|p: R| size >= p.abs()))
        {
            converged = true;
            break;
        }
        ds_prev = Some(ds);
    }
    if !converged || !s.is_finite() {
        return Err(domain("Kepler drift universal anomaly (no convergence)", s));
    }
    let [g0, g1, g2, _] = g_functions(beta, s);
    let rn = r0.mul_add(g0, eta.mul_add(g1, mu * g2));
    if !(rn.is_finite() && rn > R::ZERO) {
        return Err(domain("Kepler drift final radius", rn));
    }
    let f_m1 = -mu * g2 / r0;
    let g = r0.mul_add(g1, eta * g2);
    let fdot = -mu * g1 / (rn * r0);
    let gdot_m1 = -mu * g2 / rn;
    for ((rk, vk), (&x0, &w0)) in r.iter_mut().zip(v.iter_mut()).zip(r0v.iter().zip(&v0v)) {
        *rk += f_m1.mul_add(x0, g * w0);
        *vk += fdot.mul_add(x0, gdot_m1 * w0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Arguments x (z = ±x²) spanning the series range and up to two quarterings past π.
    const XS: [f64; 7] = [0.0, 1e-4, 0.3, 1.0, 2.5, 3.1, 6.0]; // lint: allow-literal(test arguments of the c-functions, dimensionless)
    /// Round-off allowance of the comparison (a few hundred ulp after up to 6 quarterings).
    const TOL: f64 = 1e-13; // lint: allow-literal(round-off allowance, dimensionless)

    /// Verifies: NUM-004
    #[test]
    fn stumpff_matches_trigonometric_forms() {
        for &x in &XS {
            if x == 0.0 {
                assert_eq!(
                    stumpff(0.0_f64).map(f64::to_bits),
                    [1.0, 1.0, 0.5, 1.0 / 6.0].map(f64::to_bits)
                );
                continue;
            }
            let z = x * x;
            let [c0, c1, c2, c3] = stumpff(z);
            let want = [
                x.cos(),
                x.sin() / x,
                (1.0 - x.cos()) / z,
                (x - x.sin()) / (z * x),
            ];
            let [h0, h1, h2, _] = stumpff(-z);
            let want_h = [x.cosh(), x.sinh() / x, (x.cosh() - 1.0) / z];
            // c2, c3 by the closed forms cancel catastrophically for small x; skip them there.
            let n = if x < 0.1 { 2 } else { 4 }; // lint: allow-literal(cancellation threshold of the closed forms)
            for (k, (got, w)) in [c0, c1, c2, c3].iter().zip(want).take(n).enumerate() {
                assert!(
                    (got - w).abs() <= TOL * w.abs().max(1.0),
                    "c{k}({z}) = {got} vs {w}"
                );
            }
            for (k, (got, w)) in [h0, h1, h2].iter().zip(want_h).take(n.min(3)).enumerate() {
                assert!(
                    (got - w).abs() <= TOL * w.abs().max(1.0),
                    "c{k}(-{z}) = {got} vs {w}"
                );
            }
        }
    }
}
