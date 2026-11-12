// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Neumaier's compensated summation, generic over [`Real`].
//!
//! Neumaier 1974 (ZAMM 54, 39), the "improved Kahan–Babuška" algorithm: the running sum `s` and a
//! correction `c` collecting the exact rounding error of every addition (whichever operand is
//! larger in magnitude); the total is `s + c`. Unlike Kahan's algorithm it stays accurate when an
//! addend exceeds the running sum. Its error is bounded by `2u|S| + O(n u²) Σ|xᵢ|` (Higham 2002,
//! *Accuracy and Stability of Numerical Algorithms*, §4.3).

use crate::Real;

/// A compensated running sum.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Neumaier<R: Real = f64> {
    sum: R,
    comp: R,
}

impl<R: Real> Neumaier<R> {
    /// An empty sum.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sum: R::ZERO,
            comp: R::ZERO,
        }
    }

    /// Add `x`.
    #[inline]
    pub fn add(&mut self, x: R) {
        let t = self.sum + x;
        if self.sum.abs() >= x.abs() {
            self.comp += (self.sum - t) + x;
        } else {
            self.comp += (x - t) + self.sum;
        }
        self.sum = t;
    }

    /// The compensated total.
    #[must_use]
    pub fn total(&self) -> R {
        self.sum + self.comp
    }

    /// Compensated sum of a slice.
    #[must_use]
    pub fn sum(values: &[R]) -> R {
        let mut acc = Self::new();
        for &x in values {
            acc.add(x);
        }
        acc.total()
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "error-free transformations and special values have exact expected results"
)]
mod tests {
    use super::*;
    use crate::DoubleDouble;

    /// `1 + 2^100 + 1 − 2^100 = 2`: naive summation and Kahan both return 0; Neumaier returns 2.
    ///
    /// Verifies: NUM-006
    #[test]
    fn neumaier_recovers_cancelled_addends() {
        let big = 2.0_f64.powi(100);
        let xs = [1.0, big, 1.0, -big];
        assert_eq!(xs.iter().sum::<f64>(), 0.0);
        assert_eq!(Neumaier::<f64>::sum(&xs), 2.0);
        let dd: [DoubleDouble; 4] = xs.map(DoubleDouble::from_f64);
        assert_eq!(Neumaier::<DoubleDouble>::sum(&dd), DoubleDouble::TWO);
    }
}
