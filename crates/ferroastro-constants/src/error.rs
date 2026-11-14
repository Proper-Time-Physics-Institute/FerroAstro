// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E03xx`, API-002).

use ferroastro_core::ErrorCode;

/// Errors from parsing or locating `header.440` and from the committed constants metadata.
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
         against GROUP 1040, or add the constant to data/de440-metadata.toml"
    )]
    ConstantMissing {
        /// The requested header name, e.g. `"GMS"`.
        name: String,
        /// Where it was looked up (`"header.440"` or the metadata file).
        source_name: &'static str,
    },
    /// The committed constants metadata (or the table built from it) is malformed or
    /// internally inconsistent.
    #[error(
        "FA-E0302: constants metadata problem: {problem}; fix: correct \
         crates/ferroastro-constants/data/de440-metadata.toml (names, units and citations only, \
         never a value)"
    )]
    Table {
        /// What is wrong.
        problem: String,
    },
    /// The JPL `header.440` file could not be found or read.
    #[error(
        "FA-E0303: header.440 not found ({path}: {problem}); fix: run \
         `(cd tools && uv run python fetch_jpl.py de440-header)` or set FA_DATA_DIR to the \
         directory that holds the pinned header.440"
    )]
    HeaderNotFound {
        /// Where it was looked for.
        path: String,
        /// Why it could not be used (missing, unreadable, not UTF-8).
        problem: String,
    },
    /// The `header.440` bytes do not match the pinned SHA-256.
    #[error(
        "FA-E0304: header.440 SHA-256 mismatch: expected {expected}, got {got}; fix: use the \
         unmodified JPL file pinned in tests/data/MANIFEST.toml (re-fetch it with \
         `(cd tools && uv run python fetch_jpl.py de440-header)`)"
    )]
    HeaderChecksum {
        /// The pinned digest (lowercase hex).
        expected: String,
        /// The digest of the bytes supplied (lowercase hex).
        got: String,
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
            ConstantsError::HeaderNotFound { .. } => ErrorCode::HeaderNotFound,
            ConstantsError::HeaderChecksum { .. } => ErrorCode::HeaderChecksum,
        }
    }
}
