// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `MANIFEST.json` provenance records of oracle goldens (CI-003).
//!
//! The Python runners (`tools/oracles/common.py`) write one entry per output file; this module
//! is the Rust-side check that every file of a golden directory has a complete entry and that
//! its `output_sha256` matches the bytes on disk.

use crate::OracleError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// The manifest file name in every golden directory.
pub const MANIFEST_NAME: &str = "MANIFEST.json";
/// The only manifest format version this crate reads.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// One output file's provenance record (CI-003: tool, version, image digest, runner SHA, input
/// SHA-256s, CPU, wall clock, output SHA-256, units, frame and time scale).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Output file name (relative to the golden directory).
    pub file: String,
    /// Oracle tool (`rebound`, `erfa`, `jpl`, …).
    pub tool: String,
    /// Engine version string.
    pub version: String,
    /// Docker image id (`sha256:…`) the runner executed in.
    pub image_digest: String,
    /// Git commit of the runner sources (`-dirty` when uncommitted).
    pub runner_sha: String,
    /// SHA-256 over the runner source files.
    pub runner_sha256: String,
    /// SHA-256 of every input file, by name.
    pub input_sha256: BTreeMap<String, String>,
    /// CPU model.
    pub cpu: String,
    /// Wall-clock time of the computation (s).
    pub wall_clock_s: f64,
    /// SHA-256 of the output file.
    pub output_sha256: String,
    /// Units of the output.
    pub units: String,
    /// Reference frame / centre of the output.
    pub frame: String,
    /// Time scale of the output.
    pub time_scale: String,
    /// Scenario name or job id.
    pub scenario: String,
    /// Generation time (UTC, ISO 8601).
    pub generated_utc: String,
    /// The runner command.
    pub command: String,
    /// Tool-specific diagnostics (step counts, settings read back, energy change).
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

/// A golden directory's `MANIFEST.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Format version ([`MANIFEST_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// One entry per output file.
    pub entries: Vec<ManifestEntry>,
}

/// Lower-case hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

fn manifest_err(path: &Path, problem: impl Into<String>) -> OracleError {
    OracleError::Manifest {
        path: path.display().to_string(),
        problem: problem.into(),
    }
}

impl ManifestEntry {
    /// The CI-003 fields of this entry that are empty (none for a complete entry).
    #[must_use]
    pub fn empty_fields(&self) -> Vec<&'static str> {
        let text = [
            ("file", &self.file),
            ("tool", &self.tool),
            ("version", &self.version),
            ("image_digest", &self.image_digest),
            ("runner_sha", &self.runner_sha),
            ("runner_sha256", &self.runner_sha256),
            ("cpu", &self.cpu),
            ("output_sha256", &self.output_sha256),
            ("units", &self.units),
            ("frame", &self.frame),
            ("time_scale", &self.time_scale),
            ("scenario", &self.scenario),
            ("generated_utc", &self.generated_utc),
            ("command", &self.command),
        ];
        let mut out: Vec<&'static str> = text
            .iter()
            .filter(|(_, v)| v.trim().is_empty())
            .map(|(k, _)| *k)
            .collect();
        if self.input_sha256.is_empty() || self.input_sha256.values().any(|v| v.len() != 64) {
            out.push("input_sha256");
        }
        if !(self.wall_clock_s.is_finite() && self.wall_clock_s >= 0.0) {
            out.push("wall_clock_s");
        }
        out
    }

    /// Check that input `name` was the file pinned at `sha256` (e.g. header.440 against
    /// `tests/data/MANIFEST.toml`).
    ///
    /// # Errors
    /// `FA-E1500` if the entry lists no such input, `FA-E1501` if its digest differs.
    pub fn check_input(&self, name: &str, sha256: &str) -> Result<(), OracleError> {
        let got = self
            .input_sha256
            .get(name)
            .ok_or_else(|| OracleError::Manifest {
                path: self.file.clone(),
                problem: format!("no input named {name}"),
            })?;
        if got != sha256 {
            return Err(OracleError::Checksum {
                file: format!("{} (input {name})", self.file),
                what: "input pin",
                expected: sha256.to_owned(),
                actual: got.clone(),
                fix: "regenerate the golden from the pinned input",
            });
        }
        Ok(())
    }
}

impl Manifest {
    /// Parse `MANIFEST.json` text.
    ///
    /// # Errors
    /// `FA-E1500` for invalid JSON, a missing field or another schema version.
    pub fn parse(path: &Path, text: &str) -> Result<Self, OracleError> {
        let m: Self = serde_json::from_str(text).map_err(|e| manifest_err(path, e.to_string()))?;
        if m.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(manifest_err(
                path,
                format!(
                    "schema_version {} is not {MANIFEST_SCHEMA_VERSION}",
                    m.schema_version
                ),
            ));
        }
        Ok(m)
    }

    /// The entry of `file`.
    #[must_use]
    pub fn entry(&self, file: &str) -> Option<&ManifestEntry> {
        self.entries.iter().find(|e| e.file == file)
    }

    /// Verify a golden directory (CI-003): it has a `MANIFEST.json`; every other file in it has
    /// exactly one entry with every field non-empty and an `image_digest` of the form
    /// `sha256:…`; each `output_sha256` matches the file; no entry names an absent file.
    ///
    /// # Errors
    /// `FA-E1500` for a missing, malformed or incomplete manifest, `FA-E1501` for a digest
    /// mismatch.
    pub fn verify_dir(dir: &Path) -> Result<Self, OracleError> {
        let path = dir.join(MANIFEST_NAME);
        let text =
            std::fs::read_to_string(&path).map_err(|e| manifest_err(&path, e.to_string()))?;
        let m = Self::parse(&path, &text)?;
        let mut files = Vec::new();
        for e in std::fs::read_dir(dir).map_err(|e| manifest_err(dir, e.to_string()))? {
            let e = e.map_err(|e| manifest_err(dir, e.to_string()))?;
            let name = e.file_name().to_string_lossy().into_owned();
            if e.path().is_file() && name != MANIFEST_NAME {
                files.push(name);
            }
        }
        files.sort();
        for name in &files {
            let n = m.entries.iter().filter(|e| &e.file == name).count();
            if n != 1 {
                return Err(manifest_err(
                    &path,
                    format!("{name}: {n} entries (want exactly 1)"),
                ));
            }
        }
        for e in &m.entries {
            if !files.contains(&e.file) {
                return Err(manifest_err(
                    &path,
                    format!("entry {} names no file", e.file),
                ));
            }
            let empty = e.empty_fields();
            if !empty.is_empty() {
                return Err(manifest_err(
                    &path,
                    format!("{}: empty or invalid CI-003 fields {empty:?}", e.file),
                ));
            }
            if !e.image_digest.starts_with("sha256:") {
                return Err(manifest_err(
                    &path,
                    format!(
                        "{}: image_digest {:?} is not sha256:…",
                        e.file, e.image_digest
                    ),
                ));
            }
            let bytes = std::fs::read(dir.join(&e.file))
                .map_err(|err| manifest_err(&path, format!("{}: {err}", e.file)))?;
            let actual = sha256_hex(&bytes);
            if actual != e.output_sha256 {
                return Err(OracleError::Checksum {
                    file: dir.join(&e.file).display().to_string(),
                    what: "manifest output_sha256",
                    expected: e.output_sha256.clone(),
                    actual,
                    fix: "a golden changes only through the dispatch/nightly-drift workflow and a reviewed PR (CI-004)",
                });
            }
        }
        Ok(m)
    }
}
