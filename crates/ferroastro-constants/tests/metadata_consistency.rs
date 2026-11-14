// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Metadata consistency of the DE440 constants (REF-002, PHY-002; #22).
//!
//! The repository commits `data/de440-metadata.toml`: names, units, citations and the pin of
//! `header.440`, but **no value** (maintainer decision 2026-11-14). This replaces the former
//! byte-for-byte check of a committed value table and its generator:
//!
//! * offline: the metadata pin equals `tests/data/MANIFEST.toml`, and the file carries no value;
//! * cached header (PR tier, skipped with a message when absent outside CI): every metadata name
//!   is in the pinned header, whose size and SHA-256 equal the metadata;
//! * `data` tier (network, `#[ignore]`d): the REF-002 acceptance on a fresh download.

use ferroastro_constants::De440;
use ferroastro_constants::header440::{Header, sha256_hex};
use ferroastro_constants::table::{DE440_METADATA_TOML, Metadata};
use ferroastro_testdata::fetch_product;
use ferroastro_vectors::VectorSet;

/// True if `word` looks like a Fortran/scientific real token (`0.25D+01`, `1.5e-3`).
fn is_exponent_real(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '+');
    let Some(pos) = w.find(['D', 'd', 'E', 'e']) else {
        return false;
    };
    let (mantissa, exp) = w.split_at(pos);
    let exp = exp.get(1..).unwrap_or_default();
    let exp = exp.strip_prefix(['+', '-']).unwrap_or(exp);
    mantissa.contains('.')
        && mantissa
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        && mantissa.chars().any(|c| c.is_ascii_digit())
        && !exp.is_empty()
        && exp.chars().all(|c| c.is_ascii_digit())
}

/// Verifies: REF-002, PHY-002
#[test]
fn metadata_is_pinned_to_the_manifest_and_carries_no_value() {
    let m = Metadata::de440().unwrap();
    let pinned = ferroastro_testdata::product(&m.source().product).unwrap();
    assert_eq!(
        m.source().sha256,
        pinned.sha256,
        "metadata pin != MANIFEST pin"
    );
    assert_eq!(m.source().url, pinned.url);
    assert_eq!(m.ephemeris(), "DE440");
    // No value: neither a `token`/`value` key (also refused by the parser) nor a real-number
    // token anywhere in the file.
    for (i, line) in DE440_METADATA_TOML.lines().enumerate() {
        let key = line.split('=').next().unwrap_or_default().trim();
        assert!(
            key != "token" && key != "value",
            "line {}: {line:?} carries a value",
            i + 1
        );
        assert!(
            !line.split_whitespace().any(is_exponent_real),
            "line {}: {line:?} contains a real-number token",
            i + 1
        );
    }
    assert!(is_exponent_real("0.250000000000000000D+01") && is_exponent_real("1.5e-3"));
    assert!(!is_exponent_real("DE440") && !is_exponent_real("e.g.") && !is_exponent_real("1e"));
    println!(
        "{} metadata entries; pin {} = MANIFEST de440-header",
        m.entries().len(),
        m.source().sha256
    );
}

/// Verifies: REF-002, PHY-002
#[test]
fn metadata_names_exist_in_the_pinned_header() {
    let Some(de) = De440::load_cached_or_skip("metadata_names_exist_in_the_pinned_header").unwrap()
    else {
        return;
    };
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    println!("de440-header {}", f.provenance());
    let bytes = std::fs::read(&f.path).unwrap();
    let m = Metadata::de440().unwrap();
    assert_eq!(
        bytes.len() as u64,
        m.source().bytes,
        "header size != metadata"
    );
    assert_eq!(sha256_hex(&bytes), m.source().sha256);
    let h = Header::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(h.sha256(), m.source().sha256);
    let t = de.table();
    assert!(
        t.title().contains(m.ephemeris()),
        "title {:?} does not name {}",
        t.title(),
        m.ephemeris()
    );
    assert_eq!(t.entries().len(), m.entries().len());
    for (e, meta) in t.entries().iter().zip(m.entries()) {
        assert_eq!(e.name, meta.name);
        let live = h.get(&e.name).unwrap();
        assert_eq!(live.token, e.token, "{}", e.name);
        assert_eq!(live.value.to_bits(), e.value().to_bits(), "{}", e.name);
        println!("{:8} [{}] present", e.name, e.unit);
    }
    // A modified header (one byte) is refused (FA-E0304), even though it parses.
    let mut tampered = bytes.clone();
    if let Some(b) = tampered.iter_mut().rev().find(|b| b.is_ascii_digit()) {
        *b = if *b == b'0' { b'1' } else { b'0' };
    }
    let err = De440::from_header_bytes(&tampered).unwrap_err();
    assert_eq!(
        err.code(),
        ferroastro_core::ErrorCode::HeaderChecksum,
        "{err}"
    );
}

/// Verifies: REF-002
#[test]
#[ignore = "data: downloads JPL DE440 header.440 (network); run with --profile data"]
fn data_de440_header_ref002_acceptance() {
    let fetched = fetch_product("de440-header").unwrap();
    println!("de440-header {}", fetched.provenance());
    let bytes = std::fs::read(&fetched.path).unwrap();
    let de = De440::from_header_bytes(&bytes).unwrap();
    let h = Header::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    let v = VectorSet::load_canonical().unwrap();
    let count = |n: usize| f64::from(u32::try_from(n).unwrap());
    for (name, id) in [
        ("JDEPOC", "DE440-JDEPOC"),
        ("CLIGHT", "CONST-CLIGHT-SI"),
        ("BETA", "DE440-BETA"),
        ("GAMMA", "DE440-GAMMA"),
        ("AU", "CONST-AU-IAU2012"),
        ("ASUN", "DE440-ASUN"),
        ("EMRAT", "DE440-EMRAT"),
    ] {
        let got = h.value(name).unwrap();
        let diff = v
            .get(id)
            .unwrap()
            .check(got)
            .unwrap_or_else(|e| panic!("{e}"));
        println!("{name}: header.440 = {got:?}; oracle {id}; |diff| = {diff:e}");
    }
    // GMS against Park21 after the unit conversion (the au^3/d^2 digits are not committed).
    let gms = de.gm_sun().km3_per_s2();
    let diff = v.get("DE440-GMS-KM3S2").unwrap().check(gms).unwrap();
    println!("GMS: {gms:?} km^3/s^2; oracle DE440-GMS-KM3S2; |diff| = {diff:e}");
    for (what, n, id) in [
        ("constants", h.len(), "DE440-CONSTANT-COUNT"),
        ("MA* entries", h.count_prefix("MA"), "DE440-MA-COUNT"),
    ] {
        v.get(id)
            .unwrap()
            .check(count(n))
            .unwrap_or_else(|e| panic!("{e}"));
        println!("{what}: {n} (oracle {id})");
    }
    // Every one of the 645 values must parse (the full REF-002 loader path).
    assert!(h.constants().iter().all(|c| c.value.is_finite()));
}
