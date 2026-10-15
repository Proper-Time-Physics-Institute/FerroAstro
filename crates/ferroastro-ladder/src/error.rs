// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of the ladder (`FA-E07xx`, plus those of the crates it drives).

use ferroastro_constants::ConstantsError;
use ferroastro_core::ErrorCode;
use ferroastro_forces::ForceError;
use ferroastro_integrators::IntegratorError;
use ferroastro_precession::PrecessionError;

/// Errors from building or running ladder rungs.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum LadderError {
    /// An unknown rung or rung set was requested.
    #[error("FA-E0700: unknown {what} {value:?}; valid: {valid}")]
    Selection {
        /// What was selected (e.g. "rung set").
        what: &'static str,
        /// The rejected value.
        value: String,
        /// The valid choices.
        valid: &'static str,
    },
    /// A scenario file is missing or malformed.
    #[error("FA-E0701: scenario {name}: {problem}")]
    Scenario {
        /// Scenario name.
        name: String,
        /// What is wrong.
        problem: String,
    },
    /// Constants error.
    #[error(transparent)]
    Constants(#[from] ConstantsError),
    /// Force-model error.
    #[error(transparent)]
    Force(#[from] ForceError),
    /// Estimator or element-conversion error.
    #[error(transparent)]
    Precession(#[from] PrecessionError),
    /// Integrator error.
    #[error(transparent)]
    Integrator(#[from] IntegratorError),
}

impl LadderError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            LadderError::Selection { .. } => ErrorCode::LadderSelection,
            LadderError::Scenario { .. } => ErrorCode::LadderScenario,
            LadderError::Constants(e) => e.code(),
            LadderError::Force(e) => e.code(),
            LadderError::Precession(e) => e.code(),
            LadderError::Integrator(e) => e.code(),
        }
    }
}
