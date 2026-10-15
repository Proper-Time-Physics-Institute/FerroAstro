// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E04xx`, API-002).

use ferroastro_core::ErrorCode;

/// Errors raised while integrating.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum IntegratorError {
    /// A state, acceleration or step became NaN or infinite.
    #[error(
        "FA-E0400: non-finite {what} at t = {t}; fix: check the force model for a singularity \
         (close encounter, zero distance) or the initial conditions for NaN"
    )]
    NonFinite {
        /// Which quantity (`"acceleration"`, `"position"`, `"step"`).
        what: &'static str,
        /// Time at which it happened (integrator time units).
        t: f64,
    },
    /// The adaptive step fell below the configured minimum.
    #[error(
        "FA-E0401: step size {dt:e} at t = {t} is below the minimum {min_dt:e}; fix: the problem \
         has a near-singularity or `epsilon` is too strict for f64"
    )]
    StepUnderflow {
        /// Proposed step.
        dt: f64,
        /// Configured minimum.
        min_dt: f64,
        /// Time.
        t: f64,
    },
    /// A configuration value is invalid.
    #[error("FA-E0402: invalid integrator setting {name} = {value}; valid: {valid}")]
    Config {
        /// Setting name.
        name: &'static str,
        /// Rejected value.
        value: f64,
        /// Valid range.
        valid: &'static str,
    },
    /// A slice has the wrong length.
    #[error(
        "FA-E0403: {what} has length {got}, expected {expected}; fix: pass 3 coordinates per body"
    )]
    Dimension {
        /// Which slice.
        what: &'static str,
        /// Its length.
        got: usize,
        /// The system dimension.
        expected: usize,
    },
    /// A least-squares fit is ill-posed.
    #[error("FA-E0601: fit is ill-posed: {message}; fix: supply more, distinct, finite samples")]
    Fit {
        /// What is wrong.
        message: String,
    },
    /// The dynamics (force model) reported an error with its own stable code.
    #[error("{code}: force model error: {message}")]
    Dynamics {
        /// The force model's stable code (an `FA-E05xx` code).
        code: ErrorCode,
        /// Human-readable message with remediation.
        message: String,
    },
}

impl IntegratorError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            IntegratorError::NonFinite { .. } => ErrorCode::IntegratorNonFinite,
            IntegratorError::StepUnderflow { .. } => ErrorCode::IntegratorStepUnderflow,
            IntegratorError::Config { .. } => ErrorCode::IntegratorConfig,
            IntegratorError::Dimension { .. } => ErrorCode::IntegratorDimension,
            IntegratorError::Fit { .. } => ErrorCode::FitDegenerate,
            IntegratorError::Dynamics { code, .. } => *code,
        }
    }
}
