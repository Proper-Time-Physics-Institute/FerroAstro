// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP29 long runs (nightly tier, run locally): Sun + 8 planets from header.440, Newtonian.
//!
//! * W4 (NUM-004): Wisdom–Holman, h = 365.25/90 d, 10⁶ Julian years, energy every 100 yr:
//!   the uncorrected relative energy error stays below `WH-ENERGY-1E6YR-MAX` and shows no
//!   secular drift (`WH-ENERGY-1E6YR-DRIFT-FRACTION`); the corrected series is reported. The
//!   series is written to `target/wp29/w4-energy.csv`.
//! * I4/I5 (NUM-003): IRK16 h = 2 d, 1000 yr forward then back to the ICs
//!   (`IRK16-FWD-BACK-1000YR-DR`), and the forward state at 1000 yr against IAS15
//!   (`IRK16-VS-IAS15-1000YR-DR`).
//!
//! `FA_WP29_W4_YEARS` shortens W4 for a smoke run (the assertions then still apply, but the
//! result is not the NUM-004 evidence).

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    reason = "test: failures are test failures; indices in range; step counts far below 2^53 and \
              non-negative; the Swiftest step is written as in its settings file"
)]

mod wp29_common;

use ferroastro_integrators::{Irk16, Irk16Config};
use ferroastro_propagate::{DataSources, run};
use ferroastro_symplectic::{CorrectorOrder, WhConfig, Whfast};
use ferroastro_vectors::VectorSet;
use std::fmt::Write as _;
use std::time::Instant;
use wp29_common::{header, norm, p, spec, system};

const SECONDS_PER_DAY: f64 = ferroastro_core::time::SECONDS_PER_DAY;
const DAYS_PER_YEAR: f64 = ferroastro_scenario::DAYS_PER_JULIAN_YEAR;
/// W4: steps per Julian year, span, sampling interval (WP29 plan).
const W4_STEPS_PER_YEAR: u64 = 90;
const W4_YEARS: u64 = 1_000_000; // lint: allow-literal(NUM-004 span, Julian years)
const W4_SAMPLE_YEARS: u64 = 100;
/// I4/I5: step (days) and span (Julian years).
const I4_STEP_DAYS: f64 = 2.0; // lint: allow-literal(test design: IRK16 step, days)
const I4_YEARS: f64 = 1000.0; // lint: allow-literal(test design: span, Julian years)

fn value(v: &VectorSet, id: &str) -> f64 {
    v.get(id).unwrap().value().unwrap()
}

/// Maximum |y| over a slice.
fn max_abs(y: &[f64]) -> f64 {
    y.iter().fold(0.0, |m, z| m.max(z.abs()))
}

/// Verifies: NUM-004
#[test]
#[ignore = "nightly: 9e7 Wisdom-Holman steps (~minutes in release)"]
fn nightly_wh_energy_bounded_over_a_million_years() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("wh-sun-8planets-newton-100yr.toml");
    let sys = system(&s, &data, false);
    let years: u64 = std::env::var("FA_WP29_W4_YEARS")
        .ok()
        .and_then(|y| y.parse().ok())
        .unwrap_or(W4_YEARS);
    let hs = DAYS_PER_YEAR / W4_STEPS_PER_YEAR as f64 * SECONDS_PER_DAY;
    let cfg = WhConfig {
        corrector: CorrectorOrder::Eleventh,
        output_every_step: false,
    };
    let mut wh = Whfast::new(&sys.model, &sys.gm, 0.0, &sys.x0, &sys.v0, hs, cfg).unwrap();
    let e0 = sys.model.energy_over_g(&sys.x0, &sys.v0).unwrap();
    let m = sys.x0.len();
    let (mut x, mut v) = (vec![0.0; m], vec![0.0; m]);
    let (mut t_yr, mut ek, mut ep) = (vec![0.0], vec![0.0], vec![0.0]);
    let every = W4_SAMPLE_YEARS * W4_STEPS_PER_YEAR;
    let total = years * W4_STEPS_PER_YEAR;
    let start = Instant::now();
    let mut k = 0_u64;
    while k < total {
        wh.step(&sys.model).unwrap();
        k += 1;
        if k % every == 0 {
            wh.kernel_state(&mut x, &mut v).unwrap();
            ek.push((sys.model.energy_over_g(&x, &v).unwrap() - e0) / e0);
            wh.physical_state(&sys.model, &mut x, &mut v).unwrap();
            ep.push((sys.model.energy_over_g(&x, &v).unwrap() - e0) / e0);
            t_yr.push(k as f64 / W4_STEPS_PER_YEAR as f64);
        }
    }
    let wall = start.elapsed().as_secs_f64();
    let fit = |y: &[f64]| ferroastro_precession::fit::fit_line(&t_yr, y).unwrap();
    let (fk, fp) = (fit(&ek), fit(&ep));
    let range = |y: &[f64]| {
        y.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - y.iter().copied().fold(f64::INFINITY, f64::min)
    };
    let span = years as f64;
    let drift_k = fk.slope.abs() * span / range(&ek);
    let drift_p = fp.slope.abs() * span / range(&ep);
    let tenth = ek.len() / 10;
    let (first_k, last_k) = (max_abs(&ek[..tenth]), max_abs(&ek[ek.len() - tenth..]));
    let (first_p, last_p) = (max_abs(&ep[..tenth]), max_abs(&ep[ep.len() - tenth..]));
    let max_k = max_abs(&ek);
    let vs = VectorSet::load_workspace().unwrap();
    let (b_max, b_drift) = (
        value(&vs, "WH-ENERGY-1E6YR-MAX"),
        value(&vs, "WH-ENERGY-1E6YR-DRIFT-FRACTION"),
    );
    let mut csv = String::from("t_yr,dE_over_E_uncorrected,dE_over_E_corrected\n");
    for ((t, a), b) in t_yr.iter().zip(&ek).zip(&ep) {
        let _ = writeln!(csv, "{t},{a:e},{b:e}");
    }
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/wp29");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("w4-energy.csv"), csv).unwrap();
    println!(
        "W4: Sun + 8 planets (IC hash {}), WH Jacobi h = 365.25/{W4_STEPS_PER_YEAR} d, {years} yr \
         ({total} steps, {:.1} s wall, {:.3} us/step), energy every {W4_SAMPLE_YEARS} yr ({} \
         samples):\n  uncorrected: max |dE/E| {max_k:.3e} (bound {b_max:e}); slope {:.3e} +/- \
         {:.1e} (formal) per yr; |b|T/(max-min) = {drift_k:.4} (bound {b_drift}); max first \
         tenth {first_k:.3e}, last tenth {last_k:.3e}\n  corrected (order 11): max |dE/E| \
         {:.3e}; slope {:.3e} +/- {:.1e} per yr; |b|T/(max-min) = {drift_p:.4}; max first tenth \
         {first_p:.3e}, last tenth {last_p:.3e}\n  stats {:?}; series in target/wp29/w4-energy.csv",
        sys.ic_hash,
        wall,
        wall / total as f64 * 1e6, // lint: allow-literal(seconds to microseconds)
        ek.len(),
        fk.slope,
        fk.slope_stderr,
        max_abs(&ep),
        fp.slope,
        fp.slope_stderr,
        wh.stats()
    );
    assert!(max_k <= b_max, "W4a: {max_k:e} > {b_max:e}");
    assert!(drift_k <= b_drift, "W4b: {drift_k} > {b_drift}");
}

/// Verifies: NUM-003, NUM-001
#[test]
#[ignore = "nightly: 365 250 IRK16 steps and a 1000-yr IAS15 run"]
fn nightly_irk16_forward_backward_1000_years_and_vs_ias15() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("irk16-sun-8planets-newton-100yr.toml");
    let sys = system(&s, &data, false);
    let n = sys.ids.len();
    let hs = I4_STEP_DAYS * SECONDS_PER_DAY;
    let steps = (I4_YEARS * DAYS_PER_YEAR / I4_STEP_DAYS).round() as u64;
    let mut irk = Irk16::new(3 * n, 0.0, &sys.x0, &sys.v0, hs, Irk16Config::default()).unwrap();
    let start = Instant::now();
    for _ in 0..steps {
        irk.step(&sys.model).unwrap();
    }
    let t_fwd = start.elapsed().as_secs_f64();
    let fwd_x = irk.positions().to_vec();
    let st = irk.stats();
    irk.reverse();
    for _ in 0..steps {
        irk.step(&sys.model).unwrap();
    }
    let body = |x: &[f64], i: usize| [x[i], x[n + i], x[2 * n + i]];
    let back: Vec<(i32, f64)> = (0..n)
        .map(|i| {
            let a = body(irk.positions(), i);
            let b = body(&sys.x0, i);
            (sys.ids[i], norm([a[0] - b[0], a[1] - b[1], a[2] - b[2]]))
        })
        .collect();
    // IAS15, 1000 yr, the stage A settings.
    let mut ias = spec("stage-a-sun-8planets-newton-100yr.toml");
    ias.span.years = I4_YEARS;
    ias.output.cadence_days = None;
    let t0 = Instant::now();
    let out = run(&ias, &data).unwrap();
    let t_ias = t0.elapsed().as_secs_f64();
    assert_eq!(out.ic_hash, sys.ic_hash);
    let fin = out.final_state.unwrap();
    let vs_ias: Vec<(i32, f64)> = (0..n)
        .map(|i| {
            let a = body(&fwd_x, i);
            let b = fin.positions[i].map(p);
            (sys.ids[i], norm([a[0] - b[0], a[1] - b[1], a[2] - b[2]]))
        })
        .collect();
    let vs = VectorSet::load_workspace().unwrap();
    let (b4, b5) = (
        value(&vs, "IRK16-FWD-BACK-1000YR-DR"),
        value(&vs, "IRK16-VS-IAS15-1000YR-DR"),
    );
    let mercury = |l: &[(i32, f64)]| l.iter().find(|(id, _)| *id == 1).unwrap().1;
    println!(
        "I4/I5: Sun + 8 planets (IC hash {}), IRK16 h = {I4_STEP_DAYS} d, {steps} steps each way \
         ({t_fwd:.1} s forward; {} iterations, {:.2}/step, capped {}), IAS15 1000 yr ({} steps, \
         {t_ias:.1} s):\n  forward then back |r - r0| (km): {back:?}\n  IRK16 - IAS15 at 1000 \
         yr |dr| (km): {vs_ias:?}\n  Mercury: back {:.4e} km (bound {b4}), vs IAS15 {:.4e} km \
         (bound {b5})",
        sys.ic_hash,
        st.iterations,
        st.iterations as f64 / st.steps as f64,
        st.capped_steps,
        out.stats.steps,
        mercury(&back),
        mercury(&vs_ias)
    );
    assert!(mercury(&back) <= b4, "I4");
    assert!(mercury(&vs_ias) <= b5, "I5");
}

/// Swiftest's stage A step (days; `ci/oracles/swiftest/settings.toml`, WP17) and the steps per
/// Julian year at that step (365.25 × 128).
const S1_STEP_DAYS: f64 = 0.0078125; // lint: allow-literal(Swiftest WHM step of the WP17 golden, 1/128 d)
/// Tolerance for matching a golden row's time to a sample (days).
const TIME_MATCH_DAYS: f64 = 1e-6; // lint: allow-literal(time match, days)
const S1_STEPS_PER_YEAR: u64 = 46_752; // lint: allow-literal(365.25 d x 128 steps per day)

/// Rows `(t_days, naif, [x, y, z])` of a committed heliocentric oracle golden.
fn golden(path: &str) -> Vec<(f64, i32, [f64; 3])> {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/golden")
        .join(path);
    std::fs::read_to_string(p)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("t_days"))
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let num = |i: usize| f[i].parse::<f64>().unwrap();
            (num(0), f[1].parse().unwrap(), [num(2), num(3), num(4)])
        })
        .collect()
}

/// S1 (informational, WP29 task item 4): FerroAstro's Wisdom–Holman map at Swiftest's step
/// (1/128 d, no corrector, as Swiftest's WHM) against the committed Swiftest WHM stage A golden
/// (WP17) and against FerroAstro IAS15, heliocentric, yearly for 100 yr. Reported, not asserted.
///
/// Verifies: NUM-004
#[test]
#[ignore = "nightly: 4.7e6 Wisdom-Holman steps per variant"]
fn nightly_wh_vs_swiftest_whm_stage_a() {
    let (h, prov) = header();
    let data = DataSources::none().with_header(&h, &prov);
    let s = spec("stage-a-sun-8planets-newton-100yr.toml");
    let sys = system(&s, &data, false);
    let n = sys.ids.len();
    let gold = golden(
        "swiftest/stage-a-sun-8planets-newton-100yr/stage-a-sun-8planets-newton-100yr.swiftest.states.csv",
    );
    let ias = run(&s, &data).unwrap();
    let years = s.span.years.round() as u64;
    let helio = |x: &[f64], i: usize| [0, 1, 2].map(|k| x[k * n + i] - x[k * n]);
    for corrector in [CorrectorOrder::None, CorrectorOrder::Eleventh] {
        let cfg = WhConfig {
            corrector,
            output_every_step: false,
        };
        let hs = S1_STEP_DAYS * SECONDS_PER_DAY;
        let mut wh = Whfast::new(&sys.model, &sys.gm, 0.0, &sys.x0, &sys.v0, hs, cfg).unwrap();
        let (mut x, mut v) = (vec![0.0; 3 * n], vec![0.0; 3 * n]);
        let mut worst_sw: Vec<f64> = vec![0.0; n];
        let mut worst_ias: Vec<f64> = vec![0.0; n];
        let start = Instant::now();
        for year in 0..=years {
            if year > 0 {
                for _ in 0..S1_STEPS_PER_YEAR {
                    wh.step(&sys.model).unwrap();
                }
            }
            wh.physical_state(&sys.model, &mut x, &mut v).unwrap();
            let t_days = year as f64 * DAYS_PER_YEAR;
            let rec = &ias.states[year as usize];
            for i in 1..n {
                let ours = helio(&x, i);
                let id = sys.ids[i];
                if let Some((_, _, sw)) = gold
                    .iter()
                    .find(|(t, naif, _)| (*t - t_days).abs() < TIME_MATCH_DAYS && *naif == id)
                {
                    let d = norm([ours[0] - sw[0], ours[1] - sw[1], ours[2] - sw[2]]);
                    worst_sw[i] = worst_sw[i].max(d);
                }
                let r = [0, 1, 2].map(|k| p(rec.positions[i][k]) - p(rec.positions[0][k]));
                let d = norm([ours[0] - r[0], ours[1] - r[1], ours[2] - r[2]]);
                worst_ias[i] = worst_ias[i].max(d);
            }
        }
        let fmt = |w: &[f64]| -> String {
            (1..n)
                .map(|i| format!("{} {:.4}", sys.ids[i], w[i]))
                .collect::<Vec<_>>()
                .join(", ")
        };
        println!(
            "S1: WH h = 1/128 d, corrector {} ({:.1} s): max heliocentric |dr| over {years} yr (km)\n  \
             vs Swiftest WHM golden: {}\n  vs FerroAstro IAS15: {}",
            corrector.order(),
            start.elapsed().as_secs_f64(),
            fmt(&worst_sw),
            fmt(&worst_ias)
        );
        assert!(worst_sw.iter().all(|d| d.is_finite()));
    }
}
