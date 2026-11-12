// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! UTC and leap seconds through `hifitime` 4.x (feature `hifitime`; DEP-010).
//!
//! What comes from `hifitime` (MPL-2.0, used unmodified, `no_std` build without serde):
//! * the leap-second table (`LatestLeapSeconds`, IERS-announced entries only, 1972 onwards),
//!   whose timestamps are NTP-style UTC seconds since 1900-01-01 (the IANA `leap-seconds.list`
//!   values);
//! * the leap-free proleptic Gregorian calendar arithmetic (dates on its TAI scale);
//! * the integer-nanosecond TAI instant (`hifitime::Epoch`), used by [`to_hifitime`] and
//!   [`from_hifitime`].
//!
//! What is **not** taken from `hifitime`: its UTC ↔ TAI boundary logic. In hifitime 4.3.1 it
//! applies each new leap second early, by as many seconds as the previous TAI − UTC. It compares
//! TAI seconds with the UTC-labelled table timestamps (upstream issues nyx-space/hifitime#417
//! and #255, both open on 2026-10-15). The WP8 cross-check against ERFA `dat` found it
//! independently: at 23:59:59 UTC before each leap, hifitime reports the new TAI − UTC.
//! This module applies the table with the correct comparison:
//! * a UTC label is looked up in UTC;
//! * a TAI instant is compared with `T_i + ΔAT_i`;
//! * second 60 of an inserted leap second is representable.
//!
//! Resolution is 1 ns, hifitime's grid. Earlier UTC (1961–1971 rate offsets) is refused with
//! `FA-E0804`. hifitime's own TDB is not used here: it is SPICE's one-term approximation (see
//! the `dep010_hifitime` test for the measured difference).

use crate::{Epoch, Tai, TimeError, epoch};
use ferroastro_real::Real;
pub use hifitime;
use hifitime::leap_seconds::LatestLeapSeconds;
use hifitime::{Duration, MJD_J1900, MJD_OFFSET, TimeScale};

/// Nanoseconds per second.
const NS_PER_S: i128 = 1_000_000_000;
/// Nanoseconds per day.
const NS_PER_DAY: i128 = 86_400 * NS_PER_S;
/// Nanoseconds per Julian century (hifitime's `Duration` unit of `centuries`).
const NS_PER_CENTURY: i128 = 36_525 * NS_PER_DAY;
/// Capacity for the leap-second table (hifitime 4.3.1 has 42 entries, 28 from the IERS).
const MAX_LEAPS: usize = 64;

/// Julian date of hifitime's TAI reference epoch (`MJD_J1900 + MJD_OFFSET` = 2415020.5,
/// 1900 January 1, 0 h TAI). NTP-style UTC timestamps count from the same calendar instant.
#[must_use]
pub fn hifitime_tai_zero_jd() -> f64 {
    MJD_J1900 + MJD_OFFSET
}

/// A UTC calendar date and time of day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcDateTime {
    /// Year.
    pub year: i32,
    /// Month 1–12.
    pub month: u8,
    /// Day of month.
    pub day: u8,
    /// Hour 0–23.
    pub hour: u8,
    /// Minute 0–59.
    pub minute: u8,
    /// Second 0–60 (60 only during an inserted leap second).
    pub second: u8,
    /// Nanoseconds within the second.
    pub nanos: u32,
}

/// One IERS leap-second entry: from UTC label `utc_ns` (ns since 1900-01-01 0 h, leap-free
/// count) onwards, TAI − UTC is `dat_s` seconds.
#[derive(Debug, Clone, Copy)]
struct Leap {
    utc_ns: i128,
    dat_s: i128,
}

fn unavailable(reason: &'static str) -> TimeError {
    TimeError::UtcUnavailable {
        reason,
        fix: "use a valid UTC date from 1972 January 1 onwards, or work in TAI/TT directly",
    }
}

fn conv(what: &'static str) -> TimeError {
    TimeError::Conversion { what, jd: f64::NAN }
}

/// An integer-valued binary64 below 2^53 as i128 (`None` otherwise).
fn whole(x: f64) -> Option<i128> {
    if x.is_finite() && x.abs() < 9_007_199_254_740_992.0 && x.fract() == 0.0 {
        // Integer-valued and below 2^53: exact.
        #[allow(clippy::cast_possible_truncation)]
        let v = x as i64;
        Some(i128::from(v))
    } else {
        None
    }
}

/// hifitime's IERS-announced leap seconds, ascending, in a fixed-size table.
fn leaps() -> Result<([Leap; MAX_LEAPS], usize), TimeError> {
    let mut out = [Leap {
        utc_ns: 0,
        dat_s: 0,
    }; MAX_LEAPS];
    let mut n = 0;
    for l in LatestLeapSeconds::default().filter(|l| l.announced_by_iers) {
        let (Some(t), Some(d)) = (whole(l.timestamp_tai_s), whole(l.delta_at)) else {
            return Err(conv("hifitime leap-second table (non-integer entry)"));
        };
        let slot = out
            .get_mut(n)
            .ok_or_else(|| conv("hifitime leap-second table (too many entries)"))?;
        *slot = Leap {
            utc_ns: t * NS_PER_S,
            dat_s: d,
        };
        n += 1;
    }
    Ok((out, n))
}

fn duration_ns(d: Duration) -> i128 {
    let (centuries, ns) = d.to_parts();
    i128::from(centuries) * NS_PER_CENTURY + i128::from(ns)
}

fn duration_from_ns(total: i128) -> Result<Duration, TimeError> {
    let centuries = i16::try_from(total.div_euclid(NS_PER_CENTURY))
        .map_err(|_| conv("nanoseconds -> hifitime duration"))?;
    let ns = u64::try_from(total.rem_euclid(NS_PER_CENTURY))
        .map_err(|_| conv("nanoseconds -> hifitime duration"))?;
    Ok(Duration::from_parts(centuries, ns))
}

/// TAI epoch from nanoseconds since hifitime's TAI zero (exact to the nanosecond).
fn epoch_from_tai_ns<R: Real>(total: i128) -> Result<Epoch<Tai, R>, TimeError> {
    let what = "nanoseconds -> TAI epoch";
    let days = i32::try_from(total.div_euclid(NS_PER_DAY)).map_err(|_| conv(what))?;
    // rem < 8.64e13 < 2^53: exact in binary64; split into two u32 halves to avoid a lossy cast.
    let rem = u64::try_from(total.rem_euclid(NS_PER_DAY)).map_err(|_| conv(what))?;
    let hi = u32::try_from(rem >> 32).map_err(|_| conv(what))?;
    let lo = u32::try_from(rem & 0xFFFF_FFFF).map_err(|_| conv(what))?;
    let rem_ns = R::from_f64(f64::from(hi).mul_add(4_294_967_296.0, f64::from(lo)));
    let day = R::from_f64(hifitime_tai_zero_jd()) + R::from_i32(days);
    epoch((day, rem_ns / R::from_f64(86_400e9)), what)
}

/// Nanoseconds since hifitime's TAI zero of a TAI epoch (rounded to the nearest ns).
fn tai_ns_from_epoch<R: Real>(e: Epoch<Tai, R>) -> Result<i128, TimeError> {
    let (d1, d2) = e.jd_parts();
    let rel = d1 - R::from_f64(hifitime_tai_zero_jd());
    let int_days = rel.floor();
    let frac_days = (rel - int_days) + d2;
    let extra = frac_days.floor();
    let ns_in_day = ((frac_days - extra) * R::from_f64(86_400e9)).round();
    let (days_hi, days_lo) = (int_days + extra).to_parts();
    let (ns_hi, ns_lo) = ns_in_day.to_parts();
    let (Some(dh), Some(dl), Some(nh), Some(nl)) =
        (whole(days_hi), whole(days_lo), whole(ns_hi), whole(ns_lo))
    else {
        return Err(TimeError::Conversion {
            what: "TAI epoch -> nanoseconds",
            jd: (d1 + d2).to_f64(),
        });
    };
    Ok((dh + dl) * NS_PER_DAY + nh + nl)
}

/// The TAI epoch of a `hifitime` instant (exact to hifitime's 1 ns resolution).
///
/// # Errors
/// [`TimeError::Conversion`] (`FA-E0805`) outside ±2^31 days.
pub fn from_hifitime<R: Real>(e: hifitime::Epoch) -> Result<Epoch<Tai, R>, TimeError> {
    epoch_from_tai_ns(duration_ns(e.to_tai_duration()))
}

/// The `hifitime` instant of a TAI epoch, rounded to the nearest nanosecond.
///
/// # Errors
/// [`TimeError::Conversion`] (`FA-E0805`) outside hifitime's range.
pub fn to_hifitime<R: Real>(e: Epoch<Tai, R>) -> Result<hifitime::Epoch, TimeError> {
    Ok(hifitime::Epoch::from_tai_duration(duration_from_ns(
        tai_ns_from_epoch(e)?,
    )?))
}

/// The leap-free UTC label (ns since 1900-01-01 0 h) of a calendar date and time; second 60 is
/// counted as the first second of the next day.
fn utc_label_ns(utc: UtcDateTime) -> Result<i128, TimeError> {
    if utc.second > 60 {
        return Err(unavailable("UTC second must be 0..=60"));
    }
    let h = hifitime::Epoch::maybe_from_gregorian(
        utc.year,
        utc.month,
        utc.day,
        utc.hour,
        utc.minute,
        utc.second.min(59),
        utc.nanos,
        TimeScale::TAI,
    )
    .map_err(|_| unavailable("invalid UTC calendar date or time"))?;
    let leap = if utc.second == 60 { NS_PER_S } else { 0 };
    Ok(duration_ns(h.to_tai_duration()) + leap)
}

/// TAI − UTC (seconds) in force for UTC label `label_ns`: the last entry at or before it.
fn dat_at_label(label_ns: i128) -> Result<i128, TimeError> {
    let (table, n) = leaps()?;
    table
        .iter()
        .take(n)
        .take_while(|l| l.utc_ns <= label_ns)
        .last()
        .map(|l| l.dat_s)
        .ok_or_else(|| unavailable("no IERS leap-second entry applies before 1972 January 1"))
}

/// The TAI epoch of a UTC calendar date and time (second 60 allowed during an inserted leap
/// second).
///
/// # Errors
/// [`TimeError::UtcUnavailable`] (`FA-E0804`) for an invalid date or time, a second 60 that is
/// not a leap second, or a date before 1972; [`TimeError::Conversion`] (`FA-E0805`).
pub fn tai_from_utc<R: Real>(utc: UtcDateTime) -> Result<Epoch<Tai, R>, TimeError> {
    if utc.year < 1972 {
        return Err(unavailable(
            "UTC before 1972 is not supported (pre-IERS rate offsets)",
        ));
    }
    let label = utc_label_ns(utc)?;
    // Start of this UTC second as a leap-free label; for 23:59:60 it equals the next 0 h.
    let second_start = label - i128::from(utc.nanos);
    // TAI − UTC in force during this UTC second; for 23:59:60 it is the value before the step.
    let dat = if utc.second == 60 {
        let dat = dat_at_label(second_start - NS_PER_S)?;
        let (table, n) = leaps()?;
        let inserted = table
            .iter()
            .take(n)
            .any(|l| l.utc_ns == second_start && l.dat_s == dat + 1);
        if !inserted {
            return Err(unavailable("second 60 outside an inserted leap second"));
        }
        dat
    } else {
        dat_at_label(second_start)?
    };
    epoch_from_tai_ns(label + dat * NS_PER_S)
}

/// TAI − UTC, the leap-free UTC label of a TAI instant, and whether it lies inside an inserted
/// leap second (whose label is then the next day's 0 h onwards).
fn utc_of_tai_ns(tai: i128) -> Result<(i128, i128, bool), TimeError> {
    let (table, n) = leaps()?;
    let entries = table.get(..n).unwrap_or_default();
    // Last entry already in force at this TAI instant: T_i + ΔAT_i <= tai.
    let idx = entries
        .iter()
        .rposition(|l| l.utc_ns + l.dat_s * NS_PER_S <= tai)
        .ok_or_else(|| unavailable("no IERS leap-second entry applies before 1972 January 1"))?;
    let cur = entries.get(idx).ok_or_else(|| conv("leap-second lookup"))?;
    // Inside the next entry's inserted second? It spans TAI [T_j + ΔAT_i, T_j + ΔAT_j).
    let leap = entries.get(idx + 1).is_some_and(|next| {
        next.dat_s == cur.dat_s + 1 && tai >= next.utc_ns + cur.dat_s * NS_PER_S
    });
    Ok((cur.dat_s, tai - cur.dat_s * NS_PER_S, leap))
}

/// TAI − UTC in seconds at a TAI epoch (IERS leap seconds only, 1972 onwards). During an
/// inserted leap second it is the value before the step.
///
/// # Errors
/// [`TimeError::UtcUnavailable`] (`FA-E0804`) before 1972 January 1;
/// [`TimeError::Conversion`] (`FA-E0805`).
pub fn tai_minus_utc<R: Real>(e: Epoch<Tai, R>) -> Result<f64, TimeError> {
    let (dat, _, _) = utc_of_tai_ns(tai_ns_from_epoch(e)?)?;
    i32::try_from(dat)
        .map(f64::from)
        .map_err(|_| conv("TAI - UTC"))
}

/// The UTC calendar date and time of a TAI epoch (rounded to the nanosecond; second 60 during
/// an inserted leap second).
///
/// # Errors
/// [`TimeError::UtcUnavailable`] (`FA-E0804`) before 1972; [`TimeError::Conversion`]
/// (`FA-E0805`).
pub fn utc_from_tai<R: Real>(e: Epoch<Tai, R>) -> Result<UtcDateTime, TimeError> {
    let (_, label, leap) = utc_of_tai_ns(tai_ns_from_epoch(e)?)?;
    // During the leap second the label runs from the next day's 0 h: show it as 23:59:60.
    let shown = if leap { label - NS_PER_S } else { label };
    let h = hifitime::Epoch::from_tai_duration(duration_from_ns(shown)?);
    let (year, month, day, hour, minute, second, nanos) = h.to_gregorian(TimeScale::TAI);
    Ok(UtcDateTime {
        year,
        month,
        day,
        hour,
        minute,
        second: if leap { 60 } else { second },
        nanos,
    })
}
