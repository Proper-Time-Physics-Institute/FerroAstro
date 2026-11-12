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
//! Reported: every major body's barycentric and heliocentric residual for both runs. Asserted
//! (PROVISIONAL sanity, `EIH-ALLBODY-SUN-IMPROVEMENT-MIN`): the Sun's maximum barycentric
//! residual improves by >= 2x with the small bodies (WP19/WP20: the Sun's 1.4 km residual comes
//! from the omitted asteroids, KBOs and ring moving the SSB).
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

/// E10: all-body EIH + J2 + LT with and without the small bodies against de440.bsp over
/// 1969-06-28 → +10 yr.
///
/// Recorded FAILURE (WP22): the pre-registered Sun barycentric metric measures the
/// barycentre definition (27 unmodelled KBOs), not the dynamics; kept unchanged as the record.
///
/// Verifies: PHY-014, NUM-009, NUM-019
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_eih_all_bodies_with_small_bodies_ten_years_vs_de440_bsp() {
    let (rp, rf) = e10_residuals();
    let k_sun = rp.iter().position(|r| r.0 == 10).unwrap(); // lint: allow-literal(NAIF id of the Sun)
    let k_me = rp.iter().position(|r| r.0 == 1).unwrap(); // lint: allow-literal(NAIF id of the Mercury barycentre)
    let factor = rp[k_sun].1 / rf[k_sun].1;
    println!(
        "E10: Sun barycentric residual {:.4} km -> {:.4} km (factor {factor:.3}); Mercury \
         heliocentric {:.4} m -> {:.4} m",
        rp[k_sun].1,
        rf[k_sun].1,
        rp[k_me].2 * 1e3, // lint: allow-literal(km to m)
        rf[k_me].2 * 1e3  // lint: allow-literal(km to m)
    );
    let min = VectorSet::load_canonical()
        .unwrap()
        .get("EIH-ALLBODY-SUN-IMPROVEMENT-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(factor >= min, "Sun improvement factor {factor} < {min}");
}

/// E10b, written AFTER the E10 result (post hoc, disclosed in IMPLEMENTATION-NOTES WP22 and in the
/// vector's rationale): the heliocentric residual of the Mars system against de440.bsp improves by
/// at least 10x with the small bodies. Mars is the planet the 343 asteroids were chosen for (DE430
/// selected the asteroids with the largest perturbation on the orbit of Mars), so its planets-only
/// residual is the asteroid signal; with DE440's own masses and ICs from the same ephemeris family
/// the remaining residual must be a small fraction of it. Heliocentric residuals do not depend on
/// the barycentre definition that E10's barycentric metric measures.
///
/// Verifies: PHY-014, NUM-009, REF-003
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_eih_all_bodies_mars_heliocentric_residual_improves_with_the_asteroids() {
    let (rp, rf) = e10_residuals();
    let k = rp.iter().position(|r| r.0 == 4).unwrap(); // lint: allow-literal(NAIF id of the Mars barycentre)
    let factor = rp[k].2 / rf[k].2;
    println!(
        "E10b: Mars heliocentric residual {:.4} km -> {:.4} km (factor {factor:.1})",
        rp[k].2, rf[k].2
    );
    let min = VectorSet::load_canonical()
        .unwrap()
        .get("EIH-ALLBODY-MARS-HELIO-IMPROVEMENT-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(
        factor >= min,
        "Mars heliocentric improvement {factor} < {min}"
    );
}
