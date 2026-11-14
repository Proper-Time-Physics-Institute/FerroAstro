// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP28 check D1 (data tier, real data): the speed table's de440.bsp column. header.440 and
//! de440.bsp are downloaded fresh (or taken from the cache after re-verifying the pinned SHA-256),
//! provenance printed. The NUM-024 mixed scenario (`ephemeris-mixed-epm16-40yr`) shortened to the
//! WP25 D1 run (5 yr, 10-d samples) runs once; the library's `accuracy::body_vs_spk` on its state
//! series must equal an independent computation from the `RunOutput` records (the WP25 D1 method)
//! within `BENCH-DE440-COLUMN-CONSISTENCY-KM`. The value itself is reported against WP25 D1.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_bench::Series;
use ferroastro_bench::accuracy::body_vs_spk;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Barycentric;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const SUN: i32 = 10;
const MERCURY: i32 = 1;
/// The WP25 D1 span (Julian years) and cadence (days).
const SPAN_YEARS: f64 = 5.0;
const CADENCE_DAYS: f64 = 10.0;

/// D1: the de440.bsp column's library function against the direct computation.
///
/// Verifies: NUM-024, REF-001
#[test]
#[ignore = "data: downloads JPL header.440 and de440.bsp (network); run with --profile data"]
fn data_num024_de440_column_matches_the_direct_residual() {
    let hf = ferroastro_testdata::fetch_product("de440-header").unwrap();
    let hprov = hf.provenance();
    println!("de440-header: {hprov}");
    let h = Header::parse(&std::fs::read_to_string(&hf.path).unwrap()).unwrap();
    let bf = ferroastro_testdata::fetch_product("de440-bsp").unwrap();
    println!("de440-bsp: {}", bf.provenance());
    let bytes = std::fs::read(&bf.path).unwrap();
    let spk = Spk::parse(&bytes).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = ferroastro_system::SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    println!("REF-003 states: {}", file.provenance());
    let data = DataSources::none()
        .with_header(&h, &hprov)
        .with_small_bodies(&file);
    let mut spec = ScenarioSpec::parse(
        &std::fs::read_to_string(root.join("ci/scenarios/ephemeris-mixed-epm16-40yr.toml"))
            .unwrap(),
    )
    .unwrap();
    spec.span.years = SPAN_YEARS;
    spec.output.cadence_days = Some(CADENCE_DAYS);
    let out = run(&spec, &data).unwrap();
    println!(
        "run: {} steps, IC hash {}, {} samples",
        out.stats.steps,
        out.ic_hash,
        out.states.len()
    );

    // Library path (the speed table's column), over the whole span.
    let series = Series::from_run("d1", &out);
    let t_end = SPAN_YEARS * DAYS_PER_JULIAN_YEAR;
    let (at_end, lib_max, t_max) = body_vs_spk(&series, &spk, MERCURY, t_end).unwrap();

    // Direct path (WP25 D1): JD parts + seconds, heliocentric, from the RunOutput records.
    let jd = out.epoch_jd_tdb;
    let ids = &out.states[0].bodies;
    let at = |id: i32| ids.iter().position(|&i| i == id).unwrap();
    let pos = |id: i32, e: Epoch<Tdb>| {
        spk.state::<Barycentric, f64>(NaifId(id), e)
            .unwrap()
            .position()
            .components()
            .map(ferroastro_core::units::Length::km)
    };
    let mut direct = 0.0_f64;
    for s in out.states.iter().chain(out.final_state.as_ref()) {
        let t = s.t[0] + s.t[1];
        let e = Epoch::<Tdb>::from_jd_parts(jd[0], jd[1] + t / SECONDS_PER_DAY).unwrap();
        let got = |id: i32| s.positions[at(id)].map(|p| p[0] + p[1]);
        let (g, gs) = (got(MERCURY), got(SUN));
        let (w, ws) = (pos(MERCURY, e), pos(SUN, e));
        let d = [0, 1, 2].map(|k| (g[k] - gs[k]) - (w[k] - ws[k]));
        direct = direct.max(d[0].hypot(d[1]).hypot(d[2]));
    }
    let delta = (lib_max - direct).abs();
    println!(
        "D1: Mercury heliocentric residual vs de440.bsp over {SPAN_YEARS} yr: library max {lib_max:.9} km \
         (at t = {t_max} d; {at_end:.6} km at the end), direct max {direct:.9} km, |diff| {delta:.3e} km. \
         WP25 D1 printed 0.784621 km (mixed; reported, not asserted)"
    );
    VectorSet::load_canonical()
        .unwrap()
        .get("BENCH-DE440-COLUMN-CONSISTENCY-KM")
        .unwrap()
        .check(delta)
        .unwrap();
}
