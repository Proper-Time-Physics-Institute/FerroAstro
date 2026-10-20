// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP20 check N3 (nightly, run locally): the solar J2 and Lense–Thirring contributions to
//! Mercury's perihelion rate in the full N-body model under EIH, 1000–3000 CE, compared with
//! Park et al. 2017 Table 3 (NUM-019, PHY-010, PHY-011).
//!
//! Fixed before any run (IMPLEMENTATION-NOTES WP20): the PHY-012 full system (Sun, the planets
//! with Earth and Moon resolved, Pluto) under `gr_eih` (β = γ = 1, DE440), header.440 ICs, IAS15
//! legs back and forward to J2000 ± 1000 Julian yr (+30 d); toggles J2 (Park17's 2.25e-7,
//! `PARK17-J2SUN`, WGCCRE 2015 pole) and LT (DE440 spin, with reaction); P-add, P-remove and the
//! J2×LT cross term; one IC hash; the Park2017 estimator (daily samples, Park17 mean-orbit plane,
//! quadratic + 14 Table-2 terms). J2 P-add is asserted against `PARK17-J2-RATE`; the LT row's
//! vector `NUM019-NBODY-LT-CONTRIBUTION` carries `blocked_by = "OI-35"` and gates only once OI-35
//! is closed. The evidence is written to `target/diffrun/` as markdown and JSON.

mod common;

use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, J2000_JD};
use ferroastro_diffrun::{PassageSource, Plan, Protocol, RunOptions, SwitchToggle, run_plan};
use ferroastro_propagate::{DataSources, SolarOverrides};
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const HALF_SPAN_YEARS: f64 = 1000.0; // lint: allow-literal(Park17 fit span 1000-3000 CE, J2000 +/- 1000 Julian yr as WP5)
const MARGIN_DAYS: f64 = 30.0; // lint: allow-literal(leg margin beyond the fit window, run control)
const JACKKNIFE_BLOCKS: usize = 20; // lint: allow-literal(jack-knife blocks, as the WP5 Park17 data test)
const SEGMENT_YEARS: f64 = 50.0; // lint: allow-literal(checkpoint segment length, run control)

/// Verifies: NUM-019, PHY-010, PHY-011, NUM-013, CI-009, CI-010, PHY-015
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "nightly evidence test; it reads top to bottom"
)]
#[ignore = "nightly: 8 EIH IAS15 legs of ~1000 yr; cargo nextest run --cargo-profile release --profile nightly --run-ignored only"]
fn nightly_num019_solar_j2_and_lense_thirring_contributions_under_eih() {
    let t0 = std::time::Instant::now();
    let vs = VectorSet::load_canonical().unwrap();
    let open_items = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/requirements/OPEN-ITEMS.md"),
    )
    .unwrap();
    let (h, prov) = common::header();
    let park_j2 = vs.get("PARK17-J2SUN").unwrap().value().unwrap();
    let ov = SolarOverrides {
        j2: Some(park_j2),
        pole: None,
        spin_gm: None,
        provenance: "WP20 N3: J2 = PARK17-J2SUN (Park17 Table 1); WGCCRE 2015 pole; DE440 spin"
            .into(),
    };
    let data = DataSources::none()
        .with_header(&h, &prov)
        .with_solar_overrides(&ov);
    let mut template = common::scenario("phy012-newton-full-header440.toml");
    template.forces.gr_eih = true;
    template.name = "wp20-num019".into();
    let jdepoc = h.value("JDEPOC").unwrap();
    let lo = HALF_SPAN_YEARS.mul_add(-DAYS_PER_JULIAN_YEAR, J2000_JD);
    let hi = HALF_SPAN_YEARS.mul_add(DAYS_PER_JULIAN_YEAR, J2000_JD);
    let back = -(jdepoc - (lo - MARGIN_DAYS)) / DAYS_PER_JULIAN_YEAR;
    let fwd = (hi + MARGIN_DAYS - jdepoc) / DAYS_PER_JULIAN_YEAR;
    println!("JDEPOC {jdepoc}; fit window JD {lo} .. {hi} TDB; legs {back:.6} yr and {fwd:.6} yr");
    println!("{}", ov.describe());
    let toggles = [
        SwitchToggle {
            label: "J2".into(),
            apply: |f| f.sun_j2 = true,
        },
        SwitchToggle {
            label: "LT".into(),
            apply: |f| f.lense_thirring = true,
        },
    ];
    let plan =
        Plan::switches_both("wp20-num019", &template, &toggles, &[(0, 1)], &[back, fwd]).unwrap();
    let est = common::park17_estimator(
        common::table2_model(&vs, true),
        JACKKNIFE_BLOCKS,
        PassageSource::SampledStates,
        Some((lo, hi)),
    );
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/diffrun");
    let store = out_dir.join("store-wp20");
    std::fs::create_dir_all(&store).unwrap();
    let threads = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
    let opts = RunOptions {
        segment_years: SEGMENT_YEARS,
        store: Some(store.clone()),
        threads,
    };
    println!(
        "{} configurations, {} legs, {threads} threads, store {}",
        plan.configs.len(),
        plan.leg_count(),
        store.display()
    );
    let r = run_plan(&plan, &data, &est, &opts).unwrap();
    let wall = t0.elapsed().as_secs_f64();
    let reference = |label: &str, p: Protocol| match (label, p) {
        ("J2", Protocol::Add | Protocol::Remove) => {
            vs.get("PARK17-J2-RATE").ok().and_then(|v| v.value().ok())
        }
        ("LT", Protocol::Add | Protocol::Remove) => vs
            .get("NUM019-NBODY-LT-CONTRIBUTION")
            .ok()
            .and_then(|v| v.value().ok()),
        _ => None,
    };
    let md = r.markdown(&reference);
    println!("{md}");
    let first = r.configs.first().unwrap();
    let hash0 = &first.ic_hashes[0];
    assert!(
        r.configs
            .iter()
            .all(|c| c.ic_hashes.iter().all(|x| x == hash0))
    );
    for c in &r.configs {
        println!(
            "config {}: rate {:.6} (formal {:.1e}, jack-knife {:.1e}); {} steps\n  model: {}",
            c.label,
            c.estimate.rate,
            c.estimate.formal_sigma,
            c.estimate.jackknife_sigma,
            c.steps,
            c.model
        );
    }
    println!(
        "IC source: {}\nIC hash (all configurations): {hash0}\nwall {wall:.1} s; total IAS15 steps {}",
        first.ic_source,
        r.configs.iter().map(|c| c.steps).sum::<u64>()
    );
    let stamp = format!("{}", std::process::id());
    std::fs::write(out_dir.join(format!("num019-{stamp}.md")), &md).unwrap();
    std::fs::write(
        out_dir.join(format!("num019-{stamp}.json")),
        r.to_json().unwrap(),
    )
    .unwrap();
    println!(
        "evidence written to {}",
        out_dir
            .join(format!("num019-{stamp}.{{md,json}}"))
            .display()
    );
    for p in [
        Protocol::Remove,
        Protocol::CrossTerm,
        Protocol::CrossTermRemove,
    ] {
        for w in r.rows_of(p) {
            println!(
                "{} {}: {:+.7} arcsec/cy (informational)",
                p.label(),
                w.combination.label,
                w.value
            );
        }
    }
    let add = |l: &str| {
        r.rows_of(Protocol::Add)
            .find(|w| w.combination.label == l)
            .unwrap()
            .value
    };
    let j2v = vs.get("PARK17-J2-RATE").unwrap();
    let ltv = vs.get("NUM019-NBODY-LT-CONTRIBUTION").unwrap();
    let (j2, lt) = (add("J2"), add("LT"));
    for (name, got, v) in [("J2", j2, &j2v), ("LT", lt, &ltv)] {
        let want = v.value().unwrap();
        let tol = v.tolerance.abs.unwrap();
        let blocked = v.blocking_open_item(&open_items);
        println!(
            "NUM-019 {name} P-add {got:+.7} vs {} {want}: {:+.7} arcsec/cy, margin {:.3}{}",
            v.id,
            got - want,
            (got - want).abs() / tol,
            blocked.map_or_else(String::new, |oi| format!(" (non-gating: {oi} open)"))
        );
    }
    j2v.check(j2).unwrap();
    if ltv.blocking_open_item(&open_items).is_none() {
        ltv.check(lt).unwrap();
    }
}
