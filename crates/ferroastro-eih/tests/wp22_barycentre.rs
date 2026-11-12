// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP22 checks E5 and E6 (PR tier, NUM-018; OI-75 decision of 2026-10-21, "Include dμ*/dt"): the
//! time derivative of μ* (Park et al. 2021 Eq. 2) and the barycentre velocity condition
//! Σ(μ̇ᵢ* rᵢ + μᵢ* vᵢ) = 0 (DE430 Eq. 3; Subbotin et al. 2023 Eq. 5). Bounds pre-registered in the
//! WP22 plan (IMPLEMENTATION-NOTES), kept in `tests/vectors/eih.toml`.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{epochs, header, ids, norm, sub, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::{Barycentric, Icrf, Position, State};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::{Length, TimeSpan, Velocity};
use ferroastro_eih::{
    BarycentreBody, Eih, RhsAcceleration, mu_star, mu_star_rate, relativistic_barycentre,
    relativistic_barycentre_with_rate,
};
use ferroastro_forces::{
    ForceTerm, MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::{Catalogue, InitialConditions, SystemState, Tier};

type Dd = DoubleDouble;

/// E5 half-width of the central difference (s), fixed in the WP22 plan.
const DELTA_S: f64 = 60.0; // lint: allow-literal(difference half-width fixed in the WP22 plan E5)
/// E6 samples: every 10 d over 10 yr.
const SAMPLES: u32 = 365; // lint: allow-literal(10-d sampling over 10 yr)
const STEP_DAYS: f64 = 10.0; // lint: allow-literal(output cadence, days)
/// Re-barycentring passes (μ* depends on the velocities being corrected).
const PASSES: usize = 2;

/// Newtonian accelerations of every body by all the others (km/s²).
fn newtonian<R: Real>(b: &[BarycentreBody<R>]) -> Vec<[R; 3]> {
    b.iter()
        .enumerate()
        .map(|(i, p)| {
            let mut a = [R::ZERO; 3];
            for (j, q) in b.iter().enumerate() {
                if i == j {
                    continue;
                }
                let d = [0, 1, 2].map(|k| q.position[k] - p.position[k]);
                let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                let s = q.gm / (r2 * r2.sqrt());
                for (ak, dk) in a.iter_mut().zip(d) {
                    *ak += s * dk;
                }
            }
            a
        })
        .collect()
}

/// E5: `mu_star_rate` against a central difference of `mu_star` along the second-order Taylor
/// flow, double-double, header.440 Sun + 8 planets + Moon (barycentric).
///
/// Verifies: NUM-018
#[test]
fn mu_star_rate_matches_a_central_difference_of_mu_star() {
    let (h, prov) = header();
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let ic = InitialConditions::<Dd>::from_header(&h, &prov).unwrap();
    let bodies: Vec<BarycentreBody<Dd>> = ids()
        .iter()
        .map(|&id| {
            let st = ic.states().state(id).unwrap();
            BarycentreBody {
                gm: Dd::from_f64(cat.gm(id).unwrap().km3_per_s2()),
                position: st.position().components().map(Length::km),
                velocity: st.velocity().map(Velocity::km_per_s),
            }
        })
        .collect();
    let c = Velocity::from_km_per_s(Dd::from_f64(de.speed_of_light().km_per_s())).unwrap();
    let rate = mu_star_rate(&bodies, c).unwrap();
    let acc = newtonian(&bodies);
    let shifted = |s: Dd| -> Vec<BarycentreBody<Dd>> {
        bodies
            .iter()
            .zip(&acc)
            .map(|(b, a)| BarycentreBody {
                gm: b.gm,
                position: [0, 1, 2]
                    .map(|k| b.position[k] + b.velocity[k] * s + Dd::HALF * a[k] * s * s),
                velocity: [0, 1, 2].map(|k| b.velocity[k] + a[k] * s),
            })
            .collect()
    };
    let d = Dd::from_f64(DELTA_S);
    let plus = mu_star(&shifted(d), c).unwrap();
    let minus = mu_star(&shifted(-d), c).unwrap();
    let scale = rate.iter().map(|z| z.to_f64().abs()).fold(0.0, f64::max);
    let mut worst = 0.0_f64;
    for (i, id) in ids().iter().enumerate() {
        let fd = (plus[i] - minus[i]) / (d + d);
        let e = (rate[i] - fd).to_f64().abs() / scale;
        println!(
            "E5 {:>4}: mu*' = {:+.6e}, difference {:+.6e} km^3/s^3, |diff|/max = {e:.2e}",
            id.0,
            rate[i].to_f64(),
            fd.to_f64()
        );
        worst = worst.max(e);
    }
    println!("E5: max |mu*' - difference| / max |mu*'| at delta = {DELTA_S} s: {worst:.3e}");
    assert!(scale > 0.0);
    vectors()
        .get("EIH-MUSTAR-RATE-FD-REL")
        .unwrap()
        .check(worst)
        .unwrap();
}

fn typed(p: [f64; 3], w: [f64; 3]) -> State<Icrf, Barycentric> {
    let l = |z: f64| Length::from_km(z).unwrap();
    let s = |z: f64| Velocity::from_km_per_s(z).unwrap();
    State::new(Position::new(l(p[0]), l(p[1]), l(p[2])), w.map(s))
}

/// The header.440 states of `ids` re-barycentred with (`with_rate`) or without the μ̇* term of
/// the velocity condition; returns the GMs and the states.
fn rebarycentred(with_rate: bool) -> (Vec<f64>, SystemState<Icrf, Barycentric>) {
    let (h, prov) = header();
    let de = De440::load().unwrap();
    let cat = Catalogue::de440_major(&de);
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let c = de.speed_of_light();
    let ids = ids();
    let gm: Vec<f64> = ids
        .iter()
        .map(|&id| cat.gm(id).unwrap().km3_per_s2())
        .collect();
    let mut bodies: Vec<BarycentreBody> = ids
        .iter()
        .zip(&gm)
        .map(|(&id, &g)| {
            let st = ic.states().state(id).unwrap();
            BarycentreBody {
                gm: g,
                position: st.position().components().map(Length::km),
                velocity: st.velocity().map(Velocity::km_per_s),
            }
        })
        .collect();
    for _ in 0..PASSES {
        let (r, w) = if with_rate {
            relativistic_barycentre_with_rate(&bodies, c).unwrap()
        } else {
            relativistic_barycentre(&bodies, c).unwrap()
        };
        for b in &mut bodies {
            b.position = sub(b.position, r);
            b.velocity = sub(b.velocity, w);
        }
    }
    let mut states = SystemState::new();
    for (&id, b) in ids.iter().zip(&bodies) {
        states.push(id, &typed(b.position, b.velocity)).unwrap();
    }
    (gm, states)
}

/// max_t |R*(t) − R*(0)| over a 10-yr EIH run (Sun + 8 planets + Moon, all Major, barycentric
/// Moon) from the re-barycentred header.440 states.
fn drift(with_rate: bool) -> f64 {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let c = de.speed_of_light();
    let ids = ids();
    let (gm, states) = rebarycentred(with_rate);
    let bodies: Vec<NBody> = ids
        .iter()
        .zip(&gm)
        .map(|(&id, &g)| NBody {
            id,
            gm: g,
            tier: Tier::Major,
        })
        .collect();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutual),
        Box::new(Eih::new(c, 1.0, 1.0, RhsAcceleration::Newtonian).unwrap()),
    ];
    let model = NBodyModel::<Icrf>::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    let dt0 = TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap();
    let mut p = NBodyPropagator::new(
        model,
        &states,
        None,
        ic.epoch(),
        dt0,
        Ias15Config::default(),
    )
    .unwrap();
    let centre = |s: &SystemState<Icrf, Barycentric>| -> [f64; 3] {
        let b: Vec<BarycentreBody> = ids
            .iter()
            .zip(&gm)
            .map(|(&id, &g)| {
                let st = s.state(id).unwrap();
                BarycentreBody {
                    gm: g,
                    position: st.position().components().map(Length::km),
                    velocity: st.velocity().map(Velocity::km_per_s),
                }
            })
            .collect();
        relativistic_barycentre(&b, c).unwrap().0
    };
    let r0 = centre(&p.state().unwrap());
    let mut worst = 0.0_f64;
    for e in epochs(ic.epoch(), STEP_DAYS, SAMPLES) {
        p.advance_to(e).unwrap();
        worst = worst.max(norm(sub(centre(&p.state().unwrap()), r0)));
    }
    println!(
        "E6 ({}): |R*(0)| = {:.3e} km; max |R*(t) - R*(0)| over 10 yr = {worst:.4e} km ({} steps)",
        if with_rate {
            "sum(mu*' r + mu* v) = 0"
        } else {
            "sum(mu* v) = 0 only"
        },
        norm(r0),
        p.stats().steps
    );
    worst
}

/// E6: with the full velocity condition the relativistic barycentre stays put; without the μ̇*
/// term it drifts (DE430 pp. 5–6: about 0.5 m/century for DE405).
///
/// Verifies: NUM-018
#[test]
fn the_mu_star_rate_term_stops_the_barycentre_drift() {
    let with = drift(true);
    let without = drift(false);
    println!("E6: drift ratio without / with = {:.3e}", without / with);
    let vs = vectors();
    vs.get("EIH-REL-SSB-DRIFT-10YR-KM")
        .unwrap()
        .check(with)
        .unwrap();
    let min_ratio = vs
        .get("EIH-MUSTAR-RATE-DRIFT-RATIO-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(without / with >= min_ratio, "ratio {}", without / with);
}
