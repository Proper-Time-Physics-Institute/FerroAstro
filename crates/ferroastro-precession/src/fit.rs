// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Secular precession-rate fit with the metadata every result must carry (AGENTS.md rule 4,
//! ADR-0011): frame, epoch, span, fit model, estimator, gauge, integrator and constants.

use crate::PrecessionError;
use ferroastro_analytic::advance::rate_arcsec_per_century;
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Angle, AngularRate, TimeSpan};
use std::fmt;

/// Fit model of [`fit_secular_rate`].
pub const LINEAR_FIT_MODEL: &str =
    "varpi(t) = B + varpi_dot * t, ordinary least squares, all samples equal weight";

/// Provenance of a precession result.
#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    /// Estimator, e.g. `"LRL eccentricity-vector longitude"`.
    pub estimator: &'static str,
    /// Frame in which the longitude is measured.
    pub frame: String,
    /// Initial epoch of the run.
    pub epoch: Epoch<Tdb>,
    /// Integrated span.
    pub span: TimeSpan,
    /// Force model and its gauge.
    pub dynamics: String,
    /// Integrator and settings.
    pub integrator: String,
    /// Constants set (name and checksum).
    pub constants: String,
}

/// A fitted secular precession rate.
#[derive(Debug, Clone, PartialEq)]
pub struct PrecessionEstimate {
    /// Fitted rate.
    pub rate: AngularRate,
    /// One-sigma formal error of the slope from the residual scatter. The residuals of a
    /// round-off random walk are correlated, so this **understates** the true uncertainty.
    pub rate_stderr: AngularRate,
    /// RMS of the residuals.
    pub rms_residual: Angle,
    /// Number of samples.
    pub samples: usize,
    /// Fit model.
    pub fit_model: &'static str,
    /// Provenance.
    pub metadata: Metadata,
}

impl PrecessionEstimate {
    /// The rate in ″/cy.
    #[must_use]
    pub fn arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.rate)
    }

    /// The formal error in ″/cy.
    #[must_use]
    pub fn stderr_arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.rate_stderr)
    }
}

impl fmt::Display for PrecessionEstimate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = &self.metadata;
        writeln!(
            f,
            "varpi_dot = {:e} arcsec/cy (formal 1-sigma {:e}), rms residual {:e} arcsec, {} samples",
            self.arcsec_per_century(),
            self.stderr_arcsec_per_century(),
            self.rms_residual.arcsec(),
            self.samples
        )?;
        writeln!(f, "  estimator : {}", m.estimator)?;
        writeln!(f, "  frame     : {}", m.frame)?;
        writeln!(
            f,
            "  epoch     : JD {} {} ; span {} d",
            m.epoch.jd(),
            m.epoch.scale_name(),
            m.span.seconds() / ferroastro_core::time::SECONDS_PER_DAY
        )?;
        writeln!(f, "  fit model : {}", self.fit_model)?;
        writeln!(f, "  dynamics  : {}", m.dynamics)?;
        writeln!(f, "  integrator: {}", m.integrator)?;
        write!(f, "  constants : {}", m.constants)
    }
}

/// Result of fitting `y = intercept + slope·x` (intercept evaluated at `x = 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineFit {
    /// Fitted slope.
    pub slope: f64,
    /// Fitted intercept at `x = 0`.
    pub intercept: f64,
    /// One-sigma standard error of the slope from the residual scatter (meaningful only for
    /// independent residuals).
    pub slope_stderr: f64,
    /// Root-mean-square residual.
    pub rms_residual: f64,
    /// Number of points.
    pub points: usize,
}

/// Neumaier-compensated sum.
fn compensated_sum(xs: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0_f64, 0.0_f64);
    for x in xs {
        let t = s + x;
        c += if s.abs() >= x.abs() {
            (s - t) + x
        } else {
            (x - t) + s
        };
        s = t;
    }
    s + c
}

/// Ordinary least squares `y = a + b x`, computed about the mean of `x` for conditioning.
///
/// The estimators' own line fit, so that this crate needs no integrator (CRATE-PLAN: estimators
/// contain no propagation).
///
/// # Errors
/// `FA-E0601` if fewer than 3 points, the lengths differ, any value is non-finite, or all `x`
/// are equal.
pub fn fit_line(x: &[f64], y: &[f64]) -> Result<LineFit, PrecessionError> {
    if x.len() != y.len() || x.len() < 3 {
        return Err(PrecessionError::Fit(format!(
            "line fit needs >= 3 paired points, got {} x and {} y",
            x.len(),
            y.len()
        )));
    }
    if x.iter().chain(y).any(|v| !v.is_finite()) {
        return Err(PrecessionError::Fit(
            "line fit input contains NaN or infinity".into(),
        ));
    }
    let n = x.len();
    let nf =
        f64::from(u32::try_from(n).map_err(|_| PrecessionError::Fit("too many points".into()))?);
    let xm = compensated_sum(x.iter().copied()) / nf;
    let ym = compensated_sum(y.iter().copied()) / nf;
    let sxx = compensated_sum(x.iter().map(|xi| (xi - xm) * (xi - xm)));
    let sxy = compensated_sum(x.iter().zip(y).map(|(xi, yi)| (xi - xm) * (yi - ym)));
    if sxx <= 0.0 {
        return Err(PrecessionError::Fit("all abscissae are equal".into()));
    }
    let slope = sxy / sxx;
    let ssr = compensated_sum(x.iter().zip(y).map(|(xi, yi)| {
        let r = yi - slope.mul_add(xi - xm, ym);
        r * r
    }));
    Ok(LineFit {
        slope,
        intercept: slope.mul_add(-xm, ym),
        slope_stderr: (ssr / (nf - 2.0) / sxx).sqrt(),
        rms_residual: (ssr / nf).sqrt(),
        points: n,
    })
}

/// Fit a straight line to unwrapped longitudes (rad) against time (s from the epoch).
///
/// # Errors
/// `FA-E0601` for fewer than 3 samples, mismatched lengths, non-finite values or a zero span.
pub fn fit_secular_rate(
    times_s: &[f64],
    longitudes_rad: &[f64],
    metadata: Metadata,
) -> Result<PrecessionEstimate, PrecessionError> {
    let fit = fit_line(times_s, longitudes_rad)?;
    let rate = |z: f64| {
        AngularRate::from_rad_per_s(z).map_err(|_| PrecessionError::Fit(format!("rate {z}")))
    };
    Ok(PrecessionEstimate {
        rate: rate(fit.slope)?,
        rate_stderr: rate(fit.slope_stderr)?,
        rms_residual: Angle::from_radians(fit.rms_residual)
            .map_err(|_| PrecessionError::Fit("residual".into()))?,
        samples: fit.points,
        fit_model: LINEAR_FIT_MODEL,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An exact line is recovered; degenerate inputs are refused with FA-E0601.
    #[test]
    fn line_fit_recovers_exact_line() {
        let x = [0.0, 1.0, 2.0, 3.0];
        let y = x.map(|v| 2.0_f64.mul_add(v, 1.0));
        let f = fit_line(&x, &y).unwrap();
        assert!((f.slope - 2.0).abs() <= 4.0 * f64::EPSILON);
        assert!((f.intercept - 1.0).abs() <= 8.0 * f64::EPSILON);
        assert!(f.rms_residual <= 4.0 * f64::EPSILON);
        assert!(fit_line(&x[..2], &y[..2]).is_err());
        assert!(fit_line(&[1.0; 3], &[0.0, 1.0, 2.0]).is_err());
        assert!(fit_line(&[0.0, 1.0, f64::NAN], &[0.0; 3]).is_err());
    }
}
