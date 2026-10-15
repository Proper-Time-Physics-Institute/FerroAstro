// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-010 / DEP-007: round trips A -> B -> A for every pair of {TAI, TT, TDB, TCB, TCG}, both
//! directions, at the 223 golden epochs (1550-2650), in f64 and in double-double.
//!
//! Start dates are the golden table's own TAI/TT/TDB/TCB/TCG columns ("JD at 0 h + fraction").
//! Bounds: vectors `TIME-ROUNDTRIP-F64-MAX` and `TIME-ROUNDTRIP-DD-MAX` (pre-registered).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{Golden, load, vectors};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_time::{Convert, Epoch, Tai, Tcb, Tcg, Tdb, TimeScale, Tt, seconds_between};

fn start<S: TimeScale, R: Real>(g: &Golden, row: &[String], c1: &str, c2: &str) -> Epoch<S, R> {
    Epoch::from_jd_parts(R::from_f64(g.f(row, c1)), R::from_f64(g.f(row, c2))).unwrap()
}

/// |A -> B -> A − A| in seconds.
fn trip<A: TimeScale, B: TimeScale, R: Real>(a: Epoch<A, R>) -> f64
where
    Epoch<A, R>: Convert<B, R>,
    Epoch<B, R>: Convert<A, R>,
{
    let b: Epoch<B, R> = a.to_scale().unwrap();
    let back: Epoch<A, R> = b.to_scale().unwrap();
    seconds_between(back, a).abs().to_f64()
}

fn run<R: Real>(id: &str) {
    let v = vectors();
    let g = load("erfa-time-pyerfa.csv", &v, "PYERFA-TT-TDB-MAX-DIFF");
    let mut worst = (0.0_f64, String::new());
    let mut per_pair: Vec<(String, f64)> = Vec::new();
    macro_rules! pairs {
        ($row:expr; $(($a:ty, $ca:literal, $b:ty, $cb:literal)),+ $(,)?) => {{
            let mut out = Vec::new();
            $(
                let ea: Epoch<$a, R> = start(&g, $row, concat!($ca, "1"), concat!($ca, "2"));
                let eb: Epoch<$b, R> = start(&g, $row, concat!($cb, "1"), concat!($cb, "2"));
                out.push((concat!(stringify!($a), "->", stringify!($b), "->", stringify!($a)),
                          trip::<$a, $b, R>(ea)));
                out.push((concat!(stringify!($b), "->", stringify!($a), "->", stringify!($b)),
                          trip::<$b, $a, R>(eb)));
            )+
            out
        }};
    }
    for row in &g.rows {
        let results = pairs!(row;
            (Tai, "tai", Tt, "tt"),
            (Tai, "tai", Tdb, "tdb"),
            (Tai, "tai", Tcb, "tcb"),
            (Tai, "tai", Tcg, "tcg"),
            (Tt, "tt", Tdb, "tdb"),
            (Tt, "tt", Tcb, "tcb"),
            (Tt, "tt", Tcg, "tcg"),
            (Tdb, "tdb", Tcb, "tcb"),
            (Tdb, "tdb", Tcg, "tcg"),
            (Tcb, "tcb", Tcg, "tcg"),
        );
        assert_eq!(results.len(), 20, "10 pairs x 2 directions");
        for (name, d) in results {
            assert!(d.is_finite());
            match per_pair.iter_mut().find(|(n, _)| n == name) {
                Some((_, m)) => *m = m.max(d),
                None => per_pair.push((name.to_owned(), d)),
            }
            if d > worst.0 {
                worst = (d, format!("{name} year {}", g.i(row, "iy")));
            }
        }
    }
    for (name, m) in &per_pair {
        println!("{:<14} {name:<16} max {m:.3e} s", R::NAME);
    }
    println!("{id} ({}): worst {:.3e} s at {}", R::NAME, worst.0, worst.1);
    if let Err(e) = v.get(id).unwrap().check(worst.0) {
        panic!("{e}");
    }
}

/// Verifies: DEP-010
#[test]
fn round_trips_every_scale_pair_f64() {
    run::<f64>("TIME-ROUNDTRIP-F64-MAX");
}

/// Verifies: DEP-010, DEP-007
#[test]
fn round_trips_every_scale_pair_double_double() {
    run::<DoubleDouble>("TIME-ROUNDTRIP-DD-MAX");
}

/// TDB seconds since J2000 (the propagation variable) round-trip through an epoch.
/// Verifies: DEP-010
#[test]
fn tdb_seconds_round_trip() {
    let v = vectors();
    let g = load("erfa-time-pyerfa.csv", &v, "PYERFA-TT-TDB-MAX-DIFF");
    let mut worst = 0.0_f64;
    for row in &g.rows {
        let e: Epoch<Tdb, DoubleDouble> = start(&g, row, "tdb1", "tdb2");
        let s = ferroastro_time::tdb_seconds_since_j2000(e);
        let back = ferroastro_time::tdb_from_seconds_since_j2000(s).unwrap();
        worst = worst.max(seconds_between(back, e).abs().to_f64());
    }
    println!("TDB seconds round trip (double-double): worst {worst:.3e} s");
    v.get("TIME-ROUNDTRIP-DD-MAX")
        .unwrap()
        .check(worst)
        .unwrap();
}
