// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data check (network, `data` tier): the SW92 Mercury elements in
//! `tests/vectors/mercury-sw92.toml`, and the rates and mean motions in
//! `tests/vectors/park2017-estimator.toml`, equal JPL's published Tables 1 and 2a, digit for digit.

use ferroastro_testdata::fetch_url;
use ferroastro_vectors::VectorSet;

const URL: &str = "https://ssd.jpl.nasa.gov/planets/approx_pos.html";

/// Verifies: PHY-004, NUM-011
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
    // Rates (the line under each element row): a', e', I', L', long.peri.', long.node.'.
    let rate_row = |table: &str, planet: &str| -> Vec<String> {
        let start = html.find(table).expect("table heading present");
        let mut lines = html[start..].lines();
        lines
            .by_ref()
            .find(|l| l.trim_start().starts_with(planet))
            .expect("planet row present");
        let next = lines.next().expect("rate row present");
        next.split_whitespace().map(str::to_owned).collect()
    };
    let rate_checks = [
        ("SW92-MERCURY-VARPI-RATE", "Table 1", "Mercury", 4),
        ("SW92-T2A-MERCURY-VARPI-RATE", "Table 2a", "Mercury", 4),
        ("SW92-T1-MERCURY-L-RATE", "Table 1", "Mercury", 3),
        ("SW92-T1-VENUS-L-RATE", "Table 1", "Venus", 3),
        ("SW92-T1-EMB-L-RATE", "Table 1", "EM Bary", 3),
        ("SW92-T1-JUPITER-L-RATE", "Table 1", "Jupiter", 3),
        ("SW92-T1-SATURN-L-RATE", "Table 1", "Saturn", 3),
        ("SW92-T2A-MERCURY-L-RATE", "Table 2a", "Mercury", 3),
        ("SW92-T2A-VENUS-L-RATE", "Table 2a", "Venus", 3),
        ("SW92-T2A-EMB-L-RATE", "Table 2a", "EM Bary", 3),
        ("SW92-T2A-JUPITER-L-RATE", "Table 2a", "Jupiter", 3),
        ("SW92-T2A-SATURN-L-RATE", "Table 2a", "Saturn", 3),
    ];
    for (id, table, planet, col) in rate_checks {
        let row = rate_row(table, planet);
        assert_eq!(row.len(), 6, "{table} {planet} rate row {row:?}");
        let tok = &row[col];
        let v = vs.get(id).unwrap();
        assert!(
            v.source.ends_with(tok.as_str()),
            "{id}: vector source {:?} does not quote the published token {tok}",
            v.source
        );
        v.check(tok.parse().unwrap()).unwrap();
        println!("{id}: {table} {planet} rate column {col}: published {tok} == vector (exact)");
    }
    // The arcsec/cy oracle is the Table 1 value times 3600, exactly as its source states.
    let deg = vs.get("SW92-MERCURY-VARPI-RATE").unwrap().value().unwrap();
    let arcsec = vs.get("SW92-MERCURY-VARPI-RATE-VS-DE441").unwrap();
    assert!(arcsec.source.contains("x 3600"));
    let scaled = deg * 3600.0; // lint: allow-literal(arcsec per degree, a definition)
    assert!((arcsec.value().unwrap() - scaled).abs() <= 4.0 * f64::EPSILON * scaled);
}
