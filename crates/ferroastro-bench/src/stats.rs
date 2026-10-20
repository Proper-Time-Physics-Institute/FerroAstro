// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Timing statistics of one speed-table entry (CI-008: the median of ≥ 5 runs, with the
//! run-to-run spread recorded as the range).

use serde::{Deserialize, Serialize};

/// The minimum number of runs per timed entry (CI-008).
pub const MIN_RUNS: usize = 5;

/// Wall-clock times of the runs of one entry, in run order, and their summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    /// Wall-clock seconds of each run, in run order.
    pub runs_s: Vec<f64>,
    /// Median (the mean of the two middle values for an even count).
    pub median_s: f64,
    /// Fastest run.
    pub min_s: f64,
    /// Slowest run.
    pub max_s: f64,
}

impl Timing {
    /// Summarise `runs_s`; `None` if it is empty or holds a value that is not finite and ≥ 0.
    #[must_use]
    pub fn new(runs_s: Vec<f64>) -> Option<Self> {
        if runs_s.is_empty() || runs_s.iter().any(|x| !x.is_finite() || *x < 0.0) {
            return None;
        }
        let mut s = runs_s.clone();
        s.sort_by(f64::total_cmp);
        let n = s.len();
        let mid = |k: usize| s.get(k).copied();
        let median_s = if n % 2 == 1 {
            mid(n / 2)?
        } else {
            0.5 * (mid(n / 2 - 1)? + mid(n / 2)?)
        };
        Some(Self {
            min_s: *s.first()?,
            max_s: *s.last()?,
            median_s,
            runs_s,
        })
    }

    /// The run-to-run spread: `max − min`.
    #[must_use]
    pub fn range_s(&self) -> f64 {
        self.max_s - self.min_s
    }

    /// Whether the entry has the CI-008 minimum number of runs.
    #[must_use]
    pub fn meets_minimum(&self) -> bool {
        self.runs_s.len() >= MIN_RUNS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_even_and_invalid() {
        let t = Timing::new(vec![3.0, 1.0, 2.0, 10.0, 4.0]).unwrap();
        assert_eq!(t.median_s.to_bits(), 3.0_f64.to_bits());
        assert_eq!(t.range_s().to_bits(), 9.0_f64.to_bits());
        assert!(t.meets_minimum());
        let e = Timing::new(vec![4.0, 1.0, 2.0, 3.0]).unwrap();
        assert_eq!(e.median_s.to_bits(), 2.5_f64.to_bits()); // lint: allow-literal(synthetic wall-clock seconds)
        assert!(!e.meets_minimum());
        assert!(Timing::new(vec![]).is_none());
        assert!(Timing::new(vec![1.0, f64::NAN]).is_none());
        assert!(Timing::new(vec![-1.0]).is_none());
    }
}
