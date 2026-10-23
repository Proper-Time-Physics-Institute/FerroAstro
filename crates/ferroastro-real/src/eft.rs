// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Error-free transformations of binary64 arithmetic (Joldes, Muller & Popescu 2017, ACM TOMS
//! 44(2), 15res, Algorithms 1–3): each returns the rounded result and its exact rounding error.

use crate::f64_math;

/// `2Sum` (Knuth; Møller 1965): `s + t = a + b` exactly, `s = RN(a + b)`. No precondition.
#[inline]
#[must_use]
pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let a1 = s - b;
    let b1 = s - a1;
    (s, (a - a1) + (b - b1))
}

/// `Fast2Sum` (Dekker 1971): `s + t = a + b` exactly when `a = 0`, `b = 0` or the exponent of
/// `a` is at least that of `b` (in particular when `|a| ≥ |b|`).
#[inline]
#[must_use]
pub fn fast_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let z = s - a;
    (s, b - z)
}

/// `2Prod` with a fused multiply-add (`Fast2Mult`): `p + e = a·b` exactly, `p = RN(a·b)`,
/// barring overflow and underflow of `e`.
#[inline]
#[must_use]
pub fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    (p, f64_math::fma(a, b, -p))
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "error-free transformations and special values have exact expected results"
)]
mod tests {
    use super::*;

    /// The rounding errors are exact on cases where they are known in closed form.
    ///
    /// Verifies: NUM-006
    #[test]
    fn error_terms_are_exact() {
        let tiny = f64::EPSILON / 4.0;
        assert_eq!(two_sum(1.0, tiny), (1.0, tiny));
        assert_eq!(two_sum(tiny, 1.0), (1.0, tiny));
        assert_eq!(fast_two_sum(1.0, tiny), (1.0, tiny));
        // (1 + 2^-52)^2 = 1 + 2^-51 + 2^-104: the last term is the FMA error.
        let x = 1.0 + f64::EPSILON;
        let (p, e) = two_prod(x, x);
        assert_eq!(p, 1.0 + 2.0 * f64::EPSILON);
        assert_eq!(e, f64::EPSILON * f64::EPSILON);
    }
}
