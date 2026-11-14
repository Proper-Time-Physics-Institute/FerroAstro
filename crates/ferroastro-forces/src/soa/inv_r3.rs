// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The exact inverse cube `r⁻³` from `r²` (NUM-021: ≤ 1 ulp), without fused multiply-add.
//!
//! With `s = RN(√r²)`:
//!
//! 1. `r² · s = p + p′` exactly (Dekker's two-product with Veltkamp splitting);
//! 2. the square-root residual `e = r² − s²` exactly (`s² = q + q′`; `r² − q` is exact by
//!    Sterbenz's lemma because `q ∈ [r²/2, 2r²]`);
//! 3. `r³ = r² √r² = p + p′ + ½ s e + O(u² r³)` (first-order expansion of `√(s² + e)`);
//! 4. `y₀ = RN(1/p)`, `y₀ p = t + t′` exactly, `ε = ((1 − t) − t′) − y₀ (p′ + ½ s e)`
//!    (`1 − t` is exact, `t ∈ [½, 2]`), and `y = y₀ + y₀ ε`: one Newton step for `1/r³`.
//!
//! The relative error before the last rounding is O(u²) (u = 2⁻⁵³), so the result is within
//! ½ ulp + O(u²) of `r⁻³`: faithful, and correctly rounded except within O(u) of a midpoint. In
//! a [`DoubleDouble`](ferroastro_real::DoubleDouble) lane the same steps are a Newton refinement
//! at double-double precision. Every step is a correctly rounded `+ − × ÷ √` ([`Lane`]), so the
//! scalar and SIMD results are bitwise identical.

use super::Lane;
use ferroastro_real::Real;

/// Veltkamp's splitting constant `2²⁷ + 1` for binary64.
const SPLIT: f64 = 134_217_729.0;

/// `a = hi + lo` with `hi` holding the upper 26 significand bits (Veltkamp).
#[inline]
fn split<L: Lane>(a: L) -> (L, L) {
    let c = L::splat(L::Elem::from_f64(SPLIT)) * a;
    let hi = c - (c - a);
    (hi, a - hi)
}

/// `a · b = p + e` exactly (Dekker 1971), no FMA.
#[inline]
pub(crate) fn two_prod<L: Lane>(a: L, b: L) -> (L, L) {
    let p = a * b;
    let (ah, al) = split(a);
    let (bh, bl) = split(b);
    let e = (((ah * bh - p) + ah * bl) + al * bh) + al * bl;
    (p, e)
}

/// `r⁻³` from `r²` within 1 ulp (½ ulp + O(u²)); `r² = 0` gives a non-finite value, which the
/// kernels discard with a select.
#[inline]
#[must_use]
pub fn inv_r3_exact<L: Lane>(r2: L) -> L {
    let one = L::splat(L::Elem::ONE);
    let half = L::splat(L::Elem::HALF);
    let s = r2.sqrt();
    let (p, pp) = two_prod(r2, s);
    let (q, qq) = two_prod(s, s);
    let e = (r2 - q) - qq;
    let y0 = one / p;
    let (t, tt) = two_prod(y0, p);
    let corr = pp + half * s * e;
    let eps = ((one - t) - tt) - y0 * corr;
    y0 + y0 * eps
}

/// [`inv_r3_exact`] for one value of any backend.
#[must_use]
pub fn inv_r3_exact_scalar<R: Real>(r2: R) -> R {
    inv_r3_exact(super::Scalar(r2)).0
}
