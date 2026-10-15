// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP15 checks 1a and 3 (PR tier: the committed REF-003 file and the cached header.440, CI-005):
//!
//! * the sampled small-body file covers every header.440 asteroid, records the pinned SPK and
//!   JDEPOC (REF-003, provisional while OI-24 is open);
//! * the KBO identification table follows from its rule (Pitjeva & Pitjev 2018 vectors);
//! * the DE440 KBO ring: 36 fixed points at 44 au in the `ECLIPJ2000` plane, no dipole;
//! * the relativistic SSB of the full placed body set at JDEPOC (NUM-018), non-gating while OI-24
//!   is open.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure with a clear message; counts are small"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_ephem::NaifId;
use ferroastro_system::{
    ASTEROID_NAIF_BASE, Catalogue, InitialConditions, KBO_IDENTIFICATION, KBO_RING, MassModel,
    SmallBodyStates, Tier, barycentre, kbo_ring_states, place_small_bodies,
};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap_or_else(|e| panic!("{e}"));
    let prov = f.provenance();
    println!("de440-header: {prov}");
    let text = std::fs::read_to_string(&f.path).unwrap();
    (Header::parse(&text).unwrap(), prov)
}

fn small_bodies() -> SmallBodyStates {
    let text = std::fs::read_to_string(root().join("tests/goldens/sb441-n373s-jdepoc-states.csv"))
        .unwrap();
    let s = SmallBodyStates::parse(&text).unwrap();
    println!("{}", s.provenance());
    s
}

fn open_items() -> String {
    std::fs::read_to_string(root().join("docs/requirements/OPEN-ITEMS.md")).unwrap()
}

/// WP15 check 1a: the committed file records the pinned SPK and JDEPOC and covers every header
/// asteroid; the other targets are the TNOs, including the identified KBOs.
///
/// Verifies: REF-003
#[test]
fn ref003_sampled_file_covers_every_header_asteroid_and_records_its_source() {
    let v = VectorSet::load_canonical().unwrap();
    let (h, hprov) = header();
    let s = small_bodies();
    let pin = ferroastro_testdata::product("sb441-n373s-bsp").unwrap();
    assert_eq!(
        s.source_sha256(),
        pin.sha256,
        "file not sampled from the pinned SPK"
    );
    assert_eq!(s.meta("source_file"), Some(pin.file.as_str()));
    assert_eq!(s.meta("source_url"), Some(pin.url.as_str()));
    assert!(s.meta("source_retrieved").is_some());
    let hpin = ferroastro_testdata::product("de440-header").unwrap();
    assert!(s.meta("header_file").unwrap().ends_with(&hpin.sha256));
    assert_eq!(
        s.jdepoc().unwrap().to_bits(),
        h.value("JDEPOC").unwrap().to_bits(),
        "states are not at header.440 JDEPOC"
    );
    v.get("REF003-SB441-TARGETS")
        .unwrap()
        .check(s.records().len() as f64)
        .unwrap();
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let asteroids: Vec<NaifId> = cat.ids_in(Tier::Minor).collect();
    for id in &asteroids {
        let r = s
            .get(*id)
            .unwrap_or_else(|e| panic!("header asteroid without a state: {e}"));
        let n = id.0 - ASTEROID_NAIF_BASE;
        assert_eq!(r.symbol.as_deref(), Some(format!("MA{n:04}").as_str()));
    }
    let others: Vec<NaifId> = s
        .records()
        .iter()
        .filter(|r| !asteroids.contains(&r.id))
        .map(|r| r.id)
        .collect();
    println!(
        "{} header asteroids with a state; {} other targets: {:?}",
        asteroids.len(),
        others.len(),
        others.iter().map(|i| i.0).collect::<Vec<_>>()
    );
    assert_eq!(
        others.len(),
        cat.unidentified_masses()
            .iter()
            .filter(|u| !u.symbol.starts_with("MA82"))
            .count(),
        "one non-asteroid target per header KBO mass (MA8001..MA8030)"
    );
    for k in &KBO_IDENTIFICATION {
        assert!(others.contains(&k.id), "{} is not a TNO target", k.name);
        assert!(s.get(k.id).unwrap().symbol.is_none());
    }
}

/// The KBO identification table is what its rule gives: `MA80nn` is the only header KBO GM in the
/// object's ±1σ interval and lies in no other object's interval.
///
/// Verifies: REF-003
#[test]
fn kbo_identification_follows_from_the_rule() {
    let v = VectorSet::load_canonical().unwrap();
    let (h, hprov) = header();
    let cat = Catalogue::from_header(&h, &hprov).unwrap();
    let gm_earth = cat.gm(NaifId::EARTH).unwrap().km3_per_s2();
    let kbos: Vec<(&str, f64)> = cat
        .unidentified_masses()
        .iter()
        .filter(|u| u.symbol.starts_with("MA80"))
        .map(|u| (u.symbol.as_str(), u.gm.km3_per_s2()))
        .collect();
    assert_eq!(kbos.len(), 30);
    let objects = [
        ("ERIS", "Eris"),
        ("HAUMEA", "Haumea"),
        ("MAKEMAKE", "Makemake"),
        ("2007-OR10", "2007 OR10"),
        ("QUAOAR", "Quaoar"),
        ("ORCUS", "Orcus"),
        ("2003-AZ84", "2003 AZ84"),
        ("SALACIA", "Salacia"),
        ("VARDA", "Varda"),
        ("2002-UX25", "2002 UX25"),
    ];
    let candidates: Vec<(&str, Vec<&str>)> = objects
        .iter()
        .map(|&(key, name)| {
            let vec = v.get(&format!("PP2018-T1-{key}-MASS")).unwrap();
            let (m, s) = (vec.value().unwrap(), vec.tolerance.abs.unwrap());
            let (lo, hi) = ((m - s) * gm_earth, (m + s) * gm_earth);
            let c: Vec<&str> = kbos
                .iter()
                .filter(|&&(_, gm)| lo <= gm && gm <= hi)
                .map(|&(sym, _)| sym)
                .collect();
            println!(
                "{name:<10} GM {:9.3} km^3/s^2 -> candidates {c:?}",
                m * gm_earth
            );
            (name, c)
        })
        .collect();
    let derived: Vec<(&str, &str)> = candidates
        .iter()
        .filter_map(|(name, c)| match c.as_slice() {
            [only] if candidates.iter().filter(|(_, d)| d.contains(only)).count() == 1 => {
                Some((*only, *name))
            }
            _ => None,
        })
        .collect();
    let table: Vec<(&str, &str)> = KBO_IDENTIFICATION
        .iter()
        .map(|k| (k.symbol, k.name))
        .collect();
    assert_eq!(derived, table);
}

/// The ring: 36 points with the MA8201… GMs, 44 au from the SSB in the `ECLIPJ2000` plane, at
/// rest, with no mass dipole (equal masses evenly spaced).
///
/// Verifies: NUM-009
#[test]
fn kbo_ring_points_are_fixed_on_the_44_au_ecliptic_circle() {
    let v = VectorSet::load_canonical().unwrap();
    v.get("DE440-KBO-RING-RADIUS")
        .unwrap()
        .check(KBO_RING.radius_au)
        .unwrap();
    v.get("DE440-KBO-RING-POINTS")
        .unwrap()
        .check(f64::from(KBO_RING.points))
        .unwrap();
    let (h, hprov) = header();
    let au = h.value("AU").unwrap();
    let mut cat = Catalogue::from_header(&h, &hprov).unwrap();
    cat.with_kbo_ring().unwrap();
    let ring = kbo_ring_states::<f64>(&KBO_RING, au).unwrap();
    assert_eq!(ring.len(), KBO_RING.points as usize);
    assert_eq!(cat.ids_in(Tier::Ring).collect::<Vec<_>>(), ring.ids());
    let to_ecl = ferroastro_frames::icrf_to_ecliptic_j2000::<f64>();
    let radius = KBO_RING.radius_au * au;
    let mut dipole = [0.0_f64; 3];
    for i in 0..ring.len() {
        let (p, w) = ring.raw_at(i).unwrap();
        let e = to_ecl.apply(p);
        let r = p[0].hypot(p[1]).hypot(p[2]);
        assert!(
            (r - radius).abs() <= f64::EPSILON * radius * 8.0,
            "radius {r}"
        ); // lint: allow-literal(8 ulp round-off bound of a rotation)
        assert!(
            e[2].abs() <= f64::EPSILON * radius * 8.0,
            "off the ecliptic: {}",
            e[2]
        ); // lint: allow-literal(8 ulp round-off bound of a rotation)
        assert!(
            w.iter().all(|x| x.to_bits() == 0),
            "ring point moves: {w:?}"
        );
        let gm = cat.gm(ring.ids()[i]).unwrap().km3_per_s2();
        for k in 0..3 {
            dipole[k] += gm * p[k];
        }
    }
    let total: f64 = cat
        .bodies()
        .iter()
        .filter(|b| b.tier() == Tier::Ring)
        .map(|b| b.gm().km3_per_s2())
        .sum();
    let offset = dipole[0].hypot(dipole[1]).hypot(dipole[2]) / total;
    println!(
        "ring: {} points, total GM {total:.6} km^3/s^2, dipole offset {offset:.3e} km",
        ring.len()
    );
    let bound = f64::EPSILON * radius * 64.0; // lint: allow-literal(64 ulp round-off bound of a 36-term sum)
    assert!(offset <= bound, "ring dipole {offset} km");
    // Promoting twice is refused.
    let err = cat.with_kbo_ring().unwrap_err();
    assert!(err.to_string().starts_with("FA-E1100"), "{err}");
}

/// WP15 check 3: the relativistic SSB of the full placed body set at JDEPOC (11 header bodies,
/// 343 asteroids and the identified KBOs from the REF-003 file, the ring). Non-gating while OI-24
/// is open (the vector's tolerance is the bound on what the 27 unplaced KBO masses could add).
///
/// Verifies: NUM-018, REF-003
#[test]
fn ssb_of_the_full_placed_body_set_at_jdepoc() {
    let v = VectorSet::load_canonical().unwrap();
    let (h, hprov) = header();
    let de = De440::load().unwrap();
    let s = small_bodies();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let mut cat = Catalogue::from_header(&h, &hprov).unwrap();
    cat.with_identified_kbos().unwrap();
    cat.with_kbo_ring().unwrap();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let minor: Vec<NaifId> = cat.ids_in(Tier::Minor).collect();
    let placed = place_small_bodies(&s, &minor, &sun).unwrap();
    let ring = kbo_ring_states::<f64>(&KBO_RING, h.value("AU").unwrap()).unwrap();
    let mut all = ic.states().clone();
    let mut majors_and_asteroids = ic.states().clone();
    for (id, st) in placed.states().unwrap() {
        all.push(id, &st).unwrap();
        if !KBO_IDENTIFICATION.iter().any(|k| k.id == id) {
            majors_and_asteroids.push(id, &st).unwrap();
        }
    }
    for (id, st) in ring.states().unwrap() {
        all.push(id, &st).unwrap();
    }
    let c = de.speed_of_light();
    let unplaced: f64 = cat
        .unidentified_masses()
        .iter()
        .map(|u| u.gm.km3_per_s2())
        .sum();
    println!(
        "placed: {} bodies ({} Minor incl. {} identified KBOs, {} Ring); unplaced KBO GM {unplaced:.6} km^3/s^2 ({} masses)",
        all.len(),
        minor.len(),
        KBO_IDENTIFICATION.len(),
        ring.len(),
        cat.unidentified_masses().len()
    );
    let mut result = 0.0;
    for model in [MassModel::Newtonian, MassModel::RelativisticDe440] {
        let majors = barycentre(&cat, ic.states(), model, c).unwrap();
        let mid = barycentre(&cat, &majors_and_asteroids, model, c).unwrap();
        let full = barycentre(&cat, &all, model, c).unwrap();
        println!(
            "{}: SSB offset: header bodies {:.6} km; + 343 asteroids {:.6} km; + identified KBOs \
             + ring {:.6} km (velocity {:.3e} km/s)",
            model.label(),
            majors.distance_km(),
            mid.distance_km(),
            full.distance_km(),
            full.speed_km_s()
        );
        result = full.distance_km();
    }
    let vec = v.get("SSB-FULL-SET-OFFSET-POS").unwrap();
    let tol = vec.tolerance.abs.unwrap();
    let blocked = vec.blocking_open_item(&open_items());
    println!(
        "check 3 (mu*): {result:.6} km vs bound {tol} km, margin {:.3} ({}){}",
        result / tol,
        if result <= tol { "within" } else { "OUTSIDE" },
        blocked.map_or_else(String::new, |oi| format!(", non-gating: {oi} open")),
    );
    if blocked.is_none() {
        vec.check(result).unwrap();
    }
}

/// Placement: barycentric small-body state = Sun-centred file state + the Sun's barycentric
/// state, component by component (one rounding each), for every Minor body.
///
/// Verifies: REF-003
#[test]
fn small_bodies_are_placed_relative_to_the_header_sun() {
    let (h, hprov) = header();
    let s = small_bodies();
    let ic = InitialConditions::<f64>::from_header(&h, &hprov).unwrap();
    let mut cat = Catalogue::from_header(&h, &hprov).unwrap();
    cat.with_identified_kbos().unwrap();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let sp = sun
        .position()
        .components()
        .map(ferroastro_core::units::Length::km);
    let sv = sun
        .velocity()
        .map(ferroastro_core::units::Velocity::km_per_s);
    let ids: Vec<NaifId> = cat.ids_in(Tier::Minor).collect();
    let placed = place_small_bodies(&s, &ids, &sun).unwrap();
    assert_eq!(placed.ids(), ids.as_slice());
    for (i, id) in ids.iter().enumerate() {
        let (p, w) = placed.raw_at(i).unwrap();
        let r = s.get(*id).unwrap();
        for k in 0..3 {
            assert_eq!(
                p[k].to_bits(),
                (r.position_km[k] + sp[k]).to_bits(),
                "{id:?}"
            );
            assert_eq!(
                w[k].to_bits(),
                (r.velocity_km_s[k] + sv[k]).to_bits(),
                "{id:?}"
            );
        }
    }
    let missing = place_small_bodies(&s, &[NaifId::SUN], &sun).unwrap_err();
    assert!(missing.to_string().starts_with("FA-E1100"), "{missing}");
}
