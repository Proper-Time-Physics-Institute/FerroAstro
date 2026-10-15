// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-023 round-off diagnostics for IAS15 (NUM-001) in f64: Brouwer-law slopes of the energy
//! error (expected ½) and of the mean-longitude error (expected 3/2), plus the angular-momentum
//! slope and the forward–backward reversibility error (reported).
//!
//! Method. An ensemble of Newtonian Sun + Mercury runs (DE440 GMs, SW92 a, e, I, Ω, ϖ) that
//! differ only in the initial mean anomaly. At log-spaced checkpoints each run records its
//! relative energy error, relative angular-momentum error and mean-longitude error against the
//! exact Kepler motion of its own initial osculating orbit. The ensemble RMS is fitted with
//! `ln rms = c + p ln t` from 10 orbits on (earlier points sit on the f64 evaluation floor).
//!
//! Tier: nightly (`#[ignore]`, `nightly_` prefix; `cargo nextest run --profile nightly
//! --run-ignored only`). Size: `FA_NUM023_RUNS` runs (default 64) of `FA_NUM023_ORBITS` Mercury
//! orbits (default 2000), run in parallel threads.

mod two_body;

use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::De440;
use ferroastro_core::frames::{EclipticJ2000, Heliocentric, State};
use ferroastro_core::units::Angle;
use ferroastro_integrators::diagnostics::loglog_slope;
use ferroastro_integrators::{Ias15, Ias15Config};
use ferroastro_vectors::VectorSet;
use std::f64::consts::{PI, TAU};

/// First checkpoint, in orbits (below this the f64 evaluation floor dominates).
const FIRST_ORBIT: f64 = 10.0;
/// Default ensemble size and run length (fixed before the first run; see IMPLEMENTATION-NOTES).
const DEFAULT_RUNS: u32 = 64;
const DEFAULT_ORBITS: u32 = 2000; // lint: allow-literal(run length in orbits, not a physical value)
/// Number of log-spaced checkpoints.
const CHECKPOINTS: u32 = 24;

fn env_or(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

/// Per-checkpoint errors of one run: (relative energy, relative |L|, mean-longitude in rad).
type Errors = Vec<(f64, f64, f64)>;

/// IAS15 settings: Rein & Spiegel defaults; `FA_NUM023_EPSILON` overrides ε for diagnosis.
fn cfg() -> Ias15Config {
    let mut c = Ias15Config::default();
    if let Some(eps) = std::env::var("FA_NUM023_EPSILON")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        c.epsilon = eps;
    }
    if let Some(tol) = std::env::var("FA_NUM023_PC_TOL")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        c.pc_tolerance = tol;
    }
    c
}

fn wrap(x: f64) -> f64 {
    (x + PI).rem_euclid(TAU) - PI
}

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn mean_longitude(r: [f64; 3], v: [f64; 3], mu: f64) -> (f64, f64) {
    let el = Elements::from_vectors(r, v, mu).unwrap();
    let lambda = el.node.radians() + el.arg_periapsis.radians() + el.mean_anomaly.radians();
    (lambda, el.a.km())
}

fn norm(l: [f64; 3]) -> f64 {
    l[0].hypot(l[1]).hypot(l[2])
}

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn run_one(
    model: &two_body::Model,
    x0: &[f64],
    v0: &[f64],
    mu: f64,
    checkpoints: &[f64],
) -> (Errors, u64) {
    let e0 = model.energy_over_g(x0, v0);
    let l0 = norm(model.angular_momentum_over_g(x0, v0));
    let (r0, w0) = two_body::relative(x0, v0);
    let (lambda0, a0) = mean_longitude(r0, w0, mu);
    let n0 = (mu / (a0 * a0 * a0)).sqrt();
    let mut ias = Ias15::new(
        6,
        0.0,
        x0,
        v0,
        ferroastro_core::time::SECONDS_PER_DAY,
        cfg(),
    )
    .unwrap();
    let mut out = Vec::with_capacity(checkpoints.len());
    for &t in checkpoints {
        ias.integrate_to(model, t, |_, _| Ok(())).unwrap();
        let (x, v) = (ias.positions(), ias.velocities());
        let de = (model.energy_over_g(x, v) - e0) / e0;
        let dl = (norm(model.angular_momentum_over_g(x, v)) - l0) / l0;
        let (r, w) = two_body::relative(x, v);
        let (lambda, _) = mean_longitude(r, w, mu);
        let dlam = wrap(lambda - n0.mul_add(t, lambda0));
        out.push((de, dl, dlam));
    }
    (out, ias.stats().steps)
}

/// Verifies: NUM-023, NUM-001
#[test]
#[ignore = "nightly tier: ~1e7 IAS15 steps; run with --profile nightly --run-ignored only"]
fn nightly_num023_brouwer_slopes_ias15_f64() {
    let de = De440::load().unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let (gm_sun, gm_me) = two_body::gms(&de);
    let mu = gm_sun + gm_me;
    let runs = env_or("FA_NUM023_RUNS", DEFAULT_RUNS);
    let orbits = f64::from(env_or("FA_NUM023_ORBITS", DEFAULT_ORBITS));
    let base = two_body::sw92_elements(&vs, &de);
    let period = TAU * (base.a.km().powi(3) / mu.km3_per_s2()).sqrt();
    let checkpoints: Vec<f64> = (0..CHECKPOINTS)
        .map(|j| {
            let f = f64::from(j) / f64::from(CHECKPOINTS - 1);
            FIRST_ORBIT * (orbits / FIRST_ORBIT).powf(f) * period
        })
        .collect();
    let model = two_body::Model::new(&[gm_sun, gm_me]).unwrap();
    let started = std::time::Instant::now();
    let results: Vec<(Errors, u64)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..runs)
            .map(|k| {
                let mut el = base;
                el.mean_anomaly =
                    Angle::from_radians(TAU * f64::from(k) / f64::from(runs)).unwrap();
                let rel: State<EclipticJ2000, Heliocentric> = el.to_state(mu).unwrap();
                let (x0, v0) = two_body::barycentric(&rel, gm_sun, gm_me);
                let (model, cps) = (&model, &checkpoints);
                scope.spawn(move || run_one(model, &x0, &v0, mu.km3_per_s2(), cps))
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let wall = started.elapsed().as_secs_f64();
    let total_steps: u64 = results.iter().map(|r| r.1).sum();
    let nf = f64::from(runs);
    let rms = |pick: fn(&(f64, f64, f64)) -> f64| -> Vec<f64> {
        (0..checkpoints.len())
            .map(|j| (results.iter().map(|r| pick(&r.0[j]).powi(2)).sum::<f64>() / nf).sqrt())
            .collect()
    };
    let (e_rms, l_rms, p_rms) = (rms(|z| z.0), rms(|z| z.1), rms(|z| z.2));
    let mean = |pick: fn(&(f64, f64, f64)) -> f64| -> Vec<f64> {
        (0..checkpoints.len())
            .map(|j| results.iter().map(|r| pick(&r.0[j])).sum::<f64>() / nf)
            .collect()
    };
    let (e_mean, l_mean, p_mean) = (mean(|z| z.0), mean(|z| z.1), mean(|z| z.2));
    println!(
        "NUM-023 ensemble: {runs} runs x {orbits} orbits, {total_steps} IAS15 steps in total, wall {wall:.1} s"
    );
    println!(
        "  t [orbits]   rms dE/E      rms dL/L      rms d(lambda) [rad]   (ensemble means: dE/E, dL/L, d(lambda))"
    );
    for (j, t) in checkpoints.iter().enumerate() {
        println!(
            "  {:>10.1}   {:.3e}   {:.3e}   {:.3e}   ({:+.2e}, {:+.2e}, {:+.2e})",
            t / period,
            e_rms[j],
            l_rms[j],
            p_rms[j],
            e_mean[j],
            l_mean[j],
            p_mean[j]
        );
    }
    let fe = loglog_slope(&checkpoints, &e_rms).unwrap();
    let fl = loglog_slope(&checkpoints, &l_rms).unwrap();
    let fp = loglog_slope(&checkpoints, &p_rms).unwrap();
    println!(
        "  slopes: energy {:.3} +/- {:.3}, angular momentum {:.3} +/- {:.3} (reported), phase {:.3} +/- {:.3} (formal 1-sigma)",
        fe.slope, fe.slope_stderr, fl.slope, fl.slope_stderr, fp.slope, fp.slope_stderr
    );
    let ve = vs.get("NUM023-ENERGY-SLOPE").unwrap();
    let vp = vs.get("NUM023-PHASE-SLOPE").unwrap();
    let re = ve.check(fe.slope);
    let rp = vp.check(fp.slope);
    println!("  energy slope check: {re:?}\n  phase slope check: {rp:?}");
    re.unwrap();
    rp.unwrap();
}

/// Forward–backward reversibility of one 1000-orbit Sun + Mercury run (NUM-023, reported).
///
/// Verifies: NUM-023
#[test]
#[ignore = "nightly tier"]
fn nightly_num023_forward_backward_reversibility() {
    let de = De440::load().unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let (gm_sun, gm_me) = two_body::gms(&de);
    let mu = gm_sun + gm_me;
    let el = two_body::sw92_elements(&vs, &de);
    let rel: State<EclipticJ2000, Heliocentric> = el.to_state(mu).unwrap();
    let (x0, v0) = two_body::barycentric(&rel, gm_sun, gm_me);
    let model = two_body::Model::new(&[gm_sun, gm_me]).unwrap();
    let period = TAU * (el.a.km().powi(3) / mu.km3_per_s2()).sqrt();
    let orbits = f64::from(env_or("FA_NUM023_ORBITS", DEFAULT_ORBITS)) / 2.0;
    let mut ias = Ias15::new(
        6,
        0.0,
        &x0,
        &v0,
        ferroastro_core::time::SECONDS_PER_DAY,
        cfg(),
    )
    .unwrap();
    ias.integrate_to(&model, orbits * period, |_, _| Ok(()))
        .unwrap();
    let steps_fwd = ias.stats().steps;
    ias.integrate_to(&model, 0.0, |_, _| Ok(())).unwrap();
    let (r_end, _) = two_body::relative(ias.positions(), ias.velocities());
    let (r_0, _) = two_body::relative(&x0, &v0);
    let dr = norm([r_end[0] - r_0[0], r_end[1] - r_0[1], r_end[2] - r_0[2]]);
    println!(
        "NUM-023 reversibility: {orbits} orbits forward ({steps_fwd} steps) and back: |dr| = {dr:.3e} km, |dr|/|r0| = {:.3e} (reported; no acceptance value in NUM-023)",
        dr / norm(r_0)
    );
    assert!(dr.is_finite());
}
