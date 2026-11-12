// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP20 check D1 (data tier, real data): EIH + solar J2 with and without solar Lense–Thirring
//! against de440.bsp, whose model contains DE440's LT (Park et al. 2021, §3.4). Sun + 8 planets +
//! geocentric Moon from header.440 at JDEPOC, 10 Julian yr every 10 d, IAS15 default, through the
//! scenario API (DE440 J2, WGCCRE 2015 pole, DE440 spin). header.440 and de440.bsp are downloaded
//! fresh (or taken from the cache after re-verifying the pinned SHA-256), provenance printed.
//!
//! Reported: every body's barycentric and heliocentric residual for both runs. Asserted
//! (PROVISIONAL sanity, `LT-DE440-MERCURY-SHIFT-LOG10-KM`): Mercury's LT-on minus LT-off shift
//! lies in [1e-3, 1] km.
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
    s.name = "wp20-d1-eih-j2".into();
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

/// D1: EIH + J2 with and without LT against de440.bsp over 1969-06-28 → +10 yr.
///
/// Verifies: NUM-019, PHY-011
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_lense_thirring_ten_years_from_header_ics_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let data = DataSources::none().with_header(&h, &hprov);
    let off_spec = spec();
    let mut on_spec = spec();
    on_spec.forces.lense_thirring = true;
    on_spec.name = "wp20-d1-eih-j2-lt".into();
    let off = run(&off_spec, &data).unwrap();
    let on = run(&on_spec, &data).unwrap();
    println!("model (LT on): {}", on.model);
    assert_eq!(off.ic_hash, on.ic_hash);
    let (ro, rn) = (residuals(&off, &spk), residuals(&on, &spk));
    println!(
        "  {:<6} {:>18} {:>18} {:>18} {:>18}",
        "NAIF", "no LT bary km", "LT bary km", "no LT helio km", "LT helio km"
    );
    for (a, b) in ro.iter().zip(&rn) {
        println!(
            "  {:<6} {:>18.5} {:>18.5} {:>18.5} {:>18.5}",
            a.0, a.1, b.1, a.2, b.2
        );
    }
    let k = on.states[0].bodies.iter().position(|&i| i == 1).unwrap(); // lint: allow-literal(NAIF id of the Mercury barycentre)
    let mut shift = 0.0_f64;
    for (a, b) in off.states.iter().zip(&on.states) {
        let pa = a.positions[k].map(|p| p[0] + p[1]);
        let pb = b.positions[k].map(|p| p[0] + p[1]);
        shift = shift.max(norm(sub(pa, pb)));
    }
    println!(
        "D1: Mercury max |r(LT) - r(no LT)| = {:.4} m over {} samples; Mercury heliocentric residual vs de440.bsp: {:.4} m without LT, {:.4} m with LT",
        shift * 1e3, // lint: allow-literal(km to m)
        on.states.len() - 1,
        ro[k].2 * 1e3, // lint: allow-literal(km to m)
        rn[k].2 * 1e3  // lint: allow-literal(km to m)
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("LT-DE440-MERCURY-SHIFT-LOG10-KM")
        .unwrap()
        .check(shift.log10())
        .unwrap();
}
