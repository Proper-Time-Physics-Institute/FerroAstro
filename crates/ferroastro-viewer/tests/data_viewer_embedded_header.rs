// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Data tier (DESIGN §5, #22): the header.440 that the `embed-header` feature included at build
//! time is, byte for byte, the pinned JPL file of the data cache, and the System-mode initial
//! conditions assembled from it equal those from that file. Needs the cached `de440-header`
//! product (`FA_DATA_DIR`) and a build with `--features embed-header`; prints no header value.

use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, initial_conditions};
use ferroastro_viewer::ViewerConfig;
use ferroastro_viewer::config::{EngineKind, bits};
use ferroastro_viewer::ic;

/// Verifies: API-004, REF-002
#[test]
#[ignore = "data tier: needs header.440 ($FA_DATA_DIR) and --features embed-header"]
fn data_embedded_header_is_the_pinned_file_and_gives_identical_ics() {
    let Some(embedded) = ic::HEADER_440 else {
        eprintln!(
            "SKIP data_embedded_header_is_the_pinned_file_and_gives_identical_ics: built without \
             embed-header (rerun with --features embed-header)"
        );
        return;
    };
    let product = ferroastro_testdata::product("de440-header").unwrap();
    let f = ferroastro_testdata::fetch_product("de440-header").expect("header.440");
    println!("de440-header: {}", f.provenance());
    assert_eq!(f.sha256, product.sha256);
    let file = std::fs::read(&f.path).unwrap();
    assert!(
        embedded == file.as_slice(),
        "the embedded header.440 differs from the cached file"
    );
    assert_eq!(ferroastro_testdata::sha256_hex(embedded), product.sha256);
    assert!(ic::header_provenance().contains(&product.sha256));
    // Every body record of the full model with asteroids, from the embedded header and the file.
    let full = Header::parse(std::str::from_utf8(&file).unwrap()).unwrap();
    let mut cfg = ViewerConfig::default();
    cfg.set_mask(bits::DEFAULT | bits::ASTEROIDS).unwrap();
    let spec = cfg.to_scenario(EngineKind::Ias15).unwrap();
    let sb = ic::small_bodies().unwrap();
    let ics = |h: &Header| {
        initial_conditions(
            &spec,
            &DataSources::none()
                .with_header(h, ic::header_provenance())
                .with_small_bodies(sb),
        )
        .unwrap()
    };
    let (a, b) = (ics(ic::header().unwrap()), ics(&full));
    assert!(a.records.len() > 9);
    assert_eq!(a.records, b.records);
    assert_eq!(a.ic_hash, b.ic_hash);
}
