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
}

impl PrecessionError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            PrecessionError::Domain { .. } => ErrorCode::OrbitDomain,
            PrecessionError::Fit(_) => ErrorCode::FitDegenerate,
        }
    }
}

impl From<IntegratorError> for PrecessionError {
    fn from(e: IntegratorError) -> Self {
        PrecessionError::Fit(e.to_string())
    }
}
