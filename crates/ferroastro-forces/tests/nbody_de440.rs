// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP12 N-body checks on the real DE440 initial conditions (PR tier: header.440 and de440.bsp
//! from the pinned JPL cache, CI-005, each re-verified against its SHA-256):
//!
//! * U1: the new Newtonian kernel equals the M0 kernel bit for bit on the 11 header bodies;
//! * 1a: total linear momentum with solar J2 + reaction is conserved to round-off (NUM-019);
//! * 2: two identical runs are bitwise identical, in f64 and double-double (TST-005);
//! * 4: Newtonian + J2, Sun + 8 planets + geocentric Moon from header.440, 1969-06-28 → +10 yr,
//!   against de440.bsp: the residual table is printed; only the outer planets are asserted
//!   (sanity bound, PROVISIONAL), because the model omits GR, the asteroids and the KBOs.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{Moon, cached, epochs, header, ids, model, name, propagator, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::{Barycentric, Geocentric, Icrf, State};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_forces::NewtonianPointMasses;
use ferroastro_integrators::{Ias15Config, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::InitialConditions;

/// U1: the `NewtonianMutual` term (barycentric Moon) and the M0 `NewtonianPointMasses` kernel
/// give bitwise-identical accelerations for the 11 header.440 bodies at JDEPOC.
///
/// Verifies: NUM-009
#[test]
fn newtonian_mutual_equals_the_m0_kernel_bitwise() {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let all = ids(true);
    let m = model::<f64>(&de, &all, Moon::Barycentric, false);
    let (x, v) = m.pack(ic.states(), None).unwrap();
    let mut a = vec![0.0; x.len()];
    m.acceleration(0.0, &x, &v, &mut a).unwrap();
    // The M0 kernel with interleaved (AoS) layout.
    let gms: Vec<_> = all
        .iter()
        .map(|&id| {
            ferroastro_system::Catalogue::de440_major(&de)
                .gm(id)
                .unwrap()
        })
        .collect();
    let m0 = NewtonianPointMasses::<Icrf, Barycentric>::new(&gms).unwrap();
    let n = all.len();
    let aos = |s: &[f64]| -> Vec<f64> {
        (0..n)
            .flat_map(|i| [s[i], s[n + i], s[2 * n + i]])
            .collect()
    };
    let mut a0 = vec![0.0; x.len()];
    m0.acceleration(0.0, &aos(&x), &aos(&v), &mut a0).unwrap();
    let diff = aos(&a)
        .iter()
        .zip(&a0)
        .map(|(p, q)| (p - q).abs())
        .fold(0.0, f64::max);
    let differing_bits = aos(&a)
        .iter()
        .zip(&a0)
        .filter(|(p, q)| p.to_bits() != q.to_bits())
        .count();
    println!(
        "U1: max |a_new - a_M0| = {diff:e} km/s^2; differing words {differing_bits} of {}",
        a0.len()
    );
    vectors()
        .get("NBODY-NEWTON-VS-M0-KERNEL-DIFF")
        .unwrap()
        .check(diff)
        .unwrap();
    assert_eq!(differing_bits, 0);
}

/// Sun + 8 planets + geocentric Moon, Newtonian + solar J2 with reaction, f64, from header.440.
fn wp12_model_run(years: f64, samples: u32) -> (Vec<f64>, u64, f64) {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let m = model::<f64>(&de, &ids(false), Moon::Geocentric, true);
    println!("{}", m.describe());
    let mut p = propagator(m, &ic, Ias15Config::default());
    let (p0, scale0) = p.momentum_over_g().unwrap();
    let step = years * ferroastro_core::time::DAYS_PER_JULIAN_YEAR / f64::from(samples);
    let mut worst = 0.0_f64;
    for e in epochs(ic.epoch(), step, samples) {
        p.advance_to(e).unwrap();
        let (pt, _) = p.momentum_over_g().unwrap();
        let dp = (pt[0] - p0[0]).hypot(pt[1] - p0[1]).hypot(pt[2] - p0[2]) / scale0;
        worst = worst.max(dp);
    }
    let steps = p.stats().steps;
    let mut flat = p.integrator().positions().to_vec();
    flat.extend_from_slice(p.integrator().velocities());
    (flat, steps, worst)
}

/// 1a + 2: momentum conservation with J2 on over 100 yr (NUM-019), and the same run twice is
/// bitwise identical (TST-005).
///
/// Verifies: NUM-019, TST-005
#[test]
fn momentum_is_conserved_with_j2_and_runs_are_bitwise_reproducible() {
    let years = 100.0; // lint: allow-literal(run span, WP12 plan 1a)
    let started = std::time::Instant::now();
    let (a, steps, worst) = wp12_model_run(years, 100);
    let wall = started.elapsed().as_secs_f64();
    let ulps = worst / (f64::EPSILON * (steps as f64).sqrt());
    println!(
        "1a: {years} yr, {steps} IAS15 steps ({wall:.1} s): max |P(t) - P(0)| / sum GM|v| = {worst:.3e} \
         = {ulps:.3} eps sqrt(N_steps)"
    );
    vectors()
        .get("NBODY-MOMENTUM-ROUNDOFF-ULPS")
        .unwrap()
        .check(ulps)
        .unwrap();
    let (b, steps_b, _) = wp12_model_run(years, 100);
    let differing = a
        .iter()
        .zip(&b)
        .filter(|(p, q)| p.to_bits() != q.to_bits())
        .count()
        + usize::from(steps != steps_b);
    println!(
        "2 (f64): second run: {differing} differing words of {} (+ step count {steps} vs {steps_b})",
        a.len()
    );
    vectors()
        .get("NBODY-TST005-BITWISE-DIFF")
        .unwrap()
        .check(differing as f64)
        .unwrap();
}

/// 2 (double-double): a 1-yr double-double run twice is bitwise identical (TST-005).
///
/// Verifies: TST-005
#[test]
fn double_double_runs_are_bitwise_reproducible() {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<DoubleDouble>::from_header(&h, &prov).unwrap();
    let run = || {
        let m = model::<DoubleDouble>(&de, &ids(false), Moon::Geocentric, true);
        let mut p = propagator(m, &ic, Ias15Config::default());
        let end = epochs(ic.epoch(), ferroastro_core::time::DAYS_PER_JULIAN_YEAR, 1)[0];
        p.advance_to(end).unwrap();
        let mut words: Vec<u64> = Vec::new();
        for z in p
            .integrator()
            .positions()
            .iter()
            .chain(p.integrator().velocities())
        {
            let (hi, lo) = z.to_parts();
            words.extend([hi.to_bits(), lo.to_bits()]);
        }
        (words, p.stats().steps)
    };
    let (a, sa) = run();
    let (b, sb) = run();
    let differing = a.iter().zip(&b).filter(|(p, q)| p != q).count() + usize::from(sa != sb);
    println!(
        "2 (double-double, 1 yr, {sa} steps): {differing} differing words of {}",
        a.len()
    );
    vectors()
        .get("NBODY-TST005-BITWISE-DIFF")
        .unwrap()
        .check(differing as f64)
        .unwrap();
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

fn pos<C: ferroastro_core::frames::Center>(s: &State<Icrf, C>) -> [f64; 3] {
    s.position()
        .components()
        .map(ferroastro_core::units::Length::km)
}

#[derive(Default, Clone, Copy)]
struct Row {
    bary: f64,
    helio: f64,
    final_bary: f64,
}

/// One 10-yr run against de440.bsp; returns the per-body rows (body order of `ids`) and the
/// geocentric-Moon row.
fn residuals(with_pluto: bool, spk: &Spk<'_>) -> (Vec<(NaifId, Row)>, Row, u64) {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let body_ids = ids(with_pluto);
    let m = model::<f64>(&de, &body_ids, Moon::Geocentric, true);
    let mut p = propagator(m, &ic, Ias15Config::default());
    let mut rows = vec![Row::default(); body_ids.len()];
    let mut moon_row = Row::default();
    let samples = 365; // lint: allow-literal(10-d sampling over 10 yr)
    for e in epochs(ic.epoch(), 10.0, samples) {
        p.advance_to(e).unwrap();
        let s = p.state().unwrap();
        let sun = pos(&s.state(NaifId::SUN).unwrap());
        let sun_ref = pos(&spk.state::<Barycentric, f64>(NaifId::SUN, e).unwrap());
        for (k, &id) in body_ids.iter().enumerate() {
            let got = pos(&s.state(id).unwrap());
            let want = pos(&spk.state::<Barycentric, f64>(id, e).unwrap());
            let db = norm([got[0] - want[0], got[1] - want[1], got[2] - want[2]]);
            let dh = norm([
                (got[0] - sun[0]) - (want[0] - sun_ref[0]),
                (got[1] - sun[1]) - (want[1] - sun_ref[1]),
                (got[2] - sun[2]) - (want[2] - sun_ref[2]),
            ]);
            rows[k].bary = rows[k].bary.max(db);
            rows[k].helio = rows[k].helio.max(dh);
            rows[k].final_bary = db;
        }
        let em = p.earth_moon().unwrap();
        let got = pos(&em.moon);
        let want = pos(&spk.state::<Geocentric, f64>(NaifId::MOON, e).unwrap());
        let d = norm([got[0] - want[0], got[1] - want[1], got[2] - want[2]]);
        moon_row.bary = moon_row.bary.max(d);
        moon_row.final_bary = d;
    }
    (
        body_ids.into_iter().zip(rows).collect(),
        moon_row,
        p.stats().steps,
    )
}

/// 4: Newtonian + J2 vs de440.bsp over 1969-06-28 → +10 yr (real data). The residual table is
/// the evidence; only the outer planets carry a sanity bound.
///
/// Verifies: NUM-019, NUM-009, NUM-008
#[test]
fn newtonian_j2_ten_years_from_header_ics_vs_de440_bsp() {
    let (bsp, _) = cached("de440-bsp");
    let spk = Spk::parse(&bsp).unwrap();
    let outer = [
        NaifId::JUPITER_BARYCENTER,
        NaifId::SATURN_BARYCENTER,
        NaifId::URANUS_BARYCENTER,
        NaifId::NEPTUNE_BARYCENTER,
    ];
    let mut asserted = 0.0_f64;
    for with_pluto in [false, true] {
        let (rows, moon, steps) = residuals(with_pluto, &spk);
        println!(
            "4: Newtonian + solar J2 (DE440 J2SUN/ASUN, IAU 2015 pole), header.440 ICs, IAS15 \
             default, {steps} steps, 365 samples every 10 d; {}",
            if with_pluto {
                "WITH Pluto (reported only)"
            } else {
                "Sun + 8 planets + Moon (asserted set)"
            }
        );
        println!(
            "  {:<16} {:>14} {:>14} {:>14}",
            "body", "max |dr| bary", "max |dr| helio", "final |dr| bary"
        );
        for (id, r) in &rows {
            println!(
                "  {:<16} {:>11.3} km {:>11.3} km {:>11.3} km",
                name(*id),
                r.bary,
                r.helio,
                r.final_bary
            );
            if !with_pluto && outer.contains(id) {
                asserted = asserted.max(r.bary);
            }
        }
        println!(
            "  {:<16} {:>11.3} km {:>14} {:>11.3} km",
            "Moon - Earth", moon.bary, "-", moon.final_bary
        );
    }
    println!("4: asserted outer-planet max barycentric residual {asserted:.3} km");
    vectors()
        .get("NBODY-DE440-OUTER-PLANETS-KM")
        .unwrap()
        .check(asserted)
        .unwrap();
}
