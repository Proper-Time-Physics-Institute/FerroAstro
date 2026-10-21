// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP29 checks I1–I3 of the IRK16 reference integrator (NUM-003), plus its checkpoint and dense
//! output.
//!
//! The Kepler problem is dimensionless (GM = 1, a = 1, period 2π) with Mercury's eccentricity
//! (test design, WP29 plan); it starts at pericentre, so after a whole number of orbits the exact
//! state is the initial state.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; test systems index their own fixed-length slices"
)]

use ferroastro_integrators::{
    GaussLegendre8, IRK16_STAGES, IntegratorError, Irk16, Irk16Config, SecondOrderSystem,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;
use std::f64::consts::TAU;

/// Mercury-like eccentricity of the test orbit (WP29 plan I2, I3).
const E: f64 = 0.2056; // lint: allow-literal(test design: Mercury-like eccentricity, dimensionless)
/// Orbits of check I2c.
const I2_ORBITS: u32 = 10;
/// Steps per orbit of check I2c, in double-double (plan addendum 2, post hoc: the registered
/// pairs 16/32 in f64 and 12/16 in DD failed by design; see IMPLEMENTATION-NOTES WP29).
const I2_STEPS: [u32; 4] = [24, 32, 40, 48];
/// Orbits each way and steps per orbit of check I3.
const I3_ORBITS: u32 = 100;
const I3_STEPS: u32 = 32;

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

fn pericentre() -> ([f64; 2], [f64; 2]) {
    ([1.0 - E, 0.0], [0.0, ((1.0 + E) / (1.0 - E)).sqrt()])
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn bound(vs: &VectorSet, id: &str) -> (f64, f64) {
    let v = vs.get(id).unwrap();
    (v.value().unwrap(), v.tolerance.allowed(v.value().unwrap()))
}

fn run(orbits: u32, steps_per_orbit: u32) -> Irk16 {
    let (x0, v0) = pericentre();
    let h = TAU / f64::from(steps_per_orbit);
    let mut irk = Irk16::new(2, 0.0, &x0, &v0, h, Irk16Config::default()).unwrap();
    for _ in 0..orbits * steps_per_orbit {
        irk.step(&Kepler).unwrap();
    }
    irk
}

/// Verifies: NUM-003
#[test]
fn gauss_legendre_coefficients_satisfy_b16_c8_and_match_the_printed_table() {
    let vs = VectorSet::load_workspace().unwrap();
    let d = GaussLegendre8::<DoubleDouble>::compute();
    let mut worst = 0.0_f64;
    // B(16): Σ b_i c_i^{k−1} = 1/k.
    for k in 1..=16_i32 {
        let mut s = DoubleDouble::ZERO;
        for (&b, &c) in d.b.iter().zip(&d.c) {
            s += b * c.powi(k - 1);
        }
        worst = worst.max(
            (s - DoubleDouble::ONE / DoubleDouble::from_i32(k))
                .abs()
                .to_f64(),
        );
    }
    // C(8): Σ_j a_ij c_j^{k−1} = c_i^k / k.
    for (row, &ci) in d.a.iter().zip(&d.c) {
        for k in 1..=8_i32 {
            let mut s = DoubleDouble::ZERO;
            for (&a, &cj) in row.iter().zip(&d.c) {
                s += a * cj.powi(k - 1);
            }
            let want = ci.powi(k) / DoubleDouble::from_i32(k);
            worst = worst.max((s - want).abs().to_f64());
        }
    }
    // bA and A² are consistent with A (the eliminated stage velocities).
    for j in 0..IRK16_STAGES {
        let mut s = DoubleDouble::ZERO;
        for k in 0..IRK16_STAGES {
            s += d.b[k] * d.a[k][j];
        }
        worst = worst.max((s - d.ba[j]).abs().to_f64());
    }
    let (b_cond, _) = bound(&vs, "IRK16-ORDER-CONDITIONS-DD");
    println!("I1: max residual of B(16), C(8), bA = {worst:.3e} (bound {b_cond:e})");
    assert!(worst <= b_cond);

    // Abramowitz & Stegun Table 25.4, n = 8 (numbers in the vector's quantity text).
    let v = vs.get("IRK16-GL8-AS-TABLE-25-4").unwrap();
    let list = |key: &str| -> Vec<f64> {
        let start = v.quantity.find(key).unwrap() + key.len();
        let rest = &v.quantity[start..];
        let end = rest.find(';').unwrap_or(rest.len());
        rest[..end]
            .split(',')
            .map(|z| z.trim().parse::<f64>().unwrap())
            .collect()
    };
    let (xs, ws) = (list("x = "), list("w = "));
    assert_eq!((xs.len(), ws.len()), (4, 4));
    let mut dev = 0.0_f64;
    for (k, (&xa, &wa)) in xs.iter().zip(&ws).enumerate() {
        // Node c on (0, 1) ↔ x = 2c − 1 on (−1, 1); the positive half is the upper nodes.
        let c = d.c[4 + k];
        let x = (DoubleDouble::TWO * c - DoubleDouble::ONE).to_f64();
        let w = (DoubleDouble::TWO * d.b[4 + k]).to_f64();
        println!("I1 A&S n = 8: x {x:.17} vs {xa}, w {w:.17} vs {wa}");
        dev = dev.max((x - xa).abs()).max((w - wa).abs());
    }
    let tol = v.tolerance.allowed(0.0);
    println!("I1: max |computed - A&S| = {dev:.3e} (tolerance {tol:e})");
    assert!(dev <= tol);
}

/// Planar Kepler problem with GM = 1 in any backend.
struct KeplerR;

impl<R: Real> SecondOrderSystem<R> for KeplerR {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(&self, _t: R, x: &[R], _v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let r2 = x[0].mul_add(x[0], x[1] * x[1]);
        let k = -R::ONE / (r2 * r2.sqrt());
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

/// Check I2c (plan addendum 2, post hoc): the order in double-double, where the round-off floor
/// lies far below the truncation error, as the least-squares slope of ln err against ln N.
/// The registered designs failed by design (I2 in f64 measured round-off; I2b's pair 12/16 fell
/// on a pre-asymptotic dip, 11.79): NEEDS HUMAN REVIEW in IMPLEMENTATION-NOTES WP29.
///
/// Verifies: NUM-003
#[test]
fn irk16_has_order_sixteen_on_a_kepler_orbit_in_double_double() {
    type D = DoubleDouble;
    let vs = VectorSet::load_workspace().unwrap();
    let e = D::from_f64(E);
    let x0 = [D::ONE - e, D::ZERO];
    let v0 = [D::ZERO, ((D::ONE + e) / (D::ONE - e)).sqrt()];
    let errs: Vec<f64> = I2_STEPS
        .iter()
        .map(|&n| {
            let h = D::TAU / D::from_i32(i32::try_from(n).unwrap());
            let mut irk = Irk16::<D>::new(2, D::ZERO, &x0, &v0, h, Irk16Config::default()).unwrap();
            for _ in 0..I2_ORBITS * n {
                irk.step(&KeplerR).unwrap();
            }
            let p = irk.positions();
            let err = (p[0] - x0[0]).hypot(p[1] - x0[1]).to_f64();
            let st = irk.stats();
            println!(
                "I2c (DD): {n} steps/orbit: |dx| = {err:.3e} after {I2_ORBITS} orbits; {} iterations \
                 in {} steps, capped {}",
                st.iterations, st.steps, st.capped_steps
            );
            assert_eq!(st.capped_steps, 0);
            err
        })
        .collect();
    let lx: Vec<f64> = I2_STEPS.iter().map(|&n| f64::from(n).ln()).collect();
    let ly: Vec<f64> = errs.iter().map(|e| e.ln()).collect();
    let k = f64::from(u32::try_from(lx.len()).unwrap());
    let (mx, my) = (lx.iter().sum::<f64>() / k, ly.iter().sum::<f64>() / k);
    let sxy: f64 = lx.iter().zip(&ly).map(|(x, y)| (x - mx) * (y - my)).sum();
    let sxx: f64 = lx.iter().map(|x| (x - mx) * (x - mx)).sum();
    let order = -sxy / sxx;
    let (want, tol) = bound(&vs, "IRK16-OBSERVED-ORDER-DD");
    println!("I2c: observed order (least-squares slope) {order:.3} (expected {want} +/- {tol})");
    assert!((order - want).abs() <= tol);
}

/// Verifies: NUM-003
#[test]
fn irk16_forward_then_backward_returns_to_the_initial_state() {
    let vs = VectorSet::load_workspace().unwrap();
    let (x0, v0) = pericentre();
    let mut irk = run(I3_ORBITS, I3_STEPS);
    let away = dist(irk.positions(), &x0);
    irk.reverse();
    for _ in 0..I3_ORBITS * I3_STEPS {
        irk.step(&Kepler).unwrap();
    }
    let dx = dist(irk.positions(), &x0);
    let dv = dist(irk.velocities(), &v0);
    let (b, _) = bound(&vs, "IRK16-KEPLER-FWD-BACK-REL");
    println!(
        "I3: after {I3_ORBITS} orbits forward |x - x0| = {away:.3e}; back: |dx| = {dx:.3e}, \
         |dv| = {dv:.3e}, t = {:e} (bound {b:e})",
        irk.time()
    );
    assert!(dx <= b);
}

/// A checkpoint between steps resumes bit for bit, in f64 and double-double (CI-010).
///
/// Verifies: NUM-003
#[test]
fn irk16_snapshot_resumes_bit_for_bit() {
    fn check<R: Real>() {
        struct K;
        impl<R: Real> SecondOrderSystem<R> for K {
            fn dimension(&self) -> usize {
                2
            }
            fn acceleration(
                &self,
                _t: R,
                x: &[R],
                _v: &[R],
                a: &mut [R],
            ) -> Result<(), IntegratorError> {
                let r2 = x[0].mul_add(x[0], x[1] * x[1]);
                let k = -R::ONE / (r2 * r2.sqrt());
                a[0] = k * x[0];
                a[1] = k * x[1];
                Ok(())
            }
        }
        let (x0, v0) = pericentre();
        let (x0, v0) = (x0.map(R::from_f64), v0.map(R::from_f64));
        let h = R::from_f64(TAU / f64::from(I3_STEPS));
        let mut a = Irk16::<R>::new(2, R::ZERO, &x0, &v0, h, Irk16Config::default()).unwrap();
        for _ in 0..I3_STEPS {
            a.step(&K).unwrap();
        }
        let snap = a.snapshot().unwrap();
        let mut b = Irk16::<R>::restore(&snap).unwrap();
        for _ in 0..I3_STEPS {
            a.step(&K).unwrap();
            b.step(&K).unwrap();
        }
        let bits = |z: &[R]| -> Vec<(u64, u64)> {
            z.iter()
                .map(|w| {
                    let (h, l) = w.to_parts();
                    (h.to_bits(), l.to_bits())
                })
                .collect()
        };
        assert_eq!(bits(a.positions()), bits(b.positions()), "{}", R::NAME);
        assert_eq!(bits(a.velocities()), bits(b.velocities()), "{}", R::NAME);
        assert_eq!(a.stats(), b.stats());
    }
    check::<f64>();
    check::<DoubleDouble>();
}

/// Dense output: the interpolant at mid-step agrees with a run of half the step that lands
/// there, and θ = 1 is the step end itself; event location finds the apocentre (r·v = 0, falling).
///
/// Verifies: NUM-003, NUM-010
#[test]
fn irk16_dense_output_and_events() {
    let (x0, v0) = pericentre();
    let h = TAU / f64::from(I3_STEPS);
    let mut irk = Irk16::new(2, 0.0, &x0, &v0, h, Irk16Config::default()).unwrap();
    let mut fine = Irk16::new(2, 0.0, &x0, &v0, h / 2.0, Irk16Config::default()).unwrap();
    let mut worst = 0.0_f64;
    let mut apo = None;
    for _ in 0..I3_STEPS {
        irk.step(&Kepler).unwrap();
        fine.step(&Kepler).unwrap();
        let (t0, t1) = irk.last_step_span().unwrap();
        let (mut x, mut v) = ([0.0; 2], [0.0; 2]);
        irk.dense_state(0.5 * (t0 + t1), &mut x, &mut v).unwrap();
        worst = worst.max(dist(&x, fine.positions()));
        fine.step(&Kepler).unwrap();
        irk.dense_state(t1, &mut x, &mut v).unwrap();
        assert_eq!(
            x.map(f64::to_bits),
            [irk.positions()[0], irk.positions()[1]].map(f64::to_bits)
        );
        if apo.is_none() {
            apo = irk
                .locate_event(|_, x, v| -(x[0].mul_add(v[0], x[1] * v[1])))
                .unwrap()
                .filter(|&t| t > 0.0);
        }
    }
    let apo = apo.unwrap();
    // Dense output is of order 8 between step points: at 32 steps per orbit its error is
    // ~ (2π/32)^9 ~ 4e-7 times an O(1) constant (sanity bound, not a physics oracle).
    let dense_bound = 1e-6; // lint: allow-literal(order-8 interpolation error at h = 2π/32)
    println!(
        "dense mid-step vs half-step run: max |dx| = {worst:.3e}; apocentre at t = {apo:.15} \
         (exact π, diff {:.3e})",
        apo - std::f64::consts::PI
    );
    assert!(worst <= dense_bound);
    assert!((apo - std::f64::consts::PI).abs() <= dense_bound);
}
