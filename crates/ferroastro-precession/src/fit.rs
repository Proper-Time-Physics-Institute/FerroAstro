// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Secular precession-rate fit with the metadata every result must carry (AGENTS.md rule 4,
//! ADR-0011): frame, epoch, span, fit model, estimator, gauge, integrator and constants.

use crate::PrecessionError;
use crate::analytic::rate_arcsec_per_century;
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Angle, AngularRate, TimeSpan};
use ferroastro_integrators::diagnostics::fit_line;
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
