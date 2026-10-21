// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP14 checks 4 and 5 (nightly, run locally): per-planet Newtonian contributions to Mercury's
//! perihelion rate over 1000–3000 CE vs Park et al. 2017 Table 3 (PHY-012, NUM-013(b)).
//!
//! Protocol (ADR-0014, maintainer decision C of 2026-10-20, requirements PR #19): "report both".
//! P-remove = ϖ̇(full) − ϖ̇(full − X) is ASSERTED for the seven rows within each vector's
//! tolerance; P-add = ϖ̇(Sun + Mercury + X) − ϖ̇(Sun + Mercury) and the cross terms of Park17's six
//! planet pairs (both protocols, sign convention labelled) are printed as information only.
//! Every configuration starts from the header.440 records at JDEPOC (identical per body, asserted
//! by the harness), runs Newtonian point masses with IAS15 backwards and forwards, and is
//! measured by one Park2017 estimator (daily samples, mean-orbit plane, quadratic + 14 Table-2
//! terms, J2000 ± 1000 Julian yr).
//!
//! The assertion is PROVISIONAL: P-add was pre-registered (WP14) and missed 4 of 7 rows; P-remove
//! was chosen after those results were known. The Park17 rows carry `provisional = "OI-38"`.
//! Revert condition: if a maintainer's reading of the Park 2017 text (OI-38) says P-add, the gate
//! asserts P-add again and its misses are physics discrepancies. The evidence table is written to
//! `target/diffrun/` as markdown and JSON.

mod common;

use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD};
use ferroastro_diffrun::{BodyToggle, PassageSource, Plan, Protocol, RunOptions, run_plan};
use ferroastro_propagate::DataSources;
use ferroastro_scenario::BodyName;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const HALF_SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(Park17 fit span 1000-3000 CE, J2000 +/- 1000 Julian yr as WP5)
const MARGIN_DAYS: f64 = 30.0; // lint: allow-literal(leg margin beyond the fit window, run control)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as the WP5 Park17 data test)
const SEGMENT_YEARS: f64 = 50.0; // lint: allow-literal(checkpoint segment length, run control)

/// Row label -> vector id suffix.
fn vector_id(label: &str, protocol: Protocol) -> String {
    let key = label
        .replace("Earth/Moon", "EARTH-MOON")
        .replace('+', "-")
        .to_uppercase();
    match protocol {
        Protocol::Add | Protocol::Remove => format!("PARK17-T3-{key}"),
        Protocol::CrossTerm | Protocol::CrossTermRemove => format!("PARK17-T3-X-{key}"),
    }
}

/// Verifies: PHY-012, NUM-013, CI-009, CI-010, PHY-015
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "nightly evidence test; it reads top to bottom"
)]
#[ignore = "nightly: 56 IAS15 legs of ~1000 yr; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_phy012_park17_table3_newtonian_contributions() {
    use BodyName::{
        Earth, Jupiter, Mars, Mercury, Moon, Neptune, Pluto, Saturn, Sun, Uranus, Venus,
    };
    let t0 = std::time::Instant::now();
    let vs = VectorSet::load_canonical().unwrap();
    let open_items = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/requirements/OPEN-ITEMS.md"),
    )
    .unwrap();
    let (h, prov) = common::header();
    let data = DataSources::none().with_header(&h, &prov);
    let template = common::scenario("phy012-newton-full-header440.toml");
    let jdepoc = h.value("JDEPOC").unwrap();
    let lo = HALF_SPAN_YEARS.mul_add(-DAYS_PER_JULIAN_YEAR, J2000_JD);
    let hi = HALF_SPAN_YEARS.mul_add(DAYS_PER_JULIAN_YEAR, J2000_JD);
    let back = -(jdepoc - (lo - MARGIN_DAYS)) / DAYS_PER_JULIAN_YEAR;
    let fwd = (hi + MARGIN_DAYS - jdepoc) / DAYS_PER_JULIAN_YEAR;
    println!("JDEPOC {jdepoc}; fit window JD {lo} .. {hi} TDB; legs {back:.6} yr and {fwd:.6} yr");
    let toggles = [
        BodyToggle::new("Venus", &[Venus]),
        BodyToggle::new("Earth/Moon", &[Earth, Moon]),
        BodyToggle::new("Mars", &[Mars]),
        BodyToggle::new("Jupiter", &[Jupiter]),
        BodyToggle::new("Saturn", &[Saturn]),
        BodyToggle::new("Uranus", &[Uranus]),
        BodyToggle::new("Neptune", &[Neptune]),
    ];
    let pairs = [(0, 1), (0, 3), (1, 2), (2, 3), (3, 4), (4, 5)];
    let plan = Plan::bodies(
        "wp14-phy012",
        &template,
        &[Sun, Mercury],
        &template.bodies,
        &toggles,
        &pairs,
        &[back, fwd],
    )
    .unwrap();
    assert!(template.bodies.contains(&Pluto));
    let est = common::park17_estimator(
        common::table2_model(&vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        Some((lo, hi)),
    );
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/diffrun");
    std::fs::create_dir_all(out_dir.join("store")).unwrap();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS,
        store: Some(out_dir.join("store")),
        threads,
    };
    println!(
        "{} configurations, {} legs, {threads} threads, store {}",
        plan.configs.len(),
        plan.leg_count(),
        out_dir.join("store").display()
    );
    let r = run_plan(&plan, &data, &est, &opts).unwrap();
    let wall = t0.elapsed().as_secs_f64();

    let reference = |label: &str, p: Protocol| {
        vs.get(&vector_id(label, p))
            .ok()
            .and_then(|v| v.value().ok())
    };
    let md = r.markdown(&reference);
    println!("{md}");
    let first = r.configs.first().unwrap();
    println!(
        "model: {}\nIC source: {}\nwall {wall:.1} s; total IAS15 steps {}",
        first.model,
        first.ic_source,
        r.configs.iter().map(|c| c.steps).sum::<u64>()
    );
    let stamp = format!("{}", std::process::id());
    std::fs::write(out_dir.join(format!("phy012-{stamp}.md")), &md).unwrap();
    std::fs::write(
        out_dir.join(format!("phy012-{stamp}.json")),
        r.to_json().unwrap(),
    )
    .unwrap();
    println!(
        "evidence written to {}",
        out_dir
            .join(format!("phy012-{stamp}.{{md,json}}"))
            .display()
    );

    // Informational comparisons.
    let rate = |l: &str| {
        r.configs
            .iter()
            .find(|c| c.label == l)
            .unwrap()
            .estimate
            .rate
    };
    let add_sum: f64 = r.rows_of(Protocol::Add).map(|w| w.value).sum();
    let cross_sum: f64 = r.rows_of(Protocol::CrossTerm).map(|w| w.value).sum();
    println!(
        "full - base = {:.6}; sum of P-add rows {add_sum:.6}; + the six cross terms {:.6}; remainder (higher-order interactions, Pluto) {:+.6} arcsec/cy",
        rate("full") - rate("base"),
        add_sum + cross_sum,
        rate("full") - rate("base") - add_sum - cross_sum
    );
    println!(
        "base (Sun + Mercury) rate {:.3e} arcsec/cy: the estimator floor on a closed Kepler orbit (true 0)",
        rate("base")
    );
    for p in [Protocol::CrossTerm, Protocol::CrossTermRemove] {
        for w in r.rows_of(p) {
            let park = reference(&w.combination.label, p);
            println!(
                "{} {}: {:+.6} vs Park17 interaction row {} (informational, OI-38)",
                p.label(),
                w.combination.label,
                w.value,
                park.map_or_else(|| "-".into(), |x| format!("{x:+.4}"))
            );
        }
    }

    // P-add rows: information only (ADR-0014), never asserted.
    let mut add_misses = Vec::new();
    for w in r.rows_of(Protocol::Add) {
        let v = vs
            .get(&vector_id(&w.combination.label, Protocol::Add))
            .unwrap();
        let expected = v.value().unwrap();
        let tol = v.tolerance.allowed(expected);
        let d = w.value - expected;
        println!(
            "PHY-012 {:<10} P-add    {:.6} vs Park17 {expected} : {d:+.6} arcsec/cy, margin {:.2} ({}), information only (ADR-0014)",
            w.combination.label,
            w.value,
            d.abs() / tol,
            if d.abs() <= tol { "within" } else { "OUTSIDE" },
        );
        if d.abs() > tol {
            add_misses.push(format!("{} {d:+.6}", w.combination.label));
        }
    }
    println!(
        "P-add (pre-registered in WP14, now information only): {} of {} rows outside the tolerance: {add_misses:?}",
        add_misses.len(),
        r.rows_of(Protocol::Add).count()
    );

    // PHY-012 / NUM-013(b) rows: P-remove ASSERTED (ADR-0014), PROVISIONAL under OI-38.
    let mut failures = Vec::new();
    let mut labels = std::collections::BTreeSet::new();
    let mut asserted = 0_usize;
    for w in r.rows_of(Protocol::Remove) {
        let id = vector_id(&w.combination.label, Protocol::Remove);
        let v = vs.get(&id).unwrap();
        assert!(
            v.blocking_open_item(&open_items).is_none(),
            "{id}: the PHY-012 rows gate under ADR-0014; use `provisional`, not `blocked_by`"
        );
        let provisional = v.provisional_open_item(&open_items).unwrap();
        let expected = v.value().unwrap();
        let tol = v.tolerance.allowed(expected);
        let d = w.value - expected;
        println!(
            "PHY-012 {:<10} P-remove {:.6} vs Park17 {expected} : {d:+.6} arcsec/cy, margin {:.2} ({}), ASSERTED{}",
            w.combination.label,
            w.value,
            d.abs() / tol,
            if d.abs() <= tol { "within" } else { "OUTSIDE" },
            provisional.map_or_else(String::new, |p| format!(", PROVISIONAL ({})", p.open_item)),
        );
        if let Some(p) = provisional {
            labels.insert(p.to_string());
        }
        if let Err(e) = v.check(w.value) {
            failures.push(e.to_string());
        }
        asserted += 1;
    }
    assert_eq!(
        asserted,
        toggles.len(),
        "one asserted P-remove row per planet"
    );
    for l in &labels {
        println!("{l}");
    }
    if !labels.is_empty() {
        println!(
            "PROVISIONAL (ADR-0014): P-remove was chosen after the WP14/WP17 results were known; P-add was \
             pre-registered and missed 4 of 7 rows. Revert condition: if a maintainer's reading of the Park 2017 \
             text (OI-38) says P-add, the gate asserts P-add again and the P-add misses above are physics \
             discrepancies (type:physics-discrepancy)."
        );
    }
    println!(
        "PHY-012 rows outside the tolerance under P-remove (asserted): {}",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "PHY-012 P-remove rows outside tolerance:\n{}",
        failures.join("\n")
    );
}
