// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP15 check 4 (nightly, run locally): rung N-SUM (PHY-013) and the asteroid differential N-AST.
//!
//! Configurations, all from the header.440 records at JDEPOC (identical per body, asserted by the
//! harness) plus the REF-003 small bodies, Newtonian point masses, IAS15 backwards and forwards
//! over J2000 ± 1000 Julian yr (+ 30 d), measured by the WP14 Park2017 estimator (daily samples,
//! mean-orbit plane, quadratic + 14 Table-2 terms):
//!
//! * `base` = Sun + Mercury; `base+asteroids` = + the 343 numbered asteroids (Minor);
//! * `planets` = the WP14 full system (Sun, planets with Earth and Moon resolved, Pluto);
//! * `planets+asteroids`; `full` = + the 3 identified KBOs (Minor) + the 36-point ring.
//!
//! Rows: N-AST P-add (maintainer protocol, `base+asteroids − base`, vs Park17 0.0012 ± 0.0003,
//! non-gating while OI-38 is open), N-AST P-remove (`planets+asteroids − planets`, reported),
//! the KBO + ring increment (`full − planets+asteroids`, reported). PHY-013 asserts the `full`
//! rate against 532.30 ± 0.01. Clemence 1947 is printed for information only (OI-01).

mod common;

use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD};
use ferroastro_diffrun::{
    Combination, Config, PassageSource, Plan, Protocol, RunOptions, run_plan,
};
use ferroastro_propagate::DataSources;
use ferroastro_scenario::{BodyName, ScenarioSpec};
use ferroastro_system::SmallBodyStates;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const HALF_SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(Park17 fit span 1000-3000 CE, J2000 +/- 1000 Julian yr as WP5)
const MARGIN_DAYS: f64 = 30.0; // lint: allow-literal(leg margin beyond the fit window, run control)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as the WP5 Park17 data test)
const SEGMENT_YEARS: f64 = 50.0; // lint: allow-literal(checkpoint segment length, run control)
const ASTEROIDS: u32 = 343; // lint: allow-literal(the DE440 numbered-asteroid set, a switch value)
const CLEMENCE_1947_PLANETS: f64 = 531.504; // lint: allow-literal(Clemence 1947 planets-only value [S], PHY-013 notes; printed for information only, never asserted, OI-01)

fn config(label: &str, template: &ScenarioSpec, years: &[f64]) -> Config {
    Config {
        label: label.to_owned(),
        legs: years
            .iter()
            .enumerate()
            .map(|(i, &y)| {
                let mut s = template.clone();
                s.name = format!("wp15-phy013/{label}/leg{i}");
                s.span.years = y;
                s
            })
            .collect(),
    }
}

fn combo(label: &str, protocol: Protocol, plus: &str, minus: &str) -> Combination {
    Combination {
        label: label.to_owned(),
        protocol,
        terms: vec![(1, plus.to_owned()), (-1, minus.to_owned())],
    }
}

/// Verifies: PHY-013, PHY-019, NUM-009, NUM-013, REF-003, CI-009, CI-010, PHY-015
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "nightly evidence test; it reads top to bottom"
)]
#[ignore = "nightly: 10 IAS15 legs of ~1000 yr, up to 393 bodies; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_phy013_n_sum_and_asteroid_differential() {
    use BodyName::{Mercury, Sun};
    let t0 = std::time::Instant::now();
    let vs = VectorSet::load_canonical().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let open_items = std::fs::read_to_string(root.join("docs/requirements/OPEN-ITEMS.md")).unwrap();
    let (h, prov) = common::header();
    let small = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    println!("{}", small.provenance());
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_small_bodies(&small);

    let mut planets = common::scenario("phy012-newton-full-header440.toml");
    planets.output.sample_bodies = Some(vec![Sun, Mercury]);
    let mut base = planets.clone();
    base.bodies = vec![Sun, Mercury];
    let with_asteroids = |s: &ScenarioSpec| {
        let mut t = s.clone();
        t.forces.asteroids = ASTEROIDS;
        t
    };
    let mut full = with_asteroids(&planets);
    full.forces.kbos = true;
    full.forces.kbo_ring = true;

    let jdepoc = h.value("JDEPOC").unwrap();
    let lo = HALF_SPAN_YEARS.mul_add(-DAYS_PER_JULIAN_YEAR, J2000_JD);
    let hi = HALF_SPAN_YEARS.mul_add(DAYS_PER_JULIAN_YEAR, J2000_JD);
    let back = -(jdepoc - (lo - MARGIN_DAYS)) / DAYS_PER_JULIAN_YEAR;
    let fwd = (hi + MARGIN_DAYS - jdepoc) / DAYS_PER_JULIAN_YEAR;
    let years = [back, fwd];
    println!("JDEPOC {jdepoc}; fit window JD {lo} .. {hi} TDB; legs {back:.6} yr and {fwd:.6} yr");
    let plan = Plan {
        name: "wp15-phy013".into(),
        base: vec![Sun.naif(), Mercury.naif()],
        configs: vec![
            config("base", &base, &years),
            config("base+asteroids", &with_asteroids(&base), &years),
            config("planets", &planets, &years),
            config("planets+asteroids", &with_asteroids(&planets), &years),
            config("full", &full, &years),
        ],
        combinations: vec![
            combo("Asteroids", Protocol::Add, "base+asteroids", "base"),
            combo(
                "Asteroids",
                Protocol::Remove,
                "planets+asteroids",
                "planets",
            ),
            combo("KBOs+ring", Protocol::Remove, "full", "planets+asteroids"),
            combo("N-SUM (full - base)", Protocol::Add, "full", "base"),
        ],
    };
    plan.validate().unwrap();
    let est = common::park17_estimator(
        common::table2_model(&vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        Some((lo, hi)),
    );
    let out_dir = root.join("target/diffrun");
    std::fs::create_dir_all(out_dir.join("store")).unwrap();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS,
        store: Some(out_dir.join("store")),
        threads,
    };
    println!(
        "{} configurations, {} legs, {threads} threads",
        plan.configs.len(),
        plan.leg_count()
    );
    let r = run_plan(&plan, &data, &est, &opts).unwrap();
    let wall = t0.elapsed().as_secs_f64();
    let reference = |label: &str, p: Protocol| match (label, p) {
        ("Asteroids", Protocol::Add | Protocol::Remove) => vs
            .get("PARK17-T3-ASTEROIDS")
            .ok()
            .and_then(|v| v.value().ok()),
        _ => None,
    };
    let md = r.markdown(&reference);
    println!("{md}");
    let stamp = format!("{}", std::process::id());
    std::fs::write(out_dir.join(format!("phy013-{stamp}.md")), &md).unwrap();
    std::fs::write(
        out_dir.join(format!("phy013-{stamp}.json")),
        r.to_json().unwrap(),
    )
    .unwrap();
    println!(
        "estimator {}; shared IC hash {}; wall {wall:.1} s; IAS15 steps {}",
        r.estimator,
        r.shared_ic_hash,
        r.configs.iter().map(|c| c.steps).sum::<u64>()
    );
    for c in &r.configs {
        println!(
            "config {:<18} {:>3} bodies: rate {:.6} +/- {:.6} (jack-knife) arcsec/cy, {} passages, {} steps; cross-check {}",
            c.label,
            c.bodies.len(),
            c.estimate.rate,
            c.estimate.jackknife_sigma,
            c.estimate.passages,
            c.steps,
            c.cross_check
                .as_ref()
                .map_or_else(|| "-".into(), |e| format!("{:.6}", e.rate))
        );
    }
    let full_cfg = r.configs.iter().find(|c| c.label == "full").unwrap();
    println!(
        "model (full): {}\nIC source (full): {}",
        full_cfg.model, full_cfg.ic_source
    );
    let rate = |l: &str| {
        r.configs
            .iter()
            .find(|c| c.label == l)
            .unwrap()
            .estimate
            .rate
    };

    // N-AST rows.
    let ast = vs.get("PARK17-T3-ASTEROIDS").unwrap();
    let (expected, tol) = (ast.value().unwrap(), ast.tolerance.abs.unwrap());
    let blocked = ast.blocking_open_item(&open_items);
    for w in r.rows.iter().filter(|w| w.combination.label == "Asteroids") {
        let d = w.value - expected;
        println!(
            "N-AST {:<8} {:.6} (sigma_rss {:.6}) vs Park17 {expected}: {d:+.6} arcsec/cy, margin {:.2} ({}){}",
            w.combination.protocol.label(),
            w.value,
            w.sigma_rss,
            d.abs() / tol,
            if d.abs() <= tol { "within" } else { "OUTSIDE" },
            if w.combination.protocol == Protocol::Add {
                blocked.map_or_else(String::new, |oi| format!(", non-gating: {oi} open"))
            } else {
                ", reported only".into()
            }
        );
        if w.combination.protocol == Protocol::Add && blocked.is_none() {
            ast.check(w.value).unwrap();
        }
    }
    println!(
        "KBOs + ring increment: {:+.6} arcsec/cy (reported; Park17 has no KBOs)",
        rate("full") - rate("planets+asteroids")
    );
    println!(
        "planets-only {:.6} arcsec/cy (WP14 full: 532.303644); Clemence 1947 planets-only {CLEMENCE_1947_PLANETS} (information only, OI-01, never asserted)",
        rate("planets")
    );

    // PHY-013.
    let nsum = vs.get("PHY013-N-SUM").unwrap();
    let full_rate = rate("full");
    println!(
        "PHY-013 N-SUM: full Newtonian {full_rate:.6} arcsec/cy vs {} +/- {}: {:+.6}, margin {:.3}; frame Park17 mean orbit, epoch JDEPOC {jdepoc}, span J2000 +/- {HALF_SPAN_YEARS} Julian yr, fit quadratic + 14 Table-2 terms, estimator {}",
        nsum.value().unwrap(),
        nsum.tolerance.abs.unwrap(),
        full_rate - nsum.value().unwrap(),
        (full_rate - nsum.value().unwrap()).abs() / nsum.tolerance.abs.unwrap(),
        r.estimator
    );
    nsum.check(full_rate).unwrap();
}
