// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Minimal JPL Horizons API client for the `data` tier: one heliocentric Mercury state at J2000
//! TDB, with full provenance (URL, retrieval date, SHA-256, `signature.version`, ephemeris source
//! and the reference-frame line Horizons prints).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test-only client: any failure is a test failure with a clear message"
)]

use ferroastro_core::time::J2000_JD;
use ferroastro_testdata::{Fetched, fetch_url};

const API: &str = "https://ssd.jpl.nasa.gov/api/horizons.api";

/// A heliocentric Mercury state from Horizons.
pub(crate) struct HorizonsState {
    pub(crate) fetched: Fetched,
    pub(crate) version: String,
    pub(crate) source: String,
    pub(crate) frame: String,
    pub(crate) r: [f64; 3],
    pub(crate) v: [f64; 3],
}

impl HorizonsState {
    pub(crate) fn provenance(&self) -> String {
        format!(
            "Horizons {} signature.version={} ephemeris={} reference frame: {}",
            self.fetched.provenance(),
            self.version,
            self.source,
            self.frame
        )
    }
}

/// Mercury (199) relative to the Sun's centre (500@10) at J2000 TDB, `ref_plane` one of
/// `ECLIPTIC` or `BODY EQUATOR`, ICRF, km and km/s.
pub(crate) fn mercury_state(ref_plane: &str, file: &str) -> HorizonsState {
    let q = |s: &str| format!("%27{}%27", s.replace(' ', "%20"));
    let url = format!(
        "{API}?format=json&COMMAND={}&OBJ_DATA={}&MAKE_EPHEM={}&EPHEM_TYPE={}&CENTER={}&TLIST={}\
         &REF_PLANE={}&REF_SYSTEM={}&OUT_UNITS={}&CSV_FORMAT={}&VEC_TABLE={}&VEC_LABELS={}",
        q("199"),
        q("NO"),
        q("YES"),
        q("VECTORS"),
        q("500@10"),
        q(&format!("{J2000_JD:.1}")),
        q(ref_plane),
        q("ICRF"),
        q("KM-S"),
        q("YES"),
        q("2"),
        q("NO"),
    );
    let fetched = fetch_url(&url, file, None).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&fetched.path).unwrap()).unwrap();
    let version = json["signature"]["version"]
        .as_str()
        .unwrap_or("?")
        .to_owned();
    let result = json["result"]
        .as_str()
        .expect("Horizons answer has no result")
        .to_owned();
    let line_after = |key: &str| {
        result
            .lines()
            .find(|l| l.starts_with(key))
            .map_or_else(|| "?".to_owned(), |l| l[key.len()..].trim().to_owned())
    };
    let source = result
        .lines()
        .find(|l| l.starts_with("Target body name"))
        .and_then(|l| l.split("source:").nth(1))
        .map_or_else(
            || "?".to_owned(),
            |s| s.trim().trim_end_matches('}').to_owned(),
        );
    let frame = line_after("Reference frame :");
    let soe = result.find("$$SOE").expect("no $$SOE in Horizons answer");
    let row: Vec<f64> = result[soe + 5..]
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap()
        .split(',')
        .skip(2)
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    assert!(row.len() >= 6, "Horizons row has {} numbers", row.len());
    HorizonsState {
        fetched,
        version,
        source,
        frame,
        r: [row[0], row[1], row[2]],
        v: [row[3], row[4], row[5]],
    }
}
