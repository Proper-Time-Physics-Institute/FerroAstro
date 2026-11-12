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

/// M5 review #18 SF1: a fixed-point iteration that stalls above the round-off floor is no
/// longer accepted silently. With an oversized step on the e = 0.5 Kepler orbit (h = 4 time
/// units, 0.64 of an orbit; probed: 2 of 3 steps stall, h = 2 still converges) the stall is
/// counted in `stalled_steps` (or the cap in `capped_steps`); with an ordinary step (256 per
/// orbit) neither counter moves.
///
/// Verifies: NUM-003
#[test]
fn irk16_counts_fixed_point_stalls_above_the_round_off_floor() {
    let e = 0.5; // lint: allow-literal(test orbit eccentricity)
    let x0 = [1.0 - e, 0.0];
    let v0 = [0.0, ((1.0 + e) / (1.0 - e)).sqrt()];
    let run = |h: f64, steps: u32| {
        let mut irk = Irk16::<f64>::new(2, 0.0, &x0, &v0, h, Irk16Config::default()).unwrap();
        for _ in 0..steps {
            irk.step(&KeplerR).unwrap();
        }
        irk.stats()
    };
    let bad = run(4.0, 3); // lint: allow-literal(oversized step, 0.64 orbit)
    println!(
        "SF1 oversized step (h = 4): {} steps, {} iterations, stalled {}, capped {}",
        bad.steps, bad.iterations, bad.stalled_steps, bad.capped_steps
    );
    assert!(bad.stalled_steps + bad.capped_steps > 0, "{bad:?}");
    let good = run(TAU / 256.0, 512); // lint: allow-literal(256 steps per orbit, two orbits)
    println!(
        "SF1 ordinary step (256/orbit): {} steps, {} iterations, stalled {}, capped {}",
        good.steps, good.iterations, good.stalled_steps, good.capped_steps
    );
    assert_eq!((good.stalled_steps, good.capped_steps), (0, 0), "{good:?}");
}

/// The DD position error after `orbits` orbits with `n` steps per orbit (I2b, I2c, X2 helper).
fn dd_error(orbits: u32, n: u32) -> f64 {
    type D = DoubleDouble;
    let e = D::from_f64(E);
    let x0 = [D::ONE - e, D::ZERO];
    let v0 = [D::ZERO, ((D::ONE + e) / (D::ONE - e)).sqrt()];
    let h = D::TAU / D::from_i32(i32::try_from(n).unwrap());
    let mut irk = Irk16::<D>::new(2, D::ZERO, &x0, &v0, h, Irk16Config::default()).unwrap();
    for _ in 0..orbits * n {
        irk.step(&KeplerR).unwrap();
    }
    let p = irk.positions();
    (p[0] - x0[0]).hypot(p[1] - x0[1]).to_f64()
}

/// Check I2 as pre-registered in the WP29 plan (f64, 16 and 32 steps per orbit, 10 orbits,
/// order = log2(err16/err32), `IRK16-OBSERVED-ORDER`), restored as a KNOWN FAILURE after the
/// progress review after WP29 (X4, X6): in f64 both errors sit on the round-off floor, so the
/// design cannot measure the order (WP29 measured 0.685). Listed in
/// `ci/nightly-known-failures.toml`; superseded by I2c, not deleted.
///
/// Verifies: NUM-003
#[test]
#[ignore = "nightly known failure (WP29 I2, superseded by I2c; ci/nightly-known-failures.toml)"]
fn nightly_irk16_i2_pre_registered_f64_order() {
    let vs = VectorSet::load_workspace().unwrap();
    let (x0, _) = pericentre();
    let e16 = dist(run(I2_ORBITS, 16).positions(), &x0);
    let e32 = dist(run(I2_ORBITS, 32).positions(), &x0); // lint: allow-literal(I2 design: 32 steps per orbit)
    let order = (e16 / e32).log2();
    println!(
        "I2 (f64, superseded, known failure): err16 {e16:.3e}, err32 {e32:.3e}, order {order:.3}"
    );
    vs.get("IRK16-OBSERVED-ORDER")
        .unwrap()
        .check(order)
        .unwrap();
}

/// Check I2b as pre-registered in WP29 plan addendum 1 (double-double, 12 and 16 steps per
/// orbit, 10 orbits, `IRK16-OBSERVED-ORDER-DD`), restored as a KNOWN FAILURE (X4): the pair
/// falls on a pre-asymptotic dip (WP29 measured 11.79). Superseded by I2c, not deleted.
///
/// Verifies: NUM-003
#[test]
#[ignore = "nightly known failure (WP29 I2b, superseded by I2c; ci/nightly-known-failures.toml)"]
fn nightly_irk16_i2b_pre_registered_dd_order() {
    let vs = VectorSet::load_workspace().unwrap();
    let (e12, e16) = (dd_error(I2_ORBITS, 12), dd_error(I2_ORBITS, 16)); // lint: allow-literal(I2b design: 12 and 16 steps per orbit)
    let order = (e12 / e16).log(16.0_f64 / 12.0); // lint: allow-literal(I2b design: 12 and 16 steps per orbit)
    println!(
        "I2b (DD, superseded, known failure): err12 {e12:.3e}, err16 {e16:.3e}, order {order:.3}"
    );
    vs.get("IRK16-OBSERVED-ORDER-DD")
        .unwrap()
        .check(order)
        .unwrap();
}

/// X2 runs: the f64 end positions with the compensated and the plain update, and the
/// double-double end position, after `orbits` orbits at `n` steps per orbit from identical
/// (exactly representable) initial bits.
fn x2_runs(orbits: u32, n: u32) -> ([f64; 2], [f64; 2], [DoubleDouble; 2]) {
    type D = DoubleDouble;
    let (x0, v0) = pericentre();
    let f64_run = |compensated: bool| {
        let h = TAU / f64::from(n);
        let cfg = Irk16Config {
            compensated,
            ..Irk16Config::default()
        };
        let mut irk = Irk16::new(2, 0.0, &x0, &v0, h, cfg).unwrap();
        for _ in 0..orbits * n {
            irk.step(&Kepler).unwrap();
        }
        [irk.positions()[0], irk.positions()[1]]
    };
    let xd = x0.map(D::from_f64);
    let vd = v0.map(D::from_f64);
    let h = D::from_f64(TAU / f64::from(n));
    let mut irk = Irk16::<D>::new(2, D::ZERO, &xd, &vd, h, Irk16Config::default()).unwrap();
    for _ in 0..orbits * n {
        irk.step(&KeplerR).unwrap();
    }
    (
        f64_run(true),
        f64_run(false),
        [irk.positions()[0], irk.positions()[1]],
    )
}

fn x2_err(p: [f64; 2], truth: [DoubleDouble; 2]) -> f64 {
    let d = [0, 1].map(|k| (DoubleDouble::from_f64(p[k]) - truth[k]).to_f64());
    d[0].hypot(d[1])
}

/// X2 design (100 orbits, 64 steps per orbit).
const X2_ORBITS: u32 = 100;
const X2_STEPS: u32 = 64;

/// Progress review after WP29, X2, as pre-registered: the plain update's error against the DD
/// twin must be at least `IRK16-COMPENSATION-GAIN-MIN` (3) times the compensated one's. **Known
/// failure** (first run 2026-11-12: 1.98; the compensated error is dominated by the stage and
/// iteration round-off, not by the state update). Listed in `ci/nightly-known-failures.toml`;
/// the bound is not moved. The PR-tier pin is
/// `irk16_compensated_update_changes_and_reduces_the_error`.
///
/// Verifies: NUM-003
#[test]
#[ignore = "nightly known failure (M5 review X2 pre-registered gain >= 3; ci/nightly-known-failures.toml)"]
fn nightly_irk16_compensation_gain_pre_registered() {
    let vs = VectorSet::load_workspace().unwrap();
    let (c, p, truth) = x2_runs(X2_ORBITS, X2_STEPS);
    let (ec, ep) = (x2_err(c, truth), x2_err(p, truth));
    let ratio = ep / ec;
    println!(
        "X2 (pre-registered): |r - r_DD| compensated {ec:.3e}, plain {ep:.3e}, ratio {ratio:.4}"
    );
    let min = vs
        .get("IRK16-COMPENSATION-GAIN-MIN")
        .unwrap()
        .value()
        .unwrap();
    assert!(
        ratio >= min,
        "IRK16-COMPENSATION-GAIN-MIN: ratio {ratio} < {min}"
    );
}

/// Progress review after WP29, X2, PR-tier pin (structural, no tolerance; written AFTER the
/// pre-registered gain check failed, disclosed in IMPLEMENTATION-NOTES): the compensated and the
/// plain update give different end states, and the compensated one is the closer to the DD twin.
/// A compensation that does nothing makes the two f64 runs bitwise identical and fails this.
///
/// Verifies: NUM-003
#[test]
fn irk16_compensated_update_changes_and_reduces_the_error() {
    let (c, p, truth) = x2_runs(X2_ORBITS, X2_STEPS);
    let (ec, ep) = (x2_err(c, truth), x2_err(p, truth));
    println!("X2 pin: |r - r_DD| compensated {ec:.3e}, plain {ep:.3e}");
    assert_ne!(c.map(f64::to_bits), p.map(f64::to_bits));
    assert!(ec < ep, "compensated {ec:e} >= plain {ep:e}");
}
