// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Minimal double-double arithmetic (≈ 32 significant digits) used only to precompute the
//! Gauss–Radau constants, so that each constant is the correctly rounded `f64` of its exact value
//! (Rein & Spiegel 2015 §3.2: constants precomputed in extended precision to avoid biased
//! round-off). Error-free transformations after Dekker 1971 and Knuth (`TwoSum`, FMA `TwoProd`).

use std::ops::{Add, Div, Mul, Neg, Sub};

/// A double-double number `hi + lo` with `|lo| ≤ ulp(hi)/2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Dd {
    pub(crate) hi: f64,
    pub(crate) lo: f64,
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

fn quick_two_sum(a: f64, b: f64) -> Dd {
    let s = a + b;
    Dd {
        hi: s,
        lo: b - (s - a),
    }
}

impl Dd {
    pub(crate) const fn from_f64(x: f64) -> Self {
        Self { hi: x, lo: 0.0 }
    }

    /// The nearest `f64` (round-to-nearest of `hi + lo`).
    pub(crate) fn to_f64(self) -> f64 {
        self.hi + self.lo
    }
}

impl Add for Dd {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        let (s, e) = two_sum(self.hi, b.hi);
        let (t, f) = two_sum(self.lo, b.lo);
        let r = quick_two_sum(s, e + t);
        quick_two_sum(r.hi, r.lo + f)
    }
}

impl Neg for Dd {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}

impl Sub for Dd {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        self + (-b)
    }
}

impl Mul for Dd {
    type Output = Self;
    fn mul(self, b: Self) -> Self {
        let p = self.hi * b.hi;
        let e = self.hi.mul_add(b.hi, -p);
        let e = self.hi.mul_add(b.lo, self.lo.mul_add(b.hi, e));
        quick_two_sum(p, e)
    }
}

impl Div for Dd {
    type Output = Self;
    fn div(self, b: Self) -> Self {
        let q1 = self.hi / b.hi;
        let r = self - b * Dd::from_f64(q1);
        let q2 = r.hi / b.hi;
        let r = r - b * Dd::from_f64(q2);
        let q3 = r.hi / b.hi;
        quick_two_sum(q1, q2) + Dd::from_f64(q3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1/3 · 3 = 1 to double-double precision, and the low word carries the f64 rounding of 1/3.
    ///
    /// Verifies: NUM-001
    #[test]
    fn division_and_product_are_double_double_accurate() {
        let three = Dd::from_f64(3.0);
        let third = Dd::from_f64(1.0) / three;
        let back = third * three - Dd::from_f64(1.0);
        assert!(back.to_f64().abs() < f64::EPSILON * f64::EPSILON);
        assert!(third.lo != 0.0);
        let s = Dd::from_f64(1.0) + Dd::from_f64(f64::EPSILON / 4.0);
        assert!((s - Dd::from_f64(1.0)).to_f64() > 0.0);
    }
}
