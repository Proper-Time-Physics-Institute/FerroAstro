// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The jpl oracle's REF-001 verification golden (CI-005 cached; no container started here).

use ferroastro_oracle::Manifest;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

/// The jpl image fetched header.440, testpo.440 and de440.bsp from their JPL URLs and found each
/// SHA-256 equal to its pin in `tests/data/MANIFEST.toml`; the report names the same URL and pin.
/// Its independent cross-check (jplephem on de440.bsp vs an own header.440 reader at JDEPOC, no
/// FerroAstro code) stays within the WP11 bounds that FerroAstro's own check uses.
///
/// Verifies: REF-001, CI-003
#[test]
fn jpl_image_verified_the_ref001_pins() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ci/golden/jpl/ref001-pins");
    let m = Manifest::verify_dir(&dir).unwrap();
    let e = m.entry("ref001-verification.json").unwrap();
    let report: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join(&e.file)).unwrap()).unwrap();
    println!(
        "{} ({}, image {}, runner {})",
        e.file, e.version, e.image_digest, e.runner_sha
    );
    assert_eq!(report["all_pins_ok"], serde_json::Value::Bool(true));
    let products = report["products"].as_array().unwrap();
    let names: Vec<&str> = products
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["de440-header", "de440-testpo", "de440-bsp"]);
    for p in products {
        let pin = ferroastro_testdata::product(p["name"].as_str().unwrap()).unwrap();
        println!(
            "{}: {} bytes, sha256 {}, fetched {}, Last-Modified {}",
            pin.name, p["bytes"], p["sha256"], p["fetched"], p["last_modified"]
        );
        assert_eq!(p["url"].as_str().unwrap(), pin.url);
        assert!(
            pin.url
                .starts_with("https://ssd.jpl.nasa.gov/ftp/eph/planets/")
        );
        assert_eq!(p["sha256_pinned"].as_str().unwrap(), pin.sha256);
        assert_eq!(p["sha256"].as_str().unwrap(), pin.sha256);
        e.check_input(&pin.file, &pin.sha256).unwrap();
    }
    let x = &report["header_vs_bsp_at_jdepoc"];
    let (dr, dv) = (
        x["max_abs_dr_km"].as_f64().unwrap(),
        x["max_abs_dv_km_s"].as_f64().unwrap(),
    );
    println!("jplephem: header.440 vs de440.bsp at JDEPOC: max |dr| {dr:e} km, |dv| {dv:e} km/s");
    let vs = VectorSet::load_canonical().unwrap();
    vs.get("SYSTEM-HEADER-VS-DE440-POS-MAX-DIFF")
        .unwrap()
        .check(dr)
        .unwrap();
    vs.get("SYSTEM-HEADER-VS-DE440-VEL-MAX-DIFF")
        .unwrap()
        .check(dv)
        .unwrap();
}
