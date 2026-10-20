// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP25 check D1 (data tier, real data): the Subbotin et al. 2023 reference configuration
//! (`ephemeris-mixed-epm16-40yr`, 5-yr span, 10-d samples) in the NUM-007 mixed mode and in full
//! double-double (f64 plain reported), against de440.bsp. header.440 and de440.bsp are downloaded
//! fresh (or taken from the cache after re-verifying the pinned SHA-256), provenance printed.
//!
//! Residuals are heliocentric: the re-barycentred ICs sit ~123 km from DE440's own SSB, because
//! the body subset has no KBOs or ring. Reported: every major body's maximum heliocentric
//! residual for each run. Asserted (`EPH-MIXED-DE440-HELIO-CONSISTENCY-KM`): Mercury's maximum
//! residuals of the mixed and the DD runs differ by at most 1 cm.
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
use ferroastro_scenario::{Precision, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const SUN: i32 = 10;
const MERCURY: i32 = 1;
/// D1 span (Julian years) and sample cadence (days), WP25 plan addendum.
const SPAN_YEARS: f64 = 5.0;
const CADENCE_DAYS: f64 = 10.0;
/// The major bodies reported (barycentre slots; the Earth and the Moon are physical).
const MAJORS: [i32; 10] = [1, 2, 399, 301, 4, 5, 6, 7, 8, 9];

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

fn helio_residuals(out: &RunOutput, spk: &Spk<'_>) -> Vec<(i32, f64)> {
    let jd = out.epoch_jd_tdb;
    let ids = &out.states[0].bodies;
    let at = |id: i32| ids.iter().position(|&i| i == id).unwrap();
    let mut rows: Vec<(i32, f64)> = MAJORS.iter().map(|&i| (i, 0.0)).collect();
    for s in &out.states[1..] {
        let t = s.t[0] + s.t[1];
        let e = Epoch::<Tdb>::from_jd_parts(jd[0], jd[1] + t / SECONDS_PER_DAY).unwrap();
        let got = |id: i32| s.positions[at(id)].map(|p| p[0] + p[1]);
        let want = |id: i32| pos(&spk.state::<Barycentric, f64>(NaifId(id), e).unwrap());
        let (sun, sun_ref) = (got(SUN), want(SUN));
        for row in &mut rows {
            let (g, w) = (got(row.0), want(row.0));
            let d = [0, 1, 2].map(|k| (g[k] - sun[k]) - (w[k] - sun_ref[k]));
            row.1 = row.1.max(d[0].hypot(d[1]).hypot(d[2]));
        }
    }
    rows
}

/// D1: mixed and full double-double against de440.bsp, heliocentric.
///
/// Verifies: NUM-007, REF-001
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_ephemeris_mode_mixed_and_dd_vs_de440_bsp() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let sb = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file =
        ferroastro_system::SmallBodyStates::parse(&std::fs::read_to_string(sb).unwrap()).unwrap();
    println!("REF-003 states: {}", file.provenance());
    let data = DataSources::none()
        .with_header(&h, &hprov)
        .with_small_bodies(&file);
    let mut base = scenario("ephemeris-mixed-epm16-40yr.toml");
    base.span.years = SPAN_YEARS;
    base.output.cadence_days = Some(CADENCE_DAYS);
    let with = |p: Precision, plain: bool| {
        let mut s = base.clone();
        s.integrator.precision = p;
        if plain {
            s.integrator.compensated = Some(false);
        }
        s
    };
    let (dd, mixed, plain) = std::thread::scope(|sc| {
        let a = sc.spawn(|| run(&with(Precision::Dd, false), &data).unwrap());
        let b = sc.spawn(|| run(&with(Precision::Mixed, false), &data).unwrap());
        let c = sc.spawn(|| run(&with(Precision::F64, true), &data).unwrap());
        (a.join().unwrap(), b.join().unwrap(), c.join().unwrap())
    });
    assert_eq!(dd.ic_hash, mixed.ic_hash);
    println!("IC source: {}", mixed.ic_source);
    let (rd, rm, rf) = (
        helio_residuals(&dd, &spk),
        helio_residuals(&mixed, &spk),
        helio_residuals(&plain, &spk),
    );
    println!(
        "D1: max heliocentric residual vs de440.bsp over {SPAN_YEARS} yr, {} samples (km):",
        dd.states.len() - 1
    );
    println!(
        "  {:<6} {:>16} {:>16} {:>16}",
        "NAIF", "double-double", "mixed", "f64 plain"
    );
    for ((a, b), c) in rd.iter().zip(&rm).zip(&rf) {
        println!("  {:<6} {:>16.6} {:>16.6} {:>16.6}", a.0, a.1, b.1, c.1);
    }
    let k = rd.iter().position(|r| r.0 == MERCURY).unwrap();
    let delta = (rm[k].1 - rd[k].1).abs();
    println!(
        "D1: Mercury |max res mixed - max res DD| = {delta:.3e} km (f64 plain: {:.3e} km)",
        (rf[k].1 - rd[k].1).abs()
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("EPH-MIXED-DE440-HELIO-CONSISTENCY-KM")
        .unwrap()
        .check(delta)
        .unwrap();
}
