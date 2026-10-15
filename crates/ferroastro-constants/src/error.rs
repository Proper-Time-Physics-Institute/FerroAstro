// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E03xx`, API-002).

use ferroastro_core::ErrorCode;

/// Errors from parsing `header.440` or loading the committed constants table.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConstantsError {
    /// The `header.440` text is malformed.
    #[error(
        "FA-E0300: header.440 is malformed: {problem}; fix: use the unmodified JPL file \
         (tests/data/MANIFEST.toml pins its SHA-256)"
    )]
    HeaderParse {
        /// What is wrong, with the GROUP and token where known.
        problem: String,
    },
    /// A constant name is absent.
    #[error(
        "FA-E0301: constant {name:?} is not present in {source_name}; fix: check the spelling \
         against GROUP 1040, or regenerate the table with the constant selected"
    )]
    ConstantMissing {
        /// The requested header name, e.g. `"GMS"`.
        name: String,
        /// Where it was looked up (`"header.440"` or the table file).
        source_name: &'static str,
    },
    /// The committed constants table is malformed or internally inconsistent.
    #[error(
        "FA-E0302: constants table problem: {problem}; fix: regenerate it with \
         `cargo run -p ferroastro-constants --example gen_de440_table` and never edit it by hand"
    )]
    Table {
        /// What is wrong.
        problem: String,
    },
}

impl ConstantsError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            ConstantsError::HeaderParse { .. } => ErrorCode::HeaderParse,
            ConstantsError::ConstantMissing { .. } => ErrorCode::ConstantMissing,
            ConstantsError::Table { .. } => ErrorCode::ConstantsTable,
        }
    }
}
