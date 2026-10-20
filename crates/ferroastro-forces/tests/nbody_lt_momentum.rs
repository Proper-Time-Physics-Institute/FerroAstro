// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP20 check L3 (NUM-019): total linear momentum is conserved with solar J2 and Lense–Thirring
//! on, both with their reaction on the Sun. Sun + 8 planets + geocentric Moon, Newtonian + J2 + LT
//! (DE440 constants, WGCCRE 2015 pole), header.440 ICs from the pinned cache (CI-005), 100 yr,
//! IAS15 f64; bound `NBODY-MOMENTUM-ROUNDOFF-ULPS` (WP12). Stated limit (WP20 plan): this run
//! cannot see a missing LT reaction (its momentum effect is ~1e-12 relative over 100 yr and
//! mostly periodic); the term-level check L1a is the discriminating one. The run without the LT
//! reaction is printed for comparison, not asserted.
#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure"
)]

mod common;

use common::{Moon, epochs, header, ids, model_lt, propagator, vectors};
use ferroastro_constants::De440;
use ferroastro_forces::LtReaction;
use ferroastro_integrators::Ias15Config;
use ferroastro_system::InitialConditions;

fn run(years: f64, reaction: LtReaction) -> (f64, u64) {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let m = model_lt::<f64>(&de, &ids(false), Moon::Geocentric, true, Some(reaction));
    println!("{}", m.describe());
    let mut p = propagator(m, &ic, Ias15Config::default());
    let (p0, scale0) = p.momentum_over_g();
    let samples = 100;
    let step = years * ferroastro_core::time::DAYS_PER_JULIAN_YEAR / f64::from(samples);
    let mut worst = 0.0_f64;
    for e in epochs(ic.epoch(), step, samples) {
        p.advance_to(e).unwrap();
        let (pt, _) = p.momentum_over_g();
        let dp = (pt[0] - p0[0]).hypot(pt[1] - p0[1]).hypot(pt[2] - p0[2]) / scale0;
        worst = worst.max(dp);
    }
    (worst, p.stats().steps)
}

/// Verifies: NUM-019
#[test]
fn momentum_is_conserved_with_solar_j2_and_lense_thirring() {
    let years = 100.0; // lint: allow-literal(run span, WP20 plan L3 = WP12 plan 1a)
    let t0 = std::time::Instant::now();
    let (worst, steps) = run(years, LtReaction::On);
    let ulps = worst / (f64::EPSILON * (steps as f64).sqrt());
    println!(
        "L3: {years} yr, {steps} IAS15 steps ({:.1} s): max |P(t) - P(0)| / sum GM|v| = {worst:.3e} = {ulps:.3} eps sqrt(N_steps)",
        t0.elapsed().as_secs_f64()
    );
    let (off, steps_off) = run(years, LtReaction::Off);
    println!(
        "L3 (informational, LT reaction off): {off:.3e} = {:.3} eps sqrt(N_steps)",
        off / (f64::EPSILON * (steps_off as f64).sqrt())
    );
    vectors()
        .get("NBODY-MOMENTUM-ROUNDOFF-ULPS")
        .unwrap()
        .check(ulps)
        .unwrap();
}
