// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 check E8 (nightly, release, local): the cost of body tiering with the 1PN EIH terms
//! evaluated for every moving body (NUM-009; maintainer decision D1 of 2026-10-21). NUM-009's
//! notes: "with EIH evaluated for all bodies the ratio MUST be re-measured".
//!
//! Bodies as WP15 check 2a: the 11 DE440 major bodies of header.440 (Earth and Moon in the
//! geocentric DE variables) plus the 5 header asteroids with the largest GMs as the 16 Majors;
//! the other 338 numbered asteroids (REF-003 states) as Minor. Three configurations, IAS15
//! default settings, 10 Julian years from JDEPOC, each timed 5 times in this process (median):
//!
//! * A: 16 Major, Newtonian + EIH; B: A + 338 Minor (tiered Newtonian + EIH over the tiered
//!   graph); C: B without EIH; D: the same 354 bodies all Major, Newtonian (all-mutual).
//!
//! The configurations run interleaved (A, B, C, D, A, ...), and the 1-minute load average is
//! recorded. Asserted (NUM-009 acceptance, maintainer decision 2026-11-11): B/C < 5
//! (`NUM009-TIER-COST-RATIO`) and C/D < 1 (`NUM009-TIER-VS-ALL-MUTUAL`). B/A, the ratio to the
//! 16-Major run, is reported.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "test: failures are test failures; counts are small; indices in range by construction; one linear timing test"
)]

mod common;

use ferroastro_constants::De440;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_eih::{Eih, RhsAcceleration};
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

/// One timed IAS15 run of `years` Julian years: wall seconds and accepted steps.
fn run_once(model: &NBodyModel<Icrf>, x0: &[f64], v0: &[f64], years: f64) -> (f64, u64) {
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
        years * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY,
        |_, _| Ok(()),
    )
    .unwrap();
    (t.elapsed().as_secs_f64(), ias.stats().steps)
}

/// The 1-minute load average from `/proc/loadavg` (`NaN` where unavailable).
fn load() -> f64 {
    std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|x| x.parse().ok()))
        .unwrap_or(f64::NAN)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Verifies: NUM-009
#[test]
#[ignore = "nightly: four 10-yr IAS15 configurations x 5 interleaved rounds, up to 354 bodies; release"]
fn nightly_num009_tiered_minor_bodies_cost_with_eih_for_all_bodies() {
    let v = common::vectors();
    let (h, prov) = common::header();
    let de = De440::load().unwrap();
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
    let build = |bodies: &[NBody], minor_term: bool, eih: bool| {
        let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        ];
        if minor_term {
            terms.push(Box::new(MinorFromMajors::new(MinorReaction::On)));
        }
        if eih {
            terms.push(Box::new(
                Eih::new(de.speed_of_light(), 1.0, 1.0, RhsAcceleration::Newtonian).unwrap(),
            ));
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
    let d_bodies = with(&asteroids, Tier::Major);
    a_bodies.shrink_to_fit();
    let configs = [
        ("A: 16 Major, EIH", build(&a_bodies, false, true)),
        (
            "B: 16 Major + 338 Minor, EIH all bodies",
            build(&b_bodies, true, true),
        ),
        (
            "C: 16 Major + 338 Minor, Newtonian (tiered)",
            build(&b_bodies, true, false),
        ),
        (
            "D: 354 Major, Newtonian (all-mutual)",
            build(&d_bodies, false, false),
        ),
    ];
    // Run controls (not physics): the nightly default is 10 yr x 5 rounds; a shorter local run
    // may set FA_COST_YEARS / FA_COST_ROUNDS. The configurations are INTERLEAVED (A, B, C, D,
    // A, B, ...), so a change of machine load affects every configuration alike.
    let years: f64 = std::env::var("FA_COST_YEARS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(YEARS);
    let rounds: usize = std::env::var("FA_COST_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(REPEATS);
    let packed: Vec<_> = configs
        .iter()
        .map(|(_, m)| m.pack(&states, ic.earth_moon()).unwrap())
        .collect();
    let mut walls = vec![Vec::new(); configs.len()];
    let mut steps = vec![0_u64; configs.len()];
    let mut loads = vec![load()];
    for r in 0..rounds {
        for (k, ((label, m), (x, w))) in configs.iter().zip(&packed).enumerate() {
            let (wall, n) = run_once(m, x, w, years);
            println!(
                "round {r} {label:<44} {wall:8.3} s, {n} steps, load {:.1}",
                load()
            );
            walls[k].push(wall);
            steps[k] = n;
        }
        loads.push(load());
    }
    let med: Vec<f64> = walls.iter().map(|w| median(w.clone())).collect();
    for (k, (label, m)) in configs.iter().enumerate() {
        println!(
            "{label:<44} {:>3} bodies: {} steps, median {:.3} s of {:.3?}",
            m.layout().len(),
            steps[k],
            med[k],
            walls[k]
        );
    }
    let (ratio_ba, ratio_bc, ratio_cd) = (med[1] / med[0], med[1] / med[2], med[2] / med[3]);
    println!(
        "E8 ({years} yr, {rounds} interleaved rounds, 1-min load {:.1}..{:.1}, median {:.1}): \
         B/C = {ratio_bc:.3} (NUM-009 < 5, EIH for every body vs the same tiered run without \
         EIH); C/D = {ratio_cd:.3} (NUM-009 < 1, tiered vs all-mutual); B/A = {ratio_ba:.2} \
         (reported, ratio to the 16-Major run)",
        loads.iter().copied().fold(f64::INFINITY, f64::min),
        loads.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        median(loads.clone())
    );
    v.get("NUM009-TIER-COST-RATIO")
        .unwrap()
        .check(ratio_bc)
        .unwrap();
    v.get("NUM009-TIER-VS-ALL-MUTUAL")
        .unwrap()
        .check(ratio_cd)
        .unwrap();
}
