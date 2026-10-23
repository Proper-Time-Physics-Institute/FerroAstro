// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DEP-010: cross-validation against hifitime 4.3.1 (feature `hifitime`, enabled for these tests
//! by the crate's self dev-dependency).
//!
//! * TAI − UTC from hifitime equals ERFA's `dat` (pyerfa goldens) at every 1972–2026 half-year
//!   boundary and one second before it: exact (leap seconds are integers).
//! * The same instants through hifitime's *native* UTC handling are counted and reported: in
//!   hifitime 4.3.1 it applies each leap second early (upstream issues #417 and #255), so the
//!   adapter uses only hifitime's table and leap-free calendar (see `ferroastro_time::utc`).
//! * Second by second across every IERS leap second: UTC → TAI → UTC is the identity, including
//!   23:59:60, and TAI advances by exactly 1 s per UTC second.
//! * hifitime's TDB (SPICE one-term model) against FerroAstro's (ERFA Fairhead & Bretagnon) over
//!   1550–2650: different series by design; the measured maximum is reported and held to the
//!   pre-registered analytic bound `HIFITIME-TDB-VS-ERFA-MAX-DIFF`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

mod common;

use common::{load, vectors};
use ferroastro_erfa::cal2jd;
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_time::utc::{UtcDateTime, tai_from_utc, tai_minus_utc, to_hifitime, utc_from_tai};
use ferroastro_time::{
    Convert, Epoch, J2000_JD, SECONDS_PER_DAY, Tai, Tdb, Tt, seconds_between,
    tdb_seconds_since_j2000,
};

/// UTC date-time of a (y, m, d, fd) row; fd is 0 or 86399/86400 in the golden table.
fn utc_of(y: i32, m: i32, d: i32, fd: f64) -> UtcDateTime {
    let secs = (fd * SECONDS_PER_DAY).round();
    // Whole seconds of the day, 0..86399: the casts are exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let s = secs as u32;
    UtcDateTime {
        year: y,
        month: u8::try_from(m).unwrap(),
        day: u8::try_from(d).unwrap(),
        hour: u8::try_from(s / 3600).unwrap(), // lint: allow-literal(seconds per hour)
        minute: u8::try_from((s / 60) % 60).unwrap(),
        second: u8::try_from(s % 60).unwrap(),
        nanos: 0,
    }
}

/// Verifies: DEP-010, REF-009
#[test]
fn tai_minus_utc_matches_erfa_dat_exactly() {
    let v = vectors();
    let g = load("erfa-dat-pyerfa.csv", &v, "HIFITIME-TAI-UTC-VS-ERFA-DAT");
    let (mut worst_leap, mut worst_jd) = (0.0_f64, 0.0_f64);
    let mut native_disagree = Vec::new();
    let mut steps = 0;
    let mut last = None;
    for row in &g.rows {
        let (y, m, d, fd) = (
            g.i(row, "iy"),
            g.i(row, "im"),
            g.i(row, "id"),
            g.f(row, "fd"),
        );
        let dat = g.f(row, "dat");
        let utc = utc_of(y, m, d, fd);
        let tai: Epoch<Tai> = tai_from_utc(utc).unwrap();
        let leap = tai_minus_utc(tai).unwrap();
        worst_leap = worst_leap.max((leap - dat).abs());
        // hifitime's own UTC -> TAI-UTC (report only; upstream #417/#255).
        let native = hifitime::Epoch::maybe_from_gregorian_utc(
            utc.year, utc.month, utc.day, utc.hour, utc.minute, utc.second, 0,
        )
        .unwrap()
        .leap_seconds(true)
        .unwrap();
        if (native - dat).abs() > 0.0 {
            native_disagree.push(format!(
                "{y}-{m:02}-{d:02} fd={fd}: hifitime {native} vs dat {dat}"
            ));
        }
        // Independent JD path: TAI epoch minus the UTC Julian date, in seconds.
        let (djm0, djm) = cal2jd::<f64>(y, m, d).unwrap();
        let utc_as_tai_scale: Epoch<Tai> = Epoch::from_jd_parts(djm0 + djm, fd).unwrap();
        worst_jd = worst_jd.max((seconds_between(tai, utc_as_tai_scale) - dat).abs());
        // UTC -> TAI -> UTC is the identity.
        assert_eq!(utc_from_tai(tai).unwrap(), utc, "UTC round trip at {utc:?}");
        if last.is_some_and(|l: f64| l < dat) {
            steps += 1;
        }
        last = Some(dat);
    }
    println!(
        "TAI-UTC vs erfa.dat: {} instants, {steps} leap steps seen, max |hifitime - dat| = {worst_leap:e} s, \
         max JD-path |diff| = {worst_jd:.3e} s",
        g.rows.len()
    );
    println!(
        "hifitime native UTC handling disagrees with erfa.dat at {} of {} instants (upstream \
         nyx-space/hifitime#417, #255); first: {:?}",
        native_disagree.len(),
        g.rows.len(),
        native_disagree.first()
    );
    v.get("HIFITIME-TAI-UTC-VS-ERFA-DAT")
        .unwrap()
        .check(worst_leap)
        .unwrap();
    v.get("HIFITIME-UTC-JD-PATH-MAX")
        .unwrap()
        .check(worst_jd)
        .unwrap();
}

/// Second by second over 23:59:50 .. 00:00:10 around every leap second in the golden `dat`
/// table: TAI advances by exactly 1 s per UTC second (61 s in a leap minute), 23:59:60 exists,
/// and UTC -> TAI -> UTC is the identity.
/// Verifies: DEP-010
#[test]
fn utc_round_trips_second_by_second_across_every_leap_second() {
    let v = vectors();
    let g = load("erfa-dat-pyerfa.csv", &v, "HIFITIME-TAI-UTC-VS-ERFA-DAT");
    let mut leaps = 0;
    let mut prev_dat: Option<f64> = None;
    for row in &g.rows {
        let dat = g.f(row, "dat");
        let is_step = prev_dat.is_some_and(|p| dat > p) && g.f(row, "fd") == 0.0;
        prev_dat = Some(dat);
        if !is_step {
            continue;
        }
        leaps += 1;
        let (y, m, d) = (g.i(row, "iy"), g.i(row, "im"), g.i(row, "id"));
        let (_, djm) = cal2jd::<f64>(y, m, d).unwrap();
        let eve = ferroastro_erfa::jd2cal::<f64>(2_400_000.5, djm - 1.0).unwrap(); // lint: allow-literal(MJD zero point, ERFA DJM0)
        let mut labels: Vec<UtcDateTime> = (50..=60)
            .map(|s| UtcDateTime {
                year: eve.year,
                month: u8::try_from(eve.month).unwrap(),
                day: u8::try_from(eve.day).unwrap(),
                hour: 23,
                minute: 59,
                second: s,
                nanos: 0,
            })
            .collect();
        labels.extend((0..=10).map(|s| UtcDateTime {
            year: y,
            month: u8::try_from(m).unwrap(),
            day: u8::try_from(d).unwrap(),
            hour: 0,
            minute: 0,
            second: s,
            nanos: 0,
        }));
        let mut last: Option<Epoch<Tai>> = None;
        for utc in labels {
            let tai: Epoch<Tai> = tai_from_utc(utc).unwrap();
            assert_eq!(utc_from_tai(tai).unwrap(), utc, "UTC round trip at {utc:?}");
            if let Some(l) = last {
                let step = seconds_between(tai, l);
                assert!((step - 1.0).abs() < 1e-6, "TAI step {step} s at {utc:?}"); // lint: allow-literal(one SI second per UTC second, 1 us JD resolution)
            }
            last = Some(tai);
        }
    }
    println!(
        "second-by-second checks across {leaps} leap seconds: UTC -> TAI -> UTC identity, 1 s TAI steps, 23:59:60 present"
    );
    assert!(
        leaps >= 27,
        "expected the 27 IERS leap seconds 1972-07 .. 2017-01, saw {leaps}"
    ); // lint: allow-literal(count of IERS leap seconds since 1972-01-01)
}

/// Before 1972 the adapter refuses UTC (FA-E0804) rather than guess the rubber-second offsets.
/// Verifies: DEP-010, API-002
#[test]
fn utc_before_1972_is_refused_with_a_code() {
    let e = tai_from_utc::<f64>(utc_of(1971, 12, 31, 0.0)).unwrap_err(); // lint: allow-literal(last pre-IERS UTC day)
    assert_eq!(e.code().as_str(), "FA-E0804");
    let early: Epoch<Tai> = ferroastro_time::from_calendar(1965, 1, 1, 0.0).unwrap(); // lint: allow-literal(pre-1972 date)
    assert_eq!(
        tai_minus_utc(early).unwrap_err().code().as_str(),
        "FA-E0804"
    );
}

/// hifitime TDB vs FerroAstro TDB (seconds since J2000.0 TDB) over the golden TT epochs.
/// Verifies: DEP-010
#[test]
fn hifitime_tdb_differs_from_erfa_within_the_analytic_bound() {
    let v = vectors();
    let g = load("erfa-time-pyerfa.csv", &v, "PYERFA-TT-TDB-MAX-DIFF");
    let (mut worst, mut at, mut worst_tt) = (0.0_f64, 0, 0.0_f64);
    let (mut worst_mid, mut n_mid) = (0.0_f64, 0);
    for row in &g.rows {
        type D = DoubleDouble;
        let tt: Epoch<Tt, D> =
            Epoch::from_jd_parts(D::from_f64(g.f(row, "tt1")), D::from_f64(g.f(row, "tt2")))
                .unwrap();
        let tai: Epoch<Tai, D> = tt.to_scale().unwrap();
        let tdb: Epoch<Tdb, D> = tt.to_scale().unwrap();
        let ours = tdb_seconds_since_j2000(tdb).seconds();
        let h = to_hifitime(tai).unwrap();
        // hifitime durations as exact seconds in double-double.
        let secs = |dur: hifitime::Duration| {
            let (c, ns) = dur.to_parts();
            let century = D::from_f64(36_525.0 * SECONDS_PER_DAY); // lint: allow-literal(days per Julian century)
            let whole = ns / 1_000_000_000; // lint: allow-literal(ns per second)
            let rem = ns % 1_000_000_000; // lint: allow-literal(ns per second)
            #[allow(clippy::cast_precision_loss)] // both < 2^53
            let (whole, rem) = (whole as f64, rem as f64);
            D::from_f64(f64::from(c)) * century
                + D::from_f64(whole)
                + D::from_f64(rem) / D::from_f64(1e9) // lint: allow-literal(ns per second)
        };
        let hifi_tdb = secs(h.to_tdb_duration());
        let diff = (hifi_tdb - ours).to_f64();
        if diff.abs() > worst {
            (worst, at) = (diff.abs(), g.i(row, "iy"));
        }
        let year = g.i(row, "iy");
        let window = 1950..=2050; // lint: allow-literal(ERFA dtdb stated-accuracy window, report only)
        if window.contains(&year) {
            worst_mid = worst_mid.max(diff.abs());
            n_mid += 1;
        }
        // hifitime TT (seconds since its J1900 TT origin) vs ours: same definition, 1 ns grid.
        let (t1, t2) = tt.jd_parts();
        let ours_tt_j2000 = ((t1 - D::from_f64(J2000_JD)) + t2) * D::from_f64(SECONDS_PER_DAY);
        let hifi_tt_j2000 = secs(h.to_tt_duration())
            - D::from_f64(
                (J2000_JD - ferroastro_time::utc::hifitime_tai_zero_jd()) * SECONDS_PER_DAY,
            );
        worst_tt = worst_tt.max((hifi_tt_j2000 - ours_tt_j2000).abs().to_f64());
    }
    println!(
        "hifitime TDB - ERFA TDB: max |diff| {worst:.6e} s (at year {at}); 1950-2050 ({n_mid} epochs) \
         max {worst_mid:.3e} s; hifitime TT vs ours max {worst_tt:.3e} s"
    );
    v.get("HIFITIME-TDB-VS-ERFA-MAX-DIFF")
        .unwrap()
        .check(worst)
        .unwrap();
}
