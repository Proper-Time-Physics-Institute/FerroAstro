// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `data` integration test: the committed DE440 constants table against a fresh download of the
//! real JPL header.440 (REF-002, PHY-002).
//!
//! Network-dependent, so `#[ignore]`d in the PR-fast tier. Run with
//! `cargo nextest run --workspace --profile data --run-ignored only`. Set `FA_DATA_DIR` to an
//! empty directory to force a re-download instead of reusing the checksummed cache.

use ferroastro_constants::header440::Header;
use ferroastro_constants::table::{Table, render};
use ferroastro_testdata::fetch_product;
use ferroastro_vectors::VectorSet;

fn header() -> Result<Header, Box<dyn std::error::Error>> {
    let fetched = fetch_product("de440-header")?;
    println!("de440-header {}", fetched.provenance());
    Ok(Header::parse(&std::fs::read_to_string(&fetched.path)?)?)
}

/// Verifies: REF-002, PHY-002
#[test]
#[ignore = "data: downloads JPL DE440 header.440 (network); run with --profile data"]
fn data_de440_table_matches_header() {
    let h = header().unwrap();
    let t = Table::de440().unwrap();
    println!("header title: {:?}; {} constants", h.title(), h.len());
    for e in t.entries() {
        let live = h.get(&e.name).unwrap();
        assert_eq!(
            live.token, e.token,
            "{}: table token differs from header",
            e.name
        );
        assert_eq!(live.value.to_bits(), e.value().to_bits(), "{}", e.name);
        println!("{:8} {} [{}] ok", e.name, e.token, e.unit);
    }
    // Re-rendering from the live header with the committed provenance must reproduce the
    // committed file byte for byte: nothing in it was edited by hand.
    let rendered = render(&h, t.source()).unwrap();
    assert!(
        rendered == ferroastro_constants::table::DE440_TABLE_TOML,
        "data/de440-constants.toml differs from a fresh render; regenerate it"
    );
    println!(
        "{} table entries match header.440 entry by entry",
        t.entries().len()
    );
}

/// Verifies: REF-002
#[test]
#[ignore = "data: downloads JPL DE440 header.440 (network); run with --profile data"]
fn data_de440_header_ref002_acceptance() {
    let h = header().unwrap();
    let v = VectorSet::load_workspace().unwrap();
    let count = |n: usize| f64::from(u32::try_from(n).unwrap());
    for (name, id) in [
        ("JDEPOC", "DE440-JDEPOC"),
        ("CLIGHT", "CONST-CLIGHT-SI"),
        ("BETA", "DE440-BETA"),
        ("GAMMA", "DE440-GAMMA"),
        ("AU", "CONST-AU-IAU2012"),
        ("J2SUN", "DE440-J2SUN"),
        ("ASUN", "DE440-ASUN"),
        ("GMS", "DE440-GMS-AU3D2"),
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
