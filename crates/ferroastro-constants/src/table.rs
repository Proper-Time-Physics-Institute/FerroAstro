// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The cited DE440 constants: committed **metadata**, values read at run time (PHY-002, REF-002).
//!
//! `data/de440-metadata.toml` lists which `header.440` constants FerroAstro uses, with the unit,
//! the citation, the `unit_basis` (why the unit is right) and the `open_item` of each, and the
//! URL, size and SHA-256 of the pinned header. It contains **no value** (maintainer decision of
//! 2026-11-14, #22): every number is parsed at run time from the hash-verified JPL file
//! ([`crate::De440::from_header_bytes`], `De440::load_cached`,
//! `De440::embedded`). [`Metadata::parse`] refuses a `token` or `value` key, and the
//! metadata-consistency test checks the file against `tests/data/MANIFEST.toml`.
//!
//! [`Table::from_header`] joins the metadata with a parsed [`Header`]: each entry gets the
//! verbatim GROUP 1041 token and its nearest `f64`. Entries whose meaning or unit is still an open
//! item say so in `open_item` (OI-04 GMSDOT, OI-23 CENTER) and must not be used as oracles
//! (AGENTS.md rule 8).

use crate::ConstantsError;
use crate::header440::Header;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// The committed DE440 constants metadata (TOML, no values), embedded at build time.
pub const DE440_METADATA_TOML: &str = include_str!("../data/de440-metadata.toml");

/// Repository path of [`DE440_METADATA_TOML`], for messages.
pub const DE440_METADATA_PATH: &str = "crates/ferroastro-constants/data/de440-metadata.toml";

/// Schema version of the metadata file (2: values removed, #22).
pub const METADATA_SCHEMA_VERSION: i64 = 2;

/// Citation key for the JPL DE440 header itself.
pub const CITE_HEADER: &str = "DE440hdr";
/// Citation key for the DE440 paper.
pub const CITE_PARK21: &str = "Park21";

/// Provenance of the pinned header the values are read from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceInfo {
    /// Name in `tests/data/MANIFEST.toml`.
    pub product: String,
    /// Download URL.
    pub url: String,
    /// Pinned SHA-256 (lowercase hex) of the header bytes; equal to the manifest pin.
    pub sha256: String,
    /// Size in bytes.
    pub bytes: u64,
    /// UTC date the pin was checked against a download.
    pub retrieved: String,
    /// Server `Last-Modified` header, if any.
    pub last_modified: Option<String>,
}

/// Metadata of one selected header constant. Contains no number.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryMeta {
    /// GROUP 1040 name.
    pub name: String,
    /// Unit of the header value.
    pub unit: String,
    /// What the constant is.
    pub quantity: String,
    /// Citation keys, resolved in the metadata's `[citations]`.
    pub citation: Vec<String>,
    /// Why the unit (and meaning) is right.
    pub unit_basis: String,
    /// Open item that leaves the unit or meaning unverified, if any.
    pub open_item: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMetadata {
    schema_version: i64,
    metadata_version: i64,
    ephemeris: String,
    source: SourceInfo,
    citations: BTreeMap<String, String>,
    constant: Vec<EntryMeta>,
}

/// The parsed and validated constants metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    version: i64,
    ephemeris: String,
    source: SourceInfo,
    citations: BTreeMap<String, String>,
    entries: Vec<EntryMeta>,
}

fn table_err(problem: impl Into<String>) -> ConstantsError {
    ConstantsError::Table {
        problem: problem.into(),
    }
}

fn is_lower_hex_sha256(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Metadata {
    /// The committed DE440 metadata ([`DE440_METADATA_TOML`]).
    ///
    /// # Errors
    /// See [`Metadata::parse`].
    pub fn de440() -> Result<Self, ConstantsError> {
        Self::parse(DE440_METADATA_TOML)
    }

    /// Parse and validate metadata: schema version, a lowercase-hex SHA-256 pin, unique names,
    /// a non-empty unit, `unit_basis` and citation list per entry, and every citation key
    /// resolving to a non-empty `[citations]` entry (PHY-002). Unknown keys (in particular a
    /// `token` or `value`) are refused: the metadata never carries a value (#22).
    ///
    /// # Errors
    /// [`ConstantsError::Table`] on any violation.
    pub fn parse(text: &str) -> Result<Self, ConstantsError> {
        let raw: RawMetadata =
            toml::from_str(text).map_err(|e| table_err(format!("not valid TOML: {e}")))?;
        if raw.schema_version != METADATA_SCHEMA_VERSION {
            return Err(table_err(format!(
                "schema_version {} is not {METADATA_SCHEMA_VERSION}",
                raw.schema_version
            )));
        }
        if !is_lower_hex_sha256(&raw.source.sha256) {
            return Err(table_err(format!(
                "source.sha256 {:?} is not 64 lowercase hex digits",
                raw.source.sha256
            )));
        }
        if raw.citations.values().any(|c| c.trim().is_empty()) {
            return Err(table_err("an entry of [citations] is empty"));
        }
        let mut seen = BTreeSet::new();
        for e in &raw.constant {
            if !seen.insert(e.name.as_str()) {
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
        }
        Ok(Self {
            version: raw.metadata_version,
            ephemeris: raw.ephemeris,
            source: raw.source,
            citations: raw.citations,
            entries: raw.constant,
        })
    }

    /// Content version of the metadata.
    #[must_use]
    pub const fn metadata_version(&self) -> i64 {
        self.version
    }

    /// Ephemeris label, e.g. `"DE440"`.
    #[must_use]
    pub fn ephemeris(&self) -> &str {
        &self.ephemeris
    }

    /// Provenance and pin of the header the values are read from.
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
    pub fn entries(&self) -> &[EntryMeta] {
        &self.entries
    }

    /// Check a SHA-256 (lowercase hex) against the pin.
    ///
    /// # Errors
    /// [`ConstantsError::HeaderChecksum`] if it differs.
    pub fn verify_sha256(&self, got: &str) -> Result<(), ConstantsError> {
        if got == self.source.sha256 {
            Ok(())
        } else {
            Err(ConstantsError::HeaderChecksum {
                expected: self.source.sha256.clone(),
                got: got.to_owned(),
            })
        }
    }
}

/// One table entry: metadata plus the run-time token and value from `header.440`.
#[derive(Debug, Clone, PartialEq)]
pub struct TableEntry {
    /// GROUP 1040 name.
    pub name: String,
    /// Verbatim GROUP 1041 token, read at run time.
    pub token: String,
    /// Unit of the value.
    pub unit: String,
    /// What the constant is.
    pub quantity: String,
    /// Citation keys, resolved in [`Table::citations`].
    pub citation: Vec<String>,
    /// Why the unit is right.
    pub unit_basis: String,
    /// Open item that leaves this entry unverified, if any.
    pub open_item: Option<String>,
    value: f64,
}

impl TableEntry {
    /// The value as the nearest `f64` of the token.
    #[must_use]
    pub const fn value(&self) -> f64 {
        self.value
    }
}

/// The metadata joined with a hash-verified `header.440` at run time.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    metadata: Metadata,
    title: String,
    constant_count: usize,
    ma_count: usize,
    entries: Vec<TableEntry>,
}

impl Table {
    /// Join `metadata` with `header` after checking the header's SHA-256 against the pin.
    ///
    /// # Errors
    /// [`ConstantsError::HeaderChecksum`] for any header other than the pinned one;
    /// [`ConstantsError::ConstantMissing`] if a selected constant is absent.
    pub fn from_header(metadata: &Metadata, header: &Header) -> Result<Self, ConstantsError> {
        metadata.verify_sha256(header.sha256())?;
        Self::from_header_unverified(metadata, header)
    }

    /// [`Table::from_header`] without the SHA-256 check (synthetic headers in tests only).
    pub(crate) fn from_header_unverified(
        metadata: &Metadata,
        header: &Header,
    ) -> Result<Self, ConstantsError> {
        let mut entries = Vec::with_capacity(metadata.entries().len());
        for m in metadata.entries() {
            let c = header.get(&m.name)?;
            entries.push(TableEntry {
                name: m.name.clone(),
                token: c.token.clone(),
                unit: m.unit.clone(),
                quantity: m.quantity.clone(),
                citation: m.citation.clone(),
                unit_basis: m.unit_basis.clone(),
                open_item: m.open_item.clone(),
                value: c.value,
            });
        }
        Ok(Self {
            metadata: metadata.clone(),
            title: header.title().first().cloned().unwrap_or_default(),
            constant_count: header.len(),
            ma_count: header.count_prefix("MA"),
            entries,
        })
    }

    /// The metadata this table was built from.
    #[must_use]
    pub const fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// Ephemeris label, e.g. `"DE440"`.
    #[must_use]
    pub fn ephemeris(&self) -> &str {
        self.metadata.ephemeris()
    }

    /// Provenance and pin of the header the values were read from.
    #[must_use]
    pub const fn source(&self) -> &SourceInfo {
        self.metadata.source()
    }

    /// Citation key → full citation.
    #[must_use]
    pub const fn citations(&self) -> &BTreeMap<String, String> {
        self.metadata.citations()
    }

    /// First GROUP 1010 title line of the header.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Number of constants in the header's GROUP 1040.
    #[must_use]
    pub const fn constant_count(&self) -> usize {
        self.constant_count
    }

    /// Number of header constants named `MA*` (asteroid GMs).
    #[must_use]
    pub const fn ma_count(&self) -> usize {
        self.ma_count
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
                source_name: "data/de440-metadata.toml",
            }
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;
    use std::fmt::Write as _;

    /// A synthetic header (structural fixture, not physics) that defines every name of the
    /// committed metadata, each with the token `0.250000000000000000D+01`, plus `extra`.
    pub(crate) fn synthetic_header(extra: &[(&str, &str)]) -> String {
        let meta = Metadata::de440().unwrap();
        let mut names: Vec<String> = meta.entries().iter().map(|e| e.name.clone()).collect();
        let mut tokens = vec!["0.250000000000000000D+01".to_owned(); names.len()];
        for (n, t) in extra {
            names.push((*n).to_owned());
            tokens.push((*t).to_owned());
        }
        let mut s = format!(
            "GROUP   1010\n\nSynthetic Ephemeris\n\nGROUP   1040\n\n{}\n",
            names.len()
        );
        for n in &names {
            let _ = writeln!(s, "  {n}");
        }
        let _ = write!(s, "\nGROUP   1041\n\n{}\n", tokens.len());
        for t in &tokens {
            let _ = writeln!(s, "  {t}");
        }
        s
    }

    /// Verifies: PHY-002
    #[test]
    fn committed_metadata_parses_and_every_entry_is_cited() {
        let m = Metadata::de440().unwrap();
        assert_eq!(m.ephemeris(), "DE440");
        assert_ne!(m.entries().len(), 0);
        for e in m.entries() {
            assert!(!e.citation.is_empty(), "{} has no citation", e.name);
            for key in &e.citation {
                assert!(
                    [CITE_HEADER, CITE_PARK21].contains(&key.as_str()),
                    "{}: citation {key} is neither DE440hdr nor Park21",
                    e.name
                );
                assert_ne!(m.citations()[key].trim(), "");
            }
        }
        assert!(m.citations()[CITE_HEADER].contains(&m.source().sha256));
    }

    /// Verifies: PHY-002, REF-002
    #[test]
    fn metadata_rejects_values_uncited_or_unknown_citations() {
        let base = DE440_METADATA_TOML;
        let uncited = base.replacen("citation = [\"DE440hdr\"]", "citation = []", 1);
        let unknown = base.replacen("citation = [\"DE440hdr\"]", "citation = [\"Wiki\"]", 1);
        let dup = format!(
            "{base}\n[[constant]]\nname = \"AU\"\nunit = \"km\"\n\
             quantity = \"x\"\ncitation = [\"DE440hdr\"]\nunit_basis = \"x\"\n"
        );
        // A value smuggled back in (the pre-#22 schema) is refused by name.
        let token = base.replacen("unit = \"1\"\n", "unit = \"1\"\ntoken = \"x\"\n", 1);
        let value = base.replacen("unit = \"1\"\n", "unit = \"1\"\nvalue = 1\n", 1);
        let bad_pin = base.replacen("sha256 = \"", "sha256 = \"X", 1);
        let old_schema = base.replacen("schema_version = 2", "schema_version = 1", 1);
        for (what, text) in [
            ("uncited", uncited),
            ("unknown", unknown),
            ("duplicate", dup),
            ("token", token),
            ("value", value),
            ("pin", bad_pin),
            ("schema", old_schema),
        ] {
            assert_ne!(text, base, "{what}: the fixture edit did not apply");
            let err = Metadata::parse(&text).unwrap_err();
            assert_eq!(err.code(), ErrorCode::ConstantsTable, "{what}: {err}");
        }
    }

    /// Verifies: REF-002
    #[test]
    fn table_joins_header_tokens_and_refuses_unpinned_headers() {
        let m = Metadata::de440().unwrap();
        let h = Header::parse(&synthetic_header(&[("MA0001", "0.5D+00")])).unwrap();
        // The checked constructor refuses any header but the pinned one (FA-E0304).
        let err = Table::from_header(&m, &h).unwrap_err();
        assert_eq!(err.code(), ErrorCode::HeaderChecksum, "{err}");
        assert!(err.to_string().contains(&m.source().sha256));
        assert!(err.to_string().contains(h.sha256()));
        let t = Table::from_header_unverified(&m, &h).unwrap();
        assert_eq!(t.entries().len(), m.entries().len());
        assert_eq!(t.title(), "Synthetic Ephemeris");
        assert_eq!(t.constant_count(), m.entries().len() + 1);
        assert_eq!(t.ma_count(), 1);
        for (e, meta) in t.entries().iter().zip(m.entries()) {
            assert_eq!(e.name, meta.name);
            assert_eq!(e.token, "0.250000000000000000D+01");
            assert_eq!(e.value().to_bits(), 2.5_f64.to_bits()); // lint: allow-literal(synthetic token)
            assert_eq!(e.unit, meta.unit);
            assert_eq!(e.open_item, meta.open_item);
        }
        let missing = t.get("NOPE").unwrap_err();
        assert_eq!(missing.code(), ErrorCode::ConstantMissing);
        // A header lacking a selected constant is refused (FA-E0301).
        let short = Header::parse(
            "GROUP   1040\n\n 1\n  DENUM\nGROUP   1041\n\n 1\n  0.250000000000000000D+01\n",
        )
        .unwrap();
        let err = Table::from_header_unverified(&m, &short).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ConstantMissing, "{err}");
    }
}
