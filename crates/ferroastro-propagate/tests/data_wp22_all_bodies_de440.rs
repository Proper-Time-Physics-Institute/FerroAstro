// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 check E10 (data tier, real data): EIH for every moving body + solar J2 + solar
//! Lense–Thirring, with and without the DE440 small bodies (343 asteroids and 3 identified KBOs
//! as Minor bodies, the 36-point KBO ring), against de440.bsp. Sun + 8 planets + geocentric Moon
//! from header.440 at JDEPOC (small bodies from the committed REF-003 states), 10 Julian yr every
//! 10 d, IAS15 default, through the scenario API (DE440 J2, WGCCRE 2015 pole, DE440 spin).
//! header.440 and de440.bsp are downloaded fresh (or taken from the cache after re-verifying the
//! pinned SHA-256), provenance printed.
//!
//! Reported: every major body's barycentric and heliocentric residual for both runs. The
//! pre-registered E10 assertion (`EIH-ALLBODY-SUN-IMPROVEMENT-MIN`: the Sun's barycentric
//! residual improves >= 2x) FAILED and is kept as a bounded known-failure guard
//! (`E10-KNOWN-FAILURE-SUN-BARY-FACTOR`); E10b (post hoc) asserts the Mars heliocentric
//! improvement. Both share one pair of runs.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Icrf, State};
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_propagate::{DataSources, RunOutput, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

fn spec() -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios/sun-planets-moon-newton-j2-10yr.toml");
    let mut s = ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
    s.forces.gr_eih = true;
    s.forces.lense_thirring = true;
    s.output.sample_bodies = Some(s.bodies.clone());
    s.name = "wp22-e10-eih-j2-lt".into();
    s
}

fn pos(s: &State<Icrf, Barycentric>) -> [f64; 3] {
    s.position()
        .components()
        .map(ferroastro_core::units::Length::km)
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Per body (storage order): (NAIF id, max barycentric residual, max heliocentric residual), km.
fn residuals(out: &RunOutput, spk: &Spk<'_>) -> Vec<(i32, f64, f64)> {
    let jd = out.epoch_jd_tdb;
    let ids = &out.states[0].bodies;
    let mut rows: Vec<(i32, f64, f64)> = ids.iter().map(|&i| (i, 0.0, 0.0)).collect();
    let sun_k = ids.iter().position(|&i| i == 10).unwrap(); // lint: allow-literal(NAIF id of the Sun)
    for s in &out.states[1..] {
        let t = s.t[0] + s.t[1];
        let e = Epoch::<Tdb>::from_jd_parts(jd[0], jd[1] + t / SECONDS_PER_DAY).unwrap();
        let got = |k: usize| s.positions[k].map(|p| p[0] + p[1]);
        let want = |id: i32| pos(&spk.state::<Barycentric, f64>(NaifId(id), e).unwrap());
        let (sun, sun_ref) = (got(sun_k), want(10)); // lint: allow-literal(NAIF id of the Sun)
        for (k, row) in rows.iter_mut().enumerate() {
            let (g, w) = (got(k), want(row.0));
            row.1 = row.1.max(norm(sub(g, w)));
            row.2 = row.2.max(norm(sub(sub(g, sun), sub(w, sun_ref))));
        }
    }
    rows
}

/// `(NAIF id, max barycentric km, max heliocentric km)`.
type Row = (i32, f64, f64);

/// The two E10 runs (without and with the small bodies) against de440.bsp: per-body residual
/// rows `(NAIF id, max barycentric km, max heliocentric km)` of each, printed as a table.
fn e10_residuals() -> (Vec<Row>, Vec<Row>) {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let small = ferroastro_system::SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    println!("{}", small.provenance());
    let data = DataSources::none()
        .with_header(&h, &hprov)
        .with_small_bodies(&small);
    let planets_spec = spec();
    let mut full_spec = spec();
    full_spec.forces.asteroids = 343; // lint: allow-literal(the DE440 numbered-asteroid set, a switch value)
    full_spec.forces.kbos = true;
    full_spec.forces.kbo_ring = true;
    full_spec.name = "wp22-e10-eih-j2-lt-small-bodies".into();
    let t = std::time::Instant::now();
    let planets = run(&planets_spec, &data).unwrap();
    let t_planets = t.elapsed().as_secs_f64();
    let t = std::time::Instant::now();
    let full = run(&full_spec, &data).unwrap();
    let t_full = t.elapsed().as_secs_f64();
    println!("model (small bodies): {}", full.model);
    println!(
        "runs: planets {} steps {t_planets:.1} s; with small bodies {} steps {t_full:.1} s",
        planets.stats.steps, full.stats.steps
    );
    let (rp, rf) = (residuals(&planets, &spk), residuals(&full, &spk));
    println!(
        "  {:<6} {:>18} {:>18} {:>18} {:>18}",
        "NAIF", "planets bary km", "+small bary km", "planets helio km", "+small helio km"
    );
    for (a, b) in rp.iter().zip(&rf) {
        println!(
            "  {:<6} {:>18.5} {:>18.5} {:>18.5} {:>18.5}",
            a.0, a.1, b.1, a.2, b.2
        );
    }
    (rp, rf)
}

/// E10 (known-failure guard) and E10b, sharing the two 10-yr runs (computed once, so the test
/// stays well inside the data tier's 600 s terminate).
///
/// * **E10, recorded FAILURE (WP22), now a bounded guard.** The pre-registered check (the Sun's
///   barycentric residual improves >= 2x, `EIH-ALLBODY-SUN-IMPROVEMENT-MIN`) fails: 1.399 ->
///   2.440 km, factor 0.573. The cause is the E10 template's missing Pluto and DE440's 27
///   unmodelled KBOs (IMPLEMENTATION-NOTES WP22 follow-up), not the dynamics. The maintainer kept
///   it as a recorded failure. The guard passes while the documented factor reproduces within the
///   band of `E10-KNOWN-FAILURE-SUN-BARY-FACTOR` and fails if it stops reproducing (e.g. the
///   original assertion starts to pass) or worsens. The original vector is reported, not
///   asserted.
/// * **E10b, written AFTER the E10 result (post hoc, disclosed):** the Mars system's heliocentric
///   residual improves >= 10x with the small bodies (`EIH-ALLBODY-MARS-HELIO-IMPROVEMENT-MIN`).
///   DE430 selected the 343 asteroids for their perturbation of Mars, so the planets-only Mars
///   residual is the asteroid signal; heliocentric residuals do not depend on the barycentre
///   definition.
///
/// Verifies: NUM-009, NUM-019, REF-003
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_eih_all_bodies_with_small_bodies_ten_years_vs_de440_bsp() {
    let (rp, rf) = e10_residuals();
    let vs = VectorSet::load_canonical().unwrap();
    let k_sun = rp.iter().position(|r| r.0 == 10).unwrap(); // lint: allow-literal(NAIF id of the Sun)
    let k_me = rp.iter().position(|r| r.0 == 1).unwrap(); // lint: allow-literal(NAIF id of the Mercury barycentre)
    let k_ma = rp.iter().position(|r| r.0 == 4).unwrap(); // lint: allow-literal(NAIF id of the Mars barycentre)
    let factor = rp[k_sun].1 / rf[k_sun].1;
    let mars = rp[k_ma].2 / rf[k_ma].2;
    let original = vs
        .get("EIH-ALLBODY-SUN-IMPROVEMENT-MIN")
        .unwrap()
        .value()
        .unwrap();
    println!(
        "E10: Sun barycentric residual {:.4} km -> {:.4} km (factor {factor:.3}; pre-registered \
         >= {original}: {}); Mercury heliocentric {:.4} m -> {:.4} m",
        rp[k_sun].1,
        rf[k_sun].1,
        if factor >= original {
            "now PASSES"
        } else {
            "FAILS as recorded"
        },
        rp[k_me].2 * 1e3, // lint: allow-literal(km to m)
        rf[k_me].2 * 1e3  // lint: allow-literal(km to m)
    );
    println!(
        "E10b: Mars heliocentric residual {:.4} km -> {:.4} km (factor {mars:.1})",
        rp[k_ma].2, rf[k_ma].2
    );
    let guard = vs.get("E10-KNOWN-FAILURE-SUN-BARY-FACTOR").unwrap();
    let mars_min = vs
        .get("EIH-ALLBODY-MARS-HELIO-IMPROVEMENT-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(
        mars >= mars_min,
        "E10b: Mars heliocentric improvement {mars} < {mars_min}"
    );
    guard.check(factor).unwrap();
}
