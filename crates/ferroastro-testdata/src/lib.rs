// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Cached, checksummed downloads of external data products for `data` integration tests.
//!
//! Products are listed in `tests/data/MANIFEST.toml` (URL, file name, pinned SHA-256). They are
//! downloaded with the system `curl` (no TLS crate enters the dependency graph) into
//! `$FA_DATA_DIR`, else `$CARGO_TARGET_DIR/fa-data`, else `<workspace>/target/fa-data`, and are
//! never committed. Each download writes a `<file>.provenance.toml` sidecar recording the URL,
//! retrieval date (UTC), SHA-256, size and the server's `Last-Modified` header, and
//! [`Fetched::provenance`] renders the same facts for test output.
//!
//! Tests that use this crate are network-dependent: they are `#[ignore]`d and named `data_*`, and
//! run with `cargo nextest run --workspace --profile data --run-ignored only`.

use ferroastro_core::ErrorCode;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Environment variable that overrides the download cache directory.
pub const DATA_DIR_ENV: &str = "FA_DATA_DIR";

/// Errors from fetching data products (`FA-E02xx`).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DataError {
    /// The download failed.
    #[error(
        "FA-E0200: download of {url} failed: {message}; fix: check network access and that `curl` is installed"
    )]
    Fetch {
        /// URL.
        url: String,
        /// What went wrong.
        message: String,
    },
    /// The file does not match its pinned checksum.
    #[error(
        "FA-E0201: {url} has SHA-256 {actual}, but tests/data/MANIFEST.toml pins {expected}; \
         fix: the upstream product changed; review it and re-pin deliberately (golden-change)"
    )]
    Checksum {
        /// URL.
        url: String,
        /// Pinned digest.
        expected: String,
        /// Digest of the downloaded bytes.
        actual: String,
    },
    /// The manifest is missing, malformed, or lacks the product.
    #[error("FA-E0202: data manifest problem: {0}")]
    Manifest(String),
}

impl DataError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            DataError::Fetch { .. } => ErrorCode::DataFetch,
            DataError::Checksum { .. } => ErrorCode::DataChecksum,
            DataError::Manifest(_) => ErrorCode::DataManifest,
        }
    }
}

/// One entry of `tests/data/MANIFEST.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Product {
    /// Stable short name, e.g. `"de440-header"`.
    pub name: String,
    /// Download URL.
    pub url: String,
    /// File name in the cache directory.
    pub file: String,
    /// Pinned lowercase hex SHA-256.
    pub sha256: String,
    /// Date the pin was taken.
    pub first_retrieved: String,
    /// Publisher and primary citation.
    pub publisher: String,
    /// Known redistribution terms.
    pub terms: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: i64,
    product: Vec<Product>,
}

/// A data product available on local disk, with its provenance.
#[derive(Debug, Clone)]
pub struct Fetched {
    /// Local path of the cached file.
    pub path: PathBuf,
    /// Source URL.
    pub url: String,
    /// SHA-256 of the bytes on disk (lowercase hex).
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
    /// Retrieval date (UTC, `YYYY-MM-DD`) of the cached copy.
    pub retrieved: String,
    /// Server `Last-Modified` header at retrieval, if any.
    pub last_modified: Option<String>,
    /// True if this call reused a cached copy instead of downloading.
    pub from_cache: bool,
}

impl Fetched {
    /// One-line provenance record for test output.
    #[must_use]
    pub fn provenance(&self) -> String {
        let mut s = format!(
            "provenance: url={} retrieved={} sha256={} bytes={}",
            self.url, self.retrieved, self.sha256, self.bytes
        );
        if let Some(lm) = &self.last_modified {
            let _ = write!(s, " last_modified={lm:?}");
        }
        let _ = write!(s, " cache={}", if self.from_cache { "hit" } else { "miss" });
        s
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The download cache directory (see the crate docs for the lookup order).
#[must_use]
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(DATA_DIR_ENV) {
        return PathBuf::from(dir);
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace_root().join("target"), PathBuf::from);
    target.join("fa-data")
}

/// Look up a product in `tests/data/MANIFEST.toml`.
///
/// # Errors
/// [`DataError::Manifest`] if the manifest cannot be read or parsed, or lacks `name`.
pub fn product(name: &str) -> Result<Product, DataError> {
    let path = workspace_root().join("tests/data/MANIFEST.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| DataError::Manifest(format!("cannot read {}: {e}", path.display())))?;
    let manifest: Manifest = toml::from_str(&text)
        .map_err(|e| DataError::Manifest(format!("{}: {e}", path.display())))?;
    if manifest.schema_version != 1 {
        return Err(DataError::Manifest(format!(
            "unsupported schema_version {}",
            manifest.schema_version
        )));
    }
    manifest
        .product
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| DataError::Manifest(format!("no product named {name:?}")))
}

/// Lowercase hex SHA-256 of a byte slice.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// Fetch a manifest product: reuse the cached copy when its SHA-256 matches the pin, otherwise
/// download it and verify the pin.
///
/// # Errors
/// [`DataError::Manifest`], [`DataError::Fetch`] or [`DataError::Checksum`].
pub fn fetch_product(name: &str) -> Result<Fetched, DataError> {
    let p = product(name)?;
    fetch_url(&p.url, &p.file, Some(&p.sha256))
}

/// Fetch any URL into the cache under `file`. With `pin`, a cached copy is reused only if it
/// matches, and a fresh download must match. Without `pin` (e.g. Horizons API answers, whose
/// content depends on the service version), the cached copy is always reused if present.
///
/// # Errors
/// [`DataError::Fetch`] or [`DataError::Checksum`].
pub fn fetch_url(url: &str, file: &str, pin: Option<&str>) -> Result<Fetched, DataError> {
    let fetch_err = |message: String| DataError::Fetch {
        url: url.to_owned(),
        message,
    };
    let dir = data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| fetch_err(format!("create {}: {e}", dir.display())))?;
    let path = dir.join(file);
    let sidecar = dir.join(format!("{file}.provenance.toml"));

    if let Ok(bytes) = std::fs::read(&path) {
        let sha = sha256_hex(&bytes);
        if pin.is_none_or(|p| p == sha) {
            let side = std::fs::read_to_string(&sidecar).unwrap_or_default();
            return Ok(Fetched {
                path,
                url: url.to_owned(),
                sha256: sha,
                bytes: bytes.len() as u64,
                retrieved: sidecar_field(&side, "retrieved")
                    .unwrap_or_else(|| "unknown".to_owned()),
                last_modified: sidecar_field(&side, "last_modified"),
                from_cache: true,
            });
        }
    }

    let tmp = dir.join(format!("{file}.part"));
    let headers = dir.join(format!("{file}.headers"));
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--retry",
            "3",
        ])
        .args(["--max-time", "300", "--dump-header"])
        .arg(&headers)
        .arg("--output")
        .arg(&tmp)
        .arg(url)
        .output()
        .map_err(|e| fetch_err(format!("cannot run curl: {e}")))?;
    if !output.status.success() {
        return Err(fetch_err(format!(
            "curl exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let bytes =
        std::fs::read(&tmp).map_err(|e| fetch_err(format!("read {}: {e}", tmp.display())))?;
    let sha = sha256_hex(&bytes);
    if let Some(expected) = pin {
        if expected != sha {
            return Err(DataError::Checksum {
                url: url.to_owned(),
                expected: expected.to_owned(),
                actual: sha,
            });
        }
    }
    let last_modified = std::fs::read_to_string(&headers).ok().and_then(|h| {
        // After redirects curl writes several header blocks; the final response is the last one.
        h.lines()
            .rev()
            .filter_map(|l| l.split_once(':'))
            .find(|(k, _)| k.trim().eq_ignore_ascii_case("last-modified"))
            .map(|(_, v)| v.trim().to_owned())
    });
    std::fs::rename(&tmp, &path)
        .map_err(|e| fetch_err(format!("rename into {}: {e}", path.display())))?;
    let retrieved = utc_date_today();
    let mut side = format!(
        "url = {url:?}\nretrieved = {retrieved:?}\nsha256 = {sha:?}\nbytes = {}\n",
        bytes.len()
    );
    if let Some(lm) = &last_modified {
        let _ = writeln!(side, "last_modified = {lm:?}");
    }
    std::fs::write(&sidecar, side)
        .map_err(|e| fetch_err(format!("write {}: {e}", sidecar.display())))?;
    Ok(Fetched {
        path,
        url: url.to_owned(),
        sha256: sha,
        bytes: bytes.len() as u64,
        retrieved,
        last_modified,
        from_cache: false,
    })
}

fn sidecar_field(text: &str, key: &str) -> Option<String> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"').to_owned())
}

/// Today's UTC civil date as `YYYY-MM-DD` (Hinnant's days-from-civil inverse).
fn utc_date_today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Proleptic-Gregorian `(year, month, day)` for a count of days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: TST-005
    #[test]
    fn civil_dates() {
        // Calendar arithmetic fixtures (not physics): 1970-01-01 is day 0, 2000-03-01 is day 11017.
        assert_eq!(civil_from_days(0), (1970, 1, 1)); // lint: allow-literal(calendar fixture)
        assert_eq!(civil_from_days(-1), (1969, 12, 31)); // lint: allow-literal(calendar fixture)
        assert_eq!(civil_from_days(11_017), (2000, 3, 1)); // lint: allow-literal(calendar fixture)
    }

    /// Verifies: TST-005
    #[test]
    fn sha256_of_empty_input() {
        // SHA-256 of the empty message (FIPS 180-4 example; a string, so not a numeric literal).
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(sha256_hex(b""), empty);
    }

    /// Verifies: REF-002
    #[test]
    fn manifest_products_are_well_formed() {
        let p = product("de440-header").unwrap();
        assert_eq!(p.sha256.len(), 64);
        assert!(
            p.sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert!(p.url.starts_with("https://"));
        assert_eq!(product("nope").unwrap_err().code(), ErrorCode::DataManifest);
    }
}
