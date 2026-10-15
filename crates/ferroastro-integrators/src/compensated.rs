// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compensated (Kahan) summation, used for the state update (NUM-001) and for diagnostics.
//!
//! The running value is `sum`; `comp` holds the negated low-order part that was lost, so the
//! represented total is `sum − comp`. Kahan 1965, "Further remarks on reducing truncation errors",
//! Commun. ACM 8, 40; the use for the IAS15 state update follows Rein & Spiegel 2015 §3.5.

/// Add `x` to the compensated pair `(sum, comp)`.
#[inline]
pub fn kahan_add(sum: &mut f64, comp: &mut f64, x: f64) {
    let y = x - *comp;
    let t = *sum + y;
    *comp = (t - *sum) - y;
    *sum = t;
}

/// Compensated sum of a slice.
#[must_use]
pub fn kahan_sum(values: &[f64]) -> f64 {
    let (mut s, mut c) = (0.0, 0.0);
    for &x in values {
        kahan_add(&mut s, &mut c, x);
    }
    s - c
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: NUM-001
    #[test]
    fn compensated_sum_keeps_low_order_bits() {
        // 1 followed by many increments of half an ulp of 1: naive summation loses all of them.
        let tiny = f64::EPSILON / 2.0;
        let n = 1000_u32;
        let mut values = vec![1.0];
        values.extend(std::iter::repeat_n(tiny, n as usize));
        let naive: f64 = values.iter().sum();
        let exact_excess = f64::from(n) * tiny;
        assert!(
            (naive - 1.0).abs() < tiny,
            "naive summation unexpectedly kept bits"
        );
        let compensated = kahan_sum(&values);
        assert!(((compensated - 1.0) - exact_excess).abs() <= 2.0 * f64::EPSILON);
    }
}
