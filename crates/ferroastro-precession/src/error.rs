// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E06xx`, API-002).

use ferroastro_core::ErrorCode;
use ferroastro_integrators::IntegratorError;

/// Errors from element conversions and precession estimators.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PrecessionError {
    /// A value is outside the domain of a conversion.
    #[error(
        "FA-E0600: {what} = {value} is outside the domain of this conversion; fix: elliptic \
         orbits need 0 <= e < 1, a > 0, GM > 0 and a non-degenerate state"
    )]
    Domain {
        /// Which quantity.
        what: &'static str,
        /// Its value.
        value: f64,
    },
    /// A fit could not be made.
    #[error("FA-E0601: precession fit failed: {0}")]
    Fit(String),
    /// A reported rate lacks a mandatory metadata field (PHY-015).
    #[error(
        "FA-E0602: rate report is missing its {field}; fix: every reported perihelion rate must \
         state frame, epoch, fit span, fit model, constants version, gauge and estimator"
    )]
    MissingMetadata {
        /// The missing field.
        field: &'static str,
    },
    /// Sampled input to an estimator is unusable.
    #[error(
        "FA-E0603: estimator input unusable: {0}; fix: supply strictly increasing, finite samples \
         dense enough to resolve every orbit (see the estimator's documentation)"
    )]
    EstimatorInput(String),
}

impl PrecessionError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            PrecessionError::Domain { .. } => ErrorCode::OrbitDomain,
            PrecessionError::Fit(_) => ErrorCode::FitDegenerate,
            PrecessionError::MissingMetadata { .. } => ErrorCode::ReportMetadataMissing,
            PrecessionError::EstimatorInput(_) => ErrorCode::EstimatorInput,
        }
    }
}

impl From<IntegratorError> for PrecessionError {
    fn from(e: IntegratorError) -> Self {
        PrecessionError::Fit(e.to_string())
    }
}
