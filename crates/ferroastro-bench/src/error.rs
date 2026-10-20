// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E16xx`).

use ferroastro_core::ErrorCode;

/// A benchmark or speed-table error.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BenchError {
    /// A run directory, timing record or oracle output is missing or malformed (`FA-E1600`).
    #[error("FA-E1600: {what}: {problem}; fix: {fix}")]
    Input {
        /// The file or directory.
        what: String,
        /// What is wrong.
        problem: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// A speed table is malformed or two tables cannot be compared (`FA-E1601`).
    #[error("FA-E1601: speed table {what}: {problem}")]
    Table {
        /// Which table or entry.
        what: String,
        /// What is wrong.
        problem: String,
    },
}

impl BenchError {
    /// The stable code of this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            BenchError::Input { .. } => ErrorCode::BenchInput,
            BenchError::Table { .. } => ErrorCode::BenchTable,
        }
    }

    pub(crate) fn input(what: impl Into<String>, problem: impl Into<String>) -> Self {
        BenchError::Input {
            what: what.into(),
            problem: problem.into(),
            fix: "re-run tools/bench/speed_table.sh for this run directory",
        }
    }
}
