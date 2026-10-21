// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP24 check B5 (data tier, real data): the same EIH + solar J2 model (Sun + 8 planets +
//! geocentric Moon from header.440 at JDEPOC, 10 Julian yr every 10 d, the WP20 D1 scenario
//! without LT) integrated by IAS15 (default) and by ABM13 PECEC at h = 1/16 d, both against
//! de440.bsp. header.440 and de440.bsp are downloaded fresh (or taken from the cache after
//! re-verifying the pinned SHA-256), provenance printed.
//!
//! Reported: every body's barycentric and heliocentric residual for both integrators. Asserted
//! (`ABM-DE440-RESIDUAL-CONSISTENCY`): Mercury's maximum barycentric residuals differ by at most
//! 1 m.
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

/// B5: IAS15 and ABM13 PECEC against de440.bsp over 1969-06-28 → +10 yr.
///
/// Verifies: NUM-002, REF-001
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_abm_and_ias15_ten_years_from_header_ics_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let data = DataSources::none().with_header(&h, &hprov);
    let mut ias_spec = scenario("sun-planets-moon-newton-j2-10yr.toml");
    ias_spec.forces.gr_eih = true;
    ias_spec.name = "wp24-b5-eih-j2-ias15".into();
    let mut abm_spec = ias_spec.clone();
    abm_spec.integrator = scenario("abm-sun-planets-moon-eih-40yr.toml").integrator;
    abm_spec.name = "wp24-b5-eih-j2-abm".into();
    let ias = run(&ias_spec, &data).unwrap();
    let abm = run(&abm_spec, &data).unwrap();
    println!("model (ABM): {}", abm.model);
    println!(
        "steps: IAS15 {} ({} evaluations), ABM {} ({} evaluations); {} samples every {} d",
        ias.stats.steps,
        ias.stats.force_evaluations,
        abm.stats.steps,
        abm.stats.force_evaluations,
        abm.states.len() - 1,
        abm_spec.output.cadence_days.unwrap()
    );
    assert_eq!(ias.ic_hash, abm.ic_hash);
    let (ri, ra) = (residuals(&ias, &spk), residuals(&abm, &spk));
    println!(
        "  {:<6} {:>18} {:>18} {:>18} {:>18}",
        "NAIF", "IAS15 bary km", "ABM bary km", "IAS15 helio km", "ABM helio km"
    );
    for (a, b) in ri.iter().zip(&ra) {
        println!(
            "  {:<6} {:>18.6} {:>18.6} {:>18.6} {:>18.6}",
            a.0, a.1, b.1, a.2, b.2
        );
    }
    let k = ri.iter().position(|r| r.0 == MERCURY).unwrap();
    let delta = (ra[k].1 - ri[k].1).abs();
    println!(
        "B5: Mercury max barycentric residual vs de440.bsp: IAS15 {:.6} km, ABM {:.6} km; \
         |difference| {delta:.3e} km",
        ri[k].1, ra[k].1
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("ABM-DE440-RESIDUAL-CONSISTENCY")
        .unwrap()
        .check(delta)
        .unwrap();
}
