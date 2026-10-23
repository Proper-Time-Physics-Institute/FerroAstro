// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-006 / DEP-007: one generic algorithm, monomorphised for every backend (no runtime switch).
//!
//! The algorithm is Newton's method for Kepler's equation `E − e sin E = M`. Each backend must
//! converge to its own precision: the residual is checked in units of the backend's epsilon, and
//! the f64 result must agree with the double-double one to f64 precision.

#![cfg_attr(all(feature = "f128", fa_nightly), feature(f128))]

use ferroastro_real::{DoubleDouble, Real};

/// Newton iteration for Kepler's equation, written once for every backend.
fn eccentric_anomaly<R: Real>(mean_anomaly: R, e: R) -> (R, R) {
    let mut big_e = mean_anomaly + e * mean_anomaly.sin();
    for _ in 0..60 {
        let (s, c) = big_e.sin_cos();
        let step = (big_e - e * s - mean_anomaly) / (R::ONE - e * c);
        big_e -= step;
        if step.abs() <= R::EPSILON * big_e.abs() {
            break;
        }
    }
    (big_e, (big_e - e * big_e.sin() - mean_anomaly).abs())
}

/// Verifies: NUM-006, DEP-007
#[test]
fn generic_kepler_solve_converges_in_each_backend() {
    // Dimensionless test orbit (mean anomaly 1 rad, e = 0.9): a hard case for Newton.
    let (m, e) = (1.0, 0.9); // lint: allow-literal(dimensionless test orbit)
    let (e64, r64) = eccentric_anomaly::<f64>(m, e);
    let (edd, rdd) =
        eccentric_anomaly::<DoubleDouble>(DoubleDouble::from_f64(m), DoubleDouble::from_f64(e));
    println!(
        "f64: E = {e64:.17}, residual {r64:e}; double-double: E = {:?}, residual {:e}",
        edd.to_parts(),
        rdd.to_f64()
    );
    assert!(r64 <= 4.0 * f64::EPSILON);
    assert!(rdd <= DoubleDouble::EPSILON.scale(4.0));
    assert!((edd.to_f64() - e64).abs() <= 4.0 * f64::EPSILON * e64);
    assert!(
        edd.lo() != 0.0,
        "the double-double solution carries a trailing word"
    );
    assert_eq!(<f64 as Real>::NAME, "f64");
    assert_eq!(DoubleDouble::NAME, "double-double");
}

/// The nightly-only binary128 backend agrees with double-double to double-double precision.
///
/// Verifies: DEP-007, NUM-006
#[cfg(all(feature = "f128", fa_nightly))]
#[test]
fn f128_backend_agrees_with_double_double() {
    let two = 2.0; // lint: allow-literal(test argument)
    let checks: [(
        &str,
        fn(f128) -> f128,
        fn(DoubleDouble) -> DoubleDouble,
        f64,
    ); 4] = [
        ("sqrt", Real::sqrt, Real::sqrt, two),
        ("exp", Real::exp, Real::exp, 1.0),
        ("sin", Real::sin, Real::sin, 1.0),
        ("ln", Real::ln, Real::ln, two),
    ];
    for (name, fq, fd, x) in checks {
        let q = fq(<f128 as Real>::from_f64(x));
        let d = fd(DoubleDouble::from_f64(x));
        let (h, l) = d.to_parts();
        let diff = (q - <f128 as Real>::from_parts(h, l)).abs() / q.abs();
        println!(
            "{name}({x}): f128 vs double-double relative difference {:e}",
            Real::to_f64(diff)
        );
        assert!(Real::to_f64(diff) < 1.0e-30, "{name}"); // lint: allow-literal(double-double precision ~ 1e-32)
    }
    assert!(ferroastro_real::F128_AVAILABLE);
}
