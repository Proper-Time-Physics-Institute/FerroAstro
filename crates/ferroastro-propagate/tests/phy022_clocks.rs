// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-022: the three-clock diagnostic (metric proper time `s`, flat special-relativistic
//! `τ_SR`, Gill's flow parameter `τ_K`) reproduces the derived GPS and Galileo 5/6 clock values of
//! the relativistic-dynamics#88 notes and the circular-orbit closed forms, and has the static
//! `O(v²/c²)` structure α = 0, −1, −2.
//!
//! # Scenarios
//! Two-body Earth + satellite: the satellite is a test particle about a fixed Earth (Newtonian
//! `−GM r/r³`, no J2), integrated by IAS15 with dense output; the clocks are accumulated by
//! [`ClockAccumulator`] with 8-point Gauss–Legendre quadrature on every accepted step. The GPS
//! check compares the satellite with an explicit [`RotatingGroundClock`] on the equator at
//! radius `R` (monopole potential `GM/R`, speed `ωR`), and with the same clock at rest.
//!
//! **Supported-clock assumption (H10 of the rd#88 package):** the ground clock obeys the
//! free-particle rate formula at its potential and speed. Only the ground rows depend on it.
//!
//! # Oracles and tolerances
//! Every expected value and input is a vector of `tests/vectors/phy022-clocks.toml` with its
//! provenance; the targets are derived values from the refereed notes, not observations. The
//! tolerances are justified there: half a unit in the last printed digit plus margin, against
//! an integration floor (IAS15 closure, Gauss–Legendre, offsets carried as `rate − 1`) below
//! 1e-8 µs/day and 1e-6 ns, and the `O(U²)` difference between the first-order formulas of the
//! report and the exact rates used here (3e-9 µs/day, 3e-8 ns).
//!
//! The circular-orbit and static tests use scale-free units (`GM = 1`, `r = 1`) and a synthetic
//! `c`, so that `x = GM/(rc²)` is large enough to separate the second-order terms.
//!
//! All tests are PR-tier (well under a second each); none is `#[ignore]`. The scenario test at
//! the end needs the cached header.440 (CI-005).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              reference arithmetic is written plainly"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_forces::TestParticle;
use ferroastro_integrators::{Ias15, Ias15Config};
use ferroastro_propagate::clocks::{
    CLOCKS_LABEL, ClockAccumulator, ClockModel, RotatingGroundClock, Source, ThreeClocks,
    rate_offsets,
};
use ferroastro_propagate::{DataSources, run, run_with_clocks};
use ferroastro_scenario::{Mode, ScenarioSpec};
use ferroastro_vectors::VectorSet;
use std::f64::consts::PI;
use std::path::PathBuf;

/// Seconds per day (unit conversion).
const DAY_S: f64 = 86_400.0; // lint: allow-literal(seconds per day, unit conversion)
/// µs per s and ns per s (unit conversions).
const US: f64 = 1.0e6; // lint: allow-literal(unit conversion s -> us)
const NS: f64 = 1.0e9; // lint: allow-literal(unit conversion s -> ns)
/// Uniform grid points per Galileo orbit for the extremum search (module docs, vector rationale).
const GRID: usize = 8000; // lint: allow-literal(grid points per orbit, a count)

fn vectors() -> VectorSet {
    VectorSet::load_workspace().unwrap()
}

fn value(v: &VectorSet, id: &str) -> f64 {
    v.get(id).unwrap().value().unwrap()
}

fn check(v: &VectorSet, id: &str, measured: f64) {
    let diff = v.get(id).unwrap().check(measured).unwrap();
    println!("{id}: measured {measured:.9}, |diff| {diff:.3e}");
}

/// Integrate a test particle about a point mass `gm` from `(r0, v0)` over `[0, span]`, sampled
/// on a uniform grid of `samples` intervals; returns the clock offsets at every grid point.
fn integrate(
    gm: f64,
    model: ClockModel,
    r0: [f64; 3],
    v0: [f64; 3],
    span: f64,
    samples: usize,
) -> Vec<(f64, ThreeClocks)> {
    let mu = GravParam::from_km3_per_s2(gm).unwrap();
    let sys = TestParticle::<Icrf>::new(mu, Vec::new()).unwrap();
    let cfg = Ias15Config {
        dense_output: true,
        ..Ias15Config::default()
    };
    let src = [Source {
        position: [0.0; 3],
        gm: mu,
    }];
    let mut ias = Ias15::new(3, 0.0, &r0, &v0, span / samples as f64, cfg).unwrap();
    let mut acc = ClockAccumulator::new();
    let mut out = vec![(0.0, acc.offsets())];
    for k in 1..=samples {
        let tk = span * k as f64 / samples as f64;
        ias.integrate_to(&sys, tk, |ias, _| {
            let (t0, t1) = ias.last_step_span().unwrap();
            acc.add_interval(t0, t1, |t| {
                let (mut x, mut v) = ([0.0; 3], [0.0; 3]);
                ias.dense_state(t, &mut x, &mut v).unwrap();
                Ok(model.rates(x, v, &src).unwrap())
            })
            .unwrap();
            Ok(())
        })
        .unwrap();
        out.push((tk, acc.offsets()));
    }
    out
}

fn model(v: &VectorSet) -> ClockModel {
    ClockModel::new(Velocity::from_km_per_s(value(v, "CONST-CLIGHT-SI")).unwrap()).unwrap()
}

/// PHY-022 (a): GPS satellite (circular, a = 26 560 km) minus an equatorial ground clock, over
/// one day, in µs/day: rotating ground +38.54129 / −7.109623 / −52.76053, ground at rest
/// +38.43731 / −7.2136 / −52.86451 for `s` / `τ_SR` / `τ_K`.
///
/// Verifies: PHY-022
#[test]
fn gps_satellite_minus_ground_matches_the_derived_rates() {
    let v = vectors();
    let m = model(&v);
    let gm = value(&v, "PHY022-GM-EARTH");
    let a = value(&v, "PHY022-A-GPS");
    let speed = (gm / a).sqrt();
    let track = integrate(gm, m, [a, 0.0, 0.0], [0.0, speed, 0.0], DAY_S, 64);
    let (t_end, sat) = *track.last().unwrap();
    assert!((t_end - DAY_S).abs() <= 1.0e-9); // lint: allow-literal(grid end round-off, s)
    let mu = GravParam::from_km3_per_s2(gm).unwrap();
    let radius = Length::from_km(value(&v, "PHY022-R-GROUND")).unwrap();
    for (omega, ids) in [
        (
            value(&v, "PHY022-OMEGA-EARTH"),
            [
                "PHY022-GPS-ROT-S",
                "PHY022-GPS-ROT-TAUSR",
                "PHY022-GPS-ROT-TAUK",
            ],
        ),
        (
            0.0,
            [
                "PHY022-GPS-REST-S",
                "PHY022-GPS-REST-TAUSR",
                "PHY022-GPS-REST-TAUK",
            ],
        ),
    ] {
        let ground = RotatingGroundClock {
            radius,
            omega_rad_per_s: omega,
            gm: mu,
        }
        .rates(&m)
        .unwrap();
        // Satellite minus ground, µs per day of coordinate time.
        let d = sat.minus(ground.scaled(t_end)).scaled(US * DAY_S / t_end);
        check(&v, ids[0], d.s);
        check(&v, ids[1], d.tau_sr);
        check(&v, ids[2], d.tau_k);
    }
}

/// Half the peak-to-peak of `y` on a uniform grid, each extremum refined by the parabola through
/// it and its neighbours.
fn half_peak_to_peak(y: &[f64]) -> f64 {
    let refine = |k: usize| -> f64 {
        if k == 0 || k + 1 == y.len() {
            return y[k];
        }
        let (a, b, c) = (y[k - 1], y[k], y[k + 1]);
        let den = a - 2.0 * b + c;
        if den == 0.0 {
            b
        } else {
            b - (c - a) * (c - a) / (8.0 * den)
        }
    };
    let arg = |better: fn(f64, f64) -> bool| {
        (0..y.len())
            .reduce(|i, j| if better(y[j], y[i]) { j } else { i })
            .unwrap()
    };
    let (kmax, kmin) = (arg(|p, q| p > q), arg(|p, q| p < q));
    0.5 * (refine(kmax) - refine(kmin))
}

/// PHY-022 (b): Galileo 5/6 (a = 27 977 km, e = 0.162): the eccentricity-periodic amplitude of
/// each clock over one orbit (offset minus its secular trend, half peak-to-peak) is
/// 380.6914 / 190.3457 / 0 ns for `s` / `τ_SR` / `τ_K`.
///
/// Verifies: PHY-022
#[test]
fn galileo_5_6_periodic_amplitudes_match_the_derived_values() {
    let v = vectors();
    let m = model(&v);
    let gm = value(&v, "PHY022-GM-EARTH");
    let a = value(&v, "PHY022-A-GALILEO56");
    let e = value(&v, "PHY022-E-GALILEO56");
    let period = 2.0 * PI * (a * a * a / gm).sqrt();
    // Start at perigee, where the periodic term vanishes.
    let rp = a * (1.0 - e);
    let vp = (gm * (1.0 + e) / rp).sqrt();
    let track = integrate(gm, m, [rp, 0.0, 0.0], [0.0, vp, 0.0], period, GRID);
    let (t_end, last) = *track.last().unwrap();
    let detrended = |f: fn(&ThreeClocks) -> f64| -> Vec<f64> {
        track
            .iter()
            .map(|(t, c)| (f(c) - f(&last) * t / t_end) * NS)
            .collect()
    };
    let amp = [
        half_peak_to_peak(&detrended(|c| c.s)),
        half_peak_to_peak(&detrended(|c| c.tau_sr)),
        half_peak_to_peak(&detrended(|c| c.tau_k)),
    ];
    check(&v, "PHY022-GALILEO56-PERIODIC-S", amp[0]);
    check(&v, "PHY022-GALILEO56-PERIODIC-TAUSR", amp[1]);
    check(&v, "PHY022-GALILEO56-PERIODIC-TAUK", amp[2]);
}

/// PHY-022 (c): on a circular orbit, with `x = GM/(rc²) = (GMω/c³)^{2/3}`, the clock rates per
/// unit coordinate time are `(1 − x)^{3/2}` (`s`), `(1 − x)^{1/2}` (`τ_SR`) and `(1 − x)^{−1/2}`
/// (`τ_K`). `x = 1e-3` (synthetic `c`) separates `s` from GR's `(1 − 3x)^{1/2}` by `(3/2)x²`,
/// 1.5e-6, which the bound 1e-12 resolves by six orders (the IAS15 circular orbit keeps `r` to
/// about 1e-14 over five orbits).
///
/// Verifies: PHY-022
#[test]
fn circular_orbit_rates_have_the_closed_forms() {
    let x: f64 = 1.0e-3; // lint: allow-literal(synthetic field strength GM/(r c^2))
    let c = x.powf(-0.5); // GM = r = 1
    let m = ClockModel::new(Velocity::from_km_per_s(c).unwrap()).unwrap();
    let period = 2.0 * PI;
    let track = integrate(1.0, m, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 5.0 * period, 50);
    let (t, off) = *track.last().unwrap();
    let rate = off.scaled(1.0 / t);
    let tol = 1.0e-12; // lint: allow-literal(IAS15 floor of the circular orbit, see doc)
    let expect = [
        (1.0 - x).powf(1.5) - 1.0, // lint: allow-literal(exponent 3/2 of the closed form)
        (1.0 - x).sqrt() - 1.0,
        1.0 / (1.0 - x).sqrt() - 1.0,
    ];
    println!("rates {rate:?}, closed forms {expect:?}");
    assert!((rate.s - expect[0]).abs() <= tol);
    assert!((rate.tau_sr - expect[1]).abs() <= tol);
    assert!((rate.tau_k - expect[2]).abs() <= tol);
    // GR's s rate differs at second order and is resolved.
    let gr = (1.0 - 3.0 * x).sqrt() - 1.0;
    assert!((rate.s - gr).abs() > 1.0e6 * tol); // lint: allow-literal(six orders above the bound)
}

/// PHY-022 structure: static clocks (v = 0) in a monopole field have rates `1 − U`, `1`,
/// `1/(1 − U)`, so the redshift parameter of `Δν/ν = (1 + α)ΔU` is α = 0, −1, −2 up to `O(U)`;
/// in flat space (U = 0) the three clocks agree with `√(1 − v²/c²)`; to first order the rate
/// offsets are `−U − β²/2`, `−β²/2`, `+U − β²/2`, with remainders `O(c⁻⁴)`. A clock held at rest
/// for a span accumulates exactly `rate × span`.
///
/// Verifies: PHY-022
#[test]
fn static_clocks_have_the_first_order_structure() {
    let c = 1.0e3; // lint: allow-literal(synthetic c of the scale-free units)
    let m = ClockModel::new(Velocity::from_km_per_s(c).unwrap()).unwrap();
    let mu = GravParam::from_km3_per_s2(1.0).unwrap();
    let src = [Source {
        position: [0.0; 3],
        gm: mu,
    }];
    let (r1, r2) = (1.0, 4.0);
    let (u1, u2) = (1.0 / (r1 * c * c), 1.0 / (r2 * c * c));
    let at = |r: f64| m.rates([r, 0.0, 0.0], [0.0; 3], &src).unwrap();
    let (c1, c2) = (at(r1), at(r2));
    // α from the frequency ratio of the higher (r2) to the lower (r1) clock.
    let alpha = |f1: f64, f2: f64| ((1.0 + f2) / (1.0 + f1) - 1.0) / (u1 - u2) - 1.0;
    let tol = 3.0 * u1;
    assert!(alpha(c1.s, c2.s).abs() <= tol);
    assert!((alpha(c1.tau_sr, c2.tau_sr) + 1.0).abs() <= tol);
    assert!((alpha(c1.tau_k, c2.tau_k) + 2.0).abs() <= tol);
    // Flat space: one rate.
    let flat = m.rates([1.0, 0.0, 0.0], [0.0, 300.0, 0.0], &[]).unwrap(); // lint: allow-literal(synthetic speed, km/s)
    let b2: f64 = 0.09; // lint: allow-literal((300/1000)^2)
    let g = (1.0 - b2).sqrt() - 1.0;
    for z in [flat.s, flat.tau_sr, flat.tau_k] {
        assert!((z - g).abs() <= 1.0e-15); // lint: allow-literal(round-off)
    }
    // First order, small U and v.
    let (u, b2) = (1.0e-6, 4.0e-6); // lint: allow-literal(small synthetic depth and v^2/c^2)
    let r = rate_offsets(u, b2).unwrap();
    let rem = 4.0 * (u + b2) * (u + b2);
    assert!((r.s - (-u - 0.5 * b2)).abs() <= rem);
    assert!((r.tau_sr - (-0.5 * b2)).abs() <= rem);
    assert!((r.tau_k - (u - 0.5 * b2)).abs() <= rem);
    // A clock at rest accumulates rate × span.
    let mut acc = ClockAccumulator::new();
    let span = 1.0e4; // lint: allow-literal(synthetic span, s)
    acc.add_interval(0.0, span, |_| Ok(c1)).unwrap();
    let want = c1.scaled(span);
    assert!(acc.offsets().minus(want).max_abs() <= 1.0e-12 * want.max_abs()); // lint: allow-literal(round-off)
    assert!((acc.elapsed() - span).abs() <= 0.0);
}

/// The three-clock output is opt-in and refused for ephemeris-field runs (before any data is
/// read).
///
/// Verifies: PHY-022
#[test]
fn ephemeris_field_runs_are_refused() {
    let mut s = spec("sun-mercury-newton-header440.toml");
    s.mode = Mode::EphemerisField;
    let err = run_with_clocks(&s, &DataSources::none()).unwrap_err();
    assert!(err.to_string().starts_with("FA-E1300"), "{err}");
}

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// PHY-022 (d): `run_with_clocks` returns `run`'s output bit for bit (no change to dynamics,
/// outputs or the scenario digest), carries the label, uses the Sun and Mercury as sources,
/// and Mercury's `τ_K` offset grows at the constant first-order rate `GM☉/(2ac²)` of a Kepler
/// orbit. Relative bound 1e-4: the clocks are evaluated in the DE440 barycentric frame, in which
/// the Sun + Mercury centre of mass drifts at the Sun's ICRF speed `V` (about 10 m/s), so the
/// term `v·V/c²` modulates the rate by about 2V/v = 4e-4 per orbit and leaves a residual of order
/// 1e-5 over the 4.15 orbits of a year (observed 1.5e-5, printed); the Sun's reflex enters at
/// m/M = 1.7e-7 and second-order terms at 1e-8. The cadence-limited quadrature error is reported
/// and checked below 1e-3 of the offset.
///
/// Verifies: PHY-022
#[test]
fn run_with_clocks_leaves_the_run_unchanged() {
    if De440::load_cached_or_skip("run_with_clocks_leaves_the_run_unchanged")
        .unwrap()
        .is_none()
    {
        return;
    }
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let data = DataSources::none().with_header(&h, &prov);
    let mut s = spec("sun-mercury-newton-header440.toml");
    s.span.years = 1.0;
    s.output.cadence_days = Some(0.5); // lint: allow-literal(output cadence, days)
    let digest = s.digest();
    let plain = run(&s, &data).unwrap();
    let (out, clocks) = run_with_clocks(&s, &data).unwrap();
    assert_eq!(s.digest(), digest);
    assert_eq!(out, plain);
    assert_eq!(out.differing_words(&plain), Some(0));
    assert_eq!(clocks.label, CLOCKS_LABEL);
    assert!(clocks.label.contains("comparison model"));
    assert_eq!(clocks.bodies, vec![10, 1]); // Sun, Mercury (barycentre id in n-body runs)
    assert_eq!(clocks.sources, vec![10, 1]);
    assert_eq!(clocks.omitted_sources, Vec::<i32>::new());
    assert!(clocks.samples.len() > 700); // one year at half-day cadence
    // Mercury's osculating a from the initial relative state.
    let de = data.de440("test").unwrap();
    let st = &out.states[0];
    let p = |i: usize| st.positions[i].map(|z| z[0] + z[1]);
    let w = |i: usize| st.velocities[i].map(|z| z[0] + z[1]);
    let (dr, dv) = (
        [p(1)[0] - p(0)[0], p(1)[1] - p(0)[1], p(1)[2] - p(0)[2]],
        [w(1)[0] - w(0)[0], w(1)[1] - w(0)[1], w(1)[2] - w(0)[2]],
    );
    let r = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
    let v2 = dv[0] * dv[0] + dv[1] * dv[1] + dv[2] * dv[2];
    let gm_sun = de.gm_sun().km3_per_s2();
    let gm_mercury = ferroastro_system::Catalogue::de440_major(&de)
        .gm(ferroastro_ephem::NaifId(1))
        .unwrap()
        .km3_per_s2();
    let a = 1.0 / (2.0 / r - v2 / (gm_sun + gm_mercury));
    let cl = de.speed_of_light().km_per_s();
    let last = clocks.samples.last().unwrap();
    let rate_k = last.offsets[1].tau_k / last.t;
    let want = gm_sun / (2.0 * a * cl * cl);
    println!("Mercury tau_K rate {rate_k:e}, GM/(2ac^2) {want:e}");
    assert!((rate_k / want - 1.0).abs() <= 1.0e-4); // lint: allow-literal(relative bound, see doc)
    // s runs slow and tau_K fast relative to t; the quadrature estimate is small.
    assert!(last.offsets[1].s < 0.0 && last.offsets[1].tau_sr < 0.0 && last.offsets[1].tau_k > 0.0);
    let err = clocks.quadrature_error[1];
    assert!(err.max_abs() <= 1.0e-3 * last.offsets[1].max_abs()); // lint: allow-literal(cadence bound)
}
