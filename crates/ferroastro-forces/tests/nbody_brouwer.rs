// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP12 check 1b (NUM-023 with NUM-019's J2 on): Brouwer-law growth of the energy error of an
//! N-body run with solar J2 and its reaction, IAS15 in f64.
//!
//! Model: Sun, Mercury, Venus, Earth–Moon barycentre (one body), Mars … Neptune system
//! barycentres; DE440 GMs; Newtonian mutual + solar J2 with reaction (DE440 J2SUN, ASUN, IAU 2015
//! pole); header.440 ICs (the EMB from `XB`). Ensemble: 32 runs × 100 000 yr (≈ 1e9 steps in
//! total), members differ by Mercury's initial x × (1 + k·1e-13). 32 log-spaced checkpoints from
//! 10 Mercury orbits on; `ln rms(ΔE/E) = c + p ln t` must give `p` within ±0.1 of ½ (pre-registered
//! in IMPLEMENTATION-NOTES WP12). Momentum and angular-momentum errors are reported.
//!
//! Tier: nightly (`#[ignore]`, `nightly_` prefix, runs alone through the nightly profile's
//! `threads-required`). `FA_WP12_BROUWER_YEARS` / `FA_WP12_BROUWER_RUNS` make the test
//! **report-only** (no assertion), for diagnosis.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{header, vectors};
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::{Barycentric, Icrf, Position, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_core::units::{Length, TimeSpan};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual, Pole,
    SolarJ2WithReaction,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_integrators::diagnostics::loglog_slope;
use ferroastro_system::{Catalogue, InitialConditions, SystemState, Tier};

const RUNS: u32 = 32;
const YEARS: f64 = 100_000.0; // lint: allow-literal(run length, WP12 plan 1b addendum)
const CHECKPOINTS: u32 = 32;
const FIRST_MERCURY_ORBITS: f64 = 10.0;
const PERTURBATION: f64 = 1e-13; // lint: allow-literal(ensemble IC perturbation, WP12 plan 1b addendum)

fn env_f64(name: &str) -> Option<f64> {
    std::env::var(name).ok().and_then(|s| s.parse().ok())
}

fn ids() -> Vec<NaifId> {
    vec![
        NaifId::SUN,
        NaifId::MERCURY_BARYCENTER,
        NaifId::VENUS_BARYCENTER,
        NaifId::EARTH_MOON_BARYCENTER,
        NaifId::MARS_BARYCENTER,
        NaifId::JUPITER_BARYCENTER,
        NaifId::SATURN_BARYCENTER,
        NaifId::URANUS_BARYCENTER,
        NaifId::NEPTUNE_BARYCENTER,
    ]
}

fn model(de: &De440) -> NBodyModel<Icrf> {
    let cat = Catalogue::de440_major(de);
    let bodies: Vec<NBody> = ids()
        .into_iter()
        .map(|id| NBody {
            id,
            gm: if id == NaifId::EARTH_MOON_BARYCENTER {
                de.gm(Body::EarthMoon).km3_per_s2()
            } else {
                cat.gm(id).unwrap().km3_per_s2()
            },
            tier: Tier::Major,
        })
        .collect();
    let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
        .unwrap()
        .unit_vector();
    let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
        Box::new(NewtonianMutual),
        Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                de.sun_radius(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ),
    ];
    NBodyModel::new(&bodies, MoonVariables::Barycentric, terms).unwrap()
}

/// Header ICs with the EMB as one body; Mercury's x scaled by `1 + scale`.
fn initial(ic: &InitialConditions, scale: f64) -> SystemState<Icrf, Barycentric> {
    let mut s = SystemState::new();
    for id in ids() {
        let st = if id == NaifId::EARTH_MOON_BARYCENTER {
            ic.earth_moon().unwrap().emb
        } else {
            ic.states().state(id).unwrap()
        };
        let st = if id == NaifId::MERCURY_BARYCENTER {
            let [x, y, z] = st.position().components();
            let x = Length::from_km(x.km() * (1.0 + scale)).unwrap();
            State::new(Position::new(x, y, z), st.velocity())
        } else {
            st
        };
        s.push(id, &st).unwrap();
    }
    s
}

fn angular_momentum(m: &NBodyModel<Icrf>, x: &[f64], v: &[f64]) -> [f64; 3] {
    let n = m.layout().len();
    let mut l = [0.0; 3];
    for i in 0..n {
        let g = m.layout().gm(i);
        let (r, w) = (
            [x[i], x[n + i], x[2 * n + i]],
            [v[i], v[n + i], v[2 * n + i]],
        );
        l[0] += g * r[1].mul_add(w[2], -r[2] * w[1]);
        l[1] += g * r[2].mul_add(w[0], -r[0] * w[2]);
        l[2] += g * r[0].mul_add(w[1], -r[1] * w[0]);
    }
    l
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

/// One run: (relative energy, relative momentum, relative |L|) errors at the checkpoints, steps.
fn run(de: &De440, ic: &InitialConditions, k: u32, checkpoints: &[f64]) -> (Vec<[f64; 3]>, u64) {
    let m = model(de);
    let s0 = initial(ic, f64::from(k) * PERTURBATION);
    let dt0 = TimeSpan::from_seconds(SECONDS_PER_DAY).unwrap();
    let mut p =
        NBodyPropagator::new(m, &s0, None, ic.epoch(), dt0, Ias15Config::default()).unwrap();
    let e0 = p.energy_over_g().unwrap();
    let (p0, pscale) = p.momentum_over_g();
    let l0 = angular_momentum(
        p.model(),
        p.integrator().positions(),
        p.integrator().velocities(),
    );
    let mut out = Vec::with_capacity(checkpoints.len());
    for &t in checkpoints {
        let e = ic.epoch() + TimeSpan::from_seconds(t).unwrap();
        p.advance_to(e).unwrap();
        let de_rel = (p.energy_over_g().unwrap() - e0) / e0;
        let (pt, _) = p.momentum_over_g();
        let dp = norm([pt[0] - p0[0], pt[1] - p0[1], pt[2] - p0[2]]) / pscale;
        let l = angular_momentum(
            p.model(),
            p.integrator().positions(),
            p.integrator().velocities(),
        );
        let dl = (norm(l) - norm(l0)) / norm(l0);
        out.push([de_rel, dp, dl]);
    }
    (out, p.stats().steps)
}

/// Verifies: NUM-023, NUM-019, NUM-001
#[test]
#[ignore = "nightly tier: 32 x 100 000 yr N-body runs (~1e9 IAS15 steps, ~20 min on 32 threads)"]
fn nightly_nbody_brouwer_energy_slope_with_solar_j2() {
    let report_only =
        env_f64("FA_WP12_BROUWER_YEARS").is_some() || env_f64("FA_WP12_BROUWER_RUNS").is_some();
    let years = env_f64("FA_WP12_BROUWER_YEARS").unwrap_or(YEARS);
    let runs = std::env::var("FA_WP12_BROUWER_RUNS")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(RUNS);
    let de = De440::load().unwrap();
    let (h, prov) = header();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    // Mercury's period from its heliocentric header state (vis-viva, GM_Sun + GM_Mercury).
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let me = ic.states().state(NaifId::MERCURY_BARYCENTER).unwrap();
    let r = (me.position() - sun.position()).norm().km();
    let v = norm([0, 1, 2].map(|k| me.velocity()[k].km_per_s() - sun.velocity()[k].km_per_s()));
    let mu = de.gm_sun().km3_per_s2() + de.gm(Body::Mercury).km3_per_s2();
    let a = 1.0 / (2.0 / r - v * v / mu);
    let period = std::f64::consts::TAU * (a * a * a / mu).sqrt();
    let t_end = years * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY;
    let t_first = FIRST_MERCURY_ORBITS * period;
    let checkpoints: Vec<f64> = (0..CHECKPOINTS)
        .map(|j| t_first * (t_end / t_first).powf(f64::from(j) / f64::from(CHECKPOINTS - 1)))
        .collect();
    println!("{}", model(&de).describe());
    let started = std::time::Instant::now();
    let results: Vec<(Vec<[f64; 3]>, u64)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..runs)
            .map(|k| {
                let (de, ic, cps) = (&de, &ic, &checkpoints);
                sc.spawn(move || run(de, ic, k, cps))
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let wall = started.elapsed().as_secs_f64();
    let steps: u64 = results.iter().map(|r| r.1).sum();
    let nf = f64::from(runs);
    let rms = |c: usize| -> Vec<f64> {
        (0..checkpoints.len())
            .map(|j| (results.iter().map(|r| r.0[j][c].powi(2)).sum::<f64>() / nf).sqrt())
            .collect()
    };
    let mean = |c: usize, j: usize| results.iter().map(|r| r.0[j][c]).sum::<f64>() / nf;
    let (e_rms, p_rms, l_rms) = (rms(0), rms(1), rms(2));
    println!(
        "1b: {runs} runs x {years} yr, {steps} IAS15 steps in total ({:.3e}), wall {wall:.0} s; Mercury period {:.4} d",
        steps as f64,
        period / SECONDS_PER_DAY
    );
    println!("  t [yr]        rms dE/E    rms |dP|/S   rms dL/L    (mean dE/E)");
    for (j, t) in checkpoints.iter().enumerate() {
        println!(
            "  {:>11.2}   {:.3e}   {:.3e}   {:.3e}   ({:+.2e})",
            t / (DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY),
            e_rms[j],
            p_rms[j],
            l_rms[j],
            mean(0, j)
        );
    }
    let fe = loglog_slope(&checkpoints, &e_rms).unwrap();
    let fp = loglog_slope(&checkpoints, &p_rms).unwrap();
    let fl = loglog_slope(&checkpoints, &l_rms).unwrap();
    println!(
        "  slopes: energy {:.3} +/- {:.3} (asserted vs 0.5 +/- 0.1); momentum {:.3} +/- {:.3}, angular momentum {:.3} +/- {:.3} (reported)",
        fe.slope, fe.slope_stderr, fp.slope, fp.slope_stderr, fl.slope, fl.slope_stderr
    );
    if report_only {
        println!("  REPORT ONLY: FA_WP12_BROUWER_* overrides are set; no assertion");
        return;
    }
    let check = vectors()
        .get("NUM023-ENERGY-SLOPE")
        .unwrap()
        .check(fe.slope);
    println!("  energy slope check: {check:?}");
    check.unwrap();
}
