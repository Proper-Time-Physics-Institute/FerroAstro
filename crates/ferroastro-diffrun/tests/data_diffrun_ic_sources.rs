// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Real-data tier (downloads into `$FA_DATA_DIR` if absent, SHA-256 pinned, provenance printed):
//! the harness on two JPL products. The same Sun + Mercury (+ Venus) plan built from header.440
//! (JDEPOC states) and from de440.bsp (evaluated at JDEPOC) gives different shared IC hashes, a
//! plan that mixes the two sources is refused (`FA-E1401`: the records differ at the 1e-7 km
//! level, WP11 check 1), and the P-add Venus contribution from the two sources agrees within
//! `DIFFRUN-IC-SOURCE-AGREEMENT`.

#![expect(
    clippy::unwrap_used,
    reason = "test helper: a failed unwrap is a test failure"
)]

mod common;

use ferroastro_constants::header440::Header;
use ferroastro_core::ErrorCode;
use ferroastro_diffrun::{
    BodyToggle, PassageSource, Plan, Protocol, RunOptions, check_identical_ics, run_plan,
};
use ferroastro_ephem::Spk;
use ferroastro_precession::park2017::FitModel;
use ferroastro_propagate::{DataSources, initial_conditions};
use ferroastro_scenario::{BodyName, IcSource, ScenarioSpec};
use ferroastro_vectors::VectorSet;

const LEG_YEARS: f64 = 20.0; // lint: allow-literal(run length, data-tier cost)
const CADENCE_DAYS: f64 = 1.0; // lint: allow-literal(output cadence of the estimator samples)
const JACKKNIFE_BLOCKS: usize = 10; // lint: allow-literal(jack-knife blocks for ~330 passages)

fn plan(template: &ScenarioSpec, name: &str) -> Plan {
    use BodyName::{Mercury, Sun, Venus};
    Plan::bodies(
        name,
        template,
        &[Sun, Mercury],
        &[],
        &[BodyToggle::new("Venus", &[Venus])],
        &[],
        &[-LEG_YEARS, LEG_YEARS],
    )
    .unwrap()
}

/// Verifies: NUM-013, CI-009, REF-002
#[test]
#[ignore = "network: data tier (cargo nextest run --profile data --run-ignored only)"]
fn data_diffrun_header_and_spk_ics_are_distinct_sources_with_equal_contributions() {
    let vs = VectorSet::load_canonical().unwrap();
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    let bprov = bf.provenance();
    println!("de440-bsp: {bprov}");
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let data = DataSources::none()
        .with_header(&h, &hprov)
        .with_spk(&spk, &bprov);
    let jdepoc = h.value("JDEPOC").unwrap();

    let mut th = common::scenario("sun-mercury-newton-header440.toml");
    th.output.cadence_days = Some(CADENCE_DAYS);
    let mut ts = th.clone();
    ts.initial_conditions.source = IcSource::De440Bsp;
    ts.initial_conditions.epoch_jd_tdb = Some(jdepoc);
    let (ph, ps) = (plan(&th, "wp14-data-header"), plan(&ts, "wp14-data-spk"));

    // Mixing the two sources in one plan is refused.
    let mut initial = Vec::new();
    for s in [&ph.configs[0].legs[0], &ps.configs[1].legs[0]] {
        initial.push((s.name.clone(), initial_conditions(s, &data).unwrap()));
    }
    let e = check_identical_ics(&ph, &initial).unwrap_err();
    println!("mixed sources refused: {e}");
    assert_eq!(e.code(), ErrorCode::DiffrunIcMismatch);

    let est = common::park17_estimator(
        FitModel {
            quadratic: true,
            frequencies: vec![],
        },
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        None,
    );
    let opts = RunOptions {
        threads: 4, // lint: allow-literal(worker threads, run control)
        ..RunOptions::default()
    };
    let rh = run_plan(&ph, &data, &est, &opts).unwrap();
    let rs = run_plan(&ps, &data, &est, &opts).unwrap();
    assert_ne!(rh.shared_ic_hash, rs.shared_ic_hash);
    let venus = |r: &ferroastro_diffrun::PlanResult| r.rows_of(Protocol::Add).next().unwrap().value;
    let (vh, vsp) = (venus(&rh), venus(&rs));
    println!(
        "P-add Venus over JDEPOC +/- {LEG_YEARS} yr (quadratic fit): header.440 {vh:.12}, de440.bsp {vsp:.12}, difference {:+.3e} arcsec/cy; shared IC hashes {} / {}; IC sources: {} | {}",
        vh - vsp,
        rh.shared_ic_hash,
        rs.shared_ic_hash,
        rh.configs[0].ic_source,
        rs.configs[0].ic_source
    );
    vs.get("DIFFRUN-IC-SOURCE-AGREEMENT")
        .unwrap()
        .check(vh - vsp)
        .unwrap();
}
