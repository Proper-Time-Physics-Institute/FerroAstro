// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Initial conditions and constants embedded at build time (DESIGN §5): nothing is read from the
//! network or the file system at run time.
//!
//! * System mode: the DE440 `header.440` GROUP 1041 initial conditions at `JDEPOC`, from the
//!   fetched, hash-pinned JPL file that the `embed-header` feature includes at build time
//!   ([`HEADER_440`]; `build.rs` copies it from `$FA_DATA_DIR` into `OUT_DIR` after checking its
//!   SHA-256 against `tests/data/MANIFEST.toml`). It is parsed by [`Header::parse`] and passed
//!   through the unchanged [`ferroastro_propagate::DataSources::with_header`] path. No
//!   `header.440` value is ever committed (#22). Without the feature [`header`] refuses with
//!   [`FAV_E_UNSUPPORTED`] and [`WITHOUT_EMBED_HEADER`].
//! * DE440 constants (System mode and the Rung lab): from the same embedded bytes through
//!   `De440::from_header_bytes`, which re-checks the SHA-256 pin at run time (#22, PR #53; the
//!   committed constants table is gone). Without the feature [`de440`] refuses like [`header`].
//! * Asteroids (bit 10): the REF-003 sampled states [`SMALL_BODY_STATES`]
//!   (`tests/goldens/sb441-n373s-jdepoc-states.csv`, provisional under OI-24).
//! * Fit frequencies: the SW92 Table 2a mean motions of `tests/vectors/park2017-estimator.toml`
//!   (the same vectors the diffrun and oracle estimators use for Park17 Table 2).

#![forbid(unsafe_code)]

use crate::{FAV_E_UNSUPPORTED, ViewerError};
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_system::SmallBodyStates;
use std::sync::OnceLock;

/// True if this build includes `header.440` (feature `embed-header`).
pub const EMBED_HEADER: bool = cfg!(feature = "embed-header");

/// The JPL `header.440` included at build time by the `embed-header` feature (SHA-256 checked
/// against the `de440-header` pin of `tests/data/MANIFEST.toml` by `build.rs`); `None` without it.
#[cfg(feature = "embed-header")]
pub const HEADER_440: Option<&[u8]> = Some(include_bytes!(concat!(env!("OUT_DIR"), "/header.440")));

/// The JPL `header.440` included at build time by the `embed-header` feature; `None` without it.
#[cfg(not(feature = "embed-header"))]
pub const HEADER_440: Option<&[u8]> = None;

/// The message of the System-mode refusal of a build without `embed-header` (status
/// [`FAV_E_UNSUPPORTED`]).
pub const WITHOUT_EMBED_HEADER: &str = "FA-E0303: this viewer was built without embed-header, so \
     it has no DE440 header.440 initial conditions or constants, so System mode and the Rung lab \
     are unavailable; fix: fetch the pinned file with `(cd tools && uv run python fetch_jpl.py \
     de440-header)` or set FA_DATA_DIR to the directory that holds it, then rebuild with \
     `--features embed-header` (crates/ferroastro-viewer/web/build.sh does this). The file is \
     never committed (#22).";

/// The REF-003 sampled small-body states at `JDEPOC` (Sun-centred ICRF; provisional, OI-24).
pub const SMALL_BODY_STATES: &str =
    include_str!("../../../tests/goldens/sb441-n373s-jdepoc-states.csv");

/// The Park2017 estimator vectors, holding the SW92 Table 2a mean motions.
const PARK2017_VECTORS: &str = include_str!("../../../tests/vectors/park2017-estimator.toml");

/// The provenance line of the embedded header, recorded in every run's `ic_source`.
#[cfg(feature = "embed-header")]
#[must_use]
pub fn header_provenance() -> &'static str {
    concat!(
        "header.440 ",
        env!("FAV_HEADER_URL"),
        " sha256 ",
        env!("FAV_HEADER_SHA256"),
        " (",
        env!("FAV_HEADER_BYTES"),
        " bytes, pinned ",
        env!("FAV_HEADER_PINNED"),
        "), embedded at build time (feature embed-header; SHA-256 checked against \
         tests/data/MANIFEST.toml)"
    )
}

/// The provenance line of the embedded header: none without `embed-header`.
#[cfg(not(feature = "embed-header"))]
#[must_use]
pub fn header_provenance() -> &'static str {
    "header.440 not embedded (built without embed-header)"
}

/// The parsed embedded `header.440` (parsed once per instance).
///
/// # Errors
/// [`FAV_E_UNSUPPORTED`] with [`WITHOUT_EMBED_HEADER`] in a build
/// without `embed-header`; `FA-E03xx` if the embedded file does not parse.
pub fn header() -> Result<&'static Header, ViewerError> {
    static H: OnceLock<Result<Header, String>> = OnceLock::new();
    let Some(bytes) = HEADER_440 else {
        return Err(ViewerError::new(FAV_E_UNSUPPORTED, WITHOUT_EMBED_HEADER));
    };
    H.get_or_init(|| {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| format!("FA-E0300: the embedded header.440 is not UTF-8: {e}"))?;
        Header::parse(text).map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| ViewerError::run(e.clone()))
}

/// The parsed REF-003 small-body states (parsed once per instance).
///
/// # Errors
/// `FA-E11xx` if the embedded file does not parse.
pub fn small_bodies() -> Result<&'static SmallBodyStates, ViewerError> {
    static S: OnceLock<Result<SmallBodyStates, String>> = OnceLock::new();
    S.get_or_init(|| SmallBodyStates::parse(SMALL_BODY_STATES).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| ViewerError::run(e.clone()))
}

/// The DE440 constants of the embedded `header.440` (`De440::from_header_bytes`, SHA-256
/// checked; built once per instance).
///
/// # Errors
/// [`FAV_E_UNSUPPORTED`] with [`WITHOUT_EMBED_HEADER`] in a build without `embed-header`;
/// `FA-E03xx` if the embedded header does not give the constants.
pub fn de440() -> Result<&'static De440, ViewerError> {
    static D: OnceLock<Result<De440, String>> = OnceLock::new();
    let Some(bytes) = HEADER_440 else {
        return Err(ViewerError::new(FAV_E_UNSUPPORTED, WITHOUT_EMBED_HEADER));
    };
    D.get_or_init(|| De440::from_header_bytes(bytes).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| ViewerError::run(e.clone()))
}

/// The SW92 Table 2a mean-longitude rates `[n_M, n_V, n_E(EMB), n_J, n_S]` (deg/cy) from
/// `tests/vectors/park2017-estimator.toml` (vector ids `SW92-T2A-<BODY>-L-RATE`), the inputs of
/// [`ferroastro_precession::park2017::park17_table2_frequencies`].
///
/// # Errors
/// `FA-E0002` if the embedded vector file lacks one of them.
pub fn sw92_mean_motions() -> Result<[f64; 5], ViewerError> {
    static N: OnceLock<Result<[f64; 5], String>> = OnceLock::new();
    N.get_or_init(|| {
        let table: toml::Table = toml::from_str(PARK2017_VECTORS).map_err(|e| e.to_string())?;
        let vectors = table
            .get("vector")
            .and_then(toml::Value::as_array)
            .ok_or("no [[vector]] array")?;
        let get = |body: &str| -> Result<f64, String> {
            let id = format!("SW92-T2A-{body}-L-RATE");
            let v = vectors
                .iter()
                .find(|v| v.get("id").and_then(toml::Value::as_str) == Some(id.as_str()))
                .and_then(|v| v.get("value"))
                .ok_or_else(|| format!("vector {id} missing"))?;
            match v {
                toml::Value::String(s) => s.trim().parse::<f64>().map_err(|e| e.to_string()),
                toml::Value::Float(x) => Ok(*x),
                _ => Err(format!("vector {id}: value is not a number")),
            }
        };
        Ok([
            get("MERCURY")?,
            get("VENUS")?,
            get("EMB")?,
            get("JUPITER")?,
            get("SATURN")?,
        ])
    })
    .clone()
    .map_err(|e| ViewerError::arg(format!("FA-E0002: SW92 mean motions: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: API-004
    #[test]
    fn embedded_header_parses_and_is_at_jdepoc() {
        if !EMBED_HEADER {
            eprintln!("SKIP embedded_header_parses_and_is_at_jdepoc: built without embed-header");
            return;
        }
        let h = header().unwrap();
        let de = de440().unwrap();
        let (d, f) = de.initial_epoch().unwrap().jd_parts();
        assert_eq!(h.value("JDEPOC").unwrap().to_bits(), (d + f).to_bits());
        let pin = ferroastro_testdata::product("de440-header").unwrap().sha256;
        assert!(header_provenance().contains(&format!("sha256 {pin}")));
    }

    /// Verifies: API-004
    #[test]
    fn without_embed_header_system_ics_are_refused() {
        if EMBED_HEADER {
            eprintln!("SKIP without_embed_header_system_ics_are_refused: built with embed-header");
            return;
        }
        for e in [header().unwrap_err(), de440().unwrap_err()] {
            assert_eq!(e.status, FAV_E_UNSUPPORTED);
            assert!(e.message.starts_with("FA-E0303"), "{}", e.message);
            assert!(
                e.message.contains("built without embed-header"),
                "{}",
                e.message
            );
        }
    }

    /// Verifies: API-004
    #[test]
    fn mean_motions_load() {
        let n = sw92_mean_motions().unwrap();
        assert!(n.iter().all(|x| x.is_finite() && *x > 0.0));
    }
}
