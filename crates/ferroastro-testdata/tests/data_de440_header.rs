// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `data` integration test against the real JPL DE440 ASCII header (REF-002, PHY-002).
//!
//! Network-dependent, so `#[ignore]`d in the PR-fast tier. Run it with
//! `cargo nextest run --workspace --profile data --run-ignored only`
//! (or `cargo test -p ferroastro-testdata -- --ignored --nocapture`).
//!
//! The GROUP 1040/1041 reader below is a deliberately minimal smoke-test reader. The production
//! `header.440` parser belongs in `ferroastro-ephem` / `ferroastro-constants` (ARCHITECTURE.md) and
//! should replace it.

use ferroastro_testdata::fetch_product;
use ferroastro_vectors::VectorSet;
use std::collections::BTreeMap;

/// Split the text that follows `GROUP   <n>` up to the next GROUP.
fn group<'a>(text: &'a str, n: &str) -> Option<&'a str> {
    let marker = format!("GROUP   {n}");
    let start = text.find(&marker)? + marker.len();
    let rest = &text[start..];
    Some(rest.find("GROUP").map_or(rest, |end| &rest[..end]))
}

/// Parse GROUP 1040 (count + names) and GROUP 1041 (count + Fortran `D`-exponent values).
fn constants(text: &str) -> Result<(usize, BTreeMap<String, f64>), String> {
    let mut names = group(text, "1040")
        .ok_or("no GROUP 1040")?
        .split_whitespace();
    let mut values = group(text, "1041")
        .ok_or("no GROUP 1041")?
        .split_whitespace();
    let n_names: usize = names
        .next()
        .ok_or("empty 1040")?
        .parse()
        .map_err(|e| format!("{e}"))?;
    let n_values: usize = values
        .next()
        .ok_or("empty 1041")?
        .parse()
        .map_err(|e| format!("{e}"))?;
    if n_names != n_values {
        return Err(format!(
            "GROUP 1040 declares {n_names} names but 1041 declares {n_values} values"
        ));
    }
    let names: Vec<&str> = names.collect();
    let values: Vec<f64> = values
        .map(|v| {
            v.replace(['D', 'd'], "E")
                .parse::<f64>()
                .map_err(|e| format!("{v}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    if names.len() != n_names || values.len() != n_values {
        return Err(format!(
            "declared {n_names} constants, found {} names and {} values",
            names.len(),
            values.len()
        ));
    }
    Ok((
        n_names,
        names.into_iter().map(str::to_owned).zip(values).collect(),
    ))
}

/// Verifies: REF-002, PHY-002
#[test]
#[ignore = "data: downloads JPL DE440 header.440 (network); run with --profile data"]
fn data_de440_header_defined_constants() {
    let fetched = fetch_product("de440-header").unwrap();
    println!("de440-header {}", fetched.provenance());
    let text = std::fs::read_to_string(&fetched.path).unwrap();
    let (declared, table) = constants(&text).unwrap();
    println!(
        "GROUP 1040/1041: {declared} constants declared, {} distinct names",
        table.len()
    );
    assert_eq!(
        table.len(),
        declared,
        "duplicate constant names in GROUP 1040"
    );

    let vectors = VectorSet::load_workspace().unwrap();
    for (name, id) in [("CLIGHT", "CONST-CLIGHT-SI"), ("AU", "CONST-AU-IAU2012")] {
        let v = vectors.get(id).unwrap();
        let header = table[name];
        let diff = v.check(header).unwrap();
        println!(
            "{name}: header.440 = {header:?} {unit}; oracle {id} = {:?} {unit}; |diff| = {diff:e}",
            v.value().unwrap(),
            unit = v.unit,
        );
    }
}
