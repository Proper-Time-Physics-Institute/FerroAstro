// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E14xx`, plus those of the crates it drives, with their own codes).

use ferroastro_core::ErrorCode;
use ferroastro_precession::PrecessionError;
use ferroastro_propagate::PropagateError;

/// A differential-run error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DiffrunError {
    /// The plan is inconsistent (`FA-E1400`).
    #[error("FA-E1400: differential plan {plan}: {problem}; fix: {fix}")]
    Plan {
        /// Plan name.
        plan: String,
        /// What is wrong.
        problem: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// Two runs of one plan do not share identical initial conditions (`FA-E1401`).
    #[error(
        "FA-E1401: runs {a} and {b} do not share identical initial conditions: {problem}; fix: \
         build every configuration from one scenario template, IC source and precision"
    )]
    IcMismatch {
        /// First run.
        a: String,
        /// Second run.
        b: String,
        /// The difference.
        problem: String,
    },
    /// The on-disk run store failed (`FA-E1402`).
    #[error("FA-E1402: run store {path}: {problem}")]
    Store {
        /// File or directory.
        path: String,
        /// What failed.
        problem: String,
    },
    /// A run failed (`FA-E13xx` and the codes of the crates it drives).
    #[error(transparent)]
    Propagate(#[from] PropagateError),
    /// The estimator failed (`FA-E06xx`).
    #[error(transparent)]
    Precession(#[from] PrecessionError),
}

impl DiffrunError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            DiffrunError::Plan { .. } => ErrorCode::DiffrunPlan,
            DiffrunError::IcMismatch { .. } => ErrorCode::DiffrunIcMismatch,
            DiffrunError::Store { .. } => ErrorCode::DiffrunStore,
            DiffrunError::Propagate(e) => e.code(),
            DiffrunError::Precession(e) => e.code(),
        }
    }
}

pub(crate) fn plan_error(
    plan: &str,
    problem: impl Into<String>,
    fix: &'static str,
) -> DiffrunError {
    DiffrunError::Plan {
        plan: plan.to_owned(),
        problem: problem.into(),
        fix,
    }
}
