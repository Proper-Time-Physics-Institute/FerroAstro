// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP29 check D1 (data tier, real data): Sun + 8 planets (EMB) from header.440, Newtonian +
//! solar J2 + the GR effective potential (the full model the Wisdom–Holman kick supports),
//! 10 Julian yr, 10-d samples, integrated by IAS15, `WHFast` (h = 1 d, corrector 11) and IRK16
//! (h = 2 d), each against de440.bsp. header.440 and de440.bsp are downloaded fresh (or taken
//! from the cache after re-verifying the pinned SHA-256), provenance printed.
//!
//! Asserted: Mercury's maximum barycentric residual of `WHFast` and of IRK16 equals IAS15's within
//! `WH-DE440-RESIDUAL-CONSISTENCY` and `IRK16-DE440-RESIDUAL-CONSISTENCY`. Reported: every body
//! for every integrator, and an IAS15 run with `gr_eih` instead of the GR potential (context: the
//! potential's mean-motion offset).
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
use ferroastro_scenario::{IntegratorName, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const MERCURY: i32 = 1;
/// D1 span (Julian years) and sample interval (days), WP29 plan addendum 3.
const YEARS: f64 = 10.0; // lint: allow-literal(test design: span, Julian years)
const CADENCE_DAYS: f64 = 10.0; // lint: allow-literal(test design: sample interval, days)
const WH_STEP_DAYS: f64 = 1.0; // lint: allow-literal(test design: WH step, days)

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

/// Per body: (NAIF id, max barycentric residual km).
fn residuals(out: &RunOutput, spk: &Spk<'_>) -> Vec<(i32, f64)> {
    let jd = out.epoch_jd_tdb;
    let ids = &out.states[0].bodies;
    let mut rows: Vec<(i32, f64)> = ids.iter().map(|&i| (i, 0.0)).collect();
    for s in &out.states[1..] {
        let t = s.t[0] + s.t[1];
        let e = Epoch::<Tdb>::from_jd_parts(jd[0], jd[1] + t / SECONDS_PER_DAY).unwrap();
        for (k, row) in rows.iter_mut().enumerate() {
            let g = s.positions[k].map(|p| p[0] + p[1]);
            let w = pos(&spk.state::<Barycentric, f64>(NaifId(row.0), e).unwrap());
            row.1 = row.1.max(norm([g[0] - w[0], g[1] - w[1], g[2] - w[2]]));
        }
    }
    rows
}

fn mercury(rows: &[(i32, f64)]) -> f64 {
    rows.iter().find(|r| r.0 == MERCURY).unwrap().1
}

/// Verifies: NUM-003, NUM-004, REF-001
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_wh_irk16_and_ias15_ten_years_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let data = DataSources::none().with_header(&h, &hprov);

    let mut ias = scenario("stage-a-sun-8planets-newton-100yr.toml");
    ias.name = "wp29-d1-ias15".into();
    ias.span.years = YEARS;
    ias.output.cadence_days = Some(CADENCE_DAYS);
    ias.forces.sun_j2 = true;
    ias.forces.gr_potential = true;
    let mut wh = ias.clone();
    wh.name = "wp29-d1-whfast".into();
    wh.integrator = scenario("wh-sun-8planets-newton-100yr.toml").integrator;
    wh.integrator.step_days = Some(WH_STEP_DAYS);
    let mut irk = ias.clone();
    irk.name = "wp29-d1-irk16".into();
    irk.integrator = scenario("irk16-sun-8planets-newton-100yr.toml").integrator;
    let mut eih = ias.clone();
    eih.name = "wp29-d1-ias15-eih".into();
    eih.forces.gr_potential = false;
    eih.forces.gr_eih = true;
    assert_eq!(wh.integrator.name, IntegratorName::Whfast);
    assert_eq!(irk.integrator.name, IntegratorName::Irk16);

    let runs: Vec<(&str, RunOutput)> = [
        ("IAS15", &ias),
        ("WHFast", &wh),
        ("IRK16", &irk),
        ("IAS15 gr_eih", &eih),
    ]
    .into_iter()
    .map(|(l, s)| (l, run(s, &data).unwrap()))
    .collect();
    for (l, o) in &runs {
        println!(
            "{l}: IC hash {}, {} steps; model: {}",
            o.ic_hash, o.stats.steps, o.model
        );
    }
    let res: Vec<Vec<(i32, f64)>> = runs.iter().map(|(_, o)| residuals(o, &spk)).collect();
    println!(
        "  {:<6} {:>16} {:>16} {:>16} {:>16}   (max barycentric |r - r_de440|, km)",
        "NAIF", "IAS15", "WHFast", "IRK16", "IAS15 gr_eih"
    );
    for (k, row) in res[0].iter().enumerate() {
        println!(
            "  {:<6} {:>16.6} {:>16.6} {:>16.6} {:>16.6}",
            row.0, row.1, res[1][k].1, res[2][k].1, res[3][k].1
        );
    }
    let (m_ias, m_wh, m_irk) = (mercury(&res[0]), mercury(&res[1]), mercury(&res[2]));
    let (d_wh, d_irk) = ((m_wh - m_ias).abs(), (m_irk - m_ias).abs());
    let vs = VectorSet::load_canonical().unwrap();
    let b = |id: &str| vs.get(id).unwrap().value().unwrap();
    println!(
        "D1: Mercury max residual vs de440.bsp: IAS15 {m_ias:.6} km, WHFast {m_wh:.6} km \
         (|diff| {d_wh:.3e}, bound {}), IRK16 {m_irk:.6} km (|diff| {d_irk:.3e}, bound {}); \
         IAS15 with gr_eih {:.6} km",
        b("WH-DE440-RESIDUAL-CONSISTENCY"),
        b("IRK16-DE440-RESIDUAL-CONSISTENCY"),
        mercury(&res[3])
    );
    assert!(d_wh <= b("WH-DE440-RESIDUAL-CONSISTENCY"));
    assert!(d_irk <= b("IRK16-DE440-RESIDUAL-CONSISTENCY"));
}
