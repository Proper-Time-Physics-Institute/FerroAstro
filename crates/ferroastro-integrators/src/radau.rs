// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Gauss–Radau spacings and the Newton-to-monomial conversion matrices of the 15th-order
//! Everhart scheme (Everhart 1985, "An efficient integrator that uses Gauss-Radau spacings",
//! IAU Colloq. 83, 185; Rein & Spiegel 2015, MNRAS 446, 1424, §2).
//!
//! Nothing here is transcribed from tables or code: the spacings are computed as the roots of
//! `P₇(x) + P₈(x)` on `[−1, 1]` (the left-endpoint Radau nodes, `x = −1` being the step start),
//! mapped to `[0, 1]`, and the matrices are built from the spacings by polynomial algebra. All of
//! it is done in double-double ([`crate::dd`]) and rounded to `f64` once, so each constant is the
//! correctly rounded value of the exact one (Rein & Spiegel 2015 §3.2).
//!
//! Notation. Over one step the acceleration is the degree-7 polynomial in `τ = (t − t₀)/Δt`
//! `a(τ) = a₀ + Σₘ Bₘ τᵐ` (m = 1…7; IAS15's `b₀…b₆` are `B₁…B₇`), equivalently the Newton form
//! `a(τ) = a₀ + Σₖ gₖ Nₖ(τ)` with `Nₖ(τ) = Πⱼ₍₌₀…ₖ₋₁₎ (τ − hⱼ)` and `h₀ = 0`.

use crate::dd::Dd;

/// Number of Radau nodes per step, including the step start.
pub(crate) const NODES: usize = 8;
/// Number of polynomial coefficients `B₁…B₇` (IAS15's `b₀…b₆`).
pub(crate) const TERMS: usize = 7;

/// Spacings and conversion matrices. Index `k`/`m` runs over `1..=7`; row/column 0 is unused.
#[derive(Debug, Clone)]
pub(crate) struct Radau {
    /// Node spacings `h₀ = 0 < h₁ < … < h₇ < 1`.
    pub(crate) h: [f64; NODES],
    /// `c[k][m]`: coefficient of `τᵐ` in `Nₖ(τ)`; so `Bₘ = Σₖ₍≥ₘ₎ c[k][m] gₖ`.
    pub(crate) c: [[f64; NODES]; NODES],
    /// Inverse map: `gₖ = Σₘ₍≥ₖ₎ d[k][m] Bₘ`.
    pub(crate) d: [[f64; NODES]; NODES],
}

/// `(P₇(x) + P₈(x), d/dx)` by the Bonnet recurrence.
fn radau_poly(x: f64) -> (f64, f64) {
    let (mut p_prev, mut p) = (1.0, x); // P0, P1
    let (mut dp_prev, mut dp) = (0.0, 1.0);
    let mut p7 = 0.0;
    let mut dp7 = 0.0;
    for n in 1..8_u32 {
        let nf = f64::from(n);
        // P_{n+1} = ((2n+1) x P_n − n P_{n−1}) / (n+1)
        let two_n1 = 2.0_f64.mul_add(nf, 1.0);
        let p_next = (two_n1 * x).mul_add(p, -nf * p_prev) / (nf + 1.0);
        let dp_next = two_n1.mul_add(x.mul_add(dp, p), -nf * dp_prev) / (nf + 1.0);
        p_prev = p;
        dp_prev = dp;
        p = p_next;
        dp = dp_next;
        if n == 6 {
            p7 = p;
            dp7 = dp;
        }
    }
    (p7 + p, dp7 + dp)
}

/// The seven interior roots of `P₇ + P₈` in `(−1, 1)`, ascending, found by bracketing on a fine
/// grid, bisection and a final Newton polish.
fn radau_roots() -> [f64; TERMS] {
    const GRID: u32 = 4096;
    let mut roots = [0.0; TERMS];
    let mut found = 0;
    let at = |i: u32| -1.0 + 2.0 * f64::from(i) / f64::from(GRID);
    // Skip the endpoint root x = −1 by starting the scan just inside it.
    let mut lo = at(1);
    let mut f_lo = radau_poly(lo).0;
    for i in 2..=GRID {
        let hi = at(i);
        let f_hi = radau_poly(hi).0;
        if f_lo * f_hi < 0.0 {
            let (mut a, mut b, mut fa) = (lo, hi, f_lo);
            for _ in 0..200 {
                let m = 0.5 * (a + b);
                let fm = radau_poly(m).0;
                if fm == 0.0 || (b - a) <= f64::EPSILON * m.abs().max(f64::MIN_POSITIVE) {
                    a = m;
                    b = m;
                    break;
                }
                if fa * fm < 0.0 {
                    b = m;
                } else {
                    a = m;
                    fa = fm;
                }
            }
            let mut x = 0.5 * (a + b);
            for _ in 0..3 {
                let (f, df) = radau_poly(x);
                if df != 0.0 {
                    x -= f / df;
                }
            }
            if let Some(slot) = roots.get_mut(found) {
                *slot = x;
            }
            found += 1;
        }
        lo = hi;
        f_lo = f_hi;
    }
    roots
}

/// `(P₇(x) + P₈(x), d/dx)` in double-double, by the Bonnet recurrence.
fn radau_poly_dd(x: Dd) -> (Dd, Dd) {
    let one = Dd::from_f64(1.0);
    let (mut p_prev, mut p) = (one, x);
    let (mut dp_prev, mut dp) = (Dd::from_f64(0.0), one);
    let (mut p7, mut dp7) = (p, dp);
    for n in 1..8_u32 {
        let nf = Dd::from_f64(f64::from(n));
        let two_n1 = Dd::from_f64(f64::from(2 * n + 1));
        let n1 = Dd::from_f64(f64::from(n + 1));
        let p_next = (two_n1 * x * p - nf * p_prev) / n1;
        let dp_next = (two_n1 * (p + x * dp) - nf * dp_prev) / n1;
        p_prev = p;
        dp_prev = dp;
        p = p_next;
        dp = dp_next;
        if n == 6 {
            p7 = p;
            dp7 = dp;
        }
    }
    (p7 + p, dp7 + dp)
}

impl Radau {
    /// Compute the spacings and matrices in double-double and round each to `f64`.
    #[expect(
        clippy::indexing_slicing,
        clippy::needless_range_loop,
        reason = "fixed-size 8x8 tables indexed by loop counters bounded by NODES"
    )]
    pub(crate) fn new() -> Self {
        let zero = Dd::from_f64(0.0);
        let one = Dd::from_f64(1.0);
        let half = Dd::from_f64(0.5);
        let mut hd = [zero; NODES];
        for (slot, x0) in hd.iter_mut().skip(1).zip(radau_roots()) {
            // Newton polish of the f64 root in double-double (quadratic convergence: two steps
            // take ~1e-16 to below 1e-32).
            let mut x = Dd::from_f64(x0);
            for _ in 0..3 {
                let (f, df) = radau_poly_dd(x);
                if df.hi != 0.0 {
                    x = x - f / df;
                }
            }
            *slot = (x + one) * half;
        }
        // c[k] = coefficients of N_k(τ) = N_{k−1}(τ)·(τ − h_{k−1}).
        let mut cd = [[zero; NODES]; NODES];
        cd[0][0] = one;
        for k in 1..NODES {
            for m in 0..=k {
                let shifted = if m > 0 { cd[k - 1][m - 1] } else { zero };
                cd[k][m] = shifted - hd[k - 1] * cd[k - 1][m];
            }
        }
        // B = M g with M[m][k] = c[k][m] (unit upper triangular in m ≤ k); d = M⁻¹ by back
        // substitution, stored as d[k][m] (row k of the inverse).
        let mut dd = [[zero; NODES]; NODES];
        for j in 1..NODES {
            for m in (1..NODES).rev() {
                let mut s = if m == j { one } else { zero };
                for k in (m + 1)..NODES {
                    s = s - cd[k][m] * dd[k][j];
                }
                dd[m][j] = s;
            }
        }
        let h = hd.map(Dd::to_f64);
        let c = cd.map(|row| row.map(Dd::to_f64));
        let d = dd.map(|row| row.map(Dd::to_f64));
        Self { h, c, d }
    }
}

#[cfg(test)]
#[expect(
    clippy::needless_range_loop,
    reason = "index notation mirrors the k/m subscripts of the scheme"
)]
mod tests {
    use super::*;

    /// Verifies: NUM-001
    #[test]
    fn spacings_are_radau_roots_in_order() {
        let r = Radau::new();
        assert!(r.h[0].abs() < f64::EPSILON);
        for w in r.h.windows(2) {
            assert!(w[0] < w[1], "spacings not increasing: {:?}", r.h);
        }
        assert!(r.h[NODES - 1] < 1.0);
        for &x in &r.h[1..] {
            let (f, _) = radau_poly(2.0_f64.mul_add(x, -1.0));
            assert!(f.abs() < 1e-13, "P7+P8 at node {x} = {f}"); // lint: allow-literal(f64 root-finding residual bound, not physical)
        }
        // The left endpoint is itself a root of P7 + P8 (P7(-1) = -1, P8(-1) = 1).
        assert!(radau_poly(-1.0).0.abs() < 1e-14); // lint: allow-literal(f64 round-off bound)
    }

    /// The double-double roots are accurate far below f64 resolution, and each stored spacing is
    /// the nearest f64 to its double-double value.
    ///
    /// Verifies: NUM-001
    #[test]
    fn spacings_are_correctly_rounded() {
        let r = Radau::new();
        for &h in &r.h[1..] {
            let x = Dd::from_f64(h) + Dd::from_f64(h) - Dd::from_f64(1.0);
            let (f, df) = radau_poly_dd(x);
            // Newton distance from the stored f64 node to the true root; h·eps bounds ulp(h) above.
            let dist = (f / df).to_f64().abs() / 2.0;
            let ulp = h * f64::EPSILON;
            assert!(dist <= 0.5 * ulp, "node {h}: {dist:e} > ulp/2 {ulp:e}");
        }
    }

    /// The Newton form built from c reproduces any degree-7 polynomial sampled at the nodes, and
    /// d inverts the Newton-to-monomial map.
    ///
    /// Verifies: NUM-001
    #[test]
    fn newton_and_monomial_forms_agree() {
        let r = Radau::new();
        // Arbitrary integer monomial coefficients B_1..B_7 (dimensionless test polynomial).
        let big_b = [0.0, 3.0, -2.0, 5.0, 1.0, -4.0, 2.0, 7.0];
        let a0 = 1.0;
        let poly = |t: f64| {
            let mut s = 0.0;
            for m in (1..NODES).rev() {
                s = (s + big_b[m]) * t;
            }
            a0 + s
        };
        // Divided differences at the nodes give g_k.
        let mut g = [0.0; NODES];
        for n in 1..NODES {
            let mut v = (poly(r.h[n]) - a0) / r.h[n];
            for k in 1..n {
                v = (v - g[k]) / (r.h[n] - r.h[k]);
            }
            g[n] = v;
        }
        for m in 1..NODES {
            let bm: f64 = (m..NODES).map(|k| r.c[k][m] * g[k]).sum();
            assert!((bm - big_b[m]).abs() < 1e-9, "B_{m}: {bm} vs {}", big_b[m]); // lint: allow-literal(conditioning bound of the 7x7 conversion)
        }
        for k in 1..NODES {
            let gk: f64 = (k..NODES).map(|m| r.d[k][m] * big_b[m]).sum();
            assert!((gk - g[k]).abs() < 1e-9 * g[k].abs().max(1.0), "g_{k}"); // lint: allow-literal(conditioning bound)
        }
    }
}
