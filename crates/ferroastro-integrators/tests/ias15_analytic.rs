// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! IAS15 against closed-form solutions (testing.md oracle priority 1: analytic formula).
//!
//! The problems are dimensionless (GM = 1, a = 1; ω = 1), so their parameters are mathematical,
//! not physical, values. Tolerances are f64 round-off bounds; each is justified inline.

#![expect(
    clippy::indexing_slicing,
    reason = "test systems index fixed-length state slices of their own dimension"
)]

use ferroastro_integrators::{Ias15, Ias15Config, IntegratorError, SecondOrderSystem};
use std::f64::consts::TAU;

// Dimensionless problem parameters and f64 round-off bounds (not physical values).
const DT0: f64 = 0.1; // lint: allow-literal(first trial step; the controller adapts it)
const ECCENTRICITIES: [f64; 2] = [0.5, 0.9]; // lint: allow-literal(R&S 2015 Kepler test orbits)
const KEPLER_ORBITS: f64 = 100.0; // lint: allow-literal(number of orbits)
const GYRATIONS: f64 = 100.0; // lint: allow-literal(number of turns)
const ZETA: f64 = 0.1; // lint: allow-literal(damping ratio of the test oscillator)
const HIGH_E_START: f64 = 0.8; // lint: allow-literal(Newton starting-guess switch, Danby)
/// ~1e4 steps of ~1e-16 random round-off: Brouwer gives ~1e-14; 1e-13 leaves margin.
const ENERGY_BOUND: f64 = 1e-13; // lint: allow-literal(round-off bound)
/// Along-track error grows as N^(3/2)·eps: ~1e-10 after 100 orbits at e = 0.9.
const PHASE_BOUND: f64 = 1e-9; // lint: allow-literal(round-off bound)
/// O(1) amplitude after ~1e3 steps.
const SMALL_BOUND: f64 = 1e-13; // lint: allow-literal(round-off bound)
const TURN_BOUND: f64 = 1e-10; // lint: allow-literal(phase-error bound)

/// Planar Kepler problem with GM = 1.
struct Kepler;

impl SecondOrderSystem for Kepler {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(
        &self,
        _t: f64,
        x: &[f64],
        _v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        let r2 = x[0].mul_add(x[0], x[1] * x[1]);
        let k = -1.0 / (r2 * r2.sqrt());
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

/// Damped oscillator ẍ = −x − 2ζẋ (velocity-dependent).
struct Damped {
    zeta: f64,
}

impl SecondOrderSystem for Damped {
    fn dimension(&self) -> usize {
        1
    }
    fn acceleration(
        &self,
        _t: f64,
        x: &[f64],
        v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        a[0] = (-2.0 * self.zeta).mul_add(v[0], -x[0]);
        Ok(())
    }
}

/// Charged particle in a uniform field along z: ẍ = ẋ × ẑ (velocity-dependent, speed-conserving).
struct Gyro;

impl SecondOrderSystem for Gyro {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(
        &self,
        _t: f64,
        _x: &[f64],
        v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        a[0] = v[1];
        a[1] = -v[0];
        Ok(())
    }
}

fn energy(x: &[f64], v: &[f64]) -> f64 {
    0.5_f64.mul_add(v[0].mul_add(v[0], v[1] * v[1]), -1.0 / x[0].hypot(x[1]))
}

/// Solve Kepler's equation E − e sin E = M by Newton iteration (independent of the integrator).
fn kepler_position(e: f64, m: f64) -> [f64; 2] {
    let mut big_e = if e > HIGH_E_START {
        std::f64::consts::PI
    } else {
        m
    };
    for _ in 0..50 {
        let f = (-e).mul_add(big_e.sin(), big_e) - m;
        big_e -= f / (-e).mul_add(big_e.cos(), 1.0);
    }
    [big_e.cos() - e, e.mul_add(-e, 1.0).sqrt() * big_e.sin()]
}

/// Rein & Spiegel 2015 Kepler test: after 100 orbits the energy error stays at round-off level and
/// the position agrees with the analytic orbit, for a moderately and a highly eccentric orbit.
///
/// Verifies: NUM-001
#[test]
fn kepler_orbits_match_analytic_solution() {
    for e in ECCENTRICITIES {
        // Start at pericentre: r = 1 − e, v = sqrt((1 + e)/(1 − e)).
        let x0 = [1.0 - e, 0.0];
        let v0 = [0.0, ((1.0 + e) / (1.0 - e)).sqrt()];
        let e0 = energy(&x0, &v0);
        let mut ias = Ias15::new(2, 0.0, &x0, &v0, DT0, Ias15Config::default()).unwrap();
        let t_end = KEPLER_ORBITS * TAU;
        let mut worst = 0.0_f64;
        ias.integrate_to(&Kepler, t_end, |s, _| {
            worst = worst.max(((energy(s.positions(), s.velocities()) - e0) / e0).abs());
            Ok(())
        })
        .unwrap();
        let p = ias.positions();
        let exact = kepler_position(e, 0.0);
        let pos_err = (p[0] - exact[0]).hypot(p[1] - exact[1]);
        let st = ias.stats();
        println!(
            "e = {e}: {} steps, {} rejected, {} force evals, {:.2} iterations/step, \
             max |dE/E| = {worst:e}, |x - x_exact| after 100 orbits = {pos_err:e}",
            st.steps,
            st.rejected,
            st.force_evaluations,
            f64::from(u32::try_from(st.iterations).unwrap())
                / f64::from(u32::try_from(st.steps + st.rejected).unwrap()),
        );
        assert!(worst < ENERGY_BOUND, "energy error {worst:e}");
        assert!(pos_err < PHASE_BOUND, "position error {pos_err:e}");
        assert_eq!(st.unconverged, 0);
    }
}

/// Velocity-dependent force: damped oscillator against x(t) = e^{−ζt}(cos ωd t + ζ/ωd sin ωd t).
///
/// Verifies: NUM-001
#[test]
fn damped_oscillator_matches_closed_form() {
    let zeta = ZETA;
    let sys = Damped { zeta };
    let mut ias = Ias15::new(1, 0.0, &[1.0], &[0.0], DT0, Ias15Config::default()).unwrap();
    let t_end = 10.0 * TAU;
    ias.integrate_to(&sys, t_end, |_, _| Ok(())).unwrap();
    let wd = zeta.mul_add(-zeta, 1.0).sqrt();
    let exact = (-zeta * t_end).exp() * (zeta / wd).mul_add((wd * t_end).sin(), (wd * t_end).cos());
    let err = (ias.positions()[0] - exact).abs();
    println!(
        "damped oscillator: x = {}, exact = {exact}, |err| = {err:e}",
        ias.positions()[0]
    );
    assert!(err < SMALL_BOUND, "{err:e}");
}

/// Velocity-dependent, energy-conserving force: gyration on a circle, then forward–backward.
///
/// Verifies: NUM-001, NUM-023
#[test]
fn gyration_conserves_speed_and_is_reversible() {
    let x0 = [0.0, 0.0];
    let v0 = [1.0, 0.0];
    let mut ias = Ias15::new(2, 0.0, &x0, &v0, DT0, Ias15Config::default()).unwrap();
    let t_end = GYRATIONS * TAU;
    ias.integrate_to(&Gyro, t_end, |_, _| Ok(())).unwrap();
    let speed = ias.velocities()[0].hypot(ias.velocities()[1]);
    let p = ias.positions();
    // After whole gyrations the particle is back at the origin.
    let back_err = p[0].hypot(p[1]);
    ias.integrate_to(&Gyro, 0.0, |_, _| Ok(())).unwrap();
    let q = ias.positions();
    let rev = (q[0] - x0[0]).hypot(q[1] - x0[1]);
    println!(
        "gyration: |speed - 1| = {:e}, |x(100 turns)| = {back_err:e}, forward-backward error = {rev:e}",
        (speed - 1.0).abs()
    );
    assert!((speed - 1.0).abs() < SMALL_BOUND);
    assert!(back_err < TURN_BOUND);
    assert!(rev < TURN_BOUND);
}

/// Invalid settings and inputs fail with stable codes, never panics (API-002).
///
/// Verifies: NUM-001, API-002
#[test]
fn invalid_inputs_are_coded_errors() {
    use ferroastro_core::ErrorCode;
    let bad = Ias15Config {
        epsilon: -1.0,
        ..Ias15Config::default()
    };
    let e = Ias15::new(1, 0.0, &[1.0], &[0.0], DT0, bad).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorConfig);
    let e = Ias15::new(2, 0.0, &[1.0], &[0.0], DT0, Ias15Config::default()).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorDimension);
    let e = Ias15::new(1, 0.0, &[f64::NAN], &[0.0], DT0, Ias15Config::default()).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorNonFinite);
    // A collision course in the Kepler problem: r -> 0 produces a non-finite acceleration or a
    // collapsing step, both coded errors.
    let mut ias = Ias15::new(
        2,
        0.0,
        &[1.0, 0.0],
        &[0.0, 0.0],
        DT0,
        Ias15Config::default(),
    )
    .unwrap();
    let err = ias.integrate_to(&Kepler, 10.0, |_, _| Ok(())).unwrap_err();
    assert!(
        matches!(
            err.code(),
            ErrorCode::IntegratorNonFinite | ErrorCode::IntegratorStepUnderflow
        ),
        "{err}"
    );
    println!("radial infall ends with: {err}");
}
