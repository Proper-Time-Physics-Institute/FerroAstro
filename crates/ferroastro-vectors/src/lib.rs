// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Provenance-tracked test vectors (ADR-0004, TST-002, TST-003).
//!
//! Expected values live in `tests/vectors/*.toml`, never as literals in tests. Each file holds
//! `schema_version = 1` and an array of `[[vector]]` tables. [`VectorSet::load_dir`] parses every
//! file and rejects any vector that violates the schema (`tests/vectors/schema.json` is the same
//! schema for the Python tooling):
//!
//! | field | rule |
//! |---|---|
//! | `id` | upper-case `A-Z0-9` words joined by `-`; unique across all files |
//! | `requirement` | non-empty list of `<PREFIX>-NNN` requirement ids |
//! | `quantity`, `unit`, `source`, `citation`, `tolerance_rationale`, `verified_by` | non-empty strings |
//! | `value` | float, integer, or decimal string (kept verbatim for extended precision) |
//! | `kind` | `defined`, `measured`, `computed-reference` or `analytic` |
//! | `retrieved` | `YYYY-MM-DD` |
//! | `tolerance` | table with `abs` and/or `rel`, both finite and ≥ 0 |
//! | `generated_by`, `engine` | required when `kind = "computed-reference"` (TST-003) |
//! | `blocked_by` | optional `OI-NN`: the oracle is non-gating while that open item is open ([`Vector::blocking_open_item`]) |
//! | `provisional` | optional `OI-NN`, never with `blocked_by`: the oracle gates, but its assertion is PROVISIONAL until that open item is closed; the item must exist in `OPEN-ITEMS.md` and results are labelled with it and its revert condition ([`Vector::provisional_open_item`], ADR-0014) |
//!
//! `source` and `citation` may not contain `UNVERIFIED` or `[S]`: such values cannot be oracles
//! until closed in `docs/requirements/OPEN-ITEMS.md` (PHY-018).
//!
//! A measured value `x` passes vector `v` when `|x − v| ≤ max(abs, rel·|v|)`, so `abs` acts as an
//! absolute floor under a relative tolerance.
//!
//! ```
//! # fn main() -> Result<(), ferroastro_vectors::VectorError> {
//! let set = ferroastro_vectors::VectorSet::load_workspace()?;
//! let c = set.get("CONST-CLIGHT-SI")?;
//! assert_eq!(c.unit, "km/s");
//! c.check(c.value()?)?;
//! # Ok(())
//! # }
//! ```

mod schema;

pub use schema::{Engine, Kind, Provisional, Tolerance, Vector, VectorValue};

use ferroastro_core::ErrorCode;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Environment variable that overrides the vector directory.
pub const VECTORS_DIR_ENV: &str = "FA_VECTORS_DIR";

/// Errors from loading or checking vectors. Each maps to a stable `FA-E01xx` code.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum VectorError {
    /// A vector file could not be read.
    #[error(
        "FA-E0100: cannot read vector file {path}: {source}; fix: check the path and permissions"
    )]
    Io {
        /// File or directory.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// A vector file is not valid TOML or has the wrong shape.
    #[error("FA-E0101: {path} is not a valid vector file: {message}")]
    Parse {
        /// File.
        path: PathBuf,
        /// Parser message.
        message: String,
    },
    /// A vector violates the ADR-0004 schema.
    #[error("FA-E0102: vector {id:?} in {path}: field `{field}` {problem}")]
    Schema {
        /// File.
        path: PathBuf,
        /// Vector id (may be empty if the id itself is bad).
        id: String,
        /// Offending field.
        field: &'static str,
        /// What is wrong and how to fix it.
        problem: String,
    },
    /// No vector with this id exists.
    #[error(
        "FA-E0103: no test vector with id {0:?}; fix: add it to tests/vectors/*.toml with provenance"
    )]
    UnknownId(String),
    /// Two files define the same id.
    #[error("FA-E0104: vector id {id:?} defined in both {first} and {second}")]
    DuplicateId {
        /// The id.
        id: String,
        /// First file.
        first: PathBuf,
        /// Second file.
        second: PathBuf,
    },
    /// A measured value lies outside the tolerance.
    #[error("FA-E0105: {0}")]
    ToleranceExceeded(Box<Deviation>),
    /// The canonical vectors were required but `FA_VECTORS_DIR` redirects them.
    #[error(
        "FA-E0106: {VECTORS_DIR_ENV} = {dir} would replace the committed oracles in a tier whose output is evidence \
         (data_*/nightly_* tests, the CLI report); fix: unset {VECTORS_DIR_ENV}, or perturb tests/vectors in a \
         scratch checkout for mutation probes"
    )]
    NonCanonical {
        /// The override directory.
        dir: PathBuf,
    },
}

impl VectorError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            VectorError::Io { .. } => ErrorCode::VectorIo,
            VectorError::Parse { .. } => ErrorCode::VectorParse,
            VectorError::Schema { .. } => ErrorCode::VectorSchema,
            VectorError::UnknownId(_) => ErrorCode::VectorUnknownId,
            VectorError::DuplicateId { .. } => ErrorCode::VectorDuplicateId,
            VectorError::ToleranceExceeded(_) => ErrorCode::ToleranceExceeded,
            VectorError::NonCanonical { .. } => ErrorCode::VectorOverrideRefused,
        }
    }
}

/// Every number needed to judge a failed tolerance check.
#[derive(Debug, Clone, PartialEq)]
pub struct Deviation {
    /// Vector id.
    pub id: String,
    /// Quantity name.
    pub quantity: String,
    /// Unit.
    pub unit: String,
    /// Measured value.
    pub measured: f64,
    /// Expected value.
    pub expected: f64,
    /// Absolute difference.
    pub diff: f64,
    /// Allowed absolute difference, `max(abs, rel·|expected|)`.
    pub allowed: f64,
    /// The vector's tolerance.
    pub tolerance: Tolerance,
    /// The vector's `provisional` open item, if any: the failed assertion is itself provisional.
    pub provisional: Option<String>,
}

impl std::fmt::Display for Deviation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}): measured {} {u}, expected {} {u}, |diff| = {:e} > allowed {:e} {u} \
             (abs {:?}, rel {:?})",
            self.id,
            self.quantity,
            self.measured,
            self.expected,
            self.diff,
            self.allowed,
            self.tolerance.abs,
            self.tolerance.rel,
            u = self.unit,
        )?;
        if let Some(oi) = &self.provisional {
            write!(
                f,
                "; PROVISIONAL assertion ({oi}): it reverts if {oi} is closed against its protocol \
                 (revert condition in OPEN-ITEMS.md {oi} and the vector's tolerance_rationale)"
            )?;
        }
        Ok(())
    }
}

/// The stderr banner printed whenever `FA_VECTORS_DIR` is honoured or refused.
#[must_use]
pub fn non_canonical_banner(dir: &Path) -> String {
    let rule = "!".repeat(96);
    format!(
        "{rule}\n!!! NON-CANONICAL VECTORS: {VECTORS_DIR_ENV} = {}\n!!! Every oracle and tolerance may differ from \
         the committed tests/vectors. Results produced this way are NOT evidence.\n{rule}",
        dir.display()
    )
}

/// All vectors from one directory, keyed by id.
#[derive(Debug, Default)]
pub struct VectorSet {
    vectors: BTreeMap<String, (PathBuf, Vector)>,
}

impl VectorSet {
    /// The committed vector directory, `<workspace>/tests/vectors` (never redirected).
    #[must_use]
    pub fn canonical_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/vectors")
    }

    /// The workspace vector directory: `$FA_VECTORS_DIR` if set, else
    /// [`VectorSet::canonical_dir`]. An override prints a NON-CANONICAL banner on stderr every
    /// time it is read, because it replaces every oracle at once (red-team H2 pattern).
    #[must_use]
    pub fn workspace_dir() -> PathBuf {
        match std::env::var_os(VECTORS_DIR_ENV) {
            Some(dir) => {
                let dir = PathBuf::from(dir);
                eprintln!("{}", non_canonical_banner(&dir));
                dir
            }
            None => Self::canonical_dir(),
        }
    }

    /// Load every vector under [`VectorSet::workspace_dir`] (honours `FA_VECTORS_DIR`, with a
    /// banner; for PR-tier mutation probes only).
    ///
    /// # Errors
    /// Any [`VectorError`] raised by [`VectorSet::load_dir`].
    pub fn load_workspace() -> Result<Self, VectorError> {
        Self::load_dir(&Self::workspace_dir())
    }

    /// Load the committed vectors and **refuse** an `FA_VECTORS_DIR` override. Used wherever the
    /// output is evidence: the `data_*` and `nightly_*` tiers, the ladder rung tests and the CLI
    /// report.
    ///
    /// # Errors
    /// [`VectorError::NonCanonical`] (`FA-E0106`) if `FA_VECTORS_DIR` is set, else any error of
    /// [`VectorSet::load_dir`].
    pub fn load_canonical() -> Result<Self, VectorError> {
        if let Some(dir) = std::env::var_os(VECTORS_DIR_ENV) {
            let dir = PathBuf::from(dir);
            eprintln!("{}", non_canonical_banner(&dir));
            return Err(VectorError::NonCanonical { dir });
        }
        Self::load_dir(&Self::canonical_dir())
    }

    /// Load and validate every `*.toml` file in `dir` (non-recursive, sorted by name).
    ///
    /// # Errors
    /// [`VectorError::Io`], [`VectorError::Parse`], [`VectorError::Schema`] or
    /// [`VectorError::DuplicateId`].
    pub fn load_dir(dir: &Path) -> Result<Self, VectorError> {
        let io = |source| VectorError::Io {
            path: dir.to_path_buf(),
            source,
        };
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(io)? {
            let path = entry.map_err(io)?.path();
            if path.extension().is_some_and(|e| e == "toml") {
                files.push(path);
            }
        }
        files.sort();
        let mut set = Self::default();
        for path in files {
            let text = std::fs::read_to_string(&path).map_err(|source| VectorError::Io {
                path: path.clone(),
                source,
            })?;
            set.insert_file(&path, &text)?;
        }
        Ok(set)
    }

    /// Parse one file's text and add its vectors (exposed for tests and tooling).
    ///
    /// # Errors
    /// [`VectorError::Parse`], [`VectorError::Schema`] or [`VectorError::DuplicateId`].
    pub fn insert_file(&mut self, path: &Path, text: &str) -> Result<(), VectorError> {
        for vector in schema::parse_file(path, text)? {
            if let Some((first, _)) = self.vectors.get(&vector.id) {
                return Err(VectorError::DuplicateId {
                    id: vector.id.clone(),
                    first: first.clone(),
                    second: path.to_path_buf(),
                });
            }
            self.vectors
                .insert(vector.id.clone(), (path.to_path_buf(), vector));
        }
        Ok(())
    }

    /// Look up a vector by id.
    ///
    /// # Errors
    /// [`VectorError::UnknownId`] if absent.
    pub fn get(&self, id: &str) -> Result<&Vector, VectorError> {
        self.vectors
            .get(id)
            .map(|(_, v)| v)
            .ok_or_else(|| VectorError::UnknownId(id.to_owned()))
    }

    /// Iterate over `(file, vector)` in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Vector)> {
        self.vectors.values().map(|(p, v)| (p.as_path(), v))
    }

    /// Number of vectors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// True when no vectors are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}
