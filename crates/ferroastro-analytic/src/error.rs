// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E0600`, API-002).

use ferroastro_core::ErrorCode;

/// Errors from element conversions and analytic formulas.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum AnalyticError {
    /// A value is outside the domain of a conversion or formula.
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
}

impl AnalyticError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            AnalyticError::Domain { .. } => ErrorCode::OrbitDomain,
        }
    }
}
