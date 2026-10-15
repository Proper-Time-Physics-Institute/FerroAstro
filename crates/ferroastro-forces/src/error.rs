// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E05xx`, API-002).

use ferroastro_core::ErrorCode;
use ferroastro_integrators::IntegratorError;

/// Errors from building or evaluating force models.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum ForceError {
    /// Invalid parameters.
    #[error("FA-E0500: invalid force-model parameter {name} = {value}; valid: {valid}")]
    Invalid {
        /// Parameter.
        name: &'static str,
        /// Rejected value.
        value: f64,
        /// Valid range.
        valid: &'static str,
    },
    /// Two bodies coincide.
    #[error(
        "FA-E0501: bodies {i} and {j} coincide at t = {t} s; fix: point masses cannot collide, \
         check the initial conditions"
    )]
    Singular {
        /// First body.
        i: usize,
        /// Second body.
        j: usize,
        /// Time (s).
        t: f64,
    },
}

impl ForceError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            ForceError::Invalid { .. } => ErrorCode::ForceModelInvalid,
            ForceError::Singular { .. } => ErrorCode::ForceModelSingular,
        }
    }
}

impl From<ForceError> for IntegratorError {
    fn from(e: ForceError) -> Self {
        IntegratorError::Dynamics {
            code: e.code(),
            message: e.to_string(),
        }
    }
}
