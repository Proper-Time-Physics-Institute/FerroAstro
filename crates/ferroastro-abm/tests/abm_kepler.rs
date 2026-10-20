// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP24 checks A3–A6 of `ferroastro-abm` against the closed-form Kepler solution
//! (`ferroastro-analytic`, testing.md oracle priority 1).
//!
//! * A3: Mercury's orbit (SW92 a and e, μ = DE440 GM☉) over 1000 orbits at h = 1/16 d, ABM13 PECEC
//!   and PECE in f64 with the compensated update: energy and along-track phase errors
//!   (`ABM-KEPLER-ENERGY-REL`, `ABM-KEPLER-PHASE-RAD`); the plain-sum and double-double variants
//!   are reported.
//! * A4: convergence order by halving h, p ∈ {8, 10, 12, 13, 14} × {PECE, PECEC}
//!   (`ABM-ORDER-SLOPE`).
//! * A5: NUM-010 perihelion events (`ABM-EVENT-TIME-S`).
//! * A6: checkpoint/resume is bitwise, inside and after the start-up, f64 and double-double.
//!
//! The orbit starts at aphelion on the −x axis, moving towards −y, so the mean anomaly is
//! `M(t) = π + n t` and perihelion passages fall at `t = P/2 + kP`.

#![expect(
    clippy::indexing_slicing,
    reason = "the test systems index fixed-length state slices of their own dimension"
)]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_abm::{Abm, AbmConfig, AbmSnapshot, Corrector};
use ferroastro_analytic::kepler::eccentric_anomaly;
use ferroastro_core::ErrorCode;
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

type D = DoubleDouble;

/// Seconds per day (SI, exact).
const SECONDS_PER_DAY: f64 = 86_400.0; // lint: allow-literal(SI definition of the day)
/// The Subbotin et al. 2023 step, 1/16 d (a run setting, not a physical value).
const STEPS_PER_DAY: f64 = 16.0; // lint: allow-literal(run setting: h = 1/16 d)
/// A3 run length in orbits (WP24 task text).
const A3_ORBITS: f64 = 1000.0; // lint: allow-literal(run length from the WP24 task)
/// The double-double variant of A3 is reported over this many orbits (cost).
const A3_DD_ORBITS: f64 = 100.0; // lint: allow-literal(run length, PR-tier cost)
/// Error samples every this many steps.
const SAMPLE_EVERY: u64 = 64;
/// A4/A5 run length in orbits.
const SHORT_ORBITS: u32 = 10;
/// A4: steps per orbit of the coarser run of each pair (plan check A4; chosen from the scan
/// reported in IMPLEMENTATION-NOTES WP24 so that the error at h/2 is >= 1e3 x the
/// double-double floor and the step is inside the stability range).
const A4_STEPS: [(u32, u32); 5] = [(8, 1024), (10, 1024), (12, 1024), (13, 1024), (14, 512)]; // lint: allow-literal(steps per orbit, test design)

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

fn value(vs: &VectorSet, id: &str) -> f64 {
    vs.get(id).unwrap().value().unwrap()
}

/// Planar Kepler problem `ẍ = −μx/|x|³`.
struct Kepler<R: Real> {
    mu: R,
}

impl<R: Real> SecondOrderSystem<R> for Kepler<R> {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(&self, _t: R, x: &[R], _v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let r2 = x[0] * x[0] + x[1] * x[1];
        let k = -self.mu / (r2 * r2.sqrt());
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

/// An aphelion-started ellipse.
#[derive(Clone, Copy)]
struct Orbit<R: Real> {
    mu: R,
    a: R,
    e: R,
    n: R,
    period: R,
}

impl<R: Real> Orbit<R> {
    fn new(mu: R, a: R, e: R) -> Self {
        let n = (mu / (a * a * a)).sqrt();
        Self {
            mu,
            a,
            e,
            n,
            period: R::TAU / n,
        }
    }

    /// Mercury: SW92 a and e, μ = DE440 GM☉ (km, s).
    fn mercury(vs: &VectorSet) -> Self {
        let au_km = R::from_f64(value(vs, "CONST-AU-IAU2012"));
        Self::new(
            R::from_f64(value(vs, "DE440-GMS-KM3S2")),
            R::from_f64(value(vs, "SW92-MERCURY-A")) * au_km,
            R::from_f64(value(vs, "SW92-MERCURY-E")),
        )
    }

    /// The dimensionless orbit GM = a = 1 with Mercury's eccentricity.
    fn unit(vs: &VectorSet) -> Self {
        Self::new(R::ONE, R::ONE, R::from_f64(value(vs, "SW92-MERCURY-E")))
    }

    fn start(&self) -> ([R; 2], [R; 2]) {
        let ra = self.a * (R::ONE + self.e);
        let va = (self.mu * (R::ONE - self.e) / ra).sqrt();
        ([-ra, R::ZERO], [R::ZERO, -va])
    }

    /// The analytic position at time `t` (ferroastro-analytic's Kepler-equation solver).
    fn position(&self, t: R) -> [R; 2] {
        let big_e = eccentric_anomaly(R::PI + self.n * t, self.e).unwrap();
        let (s, c) = big_e.sin_cos();
        let b = (R::ONE - self.e * self.e).sqrt();
        [self.a * (c - self.e), self.a * b * s]
    }

    fn energy(&self, x: &[R], v: &[R]) -> R {
        (v[0] * v[0] + v[1] * v[1]).scale(0.5) - self.mu / (x[0] * x[0] + x[1] * x[1]).sqrt()
    }
}

fn config(order: u32, corrector: Corrector, compensated: bool) -> AbmConfig {
    AbmConfig {
        order,
        corrector,
        compensated,
        ..AbmConfig::default()
    }
}

#[derive(Debug, Clone, Copy)]
struct Errors {
    energy: f64,
    phase: f64,
    steps: u64,
}

/// Integrate `orbits` periods at the step `h`, sampling the energy and along-track errors.
fn kepler_errors<R: Real>(o: &Orbit<R>, h: R, orbits: R, cfg: AbmConfig) -> Errors {
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let e0 = o.energy(&x0, &v0);
    let mut abm = Abm::new(2, R::ZERO, &x0, &v0, h, cfg).unwrap();
    let (mut energy, mut phase) = (0.0_f64, 0.0_f64);
    let t_end = o.period * orbits;
    abm.integrate_to(&sys, t_end, |s, _| {
        if s.stats().steps % SAMPLE_EVERY == 0 {
            let (x, v) = (s.positions(), s.velocities());
            energy = energy.max(((o.energy(x, v) - e0) / e0).abs().to_f64());
            let r = o.position(s.time());
            let ang = (r[0] * x[1] - r[1] * x[0]).atan2(r[0] * x[0] + r[1] * x[1]);
            phase = phase.max(ang.abs().to_f64());
        }
        Ok(())
    })
    .unwrap();
    Errors {
        energy,
        phase,
        steps: abm.stats().steps,
    }
}

/// A3: Mercury's Keplerian orbit over 1000 orbits at h = 1/16 d.
///
/// Verifies: NUM-002, DEP-007
#[test]
fn kepler_mercury_thousand_orbits_energy_and_phase() {
    let vs = vectors();
    let energy_tol = vs.get("ABM-KEPLER-ENERGY-REL").unwrap();
    let phase_tol = vs.get("ABM-KEPLER-PHASE-RAD").unwrap();
    let o = Orbit::<f64>::mercury(&vs);
    let h = SECONDS_PER_DAY / STEPS_PER_DAY;
    println!(
        "A3: Mercury Kepler orbit, a = {:.3} km, e = {}, P = {:.4} d, h = {h} s, {A3_ORBITS} orbits",
        o.a,
        o.e,
        o.period / SECONDS_PER_DAY
    );
    for corrector in [Corrector::Pecec, Corrector::Pece] {
        for compensated in [true, false] {
            let cfg = config(13, corrector, compensated);
            let e = kepler_errors(&o, h, A3_ORBITS, cfg);
            println!(
                "  f64 {}: {} steps, max |dE/E| {:.3e}, max |phase| {:.3e} rad{}",
                cfg.describe(),
                e.steps,
                e.energy,
                e.phase,
                if compensated {
                    " (asserted)"
                } else {
                    " (reported)"
                }
            );
            if compensated {
                energy_tol.check(e.energy).unwrap();
                phase_tol.check(e.phase).unwrap();
            }
        }
    }
    let od = Orbit::<D>::mercury(&vs);
    let hd = D::from_f64(h);
    let e = kepler_errors(
        &od,
        hd,
        D::from_f64(A3_DD_ORBITS),
        config(13, Corrector::Pecec, true),
    );
    println!(
        "  double-double ABM13 PECEC, {A3_DD_ORBITS} orbits: {} steps, max |dE/E| {:.3e}, max \
         |phase| {:.3e} rad (reported)",
        e.steps, e.energy, e.phase
    );
}

/// The maximum relative energy error over `SHORT_ORBITS` orbits of the unit orbit, in
/// double-double, with `steps` steps per orbit.
fn unit_energy_error(o: &Orbit<D>, steps: u32, cfg: AbmConfig) -> (f64, f64) {
    let h = o.period / D::from_f64(f64::from(steps));
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let e0 = o.energy(&x0, &v0);
    let mut abm = Abm::new(2, D::ZERO, &x0, &v0, h, cfg).unwrap();
    let (mut energy, mut pos) = (0.0_f64, 0.0_f64);
    for _ in 0..SHORT_ORBITS * steps {
        abm.step(&sys).unwrap();
        let (x, v) = (abm.positions(), abm.velocities());
        energy = energy.max(((o.energy(x, v) - e0) / e0).abs().to_f64());
        let r = o.position(abm.time());
        let d = ((x[0] - r[0]) * (x[0] - r[0]) + (x[1] - r[1]) * (x[1] - r[1])).sqrt();
        pos = pos.max(d.to_f64());
    }
    (energy, pos)
}

/// A4: convergence order by halving h (`ABM-ORDER-SLOPE`), on the energy error; the position
/// error slopes are reported.
///
/// Verifies: NUM-002, DEP-007
#[test]
fn convergence_order_matches_the_configured_order() {
    let vs = vectors();
    let tol = vs.get("ABM-ORDER-SLOPE").unwrap();
    let o = Orbit::<D>::unit(&vs);
    println!(
        "A4: unit Kepler orbit, e = {}, {SHORT_ORBITS} orbits, double-double; slope = \
         log2(err(h)/err(h/2))",
        o.e.to_f64()
    );
    let mut failures = Vec::new();
    for corrector in [Corrector::Pece, Corrector::Pecec] {
        for (p, steps) in A4_STEPS {
            let cfg = config(p, corrector, true);
            let (e1, p1) = unit_energy_error(&o, steps, cfg);
            let (e2, p2) = unit_energy_error(&o, 2 * steps, cfg);
            let slope = (e1 / e2).log2();
            let pslope = (p1 / p2).log2();
            println!(
                "  {}: steps/orbit {steps}/{}: energy {e1:.3e} -> {e2:.3e}, slope {slope:.3}; \
                 position {p1:.3e} -> {p2:.3e}, slope {pslope:.3} (reported)",
                cfg.describe(),
                2 * steps
            );
            if let Err(e) = tol.check(slope - f64::from(p)) {
                failures.push(e.to_string());
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// `r·v`, rising through zero at perihelion.
fn rdotv<R: Real>(_t: R, x: &[R], v: &[R]) -> R {
    x[0] * v[0] + x[1] * v[1]
}

/// Located perihelion times over `SHORT_ORBITS` orbits, with the dense-output residual
/// `|r·v| / |d(r·v)/dt|` at each.
fn events<R: Real>(o: &Orbit<R>) -> Vec<(R, f64)> {
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let h = R::from_f64(SECONDS_PER_DAY / STEPS_PER_DAY);
    let mut abm = Abm::new(2, R::ZERO, &x0, &v0, h, AbmConfig::default()).unwrap();
    let mut out = Vec::new();
    let mut prev = rdotv(R::ZERO, &x0, &v0);
    let t_end = o.period * R::from_f64(f64::from(SHORT_ORBITS));
    abm.integrate_to(&sys, t_end, |s, _| {
        let now = rdotv(s.time(), s.positions(), s.velocities());
        if prev < R::ZERO && now >= R::ZERO {
            if let Some(te) = s.locate_event(rdotv)? {
                let (mut x, mut v) = ([R::ZERO; 2], [R::ZERO; 2]);
                s.dense_state(te, &mut x, &mut v)?;
                let mut a = [R::ZERO; 2];
                sys.acceleration(te, &x, &v, &mut a)?;
                let g = rdotv(te, &x, &v);
                let dg = v[0] * v[0] + v[1] * v[1] + x[0] * a[0] + x[1] * a[1];
                out.push((te, (g / dg).abs().to_f64()));
            }
        }
        prev = now;
        Ok(())
    })
    .unwrap();
    out
}

/// A5: NUM-010 perihelion events of ABM13 PECEC at h = 1/16 d (`ABM-EVENT-TIME-S`).
///
/// Verifies: NUM-010, DEP-007
#[test]
fn perihelion_events_on_dense_output() {
    let vs = vectors();
    let tol = vs.get("ABM-EVENT-TIME-S").unwrap();
    let od = Orbit::<D>::mercury(&vs);
    let ev = events(&od);
    assert_eq!(ev.len(), usize::try_from(SHORT_ORBITS).unwrap());
    let mut worst = 0.0_f64;
    for (k, &(t, _)) in ev.iter().enumerate() {
        let want = od.period * (D::from_f64(0.5) + D::from_i32(i32::try_from(k).unwrap()));
        worst = worst.max((t - want).abs().to_f64());
    }
    println!(
        "A5: double-double, {} perihelia: max |t_event - (P/2 + kP)| = {worst:.3e} s",
        ev.len()
    );
    tol.check(worst).unwrap();
    let of = Orbit::<f64>::mercury(&vs);
    let ef = events(&of);
    assert_eq!(ef.len(), ev.len());
    let residual = ef.iter().map(|e| e.1).fold(0.0_f64, f64::max);
    let analytic = ef
        .iter()
        .enumerate()
        .map(|(k, &(t, _))| {
            of.period
                .mul_add(-(0.5 + f64::from(u32::try_from(k).unwrap())), t)
                .abs()
        })
        .fold(0.0_f64, f64::max);
    println!(
        "A5: f64, {} perihelia: max |r.v|/|d(r.v)/dt| at the event = {residual:.3e} s (asserted); \
         max |t_event - (P/2 + kP)| = {analytic:.3e} s (reported: includes the orbit's round-off \
         phase)",
        ef.len()
    );
    tol.check(residual).unwrap();
}

fn bits<R: Real>(zs: &[R]) -> Vec<(u64, u64)> {
    zs.iter()
        .map(|z| {
            let (h, l) = z.to_parts();
            (h.to_bits(), l.to_bits())
        })
        .collect()
}

/// Run `total` steps, recording the state bits after each step, optionally interrupted after
/// `cut` steps by a snapshot, an encode/decode round trip and a restore.
fn trajectory<R: Real>(o: &Orbit<R>, total: u32, cut: Option<u32>) -> Vec<Vec<(u64, u64)>> {
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let h = R::from_f64(SECONDS_PER_DAY / STEPS_PER_DAY);
    let mut abm = Abm::new(2, R::ZERO, &x0, &v0, h, AbmConfig::default()).unwrap();
    let mut out = Vec::new();
    for i in 0..total {
        if cut == Some(i) {
            let snap = abm.snapshot().unwrap();
            let snap = round_trip(&snap);
            abm = Abm::restore(&snap).unwrap();
        }
        abm.step(&sys).unwrap();
        let mut row = bits(abm.positions());
        row.extend(bits(abm.velocities()));
        // Dense output in the middle of the step must also continue bit for bit.
        let (t0, t1) = abm.last_step_span().unwrap();
        let (mut x, mut v) = ([R::ZERO; 2], [R::ZERO; 2]);
        abm.dense_state((t0 + t1).scale(0.5), &mut x, &mut v)
            .unwrap();
        row.extend(bits(&x));
        row.extend(bits(&v));
        out.push(row);
    }
    out
}

#[cfg(feature = "serde")]
fn round_trip(s: &AbmSnapshot) -> AbmSnapshot {
    let bytes = postcard::to_allocvec(s).unwrap();
    postcard::from_bytes(&bytes).unwrap()
}

#[cfg(not(feature = "serde"))]
fn round_trip(s: &AbmSnapshot) -> AbmSnapshot {
    s.clone()
}

/// A6: checkpoint/resume is bitwise, inside the start-up (after 3 steps) and after it
/// (after 50), in f64 and double-double (CI-010).
///
/// Verifies: CI-010, DEP-007
#[test]
fn checkpoint_resume_is_bitwise_inside_and_after_the_startup() {
    const TOTAL: u32 = 80;
    let vs = vectors();
    let of = Orbit::<f64>::mercury(&vs);
    let od = Orbit::<D>::mercury(&vs);
    let base_f = trajectory(&of, TOTAL, None);
    let base_d = trajectory(&od, TOTAL, None);
    for cut in [1, 3, 11, 12, 13, 50] {
        assert_eq!(trajectory(&of, TOTAL, Some(cut)), base_f, "f64, cut {cut}");
        assert_eq!(trajectory(&od, TOTAL, Some(cut)), base_d, "dd, cut {cut}");
    }
    println!(
        "A6: {TOTAL} steps, resumed after 1, 3, 11, 12, 13 and 50 steps: states and mid-step dense \
         output bitwise equal (f64 and double-double; serde round trip: {})",
        cfg!(feature = "serde")
    );
}

/// Snapshot refusals and configuration errors carry their stable codes.
///
/// Verifies: CI-010, NUM-002
#[test]
fn refusals_carry_stable_codes() {
    let vs = vectors();
    let o = Orbit::<f64>::mercury(&vs);
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let h = SECONDS_PER_DAY / STEPS_PER_DAY;
    for order in [7, 15] {
        let err = Abm::new(2, 0.0, &x0, &v0, h, config(order, Corrector::Pecec, true)).unwrap_err();
        assert_eq!(err.code(), ErrorCode::IntegratorConfig);
    }
    assert_eq!(
        Abm::new(2, 0.0, &x0, &v0, 0.0, AbmConfig::default())
            .unwrap_err()
            .code(),
        ErrorCode::IntegratorConfig
    );
    let mut abm = Abm::new(2, 0.0, &x0, &v0, h, AbmConfig::default()).unwrap();
    assert!(abm.dense_state(0.0, &mut [0.0; 2], &mut [0.0; 2]).is_err());
    for _ in 0..20 {
        abm.step(&sys).unwrap();
    }
    let snap = abm.snapshot().unwrap();
    assert_eq!(
        Abm::<D>::restore(&snap).unwrap_err().code(),
        ErrorCode::IntegratorCheckpoint
    );
    let mut bad = snap.clone();
    bad.history.pop();
    assert_eq!(
        Abm::<f64>::restore(&bad).unwrap_err().code(),
        ErrorCode::IntegratorCheckpoint
    );
    let restored = Abm::<f64>::restore(&snap).unwrap();
    assert!(restored.last_step_span().is_none());
}

/// PECE and PECEC follow their definitions (HNW III.1): one regular step recomputed outside the
/// integrator from its snapshot, with the exact coefficients of `ferroastro_abm::rational`. The
/// unit Kepler orbit at 64 steps per orbit makes the predictor error (~1e-9) far larger than the
/// round-off of the two operation orders (~1e-15), so a mode that corrects with the wrong
/// derivative is visible.
///
/// Verifies: NUM-002
#[test]
fn one_regular_step_matches_the_mode_definitions() {
    const ORDER: u32 = 8;
    const STEPS_PER_ORBIT: f64 = 64.0; // lint: allow-literal(test design: a coarse step)
    /// Round-off of two operation orders of the same sums, relative to |y|.
    const REL: f64 = 1e-13; // lint: allow-literal(round-off bound, ~500 ulp)
    let vs = vectors();
    let o = Orbit::<f64>::unit(&vs);
    let (x0, v0) = o.start();
    let sys = Kepler { mu: o.mu };
    let h = o.period / STEPS_PER_ORBIT;
    let p = usize::try_from(ORDER).unwrap();
    let ab: Vec<f64> = ferroastro_abm::rational::adams_bashforth(p)
        .unwrap()
        .iter()
        .map(|q| q.to_real().unwrap())
        .collect();
    let am: Vec<f64> = ferroastro_abm::rational::adams_moulton(p)
        .unwrap()
        .iter()
        .map(|q| q.to_real().unwrap())
        .collect();
    let f = |y: &[f64]| -> Vec<f64> {
        let mut a = [0.0; 2];
        sys.acceleration(0.0, &y[..2], &y[2..], &mut a).unwrap();
        vec![y[2], y[3], a[0], a[1]]
    };
    let mut gaps = Vec::new();
    for corrector in [Corrector::Pece, Corrector::Pecec] {
        let mut abm = Abm::new(2, 0.0, &x0, &v0, h, config(ORDER, corrector, false)).unwrap();
        for _ in 0..(p - 1) {
            abm.step(&sys).unwrap();
        }
        let snap = abm.snapshot().unwrap();
        let hist: Vec<Vec<f64>> = snap
            .history
            .iter()
            .map(|r| r.iter().map(|q| q[0]).collect())
            .collect();
        let y0: Vec<f64> = snap.y.iter().map(|q| q[0]).collect();
        let pred: Vec<f64> = (0..4)
            .map(|i| h.mul_add((0..p).map(|j| ab[j] * hist[j][i]).sum::<f64>(), y0[i]))
            .collect();
        let corr = |fnew: &[f64]| -> Vec<f64> {
            (0..4)
                .map(|i| {
                    let part: f64 = (1..p).map(|j| am[j] * hist[j - 1][i]).sum();
                    h.mul_add(am[0].mul_add(fnew[i], part), y0[i])
                })
                .collect()
        };
        let c1 = corr(&f(&pred));
        let want = match corrector {
            Corrector::Pece => c1,
            Corrector::Pecec => corr(&f(&c1)),
        };
        abm.step(&sys).unwrap();
        let got: Vec<f64> = abm
            .positions()
            .iter()
            .chain(abm.velocities())
            .copied()
            .collect();
        let gap = got
            .iter()
            .zip(&want)
            .map(|(g, w)| ((g - w) / w.abs().max(1.0)).abs())
            .fold(0.0_f64, f64::max);
        println!(
            "{}: one regular step vs the definition: max relative gap {gap:.3e}",
            corrector.label()
        );
        gaps.push(gap);
    }
    assert!(gaps.iter().all(|&g| g <= REL), "{gaps:?}");
}
