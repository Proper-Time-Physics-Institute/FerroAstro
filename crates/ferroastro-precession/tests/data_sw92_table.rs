// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data check (network, `data` tier): the SW92 Mercury elements in
//! `tests/vectors/mercury-sw92.toml` equal JPL's published Table 1, digit for digit.

use ferroastro_testdata::fetch_url;
use ferroastro_vectors::VectorSet;

const URL: &str = "https://ssd.jpl.nasa.gov/planets/approx_pos.html";

/// Verifies: PHY-004, PHY-001
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_sw92_table1_mercury() {
    // The page is not pinned (it is HTML and may be re-styled); its digest is printed instead.
    let f = fetch_url(URL, "sw92-approx_pos.html", None).unwrap();
    println!("SW92 Table 1 {}", f.provenance());
    let html = std::fs::read_to_string(&f.path).unwrap();
    let start = html.find("Table 1").expect("page has a 'Table 1'");
    let row = html[start..]
        .lines()
        .find(|l| l.trim_start().starts_with("Mercury"))
        .expect("Table 1 has a Mercury row");
    let tokens: Vec<&str> = row.split_whitespace().skip(1).collect();
    println!("Table 1 Mercury row: {tokens:?}");
    assert_eq!(tokens.len(), 6, "a, e, I, L, long.peri., long.node.");
    let vs = VectorSet::load_workspace().unwrap();
    for (id, tok) in [
        "SW92-MERCURY-A",
        "SW92-MERCURY-E",
        "SW92-MERCURY-I",
        "SW92-MERCURY-L",
        "SW92-MERCURY-VARPI",
        "SW92-MERCURY-NODE",
    ]
    .into_iter()
    .zip(tokens)
    {
        let v = vs.get(id).unwrap();
        assert!(
            v.source.ends_with(tok),
            "{id}: vector source {:?} does not quote the published token {tok}",
            v.source
        );
        let published: f64 = tok.parse().unwrap();
        v.check(published).unwrap();
        println!(
            "{id}: published {tok} == vector {} (exact)",
            v.value().unwrap()
        );
    }
}
