// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E15xx`).

use ferroastro_core::ErrorCode;

/// An oracle-golden error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OracleError {
    /// `MANIFEST.json` is missing, malformed or incomplete (`FA-E1500`).
    #[error(
        "FA-E1500: manifest {path}: {problem}; fix: regenerate the golden with tools/oracles/run_oracles.sh (CI-003)"
    )]
    Manifest {
        /// The manifest or golden directory.
        path: String,
        /// What is wrong.
        problem: String,
    },
    /// A SHA-256 differs from the manifest entry or pin (`FA-E1501`).
    #[error("FA-E1501: {file}: SHA-256 {actual} differs from {expected} ({what}); fix: {fix}")]
    Checksum {
        /// File.
        file: String,
        /// What the expected digest is (manifest output, input pin).
        what: &'static str,
        /// Expected digest.
        expected: String,
        /// Actual digest.
        actual: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// A golden file is malformed (`FA-E1502`).
    #[error("FA-E1502: golden {file}: {problem}")]
    Golden {
        /// File.
        file: String,
        /// What is wrong.
        problem: String,
    },
    /// A run and a golden cannot be compared (`FA-E1503`).
    #[error("FA-E1503: cannot compare {what}: {problem}; fix: run the golden's scenario unchanged")]
    Comparison {
        /// What was compared.
        what: String,
        /// What is missing or inconsistent.
        problem: String,
    },
}

impl OracleError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            OracleError::Manifest { .. } => ErrorCode::OracleManifest,
            OracleError::Checksum { .. } => ErrorCode::OracleChecksum,
            OracleError::Golden { .. } => ErrorCode::OracleGolden,
            OracleError::Comparison { .. } => ErrorCode::OracleComparison,
        }
    }
}
