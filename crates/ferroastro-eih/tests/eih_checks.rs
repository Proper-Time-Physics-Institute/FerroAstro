// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP19 EIH checks on the real DE440 initial conditions (PR tier: header.440 from the pinned
//! JPL cache, CI-005, re-verified against its SHA-256). The bounds were pre-registered in the WP19
//! plan (IMPLEMENTATION-NOTES) and live in `tests/vectors/eih.toml`:
//!
//! * C2: the R5 → R3 limit numerically (massless Mercury, Sun at rest);
//! * C5a/b: one fixed-point iteration of the RHS accelerations changes them at O(c⁻⁴) (NUM-018);
//! * C5c: 10-yr runs with and without the iteration;
//! * C6: the relativistic barycentre (Park et al. 2021 Eqs. 1–2) is conserved by the EIH
//!   equations, while the Newtonian barycentre oscillates.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{Gr, epochs, header, ids, model, name, norm, propagator, sub, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::{Barycentric, Icrf, State};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_eih::{BarycentreBody, Eih, RhsAcceleration, relativistic_barycentre};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    CentralTerm, ForceTerm, MoonVariables, NBody, NBodyModel, NewtonianMutual, SchwarzschildPpn,
    TestParticle,
};
use ferroastro_integrators::{Ias15Config, SecondOrderSystem};
use ferroastro_system::{Catalogue, InitialConditions, SystemState, Tier};

/// PHY-006's range edge, used as a test point of the limit (not a physical value).
const PPN_OFFSET: f64 = 1e-3; // lint: allow-literal(PHY-006 range bound |beta-1|, |gamma-1| <= 1e-3)
/// Samples of the 10-yr runs: every 10 d.
const SAMPLES: u32 = 365; // lint: allow-literal(10-d sampling over 10 yr)
const STEP_DAYS: f64 = 10.0; // lint: allow-literal(output cadence, days)
/// Relative and ulp slack of the exact triangle bound C5a (round-off of the bound itself).
const BOUND_REL_SLACK: f64 = 1e-9; // lint: allow-literal(round-off slack of an exact inequality)
const BOUND_ULPS: f64 = 8.0; // lint: allow-literal(round-off slack of an exact inequality)

fn pos<C: ferroastro_core::frames::Center>(s: &State<Icrf, C>) -> [f64; 3] {
    s.position()
        .components()
        .map(ferroastro_core::units::Length::km)
}

fn vel<C: ferroastro_core::frames::Center>(s: &State<Icrf, C>) -> [f64; 3] {
    s.velocity().map(ferroastro_core::units::Velocity::km_per_s)
}

/// C2: the R5 → R3 limit numerically. Mercury (GM = 0) at its header.440 state relative to the
/// Sun, the Sun at rest at the origin. Newton + EIH vs the test particle with Newton + R3, at
/// β = γ = 1 and at (1 + 1e-3, 1 − 1e-3); the correction parts alone; and the Sun gets exactly no
/// correction from a massless partner.
///
/// Verifies: NUM-017
#[test]
fn eih_reduces_to_the_schwarzschild_test_particle_numerically() {
    let de = De440::load().unwrap();
    let (h, _) = header();
    let v = |n: &str| h.value(n).unwrap();
    let au = v("AU");
    let r0 = ["X", "Y", "Z"].map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au);
    let v0 = ["XD", "YD", "ZD"]
        .map(|k| (v(&format!("{k}1")) - v(&format!("{k}S"))) * au / SECONDS_PER_DAY);
    let mu = de.gm_sun();
    let c = de.speed_of_light();
    let bodies = [
        NBody {
            id: NaifId::SUN,
            gm: mu.km3_per_s2(),
            tier: Tier::Major,
        },
        NBody {
            id: NaifId::MERCURY_BARYCENTER,
            gm: 0.0,
            tier: Tier::Major,
        },
    ];
    // Component-major flat vectors of 2 bodies: [x_S, x_M, y_S, y_M, z_S, z_M].
    let x = [0.0, r0[0], 0.0, r0[1], 0.0, r0[2]];
    let w = [0.0, v0[0], 0.0, v0[1], 0.0, v0[2]];
    let accel = |terms: Vec<Box<dyn ForceTerm<Icrf>>>| {
        let m = NBodyModel::<Icrf>::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
        let mut a = [0.0; 6];
        m.acceleration(0.0, &x, &w, &mut a).unwrap();
        ([a[1], a[3], a[5]], [a[0], a[2], a[4]])
    };
    for (beta, gamma) in [(1.0, 1.0), (1.0 + PPN_OFFSET, 1.0 - PPN_OFFSET)] {
        let eih = || Eih::new(c, beta, gamma, RhsAcceleration::Newtonian).unwrap();
        let (total, _) = accel(vec![Box::new(NewtonianMutual), Box::new(eih())]);
        let (corr, sun_corr) = accel(vec![Box::new(eih())]);
        let r3 = SchwarzschildPpn::new(mu, c, beta, gamma).unwrap();
        let tp = TestParticle::<Icrf>::new(mu, vec![Box::new(r3)]).unwrap();
        let mut tp_total = [0.0; 3];
        tp.acceleration(0.0, &r0, &v0, &mut tp_total).unwrap();
        let mut tp_corr = [0.0; 3];
        CentralTerm::<Icrf>::add_acceleration(&r3, r0, v0, &mut tp_corr);
        let rel_total = norm(sub(total, tp_total)) / norm(tp_total);
        let rel_corr = norm(sub(corr, tp_corr)) / norm(tp_corr);
        println!(
            "C2 beta = {beta}, gamma = {gamma}: |a| = {:.6e} km/s^2, |delta a| = {:.6e} km/s^2 \
             ({:.3e} of |a|); total rel diff {rel_total:.3e}; correction rel diff {rel_corr:.3e}; \
             Sun correction {sun_corr:?}",
            norm(tp_total),
            norm(tp_corr),
            norm(tp_corr) / norm(tp_total)
        );
        vectors()
            .get("EIH-R3-LIMIT-TOTAL-REL")
            .unwrap()
            .check(rel_total)
            .unwrap();
        vectors()
            .get("EIH-R3-LIMIT-CORRECTION-REL")
            .unwrap()
            .check(rel_corr)
            .unwrap();
        assert!(sun_corr.iter().all(|z| z.to_bits() == 0), "{sun_corr:?}");
    }
}

/// The c⁻² corrections of every body (barycentric Earth and Moon), the Newtonian accelerations,
/// positions and GMs, at the header.440 epoch.
struct Snapshot {
    ids: Vec<NaifId>,
    gm: Vec<f64>,
    r: Vec<[f64; 3]>,
    newton: Vec<[f64; 3]>,
    default: Vec<[f64; 3]>,
    iterated: Vec<[f64; 3]>,
}

fn snapshot(de: &De440) -> Snapshot {
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let ids = ids();
    let cat = Catalogue::de440_major(de);
    let gm: Vec<f64> = ids
        .iter()
        .map(|&id| cat.gm(id).unwrap().km3_per_s2())
        .collect();
    let bodies: Vec<NBody> = ids
        .iter()
        .zip(&gm)
        .map(|(&id, &g)| NBody {
            id,
            gm: g,
            tier: Tier::Major,
        })
        .collect();
    let c = de.speed_of_light();
    let eval = |term: Box<dyn ForceTerm<Icrf>>| {
        let m = NBodyModel::<Icrf>::new(&bodies, MoonVariables::Barycentric, vec![term]).unwrap();
        let (x, v) = m.pack(ic.states(), None).unwrap();
        let mut a = vec![0.0; x.len()];
        m.acceleration(0.0, &x, &v, &mut a).unwrap();
        let n = ids.len();
        let per: Vec<[f64; 3]> = (0..n).map(|i| [a[i], a[n + i], a[2 * n + i]]).collect();
        let r: Vec<[f64; 3]> = (0..n).map(|i| [x[i], x[n + i], x[2 * n + i]]).collect();
        (per, r)
    };
    let (newton, r) = eval(Box::new(NewtonianMutual));
    let (default, _) = eval(Box::new(
        Eih::new(c, 1.0, 1.0, RhsAcceleration::Newtonian).unwrap(),
    ));
    let (iterated, _) = eval(Box::new(
        Eih::new(c, 1.0, 1.0, RhsAcceleration::Iterated).unwrap(),
    ));
    Snapshot {
        ids,
        gm,
        r,
        newton,
        default,
        iterated,
    }
}

/// C5a/b (NUM-018): the iterated-minus-default difference at the header.440 epoch, Sun + 8
/// planets + Moon, β = γ = 1. (a) It obeys the exact triangle bound of Eq. 27's linear
/// dependence on `a_j`. (b) Relative to the Newtonian acceleration it is O(ε²), ε = max U/c².
///
/// Verifies: NUM-018
#[test]
fn rhs_iteration_changes_the_accelerations_at_order_c_minus_four() {
    let de = De440::load().unwrap();
    let s = snapshot(&de);
    let c = de.speed_of_light().km_per_s();
    let c2 = c * c;
    let n = s.ids.len();
    let dist = |i: usize, j: usize| norm(sub(s.r[i], s.r[j]));
    let u: Vec<f64> = (0..n)
        .map(|i| {
            (0..n)
                .filter(|&k| k != i)
                .map(|k| s.gm[k] / dist(i, k))
                .sum()
        })
        .collect();
    let eps = u.iter().copied().fold(0.0, f64::max) / c2;
    println!(
        "C5: eps = max U_i/c^2 = {eps:.4e}; eps^2 = {:.4e}",
        eps * eps
    );
    println!(
        "  {:<14} {:>12} {:>12} {:>12} {:>12} {:>14}",
        "body", "|a_N|", "|delta a|", "|Delta a|", "bound", "|Da|/(eps2 aN)"
    );
    let mut worst_factor: f64 = 0.0;
    for i in 0..n {
        let d = sub(s.iterated[i], s.default[i]);
        let dn = norm(d);
        let bound: f64 = (0..n)
            .filter(|&j| j != i)
            .map(|j| 4.0 * s.gm[j] * norm(s.default[j]) / (c2 * dist(i, j))) // lint: allow-literal(2 + 2 gamma at gamma = 1)
            .sum();
        let slack = bound.mul_add(
            BOUND_REL_SLACK,
            BOUND_ULPS * f64::EPSILON * norm(s.default[i]),
        );
        let factor = dn / (eps * eps * norm(s.newton[i]));
        worst_factor = worst_factor.max(factor);
        println!(
            "  {:<14} {:>12.4e} {:>12.4e} {:>12.4e} {:>12.4e} {:>14.4e}",
            name(s.ids[i]),
            norm(s.newton[i]),
            norm(s.default[i]),
            dn,
            bound,
            factor
        );
        assert!(
            dn <= bound + slack,
            "{}: |Delta a| {dn:e} > bound {bound:e}",
            name(s.ids[i])
        );
    }
    println!("C5b: max_i |Delta a_i| / (eps^2 |a_N,i|) = {worst_factor:.4e}");
    vectors()
        .get("EIH-ITERATION-C4-FACTOR")
        .unwrap()
        .check(worst_factor)
        .unwrap();
}

/// A 10-yr EIH run (β = γ = 1, geocentric Moon, no J2) from header.440; the barycentric system
/// state at the epoch and at every sample.
fn eih_run(rhs: RhsAcceleration) -> (Vec<f64>, Vec<SystemState<Icrf, Barycentric>>, u64) {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let m = model(&de, &ids(), true, Gr::Eih(1.0, 1.0, rhs), false);
    println!("{}", m.describe());
    let mut p = propagator(m, &ic, Ias15Config::default());
    let mut t = vec![0.0];
    let mut states = vec![p.state().unwrap()];
    for (k, e) in epochs(ic.epoch(), STEP_DAYS, SAMPLES)
        .into_iter()
        .enumerate()
    {
        p.advance_to(e).unwrap();
        t.push((k + 1) as f64 * STEP_DAYS * SECONDS_PER_DAY);
        states.push(p.state().unwrap());
    }
    (t, states, p.stats().steps)
}

/// C5c (NUM-018): 10-yr runs with Newtonian and with iterated RHS accelerations stay together.
///
/// Verifies: NUM-018
#[test]
fn iterated_and_default_runs_agree_over_ten_years() {
    let (_, a, steps_a) = eih_run(RhsAcceleration::Newtonian);
    let (_, b, steps_b) = eih_run(RhsAcceleration::Iterated);
    let mut worst: f64 = 0.0;
    for (sa, sb) in a.iter().zip(&b) {
        let d = sub(
            pos(&sa.state(NaifId::MERCURY_BARYCENTER).unwrap()),
            pos(&sb.state(NaifId::MERCURY_BARYCENTER).unwrap()),
        );
        worst = worst.max(norm(d));
    }
    println!(
        "C5c: Mercury max |r_iterated - r_default| over 10 yr = {worst:.4e} km ({steps_a} / \
         {steps_b} IAS15 steps)"
    );
    vectors()
        .get("EIH-ITERATED-RUN-MERCURY-KM")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// Largest distance of the points `p(t)` from their least-squares straight line.
fn max_linear_deviation(t: &[f64], p: &[[f64; 3]]) -> f64 {
    let n = t.len() as f64;
    let tm = t.iter().sum::<f64>() / n;
    let stt: f64 = t.iter().map(|x| (x - tm) * (x - tm)).sum();
    let mut fit = [(0.0, 0.0); 3];
    for (k, f) in fit.iter_mut().enumerate() {
        let pm = p.iter().map(|q| q[k]).sum::<f64>() / n;
        let stp: f64 = t.iter().zip(p).map(|(x, q)| (x - tm) * (q[k] - pm)).sum();
        let slope = stp / stt;
        *f = ((-slope).mul_add(tm, pm), slope);
    }
    t.iter()
        .zip(p)
        .map(|(&x, q)| norm([0, 1, 2].map(|k| q[k] - fit[k].1.mul_add(x, fit[k].0))))
        .fold(0.0, f64::max)
}

/// C6 (NUM-018, NUM-017): under EIH (β = γ = 1) the relativistic barycentre of Park et al. 2021
/// Eqs. 1–2 moves on a straight line to round-off, while the Newtonian barycentre does not.
///
/// Verifies: NUM-018, NUM-017
#[test]
fn eih_conserves_the_relativistic_barycentre() {
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let c = de.speed_of_light();
    let (t, states, steps) = eih_run(RhsAcceleration::Newtonian);
    let ids = ids();
    let gm: Vec<f64> = ids
        .iter()
        .map(|&id| cat.gm(id).unwrap().km3_per_s2())
        .collect();
    let total: f64 = gm.iter().sum();
    let mut rel = Vec::with_capacity(states.len());
    let mut newt = Vec::with_capacity(states.len());
    for s in &states {
        let bodies: Vec<BarycentreBody> = ids
            .iter()
            .zip(&gm)
            .map(|(&id, &g)| {
                let st = s.state(id).unwrap();
                BarycentreBody {
                    gm: g,
                    position: pos(&st),
                    velocity: vel(&st),
                }
            })
            .collect();
        rel.push(relativistic_barycentre(&bodies, c).unwrap().0);
        let mut r = [0.0; 3];
        for b in &bodies {
            for (rk, pk) in r.iter_mut().zip(b.position) {
                *rk = b.gm.mul_add(pk, *rk);
            }
        }
        newt.push(r.map(|z| z / total));
    }
    let d_rel = max_linear_deviation(&t, &rel);
    let d_newt = max_linear_deviation(&t, &newt);
    println!(
        "C6: 10 yr, {steps} IAS15 steps, {} samples: max deviation from a straight line: \
         relativistic barycentre {d_rel:.4e} km, Newtonian barycentre {d_newt:.4e} km \
         (ratio {:.3e})",
        t.len(),
        d_newt / d_rel
    );
    vectors()
        .get("EIH-REL-SSB-DEVIATION-KM")
        .unwrap()
        .check(d_rel)
        .unwrap();
    let min_ratio = vectors()
        .get("EIH-NEWTON-SSB-RATIO-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(d_newt / d_rel >= min_ratio, "ratio {}", d_newt / d_rel);
}
