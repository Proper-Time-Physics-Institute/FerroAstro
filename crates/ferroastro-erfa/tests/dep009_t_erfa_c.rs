// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-009 / LIC-004: every ported ERFA routine reproduces ERFA 2.0.1's own reference values
//! (`src/t_erfa_c.c`), in `f64` and in double-double.
//!
//! Expected values and tolerances are the vectors `ERFA-T-*` in
//! `tests/vectors/erfa-t_erfa_c.toml`, copied verbatim from `t_erfa_c.c` (`vvd(val, valok, dval)`).
//! The call arguments are repeated here, each line marked with its `t_erfa_c.c` origin.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::inconsistent_digit_grouping,
    reason = "test helpers: a failed unwrap is a test failure with a clear message; ERFA's \
              arguments are kept in ERFA's digit grouping"
)]

use ferroastro_erfa::{
    ErfaError, cal2jd, dtdb, jd2cal, taitt, tcbtdb, tcgtt, tdbtcb, tdbtt, tttai, tttcg, tttdb,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

/// Check `measured` against vector `id`, print the deviation, return it.
fn check(v: &VectorSet, id: &str, backend: &str, measured: f64) -> f64 {
    let diff = v
        .get(id)
        .unwrap()
        .check(measured)
        .unwrap_or_else(|e| panic!("{backend}: {e}"));
    println!("{id:<20} {backend:<14} measured {measured:<24e} |diff| {diff:e}");
    diff
}

fn d<R: Real>(x: f64) -> R {
    R::from_f64(x)
}

fn run_all<R: Real>(v: &VectorSet) {
    let b = R::NAME;
    // t_cal2jd: eraCal2jd(2003, 06, 01)
    let (djm0, djm) = cal2jd::<R>(2003, 6, 1).unwrap(); // lint: allow-literal(t_erfa_c.c t_cal2jd input)
    check(v, "ERFA-T-CAL2JD-DJM0", b, djm0.to_f64());
    check(v, "ERFA-T-CAL2JD-DJM", b, djm.to_f64());

    // t_jd2cal: eraJd2cal(2400000.5, 50123.9999)
    let cal = jd2cal::<R>(d(2_400_000.5), d(50_123.9999)).unwrap(); // lint: allow-literal(t_erfa_c.c t_jd2cal input)
    check(v, "ERFA-T-JD2CAL-Y", b, f64::from(cal.year));
    check(v, "ERFA-T-JD2CAL-M", b, f64::from(cal.month));
    check(v, "ERFA-T-JD2CAL-D", b, f64::from(cal.day));
    check(v, "ERFA-T-JD2CAL-FD", b, cal.fraction.to_f64());

    // t_taitt: eraTaitt(2453750.5, 0.892482639)
    let (t1, t2) = taitt::<R>(d(2_453_750.5), d(0.892_482_639)); // lint: allow-literal(t_erfa_c.c t_taitt input)
    check(v, "ERFA-T-TAITT-T1", b, t1.to_f64());
    check(v, "ERFA-T-TAITT-T2", b, t2.to_f64());

    // t_tttai: eraTttai(2453750.5, 0.892482639)
    let (a1, a2) = tttai::<R>(d(2_453_750.5), d(0.892_482_639)); // lint: allow-literal(t_erfa_c.c t_tttai input)
    check(v, "ERFA-T-TTTAI-A1", b, a1.to_f64());
    check(v, "ERFA-T-TTTAI-A2", b, a2.to_f64());

    // t_dtdb: eraDtdb(2448939.5, 0.123, 0.76543, 5.0123, 5525.242, 3190.0)
    let w = dtdb::<R>(
        d(2_448_939.5), // lint: allow-literal(t_erfa_c.c t_dtdb input)
        d(0.123),       // lint: allow-literal(t_erfa_c.c t_dtdb input)
        d(0.765_43),    // lint: allow-literal(t_erfa_c.c t_dtdb input)
        d(5.0123),      // lint: allow-literal(t_erfa_c.c t_dtdb input)
        d(5525.242),    // lint: allow-literal(t_erfa_c.c t_dtdb input)
        d(3190.0),      // lint: allow-literal(t_erfa_c.c t_dtdb input)
    );
    check(v, "ERFA-T-DTDB", b, w.to_f64());

    // t_tttdb: eraTttdb(2453750.5, 0.892855139, -0.000201)
    let (b1, b2) = tttdb::<R>(d(2_453_750.5), d(0.892_855_139), d(-0.000_201)); // lint: allow-literal(t_erfa_c.c t_tttdb input)
    check(v, "ERFA-T-TTTDB-B1", b, b1.to_f64());
    check(v, "ERFA-T-TTTDB-B2", b, b2.to_f64());

    // t_tdbtt: eraTdbtt(2453750.5, 0.892855137, -0.000201)
    let (t1, t2) = tdbtt::<R>(d(2_453_750.5), d(0.892_855_137), d(-0.000_201)); // lint: allow-literal(t_erfa_c.c t_tdbtt input)
    check(v, "ERFA-T-TDBTT-T1", b, t1.to_f64());
    check(v, "ERFA-T-TDBTT-T2", b, t2.to_f64());

    // t_tdbtcb: eraTdbtcb(2453750.5, 0.892855137)
    let (b1, b2) = tdbtcb::<R>(d(2_453_750.5), d(0.892_855_137)); // lint: allow-literal(t_erfa_c.c t_tdbtcb input)
    check(v, "ERFA-T-TDBTCB-B1", b, b1.to_f64());
    check(v, "ERFA-T-TDBTCB-B2", b, b2.to_f64());

    // t_tcbtdb: eraTcbtdb(2453750.5, 0.893019599)
    let (b1, b2) = tcbtdb::<R>(d(2_453_750.5), d(0.893_019_599)); // lint: allow-literal(t_erfa_c.c t_tcbtdb input)
    check(v, "ERFA-T-TCBTDB-B1", b, b1.to_f64());
    check(v, "ERFA-T-TCBTDB-B2", b, b2.to_f64());

    // t_tttcg: eraTttcg(2453750.5, 0.892482639)
    let (g1, g2) = tttcg::<R>(d(2_453_750.5), d(0.892_482_639)); // lint: allow-literal(t_erfa_c.c t_tttcg input)
    check(v, "ERFA-T-TTTCG-G1", b, g1.to_f64());
    check(v, "ERFA-T-TTTCG-G2", b, g2.to_f64());

    // t_tcgtt: eraTcgtt(2453750.5, 0.892862531)
    let (t1, t2) = tcgtt::<R>(d(2_453_750.5), d(0.892_862_531)); // lint: allow-literal(t_erfa_c.c t_tcgtt input)
    check(v, "ERFA-T-TCGTT-T1", b, t1.to_f64());
    check(v, "ERFA-T-TCGTT-T2", b, t2.to_f64());
}

/// Verifies: DEP-009, LIC-004
#[test]
fn t_erfa_c_reference_values_in_f64() {
    run_all::<f64>(&vectors());
}

/// Verifies: DEP-009, LIC-004, DEP-007
#[test]
fn t_erfa_c_reference_values_in_double_double() {
    run_all::<DoubleDouble>(&vectors());
}

/// ERFA's non-zero statuses map to stable codes; `BadDay` keeps the MJD ERFA computes anyway.
/// Verifies: DEP-009, API-002
#[test]
fn erfa_statuses_are_errors_with_codes() {
    let e = cal2jd::<f64>(-4800, 1, 1).unwrap_err(); // lint: allow-literal(year before ERFA IYMIN)
    assert_eq!((e.code(), e.erfa_status()), ("FA-E0800", -1));
    let e = cal2jd::<f64>(2003, 13, 1).unwrap_err(); // lint: allow-literal(calendar fixture)
    assert_eq!((e.code(), e.erfa_status()), ("FA-E0801", -2));
    let e = cal2jd::<f64>(2003, 2, 29).unwrap_err(); // lint: allow-literal(calendar fixture)
    assert_eq!((e.code(), e.erfa_status()), ("FA-E0802", -3));
    // 2003-02-29 is computed as 2003-03-01 by ERFA's formula.
    let (_, mar1) = cal2jd::<f64>(2003, 3, 1).unwrap(); // lint: allow-literal(calendar fixture)
    assert!(
        matches!(e, ErfaError::BadDay { mjd, .. } if f64::from(mjd).to_bits() == mar1.to_bits())
    );
    let e = jd2cal::<f64>(-68_570.0, 0.0).unwrap_err(); // lint: allow-literal(just below ERFA DJMIN)
    assert_eq!((e.code(), e.erfa_status()), ("FA-E0803", -1));
    assert!(jd2cal::<f64>(f64::NAN, 0.0).is_err());
    for msg in [e.to_string()] {
        for needle in ["FA-E0803", "Julian date", "valid range", "fix"] {
            assert!(msg.contains(needle), "{msg:?} lacks {needle:?}");
        }
    }
}
