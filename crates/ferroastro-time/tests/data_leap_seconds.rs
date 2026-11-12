// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `data` tier (DEP-010): FerroAstro's TAI − UTC (hifitime 4.3.1 table, FerroAstro boundary logic)
//! against the two published leap-second products, downloaded live:
//! * IERS Bulletin C `Leap_Second.dat` (IERS Earth Orientation Centre, Paris Observatory);
//! * IANA tz database `leap-seconds.list` (NTP timestamps).
//!
//! Network-dependent, so `#[ignore]`d. Run with
//! `cargo nextest run --workspace --profile data --run-ignored only`.
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: a failed unwrap is a test failure with a clear message"
)]

use ferroastro_erfa::jd2cal;
use ferroastro_testdata::fetch_url;
use ferroastro_time::utc::{UtcDateTime, tai_from_utc, tai_minus_utc};
use ferroastro_time::{Epoch, Tai};
use ferroastro_vectors::VectorSet;

const IERS_URL: &str = "https://hpiers.obspm.fr/iers/bul/bulc/Leap_Second.dat";
const IANA_URL: &str = "https://data.iana.org/time-zones/tzdb/leap-seconds.list";
/// MJD of the NTP epoch 1900-01-01 (IANA file convention).
const NTP_EPOCH_MJD: i64 = 15_020; // lint: allow-literal(NTP epoch as MJD, IANA file convention)

/// (MJD of 0 h UTC, TAI − UTC from then on) pairs.
type Table = Vec<(i64, i64)>;

fn iers(text: &str) -> Table {
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(f.len(), 5, "IERS row {l:?}");
            let mjd: f64 = f[0].parse().unwrap();
            assert!(mjd.fract() == 0.0, "IERS MJD {mjd}");
            #[allow(clippy::cast_possible_truncation)]
            (mjd as i64, f[4].parse().unwrap())
        })
        .collect()
}

fn iana(text: &str) -> Table {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let ntp: i64 = f[0].parse().unwrap();
            assert_eq!(ntp % 86_400, 0, "IANA timestamp {ntp} not at 0 h"); // lint: allow-literal(seconds per day)
            (ntp / 86_400 + NTP_EPOCH_MJD, f[1].parse().unwrap()) // lint: allow-literal(seconds per day)
        })
        .collect()
}

fn header_lines<'a>(text: &'a str, needles: &[&str]) -> Vec<&'a str> {
    text.lines()
        .filter(|l| needles.iter().any(|n| l.contains(n)))
        .collect()
}

fn utc_at(mjd: i64, second_of_day: u32) -> UtcDateTime {
    let cal = jd2cal::<f64>(2_400_000.5, f64::from(i32::try_from(mjd).unwrap())).unwrap(); // lint: allow-literal(MJD zero point, ERFA DJM0)
    UtcDateTime {
        year: cal.year,
        month: u8::try_from(cal.month).unwrap(),
        day: u8::try_from(cal.day).unwrap(),
        hour: u8::try_from(second_of_day / 3600).unwrap(), // lint: allow-literal(seconds per hour)
        minute: u8::try_from((second_of_day / 60) % 60).unwrap(),
        second: u8::try_from(second_of_day % 60).unwrap(),
        nanos: 0,
    }
}

/// Verifies: DEP-010
#[test]
#[ignore = "data: downloads IERS Leap_Second.dat and IANA leap-seconds.list (network); run with --profile data"]
fn data_leap_seconds_match_iers_and_iana() {
    let v = VectorSet::load_canonical().unwrap();
    let a = fetch_url(IERS_URL, "iers-Leap_Second.dat", None).unwrap();
    println!("IERS Leap_Second.dat {}", a.provenance());
    let iers_text = std::fs::read_to_string(&a.path).unwrap();
    for l in header_lines(&iers_text, &["Updated through", "expires"]) {
        println!("  IERS header: {}", l.trim_start_matches('#').trim());
    }
    let b = fetch_url(IANA_URL, "iana-leap-seconds.list", None).unwrap();
    println!("IANA leap-seconds.list {}", b.provenance());
    let iana_text = std::fs::read_to_string(&b.path).unwrap();
    for l in header_lines(
        &iana_text,
        &["#$", "#@", "#h", "Updated through", "File expires"],
    ) {
        println!("  IANA header: {l}");
    }

    let t_iers = iers(&iers_text);
    let t_iana = iana(&iana_text);
    println!(
        "IERS: {} entries, last MJD {} TAI-UTC {} s; IANA: {} entries, last MJD {} TAI-UTC {} s",
        t_iers.len(),
        t_iers.last().unwrap().0,
        t_iers.last().unwrap().1,
        t_iana.len(),
        t_iana.last().unwrap().0,
        t_iana.last().unwrap().1
    );
    assert_eq!(t_iers, t_iana, "the IERS and IANA products disagree");

    let mut worst = 0_i64;
    let mut checked = 0;
    for (i, &(mjd, dat)) in t_iers.iter().enumerate() {
        let at = tai_from_utc::<f64>(utc_at(mjd, 0)).unwrap();
        let ours: Epoch<Tai> = at;
        #[allow(clippy::cast_possible_truncation)]
        let got = tai_minus_utc(ours).unwrap() as i64;
        worst = worst.max((got - dat).abs());
        checked += 1;
        if i > 0 {
            // One second before the step: the previous value.
            let before = tai_from_utc::<f64>(utc_at(mjd - 1, 86_399)).unwrap(); // lint: allow-literal(23:59:59 as second of day)
            #[allow(clippy::cast_possible_truncation)]
            let got = tai_minus_utc(before).unwrap() as i64;
            worst = worst.max((got - t_iers[i - 1].1).abs());
            checked += 1;
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let worst_s = worst as f64;
    println!("FerroAstro TAI-UTC vs IERS/IANA: {checked} instants, max |diff| {worst_s} s");
    v.get("LEAPSEC-IERS-IANA-VS-FERROASTRO")
        .unwrap()
        .check(worst_s)
        .unwrap();
}
