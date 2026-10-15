// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Dense linear least squares by Householder QR with column equilibration (Golub & Van Loan,
//! *Matrix Computations*, 4th ed., §5.2 and §5.3). It is used by the Park-2017 fit, where the
//! design matrix has a few thousand rows and at most a few dozen columns.

use crate::PrecessionError;

/// Solution of `min ‖A c − y‖₂`.
#[derive(Debug, Clone, PartialEq)]
pub struct LsqSolution {
    /// Coefficients, one per column.
    pub coef: Vec<f64>,
    /// Diagonal of `(AᵀA)⁻¹`. Multiply by the residual variance to get formal variances.
    pub cov_unscaled_diag: Vec<f64>,
    /// Residuals `y − A c`.
    pub residuals: Vec<f64>,
    /// Residual sum of squares.
    pub rss: f64,
}

impl LsqSolution {
    /// Residual variance `rss / (n − p)`. Returns NaN if `n ≤ p`.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "row and column counts are far below 2^53"
    )]
    pub fn residual_variance(&self) -> f64 {
        let n = self.residuals.len();
        let p = self.coef.len();
        if n > p {
            self.rss / (n - p) as f64
        } else {
            f64::NAN
        }
    }

    /// Formal one-sigma error of coefficient `i`, or NaN if `i` is out of range.
    #[must_use]
    pub fn sigma(&self, i: usize) -> f64 {
        self.cov_unscaled_diag
            .get(i)
            .map_or(f64::NAN, |d| (d * self.residual_variance()).sqrt())
    }
}

/// Relative threshold on `|R_kk| / max|R_jj|` below which the design is treated as rank deficient.
pub const RANK_TOLERANCE: f64 = 1e-12;

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Solve the least-squares problem with columns `cols` (each of length `n`) and data `y`.
///
/// # Errors
/// `FA-E0601` if there are no columns, the columns have unequal lengths, `n ≤ p`, a value is not
/// finite, a column is zero, or the design is rank deficient (relative pivot <
/// [`RANK_TOLERANCE`] after equilibration).
#[expect(
    clippy::indexing_slicing,
    reason = "every index is bounded by the dimensions n and p, which are checked on entry"
)]
pub fn lstsq(cols: &[Vec<f64>], y: &[f64]) -> Result<LsqSolution, PrecessionError> {
    let fit = |m: String| PrecessionError::Fit(m);
    let p = cols.len();
    let n = y.len();
    if p == 0 || n <= p {
        return Err(fit(format!(
            "least squares needs n > p > 0 (n = {n}, p = {p})"
        )));
    }
    if cols.iter().any(|c| c.len() != n) {
        return Err(fit("design columns have unequal lengths".into()));
    }
    if !y.iter().chain(cols.iter().flatten()).all(|v| v.is_finite()) {
        return Err(fit("non-finite value in the design or the data".into()));
    }
    // Equilibrate: scale every column to unit 2-norm.
    let scale: Vec<f64> = cols.iter().map(|c| dot(c, c).sqrt()).collect();
    if scale.iter().any(|&s| s <= 0.0 || !s.is_finite()) {
        return Err(fit("a design column is identically zero".into()));
    }
    let mut a: Vec<Vec<f64>> = cols
        .iter()
        .zip(&scale)
        .map(|(c, s)| c.iter().map(|v| v / s).collect())
        .collect();
    let mut qty = y.to_vec();
    let mut rdiag = vec![0.0; p];
    for k in 0..p {
        let norm_x = dot(&a[k][k..], &a[k][k..]).sqrt();
        if norm_x == 0.0 {
            return Err(fit(format!("design column {k} is linearly dependent")));
        }
        let alpha = if a[k][k] > 0.0 { -norm_x } else { norm_x };
        let mut v: Vec<f64> = a[k][k..].to_vec();
        v[0] -= alpha;
        let vv = dot(&v, &v);
        if vv > 0.0 {
            for col in a.iter_mut().skip(k) {
                let s = 2.0 * dot(&v, &col[k..]) / vv;
                for (x, vi) in col[k..].iter_mut().zip(&v) {
                    *x -= s * vi;
                }
            }
            let s = 2.0 * dot(&v, &qty[k..]) / vv;
            for (x, vi) in qty[k..].iter_mut().zip(&v) {
                *x -= s * vi;
            }
        }
        rdiag[k] = alpha;
    }
    let rmax = rdiag.iter().fold(0.0_f64, |m, r| m.max(r.abs()));
    if let Some(k) = rdiag.iter().position(|r| r.abs() < RANK_TOLERANCE * rmax) {
        return Err(fit(format!(
            "design is rank deficient at column {k} (relative pivot {:e})",
            rdiag[k].abs() / rmax
        )));
    }
    // R is stored in a[j][i] for i ≤ j (column j, row i), with the diagonal in rdiag.
    let r = |i: usize, j: usize| if i == j { rdiag[i] } else { a[j][i] };
    // Back substitution R c = (Qᵀ y)[..p].
    let mut c = vec![0.0; p];
    for i in (0..p).rev() {
        let s: f64 = ((i + 1)..p).map(|j| r(i, j) * c[j]).sum();
        c[i] = (qty[i] - s) / r(i, i);
    }
    // R⁻¹ (upper triangular), column by column; diag((AᵀA)⁻¹) = row sums of squares of R⁻¹.
    let mut rinv = vec![vec![0.0; p]; p];
    #[expect(
        clippy::needless_range_loop,
        reason = "triangular solve reads earlier rows of rinv while writing column j"
    )]
    for j in 0..p {
        rinv[j][j] = 1.0 / r(j, j);
        for i in (0..j).rev() {
            let s: f64 = ((i + 1)..=j).map(|m| r(i, m) * rinv[m][j]).sum();
            rinv[i][j] = -s / r(i, i);
        }
    }
    let coef: Vec<f64> = c.iter().zip(&scale).map(|(ci, s)| ci / s).collect();
    let cov_unscaled_diag: Vec<f64> = rinv
        .iter()
        .zip(&scale)
        .map(|(row, s)| dot(row, row) / (s * s))
        .collect();
    let residuals: Vec<f64> = (0..n)
        .map(|i| {
            y[i] - cols
                .iter()
                .zip(&coef)
                .map(|(col, k)| col[i] * k)
                .sum::<f64>()
        })
        .collect();
    let rss = dot(&residuals, &residuals);
    Ok(LsqSolution {
        coef,
        cov_unscaled_diag,
        residuals,
        rss,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::suboptimal_flops, reason = "test fixtures favour readability")]
    use super::*;

    /// An exactly representable quadratic plus a sinusoid is recovered to round-off, and the
    /// formal errors equal the textbook straight-line formula.
    ///
    /// Verifies: NUM-011
    #[test]
    fn recovers_exact_coefficients_and_textbook_errors() {
        let t: Vec<f64> = (0..200).map(|i| f64::from(i) / 10.0 - 10.0).collect();
        let truth = [0.5, -2.0, 0.25, 3.0];
        let y: Vec<f64> = t
            .iter()
            .map(|&x| truth[0] + truth[1] * x + truth[2] * x * x + truth[3] * (0.7 * x).sin()) // lint: allow-literal(math or algorithm fixture, not physics)
            .collect();
        let cols = vec![
            vec![1.0; t.len()],
            t.clone(),
            t.iter().map(|x| x * x).collect(),
            t.iter().map(|x| (0.7 * x).sin()).collect(), // lint: allow-literal(math or algorithm fixture, not physics)
        ];
        let s = lstsq(&cols, &y).unwrap();
        for (c, e) in s.coef.iter().zip(truth) {
            assert!((c - e).abs() < 1e-12, "{c} vs {e}"); // lint: allow-literal(math or algorithm fixture, not physics)
        }
        // Straight line: var(slope) = σ² / Σ(t − t̄)², var(intercept) = σ² Σt² / (n Σ(t − t̄)²).
        let noisy: Vec<f64> = t
            .iter()
            .enumerate()
            .map(|(i, &x)| 1.0 + 2.0 * x + if i % 2 == 0 { 0.1 } else { -0.1 }) // lint: allow-literal(math or algorithm fixture, not physics)
            .collect();
        let s = lstsq(&cols[..2], &noisy).unwrap();
        let n = 200.0; // lint: allow-literal(math or algorithm fixture, not physics)
        let mean = t.iter().sum::<f64>() / n;
        let sxx: f64 = t.iter().map(|x| (x - mean).powi(2)).sum();
        let sigma2 = s.residual_variance();
        assert!((s.sigma(1) - (sigma2 / sxx).sqrt()).abs() < 1e-14); // lint: allow-literal(math or algorithm fixture, not physics)
        let st2: f64 = t.iter().map(|x| x * x).sum();
        assert!((s.sigma(0) - (sigma2 * st2 / (n * sxx)).sqrt()).abs() < 1e-14); // lint: allow-literal(math or algorithm fixture, not physics)
        assert!(s.sigma(7).is_nan());
    }

    /// Verifies: NUM-011
    #[test]
    fn rejects_degenerate_designs() {
        let t: Vec<f64> = (0..10).map(f64::from).collect();
        let code = |cols: &[Vec<f64>], y: &[f64]| lstsq(cols, y).unwrap_err().code();
        let fit = ferroastro_core::ErrorCode::FitDegenerate;
        assert_eq!(
            code(&[t.clone(), t.iter().map(|x| 2.0 * x).collect()], &t),
            fit
        );
        assert_eq!(code(&[vec![0.0; 10]], &t), fit);
        assert_eq!(code(std::slice::from_ref(&t), &t[..5]), fit);
        assert_eq!(code(&[], &t), fit);
        let mut bad = t.clone();
        bad[3] = f64::NAN;
        assert_eq!(code(&[t], &bad), fit);
    }
}
