// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Chebyshev series by the Clenshaw recurrence, generic over [`Real`].
//!
//! For `f(s) = Σₖ cₖ Tₖ(s)` on `s ∈ [−1, 1]`:
//! `bₖ = cₖ + (2s·bₖ₊₁ − bₖ₊₂)` for `k = n−1 … 1`, `f = c₀ + (s·b₁ − b₂)`, and the derivative
//! `b′ₖ = 2bₖ₊₁ + 2s·b′ₖ₊₁ − b′ₖ₊₂`, `f′ = b₁ + s·b′₁ − b′₂`. The operations are written in the
//! same order as jplephem 2.24 (the REF-004 reference reader), so in f64 both round alike; no
//! fused multiply-add is used.

use ferroastro_real::Real;

/// Value and derivative (with respect to `s`) of the Chebyshev series with coefficients
/// `c[0], c[1], …` at `s`. An empty series is zero.
pub fn clenshaw_with_derivative<R: Real>(
    c: impl DoubleEndedIterator<Item = R> + Clone,
    s: R,
) -> (R, R) {
    let s2 = s + s;
    let (mut w0, mut w1) = (R::ZERO, R::ZERO);
    let (mut dw0, mut dw1) = (R::ZERO, R::ZERO);
    let mut c0 = None;
    // Walk c[n-1] … c[0]; c[0] is applied last, outside the recurrence.
    let mut it = c.rev().peekable();
    while let Some(ck) = it.next() {
        if it.peek().is_none() {
            c0 = Some(ck);
            break;
        }
        let w2 = w1;
        w1 = w0;
        w0 = ck + (s2 * w1 - w2);
        let dw2 = dw1;
        dw1 = dw0;
        dw0 = R::TWO * w1 + dw1 * s2 - dw2;
    }
    let Some(c0) = c0 else {
        return (R::ZERO, R::ZERO);
    };
    let value = c0 + (s * w0 - w1);
    let derivative = w0 + s * dw0 - dw1;
    (value, derivative)
}

/// Value of the Chebyshev series at `s` (no derivative).
pub fn clenshaw<R: Real>(c: impl DoubleEndedIterator<Item = R> + Clone, s: R) -> R {
    clenshaw_with_derivative(c, s).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_real::DoubleDouble;

    /// Reference by the explicit three-term recurrence Tₖ₊₁ = 2s·Tₖ − Tₖ₋₁ and
    /// T′ₖ₊₁ = 2Tₖ + 2s·T′ₖ − T′ₖ₋₁. With dyadic inputs every operation is exact.
    fn direct(c: &[f64], s: f64) -> (f64, f64) {
        let (mut t0, mut t1) = (1.0, s);
        let (mut d0, mut d1) = (0.0, 1.0);
        let (mut v, mut d) = (0.0, 0.0);
        for (k, ck) in c.iter().enumerate() {
            let (t, dt) = if k == 0 { (t0, d0) } else { (t1, d1) };
            v += ck * t;
            d += ck * dt;
            if k >= 1 {
                let t2 = (2.0 * s).mul_add(t1, -t0);
                let d2 = 2.0f64.mul_add(t1, (2.0 * s).mul_add(d1, -d0));
                (t0, t1, d0, d1) = (t1, t2, d1, d2);
            }
        }
        (v, d)
    }

    /// Dyadic coefficients (exact in binary64).
    const C: [f64; 5] = [3.0, -2.0, 0.5, 2.0, -1.0];

    /// Verifies: DEP-011
    #[test]
    fn matches_the_explicit_recurrence_exactly() {
        for k in -8..=8 {
            let s = f64::from(k) / 8.0;
            let (v, d) = clenshaw_with_derivative(C.iter().copied(), s);
            let (ve, de) = direct(&C, s);
            assert_eq!(v.to_bits(), ve.to_bits(), "s = {s}: {v} vs {ve}");
            assert_eq!(d.to_bits(), de.to_bits(), "s = {s}: {d} vs {de}");
        }
        assert_eq!(clenshaw(core::iter::empty::<f64>(), 0.5).to_bits(), 0);
        let (v, d) = clenshaw_with_derivative([3.0].into_iter(), 0.5);
        assert_eq!((v.to_bits(), d.to_bits()), (3.0_f64.to_bits(), 0));
    }

    /// Verifies: DEP-007
    #[test]
    fn generic_over_double_double() {
        let s = 0.25 + 0.5 / 4.0;
        let (v, d) = clenshaw_with_derivative(
            C.iter().map(|&x| DoubleDouble::from_f64(x)),
            DoubleDouble::from_f64(s),
        );
        let (ve, de) = direct(&C, s);
        assert_eq!(v.to_parts(), (ve, 0.0));
        assert_eq!(d.to_parts(), (de, 0.0));
    }
}
