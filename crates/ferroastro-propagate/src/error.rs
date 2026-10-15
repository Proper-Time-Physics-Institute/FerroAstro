// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E13xx`, plus those of the crates it drives, with their own codes).

use ferroastro_analytic::AnalyticError;
use ferroastro_constants::ConstantsError;
use ferroastro_core::ErrorCode;
use ferroastro_forces::ForceError;
use ferroastro_frames::FrameError;
use ferroastro_integrators::IntegratorError;
use ferroastro_precession::PrecessionError;
use ferroastro_scenario::ScenarioError;
use ferroastro_system::SystemError;

/// Errors from setting up, running, checkpointing or encoding a propagation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum PropagateError {
    /// A run cannot be set up from the scenario and the data supplied.
    #[error("FA-E1300: cannot set up scenario {scenario}: {problem}; fix: {fix}")]
    Setup {
        /// Scenario name.
        scenario: String,
        /// What is missing or wrong.
        problem: String,
        /// Remediation.
        fix: &'static str,
    },
    /// A run ended without what the scenario requires.
    #[error("FA-E1301: scenario {scenario}: {problem}")]
    Run {
        /// Scenario name.
        scenario: String,
        /// What happened.
        problem: String,
    },
    /// A checkpoint is malformed, or belongs to another scenario, precision or mode.
    #[error(
        "FA-E1302: checkpoint: {problem}; fix: resume with the scenario file and data that \
         produced the checkpoint, from an unmodified checkpoint file"
    )]
    Checkpoint {
        /// What is wrong.
        problem: String,
    },
    /// An output or batch could not be encoded, decoded or scheduled.
    #[error("FA-E1303: {problem}")]
    Output {
        /// What is wrong.
        problem: String,
    },
    /// Scenario error (`FA-E12xx`).
    #[error(transparent)]
    Scenario(#[from] ScenarioError),
    /// Integrator error (`FA-E04xx`).
    #[error(transparent)]
    Integrator(#[from] IntegratorError),
    /// Force-model error (`FA-E05xx`).
    #[error(transparent)]
    Force(#[from] ForceError),
    /// System or initial-condition error (`FA-E11xx`).
    #[error(transparent)]
    System(#[from] SystemError),
    /// Estimator or passage-search error (`FA-E06xx`).
    #[error(transparent)]
    Precession(#[from] PrecessionError),
    /// Element-conversion error (`FA-E0600`).
    #[error(transparent)]
    Analytic(#[from] AnalyticError),
    /// Constants error (`FA-E03xx`).
    #[error(transparent)]
    Constants(#[from] ConstantsError),
    /// Frame error (`FA-E09xx`), kept as its code and message.
    #[error("{message}")]
    Frame {
        /// The frame error's code.
        code: ErrorCode,
        /// Its message.
        message: String,
    },
}

impl PropagateError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            PropagateError::Setup { .. } => ErrorCode::PropagateSetup,
            PropagateError::Run { .. } => ErrorCode::PropagateRun,
            PropagateError::Checkpoint { .. } => ErrorCode::PropagateCheckpoint,
            PropagateError::Output { .. } => ErrorCode::PropagateOutput,
            PropagateError::Scenario(e) => e.code(),
            PropagateError::Integrator(e) => e.code(),
            PropagateError::Force(e) => e.code(),
            PropagateError::System(e) => e.code(),
            PropagateError::Precession(e) => e.code(),
            PropagateError::Analytic(e) => e.code(),
            PropagateError::Constants(e) => e.code(),
            PropagateError::Frame { code, .. } => *code,
        }
    }
}

impl From<FrameError> for PropagateError {
    fn from(e: FrameError) -> Self {
        PropagateError::Frame {
            code: e.code(),
            message: e.to_string(),
        }
    }
}
