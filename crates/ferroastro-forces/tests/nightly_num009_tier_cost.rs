// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP15 checks 2a and 2b (nightly, release, local): the cost of body tiering (NUM-009).
//!
//! Bodies: the 11 DE440 major bodies of header.440 (Earth and Moon in the geocentric DE
//! variables) plus the 5 header asteroids with the largest GMs as the 16 Majors; the other 338
//! numbered asteroids (REF-003 states) as Minor. Three configurations, IAS15 default settings,
//! Newtonian, 10 Julian years from JDEPOC, each timed 5 times in this process (median):
//!
//! * A: 16 Major alone; B: 16 Major + 338 Minor (tiered); C: all 354 Major (all-mutual).
//!
//! 2a: B/A < 2 (NUM-009), reported only while OI-42 is open (the acceptance assumes EIH Majors).
//! 2b: B/C < 1 (tiering is cheaper than all-mutual), asserted.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test: failures are test failures; counts are small; indices in range by construction; one linear timing test"
)]

mod common;

use ferroastro_core::frames::Icrf;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors, MinorReaction,
    MoonVariables, NBody, NBodyModel, NewtonianMutual,
};
use ferroastro_integrators::{Ias15, Ias15Config};
use ferroastro_system::{Catalogue, InitialConditions, SmallBodyStates, Tier, place_small_bodies};
use std::path::PathBuf;

const YEARS: f64 = 10.0; // lint: allow-literal(run length, nightly cost)
const REPEATS: usize = 5; // lint: allow-literal(median of 5, as CI-008)
const PROMOTED: usize = 5; // lint: allow-literal(16 Majors = 11 DE440 majors + 5 asteroids, NUM-009 interpretation, IMPLEMENTATION-NOTES WP15)

struct Timing {
    label: &'static str,
    bodies: usize,
    steps: u64,
    median_s: f64,
}

fn time(label: &'static str, model: &NBodyModel<Icrf>, x0: &[f64], v0: &[f64]) -> Timing {
    let mut walls = Vec::new();
    let mut steps = 0;
    for _ in 0..REPEATS {
        let mut ias = Ias15::new(
            x0.len(),
            0.0,
            x0,
            v0,
            0.1 * SECONDS_PER_DAY, // lint: allow-literal(first trial step 0.1 d, the propagate default)
            Ias15Config::default(),
        )
        .unwrap();
        let t = std::time::Instant::now();
        ias.integrate_to(
            model,
            YEARS * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY,
            |_, _| Ok(()),
        )
        .unwrap();
        walls.push(t.elapsed().as_secs_f64());
        steps = ias.stats().steps;
    }
    walls.sort_by(f64::total_cmp);
    let median_s = walls[REPEATS / 2];
    println!(
        "{label:<38} {:>3} bodies: {steps} steps, median {median_s:.3} s of {walls:.3?} ({:.1} us/step)",
        model.layout().len(),
        1e6 * median_s / steps as f64 // lint: allow-literal(seconds to microseconds)
    );
    Timing {
        label,
        bodies: model.layout().len(),
        steps,
        median_s,
    }
}

/// Verifies: NUM-009
#[test]
#[ignore = "nightly: three 10-yr IAS15 runs x 5 repeats, up to 354 mutually interacting bodies; release"]
fn nightly_num009_tiered_minor_bodies_cost() {
    let v = common::vectors();
    let (h, prov) = common::header();
    let de = ferroastro_constants::De440::load().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = SmallBodyStates::parse(
        &std::fs::read_to_string(root.join("tests/goldens/sb441-n373s-jdepoc-states.csv")).unwrap(),
    )
    .unwrap();
    println!("{}", file.provenance());
    let cat = Catalogue::from_header(&h, &prov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let mut asteroids: Vec<(NaifId, f64)> = cat
        .bodies()
        .iter()
        .filter(|b| b.tier() == Tier::Minor)
        .map(|b| (b.id(), b.gm().km3_per_s2()))
        .collect();
    asteroids.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.0.cmp(&b.0.0)));
    let (promoted, minor) = asteroids.split_at(PROMOTED);
    println!(
        "16 Majors: the 11 DE440 majors + asteroids {:?} (largest header GMs); {} Minor",
        promoted.iter().map(|a| a.0.0).collect::<Vec<_>>(),
        minor.len()
    );
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let ids: Vec<NaifId> = asteroids.iter().map(|a| a.0).collect();
    let placed = place_small_bodies(&file, &ids, &sun).unwrap();
    let mut states = ic.states().clone();
    for (id, s) in placed.states().unwrap() {
        states.push(id, &s).unwrap();
    }
    let majors: Vec<NBody> = cat
        .bodies()
        .iter()
        .filter(|b| b.tier() == Tier::Major)
        .map(|b| NBody {
            id: b.id(),
            gm: b.gm().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    let with = |extra: &[(NaifId, f64)], tier: Tier| -> Vec<NBody> {
        let mut out = majors.clone();
        out.extend(extra.iter().map(|&(id, gm)| NBody { id, gm, tier }));
        out
    };
    let build = |bodies: &[NBody], minor_term: bool| {
        let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        ];
        if minor_term {
            terms.push(Box::new(MinorFromMajors::new(MinorReaction::On)));
        }
        NBodyModel::<Icrf>::new(
            bodies,
            MoonVariables::Geocentric {
                emrat: de.earth_moon_mass_ratio(),
            },
            terms,
        )
        .unwrap()
    };
    let mut a_bodies = with(promoted, Tier::Major);
    let b_bodies = {
        let mut b = a_bodies.clone();
        b.extend(minor.iter().map(|&(id, gm)| NBody {
            id,
            gm,
            tier: Tier::Minor,
        }));
        b
    };
    let c_bodies = with(&asteroids, Tier::Major);
    a_bodies.shrink_to_fit();
    let configs = [
        ("A: 16 Major", build(&a_bodies, false)),
        ("B: 16 Major + 338 Minor (tiered)", build(&b_bodies, true)),
        ("C: 354 Major (all-mutual)", build(&c_bodies, false)),
    ];
    let mut t = Vec::new();
    for (label, m) in &configs {
        let (x, w) = m.pack(&states, ic.earth_moon()).unwrap();
        t.push(time(label, m, &x, &w));
    }
    let (a, b, c) = (&t[0], &t[1], &t[2]);
    let per_step = |x: &Timing| x.median_s / x.steps as f64;
    let ratio_ab = b.median_s / a.median_s;
    let ratio_bc = b.median_s / c.median_s;
    println!(
        "2a NUM-009 B/A = {ratio_ab:.2} (per step {:.2}); 2b B/C = {ratio_bc:.3} (per step {:.3}); bodies {}/{}/{}; labels {}/{}/{}",
        per_step(b) / per_step(a),
        per_step(b) / per_step(c),
        a.bodies,
        b.bodies,
        c.bodies,
        a.label,
        b.label,
        c.label
    );
    let open_items = std::fs::read_to_string(root.join("docs/requirements/OPEN-ITEMS.md")).unwrap();
    let va = v.get("NUM009-TIER-COST-RATIO").unwrap();
    let blocked = va.blocking_open_item(&open_items);
    println!(
        "2a: {ratio_ab:.2} vs NUM-009 < {} ({}){}",
        va.tolerance.abs.unwrap(),
        if ratio_ab < va.tolerance.abs.unwrap() {
            "within"
        } else {
            "OUTSIDE"
        },
        blocked.map_or_else(String::new, |oi| format!(", non-gating: {oi} open"))
    );
    if blocked.is_none() {
        va.check(ratio_ab).unwrap();
    }
    v.get("NUM009-TIER-VS-ALL-MUTUAL")
        .unwrap()
        .check(ratio_bc)
        .unwrap();
}
