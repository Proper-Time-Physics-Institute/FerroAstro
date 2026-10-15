// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-012: the f64 and double-double backends against an arbitrary-precision oracle
//! (`astro-float`, MIT, 384-bit working precision), as `proptest` property tests.
//!
//! Every operation has a pre-registered bound in `tests/vectors/real-backends.toml` (proved
//! bounds cite their theorem; elementary-function bounds are engineering bounds). Each test runs a
//! deterministic `proptest` runner (`ChaCha` seed, no failure persistence), asserts every case
//! against the bound, and prints the largest error it saw, the bound and the number of cases.
//!
//! Error units: `u² = 2⁻¹⁰⁶` (relative or absolute, as the vector says) for double-double; ulp of
//! the exact result for f64. Inputs are normalised double-words `hi + lo` with `hi = ±m·2ᵉ`,
//! `m ∈ [1, 2)` uniform in its 52 fraction bits, `lo = hi·t·2⁻⁵³`, `t ∈ (−1, 1)`.

#![allow(
    clippy::unwrap_used,
    reason = "test file: a failed unwrap is a test failure with a clear message"
)]

mod common;

use astro_float::{BigFloat, Consts, RoundingMode, Sign};
use ferroastro_real::{DoubleDouble, Real};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestCaseError, TestRng, TestRunner};
use std::cell::{Cell, RefCell};

/// Oracle working precision (bits): far beyond the 106 of a double-double.
const P: usize = 384;
const RM: RoundingMode = RoundingMode::ToEven;
/// Cases per operation (basic operations, elementary functions).
const BASIC_CASES: u32 = 4096; // lint: allow-literal(proptest case count, not physical)
const ELEMENTARY_CASES: u32 = 1024; // lint: allow-literal(proptest case count, not physical)

fn big(x: f64) -> BigFloat {
    BigFloat::from_f64(x, P)
}

fn big_dd<R: Real>(x: R) -> BigFloat {
    let (h, l) = x.to_parts();
    big(h).add(&big(l), P, RM)
}

/// Nearest-ish `f64` of a `BigFloat` (truncated to the leading 64-bit word; used only to report
/// and compare errors, where 1e-19 relative is ample). Self-checked in `oracle_conversion`.
fn to_f64(b: &BigFloat) -> f64 {
    if b.is_zero() {
        return 0.0;
    }
    let Some((m, _, s, e, _)) = b.as_raw_parts() else {
        return f64::NAN;
    };
    let top = *m.last().unwrap();
    // The mantissa is 0.1xxx... in binary: value = top / 2^64 * 2^e.
    let hi32 = f64::from(u32::try_from(top >> 32).unwrap());
    let lo32 = f64::from(u32::try_from(top & 0xffff_ffff).unwrap());
    let frac = hi32.mul_add(2.0_f64.powi(32), lo32) * 2.0_f64.powi(-64); // lint: allow-literal(2^32 and 2^64, word-size constants)
    let v = frac * 2.0_f64.powi(e);
    if s == Sign::Neg { -v } else { v }
}

const U2: f64 = f64::EPSILON * f64::EPSILON / 4.0; // 2^-106

fn rel_u2(computed: &BigFloat, exact: &BigFloat) -> f64 {
    let d = computed.sub(exact, P, RM).abs();
    if exact.is_zero() {
        return if d.is_zero() { 0.0 } else { f64::INFINITY };
    }
    to_f64(&d.div(&exact.abs(), P, RM)) / U2
}

fn abs_u2(computed: &BigFloat, exact: &BigFloat) -> f64 {
    to_f64(&computed.sub(exact, P, RM).abs()) / U2
}

/// Error of an f64 result in ulps of the exact value (binary64 ulp of [2^(e-1), 2^e) is 2^(e-53)).
fn ulp_err(computed: f64, exact: &BigFloat) -> f64 {
    let d = big(computed).sub(exact, P, RM).abs();
    if exact.is_zero() {
        return if d.is_zero() { 0.0 } else { f64::INFINITY };
    }
    let e = exact.exponent().unwrap();
    to_f64(&d) / 2.0_f64.powi(e - 53) // lint: allow-literal(binary64 significand width 53)
}

fn normal(neg: bool, e: i32, frac: u64) -> f64 {
    let bits = (u64::try_from(1023 + e).unwrap() << 52) | (frac >> 12); // lint: allow-literal(binary64 exponent bias)
    let x = f64::from_bits(bits);
    if neg { -x } else { x }
}

/// Normalised double-words with leading exponent in `e_range`.
fn dd(e_range: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = DoubleDouble> + Clone {
    (any::<bool>(), e_range, any::<u64>(), -1.0_f64..1.0).prop_map(|(neg, e, frac, t)| {
        let hi = normal(neg, e, frac);
        DoubleDouble::from_parts(hi, hi * t * f64::EPSILON / 2.0)
    })
}

/// Double-words uniform in `[lo, hi]` (leading word uniform, trailing word as in [`dd`]).
fn dd_uniform(lo: f64, hi: f64) -> impl Strategy<Value = DoubleDouble> + Clone {
    (lo..=hi, -1.0_f64..1.0)
        .prop_map(|(x, t)| DoubleDouble::from_parts(x, x * t * f64::EPSILON / 2.0))
}

/// Double-words with log-uniform magnitude in `[2^lo_e, 2^hi_e]` and random sign.
fn dd_log(lo_e: i32, hi_e: i32) -> impl Strategy<Value = DoubleDouble> + Clone {
    dd(lo_e..=hi_e - 1)
}

fn f64s(e_range: std::ops::RangeInclusive<i32>) -> impl Strategy<Value = f64> + Clone {
    (any::<bool>(), e_range, any::<u64>()).prop_map(|(neg, e, frac)| normal(neg, e, frac))
}

/// Run `cases` deterministic cases, assert each error against the vector bound, print the max.
fn check<S: Strategy>(id: &str, cases: u32, strategy: S, err: impl Fn(S::Value) -> f64)
where
    S::Value: std::fmt::Debug,
{
    let v = common::Bound::load(id);
    let bound = v.allowed();
    let cfg = Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner =
        TestRunner::new_with_rng(cfg, TestRng::deterministic_rng(RngAlgorithm::ChaCha));
    let max = Cell::new(0.0_f64);
    let worst = RefCell::new(String::new());
    let result = runner.run(&strategy, |x| {
        let label = format!("{x:?}");
        let e = err(x);
        if e > max.get() || e.is_nan() {
            max.set(e);
            *worst.borrow_mut() = label;
        }
        if e.is_nan() || e > bound {
            return Err(TestCaseError::fail(format!("{id}: error {e} > {bound}")));
        }
        Ok(())
    });
    println!(
        "{id}: max error {:.4} {} over {cases} cases (bound {bound}); worst input {}",
        max.get(),
        v.unit,
        worst.borrow()
    );
    result.unwrap();
    v.check(max.get());
}

/// The `BigFloat → f64` helper recovers binary64 values (it is used to report every error).
#[test]
fn oracle_conversion() {
    let samples = [1.0, -0.75, 3.0e-300, 1.0e300, core::f64::consts::PI]; // lint: allow-literal(conversion self-check samples)
    for x in samples {
        let back = to_f64(&big(x));
        assert!((back - x).abs() <= x.abs() * f64::EPSILON, "{x} -> {back}");
    }
    assert_eq!(big(1.0).exponent(), Some(1));
}

/// Verifies: DEP-012, NUM-006
#[test]
fn dd_add_sub_within_jmp2017_theorem_3_1() {
    let s = (dd(-60..=60), dd(-60..=60));
    check("REAL-DD-ADD-REL-ERR", BASIC_CASES, s.clone(), |(a, b)| {
        rel_u2(&big_dd(a + b), &big_dd(a).add(&big_dd(b), P, RM))
    });
    // Same exponents force heavy cancellation, the hard case of Alg. 6.
    check("REAL-DD-SUB-REL-ERR", BASIC_CASES, s, |(a, b)| {
        rel_u2(&big_dd(a - b), &big_dd(a).sub(&big_dd(b), P, RM))
    });
    check(
        "REAL-DD-SUB-REL-ERR",
        BASIC_CASES,
        (dd(0..=0), dd(0..=0)),
        |(a, b)| rel_u2(&big_dd(a - b), &big_dd(a).sub(&big_dd(b), P, RM)),
    );
}

/// Verifies: DEP-012, NUM-006
#[test]
fn dd_mul_scale_muladd_within_jmp2017() {
    check(
        "REAL-DD-MUL-REL-ERR",
        BASIC_CASES,
        (dd(-60..=60), dd(-60..=60)),
        |(a, b)| rel_u2(&big_dd(a * b), &big_dd(a).mul(&big_dd(b), P, RM)),
    );
    check(
        "REAL-DD-SCALE-REL-ERR",
        BASIC_CASES,
        (dd(-60..=60), f64s(-60..=60)),
        |(a, k)| rel_u2(&big_dd(a.scale(k)), &big_dd(a).mul(&big(k), P, RM)),
    );
    check(
        "REAL-DD-MULADD-ERR",
        BASIC_CASES,
        (dd(-30..=30), dd(-30..=30), dd(-60..=60)),
        |(a, b, c)| {
            let ab = big_dd(a).mul(&big_dd(b), P, RM);
            let exact = ab.add(&big_dd(c), P, RM);
            let scale = ab.abs().add(&big_dd(c).abs(), P, RM);
            let d = big_dd(a.mul_add(b, c)).sub(&exact, P, RM).abs();
            to_f64(&d.div(&scale, P, RM)) / U2
        },
    );
}

/// Verifies: DEP-012, NUM-006
#[test]
fn dd_div_recip_sqrt_within_proved_bounds() {
    check(
        "REAL-DD-DIV-REL-ERR",
        BASIC_CASES,
        (dd(-60..=60), dd(-60..=60)),
        |(a, b)| rel_u2(&big_dd(a / b), &big_dd(a).div(&big_dd(b), P, RM)),
    );
    check("REAL-DD-RECIP-REL-ERR", BASIC_CASES, dd(-200..=200), |a| {
        rel_u2(&big_dd(a.recip()), &big_dd(a).reciprocal(P, RM))
    });
    check(
        "REAL-DD-HYPOT-REL-ERR",
        BASIC_CASES,
        (dd(-200..=200), dd(-200..=200)),
        |(a, b)| {
            let (x, y) = (big_dd(a), big_dd(b));
            let exact = x.mul(&x, P, RM).add(&y.mul(&y, P, RM), P, RM).sqrt(P, RM);
            rel_u2(&big_dd(a.hypot(b)), &exact)
        },
    );
    check("REAL-DD-SQRT-REL-ERR", BASIC_CASES, dd(-200..=200), |a| {
        let a = a.abs();
        rel_u2(&big_dd(a.sqrt()), &big_dd(a).sqrt(P, RM))
    });
}

/// Verifies: DEP-012
#[test]
fn dd_exp_family_within_engineering_bounds() {
    let cc = RefCell::new(Consts::new().unwrap());
    check(
        "REAL-DD-EXP-REL-ERR",
        ELEMENTARY_CASES,
        dd_uniform(-600.0, 700.0), // lint: allow-literal(DEP-012 test domain, see vector)
        |x| {
            let exact = big_dd(x).exp(P, RM, &mut cc.borrow_mut());
            rel_u2(&big_dd(x.exp()), &exact)
        },
    );
    let expm1 = |x: DoubleDouble| {
        let exact = big_dd(x)
            .exp(P, RM, &mut cc.borrow_mut())
            .sub(&big(1.0), P, RM);
        rel_u2(&big_dd(x.exp_m1()), &exact)
    };
    check(
        "REAL-DD-EXPM1-REL-ERR",
        ELEMENTARY_CASES,
        dd_uniform(-30.0, 30.0), // lint: allow-literal(DEP-012 test domain, see vector)
        expm1,
    );
    check(
        "REAL-DD-EXPM1-REL-ERR",
        ELEMENTARY_CASES,
        dd_log(-40, -1),
        expm1,
    );
    check(
        "REAL-DD-POWF-REL-ERR",
        ELEMENTARY_CASES,
        (dd_uniform(1.0e-3, 1.0e3), dd_uniform(-20.0, 20.0)), // lint: allow-literal(DEP-012 test domain, see vector)
        |(x, y)| {
            let exact = big_dd(x).pow(&big_dd(y), P, RM, &mut cc.borrow_mut());
            rel_u2(&big_dd(x.powf(y)), &exact)
        },
    );
}

/// Verifies: DEP-012
#[test]
fn dd_log_family_within_engineering_bounds() {
    let cc = RefCell::new(Consts::new().unwrap());
    let ln = |x: DoubleDouble| {
        let x = x.abs();
        let exact = big_dd(x).ln(P, RM, &mut cc.borrow_mut());
        rel_u2(&big_dd(x.ln()), &exact)
    };
    check("REAL-DD-LN-REL-ERR", ELEMENTARY_CASES, dd(-500..=500), ln);
    check(
        "REAL-DD-LN-REL-ERR",
        ELEMENTARY_CASES,
        dd_log(-60, -4).prop_map(|d| DoubleDouble::ONE + d),
        ln,
    );
    let ln1p = |x: DoubleDouble| {
        let exact = big_dd(x)
            .add(&big(1.0), P, RM)
            .ln(P, RM, &mut cc.borrow_mut());
        rel_u2(&big_dd(x.ln_1p()), &exact)
    };
    check(
        "REAL-DD-LN1P-REL-ERR",
        ELEMENTARY_CASES,
        dd_uniform(-0.9, 100.0), // lint: allow-literal(DEP-012 test domain, see vector)
        ln1p,
    );
    check(
        "REAL-DD-LN1P-REL-ERR",
        ELEMENTARY_CASES,
        dd_log(-40, -1),
        ln1p,
    );
}

/// Verifies: DEP-012
#[test]
fn dd_trig_within_engineering_bounds() {
    let cc = RefCell::new(Consts::new().unwrap());
    let dom = dd_uniform(-1.0e4, 1.0e4); // lint: allow-literal(DEP-012 test domain, see vector)
    check("REAL-DD-SIN-ABS-ERR", ELEMENTARY_CASES, dom.clone(), |x| {
        abs_u2(
            &big_dd(x.sin()),
            &big_dd(x).sin(P, RM, &mut cc.borrow_mut()),
        )
    });
    check("REAL-DD-COS-ABS-ERR", ELEMENTARY_CASES, dom, |x| {
        abs_u2(
            &big_dd(x.cos()),
            &big_dd(x).cos(P, RM, &mut cc.borrow_mut()),
        )
    });
    let unit = dd_uniform(-0.999, 0.999); // lint: allow-literal(DEP-012 test domain, see vector)
    check(
        "REAL-DD-ASIN-ABS-ERR",
        ELEMENTARY_CASES,
        unit.clone(),
        |x| {
            abs_u2(
                &big_dd(x.asin()),
                &big_dd(x).asin(P, RM, &mut cc.borrow_mut()),
            )
        },
    );
    check("REAL-DD-ACOS-ABS-ERR", ELEMENTARY_CASES, unit, |x| {
        abs_u2(
            &big_dd(x.acos()),
            &big_dd(x).acos(P, RM, &mut cc.borrow_mut()),
        )
    });
    let side = dd_uniform(-1.0e3, 1.0e3); // lint: allow-literal(DEP-012 test domain, see vector)
    check(
        "REAL-DD-ATAN2-ABS-ERR",
        ELEMENTARY_CASES,
        (side.clone(), side),
        |(y, x)| {
            let mut cc = cc.borrow_mut();
            let (by, bx) = (big_dd(y), big_dd(x));
            let base = by.div(&bx, P, RM).atan(P, RM, &mut cc);
            let pi = cc.pi(P, RM);
            let exact = if x.hi() > 0.0 {
                base
            } else if y.hi() >= 0.0 {
                base.add(&pi, P, RM)
            } else {
                base.sub(&pi, P, RM)
            };
            abs_u2(&big_dd(y.atan2(x)), &exact)
        },
    );
}

/// Verifies: DEP-012, NUM-006
#[test]
fn f64_basic_operations_are_correctly_rounded() {
    let pair = (f64s(-100..=100), f64s(-100..=100));
    check("REAL-F64-ADD-ULP", BASIC_CASES, pair.clone(), |(a, b)| {
        ulp_err(add(a, b), &big(a).add(&big(b), P, RM))
    });
    check("REAL-F64-SUB-ULP", BASIC_CASES, pair.clone(), |(a, b)| {
        ulp_err(sub(a, b), &big(a).sub(&big(b), P, RM))
    });
    check("REAL-F64-MUL-ULP", BASIC_CASES, pair.clone(), |(a, b)| {
        ulp_err(mul(a, b), &big(a).mul(&big(b), P, RM))
    });
    check("REAL-F64-DIV-ULP", BASIC_CASES, pair, |(a, b)| {
        ulp_err(div(a, b), &big(a).div(&big(b), P, RM))
    });
    check("REAL-F64-SQRT-ULP", BASIC_CASES, f64s(-100..=100), |a| {
        let a = a.abs();
        ulp_err(Real::sqrt(a), &big(a).sqrt(P, RM))
    });
    check("REAL-F64-RECIP-ULP", BASIC_CASES, f64s(-100..=100), |a| {
        ulp_err(Real::recip(a), &big(a).reciprocal(P, RM))
    });
    check(
        "REAL-F64-MULADD-ULP",
        BASIC_CASES,
        (f64s(-50..=50), f64s(-50..=50), f64s(-100..=100)),
        |(a, b, c)| {
            let exact = big(a).mul(&big(b), P, RM).add(&big(c), P, RM);
            ulp_err(Real::mul_add(a, b, c), &exact)
        },
    );
}

/// The four operations through the `Real` bound (for f64 they are the hardware operations).
fn add<R: Real>(a: R, b: R) -> R {
    a + b
}
fn sub<R: Real>(a: R, b: R) -> R {
    a - b
}
fn mul<R: Real>(a: R, b: R) -> R {
    a * b
}
fn div<R: Real>(a: R, b: R) -> R {
    a / b
}
