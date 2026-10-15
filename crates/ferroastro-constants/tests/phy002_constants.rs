// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Offline checks of the committed DE440 constants table against provenance-tracked vectors
//! (PHY-002, REF-002). The `data` test `data_de440_table_matches_header` checks the same table
//! against a fresh download of header.440.

use ferroastro_constants::table::{CITE_HEADER, CITE_PARK21, Table};
use ferroastro_constants::{Body, De440};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_vectors::{VectorError, VectorSet};

/// Check `measured` against vector `id` and print the comparison (test output is the evidence).
fn check(id: &str, measured: f64, vectors: &VectorSet) -> Result<(), VectorError> {
    let v = vectors.get(id)?;
    let expected = v.value()?;
    let diff = v.check(measured)?;
    println!(
        "{id}: measured {measured:?} {u}; expected {expected:?} {u}; |diff| = {diff:e}; allowed {:e}",
        v.tolerance.allowed(expected),
        u = v.unit
    );
    Ok(())
}

/// Verifies: PHY-002
#[test]
fn asserted_de440_constants() {
    let de = De440::load().unwrap();
    let v = VectorSet::load_workspace().unwrap();
    check("DE440-GMS-KM3S2", de.gm_sun().km3_per_s2(), &v).unwrap();
    check("CONST-CLIGHT-SI", de.speed_of_light().km_per_s(), &v).unwrap();
    check("CONST-AU-IAU2012", de.au().km(), &v).unwrap();
    check("DE440-ASUN", de.sun_radius().km(), &v).unwrap();
    check("DE440-MOISUN", de.sun_moment_of_inertia(), &v).unwrap();
    check("DE440-EMRAT", de.earth_moon_mass_ratio(), &v).unwrap();
    let omega_rad_d = de.sun_rotation_rate().rad_per_s() * SECONDS_PER_DAY;
    check("DE440-OMGSUN-RADD", omega_rad_d, &v).unwrap();
    check("PARK21-OMGSUN-DEGD", omega_rad_d.to_degrees(), &v).unwrap();
    check("DE440-J2SUN", de.sun_j2(), &v).unwrap();
    let gms = de.table().get("GMS").unwrap();
    assert_eq!(gms.unit, "au^3/d^2");
    check("DE440-GMS-AU3D2", gms.value(), &v).unwrap();
}

/// Verifies: PHY-002
#[test]
fn planetary_gms_match_park21_table_2() {
    let de = De440::load().unwrap();
    let v = VectorSet::load_workspace().unwrap();
    for (body, id) in [
        (Body::Mercury, "DE440-GM-MERCURY-KM3S2"),
        (Body::Venus, "DE440-GM-VENUS-KM3S2"),
        (Body::Earth, "DE440-GM-EARTH-KM3S2"),
        (Body::Moon, "DE440-GM-MOON-KM3S2"),
        (Body::Mars, "DE440-GM-MARS-KM3S2"),
        (Body::Jupiter, "DE440-GM-JUPITER-KM3S2"),
        (Body::Saturn, "DE440-GM-SATURN-KM3S2"),
        (Body::Uranus, "DE440-GM-URANUS-KM3S2"),
        (Body::Neptune, "DE440-GM-NEPTUNE-KM3S2"),
        (Body::Pluto, "DE440-GM-PLUTO-KM3S2"),
    ] {
        check(id, de.gm(body).km3_per_s2(), &v).unwrap();
    }
    // Earth + Moon must rebuild the Earth-Moon system GM (structural, not a physics value).
    let em = de.gm(Body::EarthMoon).km3_per_s2();
    let sum = de.gm(Body::Earth).km3_per_s2() + de.gm(Body::Moon).km3_per_s2();
    assert!((sum - em).abs() <= 4.0 * f64::EPSILON * em, "{sum} vs {em}");
}

/// Verifies: REF-002
#[test]
fn table_scalars_and_counts() {
    let de = De440::load().unwrap();
    let v = VectorSet::load_workspace().unwrap();
    let epoch = de.initial_epoch().unwrap();
    assert_eq!(epoch.scale_name(), "TDB");
    check("DE440-JDEPOC", epoch.jd(), &v).unwrap();
    check("DE440-BETA", de.ppn_beta(), &v).unwrap();
    check("DE440-GAMMA", de.ppn_gamma(), &v).unwrap();
    let src = de.table().source();
    check(
        "DE440-CONSTANT-COUNT",
        f64::from(u32::try_from(src.constant_count).unwrap()),
        &v,
    )
    .unwrap();
    check(
        "DE440-MA-COUNT",
        f64::from(u32::try_from(src.ma_count).unwrap()),
        &v,
    )
    .unwrap();
}

/// Verifies: PHY-002, REF-002
#[test]
fn table_is_cited_versioned_and_pinned_to_the_manifest() {
    let t = Table::de440().unwrap();
    let pinned = ferroastro_testdata::product(&t.source().product).unwrap();
    assert_eq!(
        t.source().sha256,
        pinned.sha256,
        "table built from the pinned header"
    );
    assert_eq!(t.source().url, pinned.url);
    for e in t.entries() {
        assert!(!e.citation.is_empty(), "{} uncited", e.name);
        assert!(
            e.citation
                .iter()
                .all(|k| k == CITE_HEADER || k == CITE_PARK21),
            "{}: citation must be DE440hdr or Park21",
            e.name
        );
    }
    println!(
        "table v{} ({}) from {} sha256={} retrieved={}",
        t.table_version(),
        t.ephemeris(),
        t.source().url,
        t.source().sha256,
        t.source().retrieved
    );
}

/// Verifies: PHY-002
#[test]
fn open_item_entries_are_flagged_and_not_exposed() {
    // OI-04 (GMSDOT units) and OI-23 (CENTER meaning) are UNVERIFIED: never used as values here.
    let t = Table::de440().unwrap();
    assert_eq!(t.get("GMSDOT").unwrap().open_item.as_deref(), Some("OI-04"));
    assert_eq!(t.get("CENTER").unwrap().open_item.as_deref(), Some("OI-23"));
    let flagged: Vec<&str> = t
        .entries()
        .iter()
        .filter(|e| e.open_item.is_some())
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(flagged, ["CENTER", "GMSDOT"]);
}

/// Verifies: API-001
#[test]
fn body_discriminants_follow_all() {
    for (i, b) in Body::ALL.iter().enumerate() {
        assert_eq!(*b as usize, i, "{b:?}");
    }
}
