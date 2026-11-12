// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-009: every two-part-date routine gives the same instant whichever part is larger.
//!
//! ERFA accepts any split `(d1, d2)` of a Julian date and branches on `|d1| > |d2|`. The
//! `t_erfa_c.c` values and the pyerfa goldens only use "(JD at 0 h, fraction)", i.e. the first
//! branch, so this test exchanges the parts (and also tries `(JD, 0)` against `(0, JD)`) and
//! requires the same instant in double-double, where round-off is far below any physical term.
//! Bound: vector `ERFA-SPLIT-ORDER-DD-MAX` (pre-registered).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_erfa::{
    DAYSEC, cal2jd, dtdb, taitt, tcbtdb, tcgtt, tdbtcb, tdbtt, tttai, tttcg, tttdb,
};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_vectors::VectorSet;

type D = DoubleDouble;
/// A two-part-date routine.
type Routine = fn(D, D) -> (D, D);

/// |(a1 + a2) - (b1 + b2)| in seconds.
fn sec((a1, a2): (D, D), (b1, b2): (D, D)) -> f64 {
    (((a1 + a2) - (b1 + b2)) * D::from_f64(DAYSEC))
        .abs()
        .to_f64()
}

/// Verifies: DEP-009, DEP-007
#[test]
fn every_routine_is_independent_of_the_split_order() {
    let v = VectorSet::load_canonical().unwrap();
    let routines: [(&str, Routine); 8] = [
        ("taitt", taitt),
        ("tttai", tttai),
        ("tttdb", |a, b| {
            tttdb(a, b, dtdb(a, b, D::ZERO, D::ZERO, D::ZERO, D::ZERO))
        }),
        ("tdbtt", |a, b| {
            tdbtt(a, b, dtdb(a, b, D::ZERO, D::ZERO, D::ZERO, D::ZERO))
        }),
        ("tdbtcb", tdbtcb),
        ("tcbtdb", tcbtdb),
        ("tttcg", tttcg),
        ("tcgtt", tcgtt),
    ];
    let mut worst = (0.0_f64, "");
    let mut n = 0;
    // Dates every 25 years 1550-2650, each with a different month, day and fraction (fixtures).
    let (first, last) = (1550, 2650); // lint: allow-literal(date-range fixture, as the pyerfa goldens)
    for (k, year) in (0_i32..).zip((first..=last).step_by(25)) {
        let (j0, mjd) = cal2jd::<D>(year, 1 + k % 12, 1 + k % 28).unwrap();
        let jd0 = j0 + mjd;
        let frac = D::from_i32((k * 37) % 100) / D::from_i32(100); // lint: allow-literal(fraction-of-day fixture)
        let jd = jd0 + frac;
        for (name, f) in routines {
            for d in [
                sec(f(jd0, frac), f(frac, jd0)),
                sec(f(jd, D::ZERO), f(D::ZERO, jd)),
                sec(f(jd0, frac), f(D::ZERO, jd)),
            ] {
                n += 1;
                if d > worst.0 {
                    worst = (d, name);
                }
            }
        }
        // dtdb is a function of d1 + d2 only.
        let a = dtdb(jd0, frac, D::ZERO, D::ZERO, D::ZERO, D::ZERO);
        let b = dtdb(frac, jd0, D::ZERO, D::ZERO, D::ZERO, D::ZERO);
        let c = dtdb(D::ZERO, jd, D::ZERO, D::ZERO, D::ZERO, D::ZERO);
        for d in [(a - b).abs().to_f64(), (a - c).abs().to_f64()] {
            n += 1;
            if d > worst.0 {
                worst = (d, "dtdb");
            }
        }
    }
    println!(
        "split-order checks: {n}, worst {:.3e} s ({})",
        worst.0, worst.1
    );
    if let Err(e) = v.get("ERFA-SPLIT-ORDER-DD-MAX").unwrap().check(worst.0) {
        panic!("{e}");
    }
}
