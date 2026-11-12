// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! IAS15 checkpoint/resume (CI-010): a run interrupted between two steps, snapshotted and
//! restored into a new integrator continues bit for bit like the uninterrupted run, in f64 and
//! double-double. The problem is the dimensionless Kepler orbit (GM = 1, a = 1), so its parameters
//! are mathematical values, not physical ones.

#![expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "test: fixed-length state slices; a failed unwrap is a test failure"
)]

use ferroastro_core::ErrorCode;
use ferroastro_integrators::{Ias15, Ias15Config, IntegratorError, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};

const E: f64 = 0.5; // lint: allow-literal(test-orbit eccentricity, R&S 2015 Kepler test)
const DT0: f64 = 0.1; // lint: allow-literal(first trial step; the controller adapts it)
const STEPS: usize = 600;
const INTERRUPT: usize = 257;

/// Planar Kepler problem with GM = 1, in backend `R`.
struct Kepler;

impl<R: Real> SecondOrderSystem<R> for Kepler {
    fn dimension(&self) -> usize {
        2
    }
    fn acceleration(&self, _t: R, x: &[R], _v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let r2 = x[0].mul_add(x[0], x[1] * x[1]);
        let k = -(R::ONE / (r2 * r2.sqrt()));
        a[0] = k * x[0];
        a[1] = k * x[1];
        Ok(())
    }
}

fn start<R: Real>() -> Ias15<R> {
    // Periapsis of an a = 1 orbit: r = 1 − e, v = √((1 + e)/(1 − e)).
    let e = R::from_f64(E);
    let x0 = [R::ONE - e, R::ZERO];
    let v0 = [R::ZERO, ((R::ONE + e) / (R::ONE - e)).sqrt()];
    let cfg = Ias15Config {
        dense_output: true,
        ..Ias15Config::default()
    };
    Ias15::new(2, R::ZERO, &x0, &v0, R::from_f64(DT0), cfg).unwrap()
}

/// Bit patterns of time, positions and velocities after every step.
fn trace<R: Real>(ias: &mut Ias15<R>, steps: usize) -> Vec<Vec<(u64, u64)>> {
    let bits = |z: R| {
        let (h, l) = z.to_parts();
        (h.to_bits(), l.to_bits())
    };
    let inf = R::from_f64(f64::INFINITY);
    (0..steps)
        .map(|_| {
            ias.step(&Kepler, inf).unwrap();
            std::iter::once(ias.time())
                .chain(ias.positions().iter().copied())
                .chain(ias.velocities().iter().copied())
                .map(bits)
                .collect()
        })
        .collect()
}

fn resume_is_bitwise<R: Real>() {
    let mut full = start::<R>();
    let reference = trace(&mut full, STEPS);
    let mut first = start::<R>();
    let mut got = trace(&mut first, INTERRUPT);
    let snap = first.snapshot().unwrap();
    drop(first);
    let mut resumed = Ias15::<R>::restore(&snap).unwrap();
    got.extend(trace(&mut resumed, STEPS - INTERRUPT));
    let differing = reference.iter().zip(&got).filter(|(a, b)| a != b).count();
    println!(
        "{}: {STEPS} steps, interrupted after {INTERRUPT}: {differing} differing steps; \
         stats equal: {}",
        R::NAME,
        full.stats() == resumed.stats()
    );
    assert_eq!(differing, 0);
    assert_eq!(full.stats(), resumed.stats());
    assert_eq!(full.snapshot().unwrap(), resumed.snapshot().unwrap());
}

/// Verifies: CI-010
#[test]
fn ias15_resume_from_snapshot_is_bitwise_in_f64() {
    resume_is_bitwise::<f64>();
}

/// Verifies: CI-010, DEP-007
#[test]
fn ias15_resume_from_snapshot_is_bitwise_in_double_double() {
    resume_is_bitwise::<DoubleDouble>();
}

/// A snapshot is refused by another backend, with wrong lengths, or with a non-normalised
/// double-double pair, each with FA-E0405.
///
/// Verifies: CI-010, API-002
#[test]
fn bad_snapshots_are_refused_with_a_code() {
    let mut dd = start::<DoubleDouble>();
    let _ = trace(&mut dd, 3);
    let snap = dd.snapshot().unwrap();
    let e = Ias15::<f64>::restore(&snap).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorCheckpoint);
    assert!(e.to_string().contains("backend"), "{e}");

    let mut short = snap.clone();
    short.x.pop();
    let e = Ias15::<DoubleDouble>::restore(&short).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorCheckpoint, "{e}");

    let mut edited = snap.clone();
    // |lo| >= ulp(hi)/2: not a normalised double-double.
    edited.x[0][1] = edited.x[0][0];
    let e = Ias15::<DoubleDouble>::restore(&edited).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorCheckpoint, "{e}");

    let mut nan = snap;
    nan.v[1][0] = f64::NAN;
    let e = Ias15::<DoubleDouble>::restore(&nan).unwrap_err();
    assert_eq!(e.code(), ErrorCode::IntegratorCheckpoint, "{e}");
}
