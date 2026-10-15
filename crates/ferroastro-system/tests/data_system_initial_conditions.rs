// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Data tier (network, `#[ignore]`, run with the nextest `data` profile):
//!
//! * WP11 check 7: JPL Horizons DE441 vectors at JDEPOC assembled with
//!   `InitialConditions::from_states` against the header.440 ICs (gross bound: DE441 ≠ DE440);
//! * WP11 check 4 (with asteroids, report only until WP15): the barycentre of the header bodies
//!   plus the 343 numbered asteroids placed from sb441-n373s.bsp at JDEPOC.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Heliocentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_system::{
    Catalogue, DE440_MAJOR, InitialConditions, MassModel, SystemState, Tier, barycentre,
};
use ferroastro_testdata::{fetch_product, horizons};
use ferroastro_vectors::VectorSet;

fn fetch(name: &str) -> (Vec<u8>, String) {
    let f = fetch_product(name).unwrap_or_else(|e| panic!("{e}"));
    let prov = f.provenance();
    println!("{name}: {prov}");
    (std::fs::read(&f.path).unwrap(), prov)
}

fn header() -> (Header, String) {
    let (bytes, prov) = fetch("de440-header");
    (
        Header::parse(&String::from_utf8(bytes).unwrap()).unwrap(),
        prov,
    )
}

fn check(v: &VectorSet, id: &str, value: f64, at: &str) {
    let vector = v.get(id).unwrap();
    let allowed = vector.tolerance.allowed(vector.value().unwrap());
    println!(
        "{id:<44} max |diff| {value:.3e} {} (margin {:.3}; at {at})",
        vector.unit,
        value / allowed
    );
    if let Err(e) = vector.check(value) {
        panic!("{e}");
    }
}

/// WP11 check 7: Horizons DE441 barycentric vectors at JDEPOC vs the header.440 ICs.
///
/// Verifies: REF-002
#[test]
#[ignore = "data tier: JPL Horizons API"]
fn data_horizons_de441_initial_conditions_vs_header_gross_bound() {
    let v = VectorSet::load_canonical().unwrap();
    let (h, hprov) = header();
    let jd = h.value("JDEPOC").unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let mut states = Vec::new();
    for &(id, name) in &DE440_MAJOR {
        let q = horizons::VectorQuery {
            command: format!("{}", id.0),
            center: "500@0".into(),
            ref_plane: horizons::RefPlane::Frame,
            start_jd_tdb: jd,
            stop_jd_tdb: jd + 1.0,
            step_days: 1,
        };
        let t = horizons::fetch_vectors(&q).unwrap();
        for c in &t.chunks {
            println!("{name}: {}", c.line());
        }
        assert_eq!(
            t.sources(),
            vec!["DE441".to_owned()],
            "Horizons source changed"
        );
        assert!(
            t.jd_tdb
                .first()
                .is_some_and(|&j| j.to_bits() == jd.to_bits()),
            "first row is not JDEPOC"
        );
        let (r, vel) = (t.r_km.first().unwrap(), t.v_km_s.first().unwrap());
        let l = |x: f64| Length::from_km(x).unwrap();
        let s = |x: f64| Velocity::from_km_per_s(x).unwrap();
        states.push((
            id,
            State::new(Position::new(l(r[0]), l(r[1]), l(r[2])), vel.map(s)),
        ));
    }
    let epoch = Epoch::<Tdb>::from_jd_parts(jd, 0.0).unwrap();
    let hz = InitialConditions::from_states(
        epoch,
        states,
        "JPL Horizons API vectors, CENTER 500@0, ICRF, TDB, DE441",
    )
    .unwrap();
    let (mut pos, mut vel) = ((0.0_f64, String::new()), (0.0_f64, String::new()));
    println!("check 7: header.440 (DE440) minus Horizons (DE441) at JDEPOC, barycentric ICRF");
    for &(id, name) in &DE440_MAJOR {
        let a = ic.states().state(id).unwrap();
        let b = hz.states().state(id).unwrap();
        let dp = a
            .position()
            .components()
            .iter()
            .zip(&b.position().components())
            .map(|(x, y)| (x.km() - y.km()).abs())
            .fold(0.0, f64::max);
        let dv = a
            .velocity()
            .iter()
            .zip(&b.velocity())
            .map(|(x, y)| (x.km_per_s() - y.km_per_s()).abs())
            .fold(0.0, f64::max);
        println!(
            "  {name:<16} NAIF {:>3}: |dr| {dp:.3e} km, |dv| {dv:.3e} km/s",
            id.0
        );
        if dp > pos.0 {
            pos = (dp, name.to_owned());
        }
        if dv > vel.0 {
            vel = (dv, name.to_owned());
        }
    }
    check(
        &v,
        "SYSTEM-HORIZONS-DE441-VS-HEADER-POS-GROSS",
        pos.0,
        &pos.1,
    );
    check(
        &v,
        "SYSTEM-HORIZONS-DE441-VS-HEADER-VEL-GROSS",
        vel.0,
        &vel.1,
    );
}

/// WP11 check 4 with asteroids (report only; the assertion waits for WP15): the 343 numbered
/// asteroids of the header catalogue placed from sb441-n373s.bsp (Sun-centred) at JDEPOC, added
/// to the header bodies, and the barycentre under both mass models.
///
/// Verifies: NUM-018, NUM-009
#[test]
#[ignore = "data tier: downloads sb441-n373s.bsp (936.6 MB)"]
fn data_sb441_asteroids_at_jdepoc_barycentre_report() {
    let v = VectorSet::load_canonical().unwrap();
    let (h, hprov) = header();
    let (bsp, _) = fetch("sb441-n373s-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    let de = De440::load().unwrap();
    let c = de.speed_of_light();
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let epoch = ic.epoch();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let mut helio = SystemState::<Icrf, Heliocentric>::new();
    let mut missing = Vec::new();
    for id in cat.ids_in(Tier::Minor) {
        match spk.state::<Heliocentric, f64>(id, epoch) {
            Ok(s) => helio.push(id, &s).unwrap(),
            Err(e) => missing.push((id.0, e.to_string())),
        }
    }
    println!(
        "numbered asteroids placed from sb441-n373s.bsp at JDEPOC: {} (missing: {missing:?})",
        helio.len()
    );
    assert!(missing.is_empty(), "asteroids without a state: {missing:?}");
    v.get("SYSTEM-HEADER-NUMBERED-ASTEROIDS")
        .unwrap()
        .check(f64::from(u32::try_from(helio.len()).unwrap()))
        .unwrap();
    let asteroids = helio.barycentric(&sun);
    let mut all = ic.states().clone();
    for (id, s) in asteroids.states().unwrap() {
        all.push(id, &s).unwrap();
    }
    let unidentified: f64 = cat
        .unidentified_masses()
        .iter()
        .map(|u| u.gm.km3_per_s2())
        .sum();
    for model in [MassModel::Newtonian, MassModel::RelativisticDe440] {
        let without = barycentre(&cat, ic.states(), model, c).unwrap();
        let with = barycentre(&cat, &all, model, c).unwrap();
        println!(
            "REPORT (no assertion until WP15), {}: barycentre offset without asteroids {:.6} km \
             ({} bodies), with the 343 numbered asteroids {:.6} km ({} bodies); velocity {:.6e} / \
             {:.6e} km/s",
            model.label(),
            without.distance_km(),
            without.bodies,
            with.distance_km(),
            with.bodies,
            without.speed_km_s(),
            with.speed_km_s()
        );
    }
    println!(
        "not placed: {} unidentified MA entries (MA8001..MA8236), sum GM/GMS = {:.6e} (OI-24)",
        cat.unidentified_masses().len(),
        unidentified / de.gm_sun().km3_per_s2()
    );
}

/// WP15 check 1b: the REF-003 sampled file (jplephem 2.24) round-trips through
/// `ferroastro-ephem`: every target of the pinned sb441-n373s.bsp evaluated at the file's JDEPOC
/// equals its row (the WP10 REF-004 targets, 1 mm and 1 um/s).
///
/// Verifies: REF-003, REF-004
#[test]
#[ignore = "data tier: downloads sb441-n373s.bsp (936.6 MB)"]
fn data_ref003_sampled_states_round_trip_through_ferroastro_ephem() {
    use ferroastro_system::SmallBodyStates;
    let v = VectorSet::load_canonical().unwrap();
    let (bsp, sprov) = fetch("sb441-n373s-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file = SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    println!("{}", file.provenance());
    assert!(sprov.contains(file.source_sha256()), "file and SPK differ");
    let epoch = Epoch::<Tdb>::from_jd_parts(file.jdepoc().unwrap(), 0.0).unwrap();
    let (mut pos, mut vel) = ((0.0_f64, 0), (0.0_f64, 0));
    for r in file.records() {
        let s = spk.state::<Heliocentric, f64>(r.id, epoch).unwrap();
        let p = s.position().components().map(Length::km);
        let w = s.velocity().map(Velocity::km_per_s);
        for k in 0..3 {
            let dp = (p[k] - r.position_km[k]).abs();
            let dv = (w[k] - r.velocity_km_s[k]).abs();
            if dp > pos.0 {
                pos = (dp, r.id.0);
            }
            if dv > vel.0 {
                vel = (dv, r.id.0);
            }
        }
    }
    println!(
        "{} targets evaluated by ferroastro-ephem",
        file.records().len()
    );
    check(
        &v,
        "SB441-JPLEPHEM-POS-MAX-DIFF",
        pos.0,
        &format!("NAIF {}", pos.1),
    );
    check(
        &v,
        "SB441-JPLEPHEM-VEL-MAX-DIFF",
        vel.0,
        &format!("NAIF {}", vel.1),
    );
}

/// WP15 addendum: the NAIF ids of the identified KBOs name the expected objects in JPL Horizons,
/// and Horizons' heliocentric positions at JDEPOC agree with the sampled sb441 states to a gross
/// bound (identity, not accuracy: Horizons uses its current orbit solutions).
///
/// Verifies: REF-003
#[test]
#[ignore = "data tier: JPL Horizons API"]
fn data_identified_kbo_ids_name_the_expected_objects_in_horizons() {
    use ferroastro_system::{ASTEROID_NAIF_BASE, KBO_IDENTIFICATION, SmallBodyStates};
    let v = VectorSet::load_canonical().unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file = SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let jd = file.jdepoc().unwrap();
    let mut worst = (0.0_f64, String::new());
    for k in &KBO_IDENTIFICATION {
        let number = k.id.0 - ASTEROID_NAIF_BASE;
        let q = horizons::VectorQuery {
            command: format!("{number};"),
            center: "500@10".into(),
            ref_plane: horizons::RefPlane::Frame,
            start_jd_tdb: jd,
            stop_jd_tdb: jd + 1.0,
            step_days: 1,
        };
        let t = horizons::fetch_vectors(&q).unwrap();
        for c in &t.chunks {
            println!("{}: {}", k.name, c.line());
            assert!(
                c.target_name.contains(k.name) && c.target_name.contains(&number.to_string()),
                "NAIF {} is {:?} in Horizons, expected {}",
                k.id.0,
                c.target_name,
                k.name
            );
        }
        let r = t.r_km.first().unwrap();
        let s = file.get(k.id).unwrap();
        let d = (0..3)
            .map(|i| (r[i] - s.position_km[i]).abs())
            .fold(0.0, f64::max);
        println!(
            "  {} ({}): |Horizons - sb441| = {d:.1} km at JD {jd}",
            k.name, k.symbol
        );
        if d > worst.0 {
            worst = (d, k.name.to_owned());
        }
    }
    check(&v, "REF003-KBO-HORIZONS-POS-GROSS", worst.0, &worst.1);
}
