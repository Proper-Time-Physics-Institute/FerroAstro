// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP23 check E9 (nightly, long, run locally): rung EIH-FULL (PHY-014). DE440-equivalent physics
//! reproduces Park17's total secular perihelion rate with the DE440 constants, 575.3093 ± 0.005
//! ″/cy (PHY-014 acceptance; vector `PHY014-EIH-FULL-RATE`).
//!
//! Fixed before any run (IMPLEMENTATION-NOTES WP22 plan, check E9): the scenario
//! `ci/scenarios/phy014-eih-full-header440.toml` (the PHY-012 full system with Earth and Moon
//! resolved and Pluto, `gr_eih` for every moving body over the tiered graph with β = γ = 1 from
//! DE440, solar J2 = DE440 2.19614e-7 with the WGCCRE 2015 pole, solar Lense–Thirring with the
//! DE440 spin and the reaction on the Sun, 343 numbered asteroids and the 3 identified KBOs as
//! Minor bodies, the 36-point KBO ring as fixed masses), header.440 ICs at JDEPOC unchanged, IAS15
//! legs back and forward to J2000 ± 1000 Julian yr (+30 d), the Park2017 estimator (daily
//! samples, Park17 mean-orbit plane, quadratic + 14 Table-2 terms). One configuration, no
//! differential: the total rate is asserted. Prediction: 575.30916. Legs checkpoint every 50 yr
//! into `target/diffrun/store-wp23` and resume from there after an interruption (CI-010).

mod common;

use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD};
use ferroastro_diffrun::{Config, PassageSource, Plan, RunOptions, run_plan};
use ferroastro_propagate::DataSources;
use ferroastro_scenario::BodyName;
use ferroastro_system::SmallBodyStates;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const HALF_SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(Park17 fit span 1000-3000 CE, J2000 +/- 1000 Julian yr as WP5)
const MARGIN_DAYS: f64 = 30.0; // lint: allow-literal(leg margin beyond the fit window, run control)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as the WP5 Park17 data test)
const SEGMENT_YEARS: f64 = 50.0; // lint: allow-literal(checkpoint segment length, run control)
/// WP22 plan E9 prediction (reported, not asserted).
const PREDICTION: f64 = 575.30916; // lint: allow-literal(WP22 plan E9 prediction, printed for information only)

/// Verifies: PHY-014, PHY-015, NUM-009, NUM-019, NUM-020, CI-009, CI-010
#[test]
#[ignore = "nightly (long): 2 EIH IAS15 legs of ~1000 yr with 393 moving bodies; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_phy014_eih_full_rate_with_de440_constants() {
    use BodyName::{Mercury, Sun};
    let t0 = std::time::Instant::now();
    let vs = VectorSet::load_canonical().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (h, prov) = common::header();
    let small = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    println!("{}", small.provenance());
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&small);
    let mut template = common::scenario("phy014-eih-full-header440.toml");
    template.output.sample_bodies = Some(vec![Sun, Mercury]);
    let jdepoc = h.value("JDEPOC").unwrap();
    let lo = HALF_SPAN_YEARS.mul_add(-DAYS_PER_JULIAN_YEAR, J2000_JD);
    let hi = HALF_SPAN_YEARS.mul_add(DAYS_PER_JULIAN_YEAR, J2000_JD);
    let back = -(jdepoc - (lo - MARGIN_DAYS)) / DAYS_PER_JULIAN_YEAR;
    let fwd = (hi + MARGIN_DAYS - jdepoc) / DAYS_PER_JULIAN_YEAR;
    println!("JDEPOC {jdepoc}; fit window JD {lo} .. {hi} TDB; legs {back:.6} yr and {fwd:.6} yr");
    let legs = [back, fwd]
        .iter()
        .enumerate()
        .map(|(i, &y)| {
            let mut s = template.clone();
            s.name = format!("wp23-phy014/eih-full/leg{i}");
            s.span.years = y;
            s
        })
        .collect();
    let plan = Plan {
        name: "wp23-phy014".into(),
        base: vec![Sun.naif(), Mercury.naif()],
        configs: vec![Config {
            label: "eih-full".into(),
            legs,
        }],
        combinations: Vec::new(),
    };
    plan.validate().unwrap();
    let est = common::park17_estimator(
        common::table2_model(&vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        Some((lo, hi)),
    );
    let out_dir = root.join("target/diffrun");
    let store = out_dir.join("store-wp23");
    std::fs::create_dir_all(&store).unwrap();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS,
        store: Some(store.clone()),
        threads,
    };
    println!(
        "{} legs, {threads} threads, store {}",
        plan.leg_count(),
        store.display()
    );
    let r = run_plan(&plan, &data, &est, &opts).unwrap();
    let wall = t0.elapsed().as_secs_f64();
    let stamp = format!("{}", std::process::id());
    let md = r.markdown(&|_, _| None);
    println!("{md}");
    std::fs::write(out_dir.join(format!("phy014-{stamp}.md")), &md).unwrap();
    std::fs::write(
        out_dir.join(format!("phy014-{stamp}.json")),
        r.to_json().unwrap(),
    )
    .unwrap();
    let c = r.configs.first().unwrap();
    println!(
        "model: {}\nIC source: {}\nIC hash: {}\nestimator {}; {} passages; {} IAS15 steps; wall {wall:.1} s",
        c.model, c.ic_source, c.ic_hashes[0], r.estimator, c.estimate.passages, c.steps
    );
    let v = vs.get("PHY014-EIH-FULL-RATE").unwrap();
    let want = v.value().unwrap();
    let tol = v.tolerance.abs.unwrap();
    let rate = c.estimate.rate;
    println!(
        "PHY-014 EIH-FULL: {rate:.6} arcsec/cy (formal {:.1e}, jack-knife {:.1e}) vs {want} +/- \
         {tol}: {:+.6}, margin {:.3}; WP22 prediction {PREDICTION} ({:+.6}); frame Park17 mean \
         orbit, epoch JDEPOC {jdepoc} (TDB), span J2000 +/- {HALF_SPAN_YEARS} Julian yr, fit \
         quadratic + 14 Table-2 terms, gauge PPN isotropic/harmonic, J2 and solar spin DE440 \
         (no overrides)",
        c.estimate.formal_sigma,
        c.estimate.jackknife_sigma,
        rate - want,
        (rate - want).abs() / tol,
        rate - PREDICTION
    );
    assert!(c.model.contains("eih-1pn"), "{}", c.model);
    v.check(rate).unwrap();
}
