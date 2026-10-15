// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Straight-line least-squares fits and the Brouwer-law slope diagnostic (NUM-023).
//!
//! Brouwer 1937 (AJ 46, 149): with unbiased rounding, the error of a conserved quantity (energy,
//! angular momentum) random-walks as `t^{1/2}`, and the along-track (phase) error as `t^{3/2}`.
//! [`loglog_slope`] fits `ln y = c + p ln t`; NUM-023 expects `p ≈ ½` and `p ≈ 3/2`.

use crate::IntegratorError;
use crate::compensated::kahan_sum;

/// Result of fitting `y = intercept + slope·x` (intercept evaluated at `x = 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineFit {
    /// Fitted slope.
    pub slope: f64,
    /// Fitted intercept at `x = 0`.
    pub intercept: f64,
    /// One-sigma standard error of the slope from the residual scatter (0 for an exact fit;
    /// meaningful only for independent residuals).
    pub slope_stderr: f64,
    /// Root-mean-square residual.
    pub rms_residual: f64,
    /// Number of points.
    pub points: usize,
}

fn degenerate(message: String) -> IntegratorError {
    IntegratorError::Fit { message }
}

/// Ordinary least squares `y = a + b x`, computed about the mean of `x` for conditioning.
///
/// # Errors
/// `FA-E0601` if fewer than 3 points, the lengths differ, any value is non-finite, or all `x`
/// are equal.
pub fn fit_line(x: &[f64], y: &[f64]) -> Result<LineFit, IntegratorError> {
    if x.len() != y.len() || x.len() < 3 {
        return Err(degenerate(format!(
            "line fit needs >= 3 paired points, got {} x and {} y",
            x.len(),
            y.len()
        )));
    }
    if x.iter().chain(y).any(|v| !v.is_finite()) {
        return Err(degenerate("line fit input contains NaN or infinity".into()));
    }
    let n = x.len();
    let nf = f64::from(u32::try_from(n).map_err(|_| degenerate("too many points".into()))?);
    let xm = kahan_sum(x) / nf;
    let ym = kahan_sum(y) / nf;
    let sxx = kahan_sum(&x.iter().map(|xi| (xi - xm) * (xi - xm)).collect::<Vec<_>>());
    let sxy = kahan_sum(
        &x.iter()
            .zip(y)
            .map(|(xi, yi)| (xi - xm) * (yi - ym))
            .collect::<Vec<_>>(),
    );
    if sxx <= 0.0 {
        return Err(degenerate("all abscissae are equal".into()));
    }
    let slope = sxy / sxx;
    let intercept = slope.mul_add(-xm, ym);
    let ssr = kahan_sum(
        &x.iter()
            .zip(y)
            .map(|(xi, yi)| {
                let r = yi - slope.mul_add(xi - xm, ym);
                r * r
            })
            .collect::<Vec<_>>(),
    );
    let slope_stderr = (ssr / (nf - 2.0) / sxx).sqrt();
    Ok(LineFit {
        slope,
        intercept,
        slope_stderr,
        rms_residual: (ssr / nf).sqrt(),
        points: n,
    })
}

/// Fit `ln y = c + p·ln t` and return the fit in log space (`slope` is the exponent `p`).
/// Points with `t <= 0` or `y <= 0` are skipped.
///
/// # Errors
/// `FA-E0601` if fewer than 3 usable points remain (see [`fit_line`]).
pub fn loglog_slope(t: &[f64], y: &[f64]) -> Result<LineFit, IntegratorError> {
    let (lx, ly): (Vec<f64>, Vec<f64>) = t
        .iter()
        .zip(y)
        .filter(|&(&ti, &yi)| ti > 0.0 && yi > 0.0)
        .map(|(ti, yi)| (ti.ln(), yi.ln()))
        .unzip();
    fit_line(&lx, &ly)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;

    const EXPONENT: f64 = 1.5; // lint: allow-literal(test power-law exponent)
    const ULPS: f64 = 64.0; // lint: allow-literal(round-off allowance in ulps)

    /// Verifies: NUM-023
    #[test]
    fn exact_line_and_power_law_are_recovered() {
        let x: Vec<f64> = (1..=20).map(f64::from).collect();
        let y: Vec<f64> = x.iter().map(|v| 3.0 * v - 2.0).collect();
        let f = fit_line(&x, &y).unwrap();
        assert!((f.slope - 3.0).abs() < 8.0 * f64::EPSILON);
        assert!((f.intercept + 2.0).abs() < ULPS * f64::EPSILON);
        let p: Vec<f64> = x.iter().map(|v| 5.0 * v.powf(EXPONENT)).collect();
        let g = loglog_slope(&x, &p).unwrap();
        assert!((g.slope - EXPONENT).abs() < ULPS * f64::EPSILON);
    }

    /// Verifies: NUM-023
    #[test]
    fn degenerate_inputs_are_errors() {
        assert_eq!(
            fit_line(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0])
                .unwrap_err()
                .code(),
            ErrorCode::FitDegenerate
        );
        assert!(fit_line(&[1.0, 2.0], &[1.0, 2.0]).is_err());
        assert!(fit_line(&[1.0, 2.0, f64::NAN], &[1.0, 2.0, 3.0]).is_err());
    }
}
