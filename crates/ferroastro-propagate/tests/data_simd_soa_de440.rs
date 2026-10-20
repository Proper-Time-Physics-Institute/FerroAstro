// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP26 check D1 (data tier, real data): the WP24 B5 configuration (EIH + solar J2, Sun + 8
//! planets + geocentric Moon from header.440 at JDEPOC, 10 Julian yr every 10 d, IAS15 f64)
//! with the pairwise kernels and with the SoA kernels (`integrator.force_kernel = "soa"`, the
//! preferred lane backend), both against de440.bsp. header.440 and de440.bsp are downloaded fresh
//! (or taken from the cache after re-verifying the pinned SHA-256), provenance printed.
//!
//! Reported: every body's barycentric and heliocentric residual for both kernels, and the
//! kernels' mutual distance. Asserted (`SIMD-SOA-DE440-RESIDUAL-CONSISTENCY`): Mercury's maximum
//! barycentric residuals differ by at most 1 m.
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
use ferroastro_scenario::{ForceKernel, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

/// NAIF ids of the Sun and of the Mercury barycentre (storage slots of the N-body run).
const SUN: i32 = 10;
const MERCURY: i32 = 1;

fn scenario(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
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
    let sun_k = ids.iter().position(|&i| i == SUN).unwrap();
    for s in &out.states[1..] {
        let t = s.t[0] + s.t[1];
        let e = Epoch::<Tdb>::from_jd_parts(jd[0], jd[1] + t / SECONDS_PER_DAY).unwrap();
        let got = |k: usize| s.positions[k].map(|p| p[0] + p[1]);
        let want = |id: i32| pos(&spk.state::<Barycentric, f64>(NaifId(id), e).unwrap());
        let (sun, sun_ref) = (got(sun_k), want(SUN));
        for (k, row) in rows.iter_mut().enumerate() {
            let (g, w) = (got(k), want(row.0));
            row.1 = row.1.max(norm(sub(g, w)));
            row.2 = row.2.max(norm(sub(sub(g, sun), sub(w, sun_ref))));
        }
    }
    rows
}

/// D1: pairwise and SoA kernels against de440.bsp over 1969-06-28 -> +10 yr.
///
/// Verifies: NUM-021, REF-001
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_soa_and_pairwise_kernels_ten_years_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let data = DataSources::none().with_header(&h, &hprov);
    let mut pw_spec = scenario("sun-planets-moon-newton-j2-10yr.toml");
    pw_spec.forces.gr_eih = true;
    pw_spec.name = "wp26-d1-eih-j2-pairwise".into();
    let mut soa_spec = pw_spec.clone();
    soa_spec.integrator.force_kernel = Some(ForceKernel::Soa);
    soa_spec.name = "wp26-d1-eih-j2-soa".into();
    let pw = run(&pw_spec, &data).unwrap();
    let soa = run(&soa_spec, &data).unwrap();
    println!(
        "lane backend of the soa run: {} (feature simd {})",
        ferroastro_forces::SoaBackend::preferred().label(),
        if cfg!(feature = "simd") { "on" } else { "off" }
    );
    println!(
        "steps: pairwise {} ({} evaluations), soa {} ({} evaluations); {} samples every {} d",
        pw.stats.steps,
        pw.stats.force_evaluations,
        soa.stats.steps,
        soa.stats.force_evaluations,
        soa.states.len() - 1,
        soa_spec.output.cadence_days.unwrap()
    );
    assert_eq!(pw.ic_hash, soa.ic_hash);
    let (rp, rs) = (residuals(&pw, &spk), residuals(&soa, &spk));
    println!(
        "  {:<6} {:>18} {:>18} {:>18} {:>18}",
        "NAIF", "pairwise bary km", "soa bary km", "pairwise helio km", "soa helio km"
    );
    for (a, b) in rp.iter().zip(&rs) {
        println!(
            "  {:<6} {:>18.6} {:>18.6} {:>18.6} {:>18.6}",
            a.0, a.1, b.1, a.2, b.2
        );
    }
    let mut mutual = 0.0_f64;
    for (a, b) in pw.states.iter().zip(&soa.states) {
        for (p, q) in a.positions.iter().zip(&b.positions) {
            let d: [f64; 3] = core::array::from_fn(|c| (p[c][0] + p[c][1]) - (q[c][0] + q[c][1]));
            mutual = mutual.max(norm(d));
        }
    }
    let k = rp.iter().position(|r| r.0 == MERCURY).unwrap();
    let delta = (rs[k].1 - rp[k].1).abs();
    println!(
        "D1: Mercury max barycentric residual vs de440.bsp: pairwise {:.6} km, soa {:.6} km; \
         |difference| {delta:.3e} km; max |r_soa - r_pairwise| over all bodies and samples \
         {mutual:.3e} km",
        rp[k].1, rs[k].1
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("SIMD-SOA-DE440-RESIDUAL-CONSISTENCY")
        .unwrap()
        .check(delta)
        .unwrap();
}
