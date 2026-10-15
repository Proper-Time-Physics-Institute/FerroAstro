// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The versioned, cited DE440 constants table (PHY-002, REF-002).
//!
//! `data/de440-constants.toml` is **generated** from `header.440` by
//! `cargo run -p ferroastro-constants --example gen_de440_table`, which downloads the pinned
//! product, parses it with [`crate::header440`] and calls [`render`]. Nobody types a value: each
//! entry stores the verbatim GROUP 1041 token. The table records the SHA-256 of the header it came
//! from, and the `data` test `data_de440_table_matches_header` re-downloads the header and checks
//! that re-rendering reproduces the committed file byte for byte.
//!
//! Which constants are selected, and the unit and citation of each, is metadata in
//! [`DE440_SELECTION`]. The header itself states no units; every unit carries a `unit_basis` that
//! says why it is right, and entries whose meaning or unit is still an open item say so in
//! `open_item` (OI-04 GMSDOT, OI-23 CENTER). Such entries must not be used as oracles (rule 8).

use crate::ConstantsError;
use crate::header440::{Header, parse_fortran_real};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The committed DE440 constants table (TOML), embedded at build time.
pub const DE440_TABLE_TOML: &str = include_str!("../data/de440-constants.toml");

/// Schema version of the table file.
pub const TABLE_SCHEMA_VERSION: i64 = 1;

/// Version of the table content. Bump it whenever the selection, units or source change.
pub const TABLE_VERSION: i64 = 1;

/// Citation key for the JPL DE440 header itself.
pub const CITE_HEADER: &str = "DE440hdr";
/// Citation key for the DE440 paper.
pub const CITE_PARK21: &str = "Park21";
/// Full citation of the DE440 paper.
pub const PARK21: &str = "Park, R. S., Folkner, W. M., Williams, J. G. & Boggs, D. H. 2021, \
    The JPL Planetary and Lunar Ephemerides DE440 and DE441, AJ 161, 105, \
    doi:10.3847/1538-3881/abd414";

/// Metadata for one selected header constant. Contains no numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntrySpec {
    /// GROUP 1040 name.
    pub name: &'static str,
    /// What the constant is.
    pub quantity: &'static str,
    /// Unit of the header value.
    pub unit: &'static str,
    /// Citation keys (see [`CITE_HEADER`], [`CITE_PARK21`]).
    pub citations: &'static [&'static str],
    /// Why the unit (and meaning) is right.
    pub unit_basis: &'static str,
    /// Open item that leaves the unit or meaning unverified, if any.
    pub open_item: Option<&'static str>,
}

const HDR: &[&str] = &[CITE_HEADER];
const HDR_PARK: &[&str] = &[CITE_HEADER, CITE_PARK21];
const GM_BASIS: &str = "GM in au^3/d^2: converting with AU (km) and 86400 s/d reproduces the \
    km^3 s^-2 values of Park21 Table 2 (checked by the PHY-002 tests)";

macro_rules! gm {
    ($name:literal, $what:literal) => {
        EntrySpec {
            name: $name,
            quantity: $what,
            unit: "au^3/d^2",
            citations: HDR_PARK,
            unit_basis: GM_BASIS,
            open_item: None,
        }
    };
}

/// The header constants copied into the table, in header order (PHY-002's list plus the REF-002
/// scalars). States (`X1` … `ZDC`) are not copied: their frame and units are OI-23.
pub const DE440_SELECTION: &[EntrySpec] = &[
    EntrySpec {
        name: "DENUM",
        quantity: "development ephemeris number",
        unit: "1",
        citations: HDR,
        unit_basis: "identifier",
        open_item: None,
    },
    EntrySpec {
        name: "JDEPOC",
        quantity: "epoch of the initial conditions, Julian date",
        unit: "d",
        citations: HDR,
        unit_basis: "Julian date (REF-002 acceptance); the DE ephemeris argument is TDB",
        open_item: None,
    },
    EntrySpec {
        name: "CENTER",
        quantity: "centre of the initial-condition states (flag)",
        unit: "1",
        citations: HDR,
        unit_basis: "flag; whether 0 means the solar-system barycentre in ICRF is an open item",
        open_item: Some("OI-23"),
    },
    EntrySpec {
        name: "CLIGHT",
        quantity: "speed of light c",
        unit: "km/s",
        citations: HDR,
        unit_basis: "equals the SI defining constant 299792.458 km/s (vector CONST-CLIGHT-SI)",
        open_item: None,
    },
    EntrySpec {
        name: "BETA",
        quantity: "PPN parameter beta",
        unit: "1",
        citations: HDR,
        unit_basis: "dimensionless",
        open_item: None,
    },
    EntrySpec {
        name: "GAMMA",
        quantity: "PPN parameter gamma",
        unit: "1",
        citations: HDR,
        unit_basis: "dimensionless",
        open_item: None,
    },
    EntrySpec {
        name: "AU",
        quantity: "astronomical unit",
        unit: "km",
        citations: HDR,
        unit_basis: "equals the IAU 2012 B2 au 149597870.700 km (vector CONST-AU-IAU2012)",
        open_item: None,
    },
    EntrySpec {
        name: "EMRAT",
        quantity: "Earth/Moon mass ratio",
        unit: "1",
        citations: HDR_PARK,
        unit_basis: "dimensionless; agrees with GMEarth/GMMoon of Park21 Table 2 to the 1e-10 \
            relative precision of the printed GMs",
        open_item: None,
    },
    gm!("GM1", "GM of Mercury"),
    gm!("GM2", "GM of Venus"),
    gm!("GMB", "GM of the Earth-Moon system"),
    gm!("GM4", "GM of the Mars system"),
    gm!("GM5", "GM of the Jupiter system"),
    gm!("GM6", "GM of the Saturn system"),
    gm!("GM7", "GM of the Uranus system"),
    gm!("GM8", "GM of the Neptune system"),
    gm!("GM9", "GM of the Pluto system"),
    gm!("GMS", "GM of the Sun"),
    EntrySpec {
        name: "GMSDOT",
        quantity: "rate of change of GM of the Sun",
        unit: "unverified",
        citations: HDR,
        unit_basis: "not stated in the header or in the parts of Park21 read so far; \
            presumably au^3/d^2 per year",
        open_item: Some("OI-04"),
    },
    EntrySpec {
        name: "ASUN",
        quantity: "equatorial radius of the Sun (J2 reference radius)",
        unit: "km",
        citations: HDR_PARK,
        unit_basis: "Park21 Sec. 3.4 (LT acceleration, after Eq. 39): the Sun's equatorial radius (696,000 km)",
        open_item: None,
    },
    EntrySpec {
        name: "J2SUN",
        quantity: "dynamical form factor J2 of the Sun",
        unit: "1",
        citations: HDR,
        unit_basis: "dimensionless zonal harmonic referred to ASUN",
        open_item: None,
    },
    EntrySpec {
        name: "MOISUN",
        quantity: "polar moment of inertia of the Sun, C/(M R^2)",
        unit: "1",
        citations: HDR_PARK,
        unit_basis: "Park21 Sec. 3.4 (after Eq. 39): C/(M R^2) = 0.06884",
        open_item: None,
    },
    EntrySpec {
        name: "OMGSUN",
        quantity: "rotation rate of the Sun",
        unit: "rad/d",
        citations: HDR_PARK,
        unit_basis: "Park21 Sec. 3.4 (after Eq. 39) quotes 14.1844 deg/day, which the header value in rad/d \
            reproduces to the printed digits (checked by the PHY-002 tests)",
        open_item: None,
    },
];

/// Provenance of the header a table is generated from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInfo {
    /// Name in `tests/data/MANIFEST.toml`.
    pub product: String,
    /// Download URL.
    pub url: String,
    /// SHA-256 (lowercase hex) of the header bytes.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
    /// UTC date the header was retrieved.
    pub retrieved: String,
    /// Server `Last-Modified` header, if any.
    pub last_modified: Option<String>,
    /// GROUP 1010 title.
    pub title: String,
    /// Number of constants in GROUP 1040.
    pub constant_count: u64,
    /// Number of constants named `MA*` (asteroid GMs).
    pub ma_count: u64,
}

/// One table entry.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableEntry {
    /// GROUP 1040 name.
    pub name: String,
    /// Verbatim GROUP 1041 token.
    pub token: String,
    /// Unit of the value.
    pub unit: String,
    /// What the constant is.
    pub quantity: String,
    /// Citation keys, resolved in the table's `[citations]`.
    pub citation: Vec<String>,
    /// Why the unit is right.
    pub unit_basis: String,
    /// Open item that leaves this entry unverified, if any.
    pub open_item: Option<String>,
    #[serde(skip)]
    value: f64,
}

impl TableEntry {
    /// The value as the nearest `f64` of the token.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTable {
    schema_version: i64,
    table_version: i64,
    ephemeris: String,
    source: SourceInfo,
    citations: BTreeMap<String, String>,
    constant: Vec<TableEntry>,
}

/// A parsed and validated constants table.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    version: i64,
    ephemeris: String,
    source: SourceInfo,
    citations: BTreeMap<String, String>,
    entries: Vec<TableEntry>,
}

fn table_err(problem: impl Into<String>) -> ConstantsError {
    ConstantsError::Table {
        problem: problem.into(),
    }
}

impl Table {
    /// The committed DE440 table ([`DE440_TABLE_TOML`]).
    ///
    /// # Errors
    /// See [`Table::parse`].
    pub fn de440() -> Result<Self, ConstantsError> {
        Self::parse(DE440_TABLE_TOML)
    }

    /// Parse and validate a table: schema version, unique names, finite tokens, and a non-empty
    /// citation list whose every key resolves to a non-empty `[citations]` entry (PHY-002).
    ///
    /// # Errors
    /// [`ConstantsError::Table`] on any violation.
    pub fn parse(text: &str) -> Result<Self, ConstantsError> {
        let raw: RawTable =
            toml::from_str(text).map_err(|e| table_err(format!("not valid TOML: {e}")))?;
        if raw.schema_version != TABLE_SCHEMA_VERSION {
            return Err(table_err(format!(
                "schema_version {} is not {TABLE_SCHEMA_VERSION}",
                raw.schema_version
            )));
        }
        if raw.citations.values().any(|c| c.trim().is_empty()) {
            return Err(table_err("an entry of [citations] is empty"));
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut entries = Vec::with_capacity(raw.constant.len());
        for mut e in raw.constant {
            if !seen.insert(e.name.clone()) {
                return Err(table_err(format!("constant {:?} appears twice", e.name)));
            }
            if e.citation.is_empty() {
                return Err(table_err(format!("constant {:?} has no citation", e.name)));
            }
            if let Some(key) = e.citation.iter().find(|k| !raw.citations.contains_key(*k)) {
                return Err(table_err(format!(
                    "constant {:?} cites {key:?}, which [citations] does not define",
                    e.name
                )));
            }
            if e.unit.trim().is_empty() || e.unit_basis.trim().is_empty() {
                return Err(table_err(format!(
                    "constant {:?} lacks a unit or unit_basis",
                    e.name
                )));
            }
            e.value = parse_fortran_real(&e.token)
                .map_err(|err| table_err(format!("constant {:?}: {err}", e.name)))?;
            entries.push(e);
        }
        Ok(Self {
            version: raw.table_version,
            ephemeris: raw.ephemeris,
            source: raw.source,
            citations: raw.citations,
            entries,
        })
    }

    /// Content version ([`TABLE_VERSION`] when generated by this crate).
    #[must_use]
    pub const fn table_version(&self) -> i64 {
        self.version
    }

    /// Ephemeris label, e.g. `"DE440"`.
    #[must_use]
    pub fn ephemeris(&self) -> &str {
        &self.ephemeris
    }

    /// Provenance of the header the table was generated from.
    #[must_use]
    pub const fn source(&self) -> &SourceInfo {
        &self.source
    }

    /// Citation key → full citation.
    #[must_use]
    pub const fn citations(&self) -> &BTreeMap<String, String> {
        &self.citations
    }

    /// Entries in header order.
    #[must_use]
    pub fn entries(&self) -> &[TableEntry] {
        &self.entries
    }

    /// One entry by name.
    ///
    /// # Errors
    /// [`ConstantsError::ConstantMissing`] if absent.
    pub fn get(&self, name: &str) -> Result<&TableEntry, ConstantsError> {
        self.entries.iter().find(|e| e.name == name).ok_or_else(|| {
            ConstantsError::ConstantMissing {
                name: name.to_owned(),
                source_name: "data/de440-constants.toml",
            }
        })
    }
}

fn q(s: &str) -> String {
    toml::Value::String(s.to_owned()).to_string()
}

/// Render the table for `header` (selection [`DE440_SELECTION`]) with provenance `source`.
///
/// `source.title`, `constant_count` and `ma_count` are recomputed from `header`; the other
/// fields are taken as given. The output is deterministic, so re-rendering from the same header
/// and provenance reproduces the committed file exactly.
///
/// # Errors
/// [`ConstantsError::ConstantMissing`] if a selected constant is absent from the header.
pub fn render(header: &Header, source: &SourceInfo) -> Result<String, ConstantsError> {
    let mut s = String::new();
    let title = header.title().first().cloned().unwrap_or_default();
    // REUSE-IgnoreStart
    let _ = writeln!(
        s,
        "# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute\n\
         # SPDX-License-Identifier: Apache-2.0\n\
         # GENERATED FILE: do not edit by hand (REF-002). Regenerate with\n\
         #   cargo run -p ferroastro-constants --example gen_de440_table\n\
         # Each `token` is copied verbatim from JPL DE440 header.440 GROUP 1041 by the generator.\n\
         # Units are not stated in the header: `unit_basis` says why each unit is right, and an\n\
         # `open_item` marks entries that must not be used as oracles (AGENTS.md rule 8).\n\
         schema_version = {TABLE_SCHEMA_VERSION}\n\
         table_version = {TABLE_VERSION}\n\
         ephemeris = \"DE440\"\n"
    );
    // REUSE-IgnoreEnd
    let _ = writeln!(s, "[source]");
    let _ = writeln!(s, "product = {}", q(&source.product));
    let _ = writeln!(s, "url = {}", q(&source.url));
    let _ = writeln!(s, "sha256 = {}", q(&source.sha256));
    let _ = writeln!(s, "bytes = {}", source.bytes);
    let _ = writeln!(s, "retrieved = {}", q(&source.retrieved));
    if let Some(lm) = &source.last_modified {
        let _ = writeln!(s, "last_modified = {}", q(lm));
    }
    let _ = writeln!(s, "title = {}", q(&title));
    let _ = writeln!(s, "constant_count = {}", header.len());
    let _ = writeln!(s, "ma_count = {}\n", header.count_prefix("MA"));
    let _ = writeln!(s, "[citations]");
    let _ = writeln!(
        s,
        "{CITE_HEADER} = {}",
        q(&format!(
            "JPL Solar System Dynamics, DE440 ASCII header header.440, GROUP 1040/1041, {} \
             (sha256 {})",
            source.url, source.sha256
        ))
    );
    let _ = writeln!(s, "{CITE_PARK21} = {}", q(PARK21));
    for spec in DE440_SELECTION {
        let c = header.get(spec.name)?;
        let cites: Vec<String> = spec.citations.iter().map(|k| q(k)).collect();
        let _ = writeln!(s, "\n[[constant]]");
        let _ = writeln!(s, "name = {}", q(spec.name));
        let _ = writeln!(s, "token = {}", q(&c.token));
        let _ = writeln!(s, "unit = {}", q(spec.unit));
        let _ = writeln!(s, "quantity = {}", q(spec.quantity));
        let _ = writeln!(s, "citation = [{}]", cites.join(", "));
        let _ = writeln!(s, "unit_basis = {}", q(spec.unit_basis));
        if let Some(oi) = spec.open_item {
            let _ = writeln!(s, "open_item = {}", q(oi));
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;

    /// Verifies: PHY-002
    #[test]
    fn committed_table_parses_and_every_entry_is_cited() {
        let t = Table::de440().unwrap();
        assert_eq!(t.table_version(), TABLE_VERSION);
        assert_eq!(t.ephemeris(), "DE440");
        assert_eq!(t.entries().len(), DE440_SELECTION.len());
        for (e, spec) in t.entries().iter().zip(DE440_SELECTION) {
            assert_eq!(e.name, spec.name, "table order follows the selection");
            assert!(!e.citation.is_empty(), "{} has no citation", e.name);
            for key in &e.citation {
                assert!(
                    [CITE_HEADER, CITE_PARK21].contains(&key.as_str()),
                    "{}: citation {key} is neither DE440hdr nor Park21",
                    e.name
                );
                assert!(!t.citations()[key].trim().is_empty());
            }
            assert_eq!(e.open_item.as_deref(), spec.open_item);
            // Offline staleness check: the metadata must equal the generator's selection, so a
            // hand edit or a forgotten regeneration fails PR CI, not only the `data` job.
            assert_eq!(e.unit, spec.unit, "{}: regenerate the table", e.name);
            assert_eq!(
                e.quantity, spec.quantity,
                "{}: regenerate the table",
                e.name
            );
            assert_eq!(e.unit_basis, spec.unit_basis, "{}: regenerate", e.name);
            assert_eq!(e.citation, spec.citations, "{}: regenerate", e.name);
        }
        assert!(t.citations()[CITE_HEADER].contains(&t.source().sha256));
    }

    /// Verifies: PHY-002, REF-002
    #[test]
    fn table_rejects_uncited_or_unknown_citations() {
        let base = DE440_TABLE_TOML;
        let uncited = base.replacen("citation = [\"DE440hdr\"]", "citation = []", 1);
        let unknown = base.replacen("citation = [\"DE440hdr\"]", "citation = [\"Wiki\"]", 1);
        let dup = format!(
            "{base}\n[[constant]]\nname = \"AU\"\ntoken = \"0.1D+01\"\nunit = \"km\"\n\
             quantity = \"x\"\ncitation = [\"DE440hdr\"]\nunit_basis = \"x\"\n"
        );
        let bad_token = base.replacen("token = \"0.", "token = \"X0.", 1);
        for (what, text) in [
            ("uncited", uncited),
            ("unknown", unknown),
            ("duplicate", dup),
            ("token", bad_token),
        ] {
            let err = Table::parse(&text).unwrap_err();
            assert_eq!(err.code(), ErrorCode::ConstantsTable, "{what}: {err}");
        }
        let missing = Table::de440().unwrap().get("NOPE").unwrap_err();
        assert_eq!(missing.code(), ErrorCode::ConstantMissing);
    }
}
