// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP12 check 3 (NUM-008): the geocentric-Moon formulation (DE variables: EMB + Moon − Earth,
//! Taylor differential) against the barycentric formulation, Sun + 8 planets + Moon, Newtonian +
//! solar J2, 10 yr from the header.440 ICs (PR tier, pinned JPL cache).
//!
//! (a) both formulations in double-double at IAS15 ε = 1e-12 agree on Moon − Earth;
//! (b) the round-off error of each f64 run (minus its own double-double truth) is measured, and
//!     the gain factor barycentric / geocentric is reported (asserted > 1);
//! (c) the two f64 runs agree to within twice the barycentric run's own round-off error.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{Moon, epochs, header, ids, model, propagator, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::{Barycentric, Icrf};
use ferroastro_ephem::NaifId;
use ferroastro_integrators::Ias15Config;
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::{InitialConditions, SystemState};

/// Moon − Earth (km) of a barycentric system, computed in `R`.
fn rho_of<R: Real>(s: &SystemState<Icrf, Barycentric, R>) -> [R; 3] {
    let e = s.state(NaifId::EARTH).unwrap().position().components();
    let m = s.state(NaifId::MOON).unwrap().position().components();
    [0, 1, 2].map(|k| m[k].km() - e[k].km())
}

/// Moon − Earth every 10 d over 10 yr, in `R`, and the step count.
fn run<R: Real>(moon: Moon, epsilon: f64) -> (Vec<[R; 3]>, u64) {
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<R>::from_header(&h, &prov).unwrap();
    let m = model::<R>(&de, &ids(false), moon, true);
    let cfg = Ias15Config {
        epsilon,
        ..Ias15Config::default()
    };
    let mut p = propagator(m, &ic, cfg);
    let mut out = Vec::new();
    for e in epochs(ic.epoch(), 10.0, 365) {
        p.advance_to(e).unwrap();
        out.push(match moon {
            Moon::Geocentric => {
                let em = p.earth_moon().unwrap();
                em.moon
                    .position()
                    .components()
                    .map(ferroastro_core::units::Length::km)
            }
            Moon::Barycentric => rho_of(&p.state().unwrap()),
        });
    }
    (out, p.stats().steps)
}

/// max over samples of |a − b| (km), both converted to f64 after the subtraction in DD.
fn max_diff_dd(a: &[[DoubleDouble; 3]], b: &[[DoubleDouble; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let d = [0, 1, 2].map(|k| (p[k] - q[k]).to_f64());
            d[0].hypot(d[1]).hypot(d[2])
        })
        .fold(0.0, f64::max)
}

fn max_diff_f64_dd(a: &[[f64; 3]], b: &[[DoubleDouble; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| {
            let d = [0, 1, 2].map(|k| (DoubleDouble::from_f64(p[k]) - q[k]).to_f64());
            d[0].hypot(d[1]).hypot(d[2])
        })
        .fold(0.0, f64::max)
}

fn max_diff_f64(a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p[0] - q[0]).hypot(p[1] - q[1]).hypot(p[2] - q[2]))
        .fold(0.0, f64::max)
}

/// Verifies: NUM-008
#[test]
fn geocentric_and_barycentric_moon_agree_and_geocentric_keeps_more_bits() {
    let truth_eps = 1e-12; // lint: allow-literal(IAS15 epsilon of the DD truth runs, WP12 plan 3a)
    let default_eps = Ias15Config::default().epsilon;
    let started = std::time::Instant::now();
    let ((geo_dd, s_gd), (bary_dd, s_bd), (geo, s_g), (bary, s_b)) = std::thread::scope(|sc| {
        let a = sc.spawn(|| run::<DoubleDouble>(Moon::Geocentric, truth_eps));
        let b = sc.spawn(|| run::<DoubleDouble>(Moon::Barycentric, truth_eps));
        let c = sc.spawn(|| run::<f64>(Moon::Geocentric, default_eps));
        let d = sc.spawn(|| run::<f64>(Moon::Barycentric, default_eps));
        (
            a.join().unwrap(),
            b.join().unwrap(),
            c.join().unwrap(),
            d.join().unwrap(),
        )
    });
    println!(
        "3: 10 yr, 365 samples; steps: DD geo {s_gd}, DD bary {s_bd} (eps {truth_eps:e}); f64 geo {s_g}, f64 bary {s_b} (eps {default_eps:e}); wall {:.1} s",
        started.elapsed().as_secs_f64()
    );
    let vs = vectors();
    // (a) formulation agreement in double-double.
    let a = max_diff_dd(&geo_dd, &bary_dd);
    println!("3a: max |rho_geo,DD - rho_bary,DD| = {a:.3e} km");
    vs.get("NBODY-MOON-GEO-VS-BARY-DD-KM")
        .unwrap()
        .check(a)
        .unwrap();
    // (b) round-off of each f64 formulation against its own DD truth.
    let err_geo = max_diff_f64_dd(&geo, &geo_dd);
    let err_bary = max_diff_f64_dd(&bary, &bary_dd);
    let gain = err_bary / err_geo;
    println!(
        "3b: f64 round-off of Moon - Earth over 10 yr: geocentric {err_geo:.3e} km, barycentric {err_bary:.3e} km; gain factor {gain:.2}"
    );
    let min_gain = vs
        .get("NBODY-MOON-ROUNDOFF-GAIN-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(gain > min_gain, "gain {gain} not above {min_gain}");
    // (c) the two f64 runs agree within twice the barycentric round-off.
    let c = max_diff_f64(&geo, &bary);
    let ratio = c / err_bary;
    println!(
        "3c: max |rho_geo,f64 - rho_bary,f64| = {c:.3e} km = {ratio:.3} x the barycentric round-off"
    );
    let max_ratio = vs
        .get("NBODY-MOON-F64-AGREEMENT-FACTOR")
        .unwrap()
        .value()
        .unwrap();
    assert!(ratio <= max_ratio, "ratio {ratio} above {max_ratio}");
}
