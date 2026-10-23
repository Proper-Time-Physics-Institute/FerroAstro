// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Typed time-scale conversions for FerroAstro (DEP-010, REF-009, DEP-007).
//!
//! * The uniform scales TAI, TT, TDB, TCB and TCG are type markers from `ferroastro-core`;
//!   an [`Epoch<S, R>`] is a two-part Julian date on scale `S` in backend `R` (`f64` by default,
//!   or double-double). [`Convert`] maps any scale to any other, e.g. `Epoch<Tt>` → `Epoch<Tdb>`.
//! * The conversions are the ERFA 2.0.1 algorithms ported in `ferroastro-erfa`:
//!   TT = TAI + 32.184 s; TCG from TT by `L_G` (IAU 2000 B1.9); TDB from TCB by `L_B` and
//!   `TDB_0` (IAU 2006 B3); TDB − TT from the geocentric Fairhead & Bretagnon (1990) series
//!   (`dtdb` with u = v = 0), the TDB of a barycentric ephemeris such as DE440.
//! * **TDB → TT** is the exact inverse of TT → TDB: the fixed point of
//!   `tt = tdb − dtdb(tt)`, iterated [`TDB_TO_TT_ITERATIONS`] times (contraction factor
//!   |d dtdb/dt| ≈ 3.3e-10, so 3 iterations leave ≈ 6e-32 s). ERFA's `tdbtt` instead takes a
//!   caller-supplied TDB − TT; evaluating it once at the TDB date (as pyerfa users usually do)
//!   differs from the exact inverse by ≈ 6e-13 s.
//! * Conversions route along the tree TAI–TT–TCG, TT–TDB–TCB, so each pair takes the shortest
//!   path (TDB → TCB never passes through TT).
//! * Propagation runs in TDB seconds since J2000.0: [`tdb_seconds_since_j2000`] and
//!   [`tdb_from_seconds_since_j2000`].
//! * UTC exists only behind the optional `hifitime` feature (module `utc`); it is not a uniform scale
//!   and never a type marker. `Epoch<Tai>` is its anchor.
//!
//! ```
//! use ferroastro_time::{Convert, Epoch, Tdb, Tt, from_calendar};
//!
//! // 2006 January 15, 21:24:37.5 TT, to TDB.
//! let tt: Epoch<Tt> = from_calendar(2006, 1, 15, 0.892_100_694_444_444_4)?;
//! let tdb: Epoch<Tdb> = tt.to_scale()?;
//! let dt = ((tdb.jd_parts().0 - tt.jd_parts().0) + (tdb.jd_parts().1 - tt.jd_parts().1)) * 86_400.0;
//! assert!(dt.abs() < 2e-3); // TDB − TT is below 2 ms
//! # Ok::<(), ferroastro_time::TimeError>(())
//! ```

use ferroastro_core::ErrorCode;
use ferroastro_core::units::TimeSpan;
use ferroastro_erfa::{
    Calendar, ErfaError, cal2jd, dtdb, jd2cal, taitt, tcbtdb, tcgtt, tdbtcb, tdbtt, tttai, tttcg,
    tttdb,
};
use ferroastro_real::Real;

pub use ferroastro_core::time::{
    DAYS_PER_JULIAN_CENTURY, Epoch, J2000_JD, SECONDS_PER_DAY, Tai, Tcb, Tcg, Tdb, TimeScale, Tt,
};
pub use ferroastro_erfa::Calendar as CalendarDate;

#[cfg(feature = "hifitime")]
pub mod utc;

/// Fixed-point iterations used by TDB → TT (see the crate documentation).
pub const TDB_TO_TT_ITERATIONS: usize = 3;

/// Errors of the time-scale layer, each with a stable `FA-E08xx` code (API-002).
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum TimeError {
    /// A calendar input or Julian date was rejected by the ERFA routines (`FA-E0800`–`FA-E0803`).
    #[error("{0}")]
    Calendar(ErfaError),
    /// A conversion produced a non-finite or unrepresentable date (`FA-E0805`).
    #[error(
        "FA-E0805: {what} gave a non-finite or unrepresentable date (input JD {jd} d); valid \
         range: finite two-part Julian dates within ±1e9 d; fix: check the input epoch"
    )]
    Conversion {
        /// The conversion that failed.
        what: &'static str,
        /// The input Julian date (binary64).
        jd: f64,
    },
    /// UTC or leap seconds are unavailable (`FA-E0804`).
    #[error(
        "FA-E0804: {reason}; valid range: UTC from 1972 January 1 with the `hifitime` feature; fix: {fix}"
    )]
    UtcUnavailable {
        /// Why.
        reason: &'static str,
        /// Remediation.
        fix: &'static str,
    },
}

impl TimeError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            TimeError::Calendar(e) => erfa_code(e),
            TimeError::Conversion { .. } => ErrorCode::TimeConversion,
            TimeError::UtcUnavailable { .. } => ErrorCode::UtcUnavailable,
        }
    }
}

/// The `ferroastro-core` code of an ERFA status (the strings are identical; tested).
#[must_use]
pub const fn erfa_code(e: &ErfaError) -> ErrorCode {
    match e {
        ErfaError::BadYear { .. } => ErrorCode::CalendarYear,
        ErfaError::BadMonth { .. } => ErrorCode::CalendarMonth,
        ErfaError::BadDay { .. } => ErrorCode::CalendarDay,
        // `ErfaError` is non-exhaustive; every other status is a date-range failure.
        _ => ErrorCode::JulianDateRange,
    }
}

impl From<ErfaError> for TimeError {
    fn from(e: ErfaError) -> Self {
        TimeError::Calendar(e)
    }
}

/// Build an epoch from conversion output, rejecting non-finite parts.
fn epoch<S: TimeScale, R: Real>(
    (d1, d2): (R, R),
    what: &'static str,
) -> Result<Epoch<S, R>, TimeError> {
    Epoch::from_jd_parts(d1, d2).map_err(|_| TimeError::Conversion {
        what,
        jd: (d1 + d2).to_f64(),
    })
}

// ---------------------------------------------------------------------------------------------
// Edge conversions on two-part dates (the tree TAI–TT–TCG, TT–TDB–TCB).

fn tai_tt<R: Real>((a, b): (R, R)) -> (R, R) {
    taitt(a, b)
}
fn tt_tai<R: Real>((a, b): (R, R)) -> (R, R) {
    tttai(a, b)
}
fn tt_tcg<R: Real>((a, b): (R, R)) -> (R, R) {
    tttcg(a, b)
}
fn tcg_tt<R: Real>((a, b): (R, R)) -> (R, R) {
    tcgtt(a, b)
}
fn tdb_tcb<R: Real>((a, b): (R, R)) -> (R, R) {
    tdbtcb(a, b)
}
fn tcb_tdb<R: Real>((a, b): (R, R)) -> (R, R) {
    tcbtdb(a, b)
}

/// Geocentric TDB − TT in seconds at a two-part date (ERFA `dtdb` with u = v = 0).
#[must_use]
pub fn tdb_minus_tt<R: Real>(d1: R, d2: R) -> R {
    dtdb(d1, d2, R::ZERO, R::ZERO, R::ZERO, R::ZERO)
}

fn tt_tdb<R: Real>((a, b): (R, R)) -> (R, R) {
    tttdb(a, b, tdb_minus_tt(a, b))
}

/// Exact inverse of [`tt_tdb`]: the fixed point of `tt = tdb − dtdb(tt)`.
fn tdb_tt<R: Real>((b1, b2): (R, R)) -> (R, R) {
    let mut tt = (b1, b2);
    for _ in 0..TDB_TO_TT_ITERATIONS {
        tt = tdbtt(b1, b2, tdb_minus_tt(tt.0, tt.1));
    }
    tt
}

/// Conversion of an epoch to time scale `T`.
pub trait Convert<T: TimeScale, R: Real> {
    /// The same instant on scale `T`.
    ///
    /// # Errors
    /// [`TimeError::Conversion`] (`FA-E0805`) if the result is not finite.
    fn to_scale(self) -> Result<Epoch<T, R>, TimeError>;
}

impl<S: TimeScale, R: Real> Convert<S, R> for Epoch<S, R> {
    fn to_scale(self) -> Result<Epoch<S, R>, TimeError> {
        Ok(self)
    }
}

macro_rules! route {
    ($from:ty => $to:ty : $($step:ident),+) => {
        impl<R: Real> Convert<$to, R> for Epoch<$from, R> {
            fn to_scale(self) -> Result<Epoch<$to, R>, TimeError> {
                let parts = self.jd_parts();
                $(let parts = $step(parts);)+
                epoch(parts, concat!(stringify!($from), " -> ", stringify!($to)))
            }
        }
    };
}

route!(Tai => Tt: tai_tt);
route!(Tai => Tdb: tai_tt, tt_tdb);
route!(Tai => Tcb: tai_tt, tt_tdb, tdb_tcb);
route!(Tai => Tcg: tai_tt, tt_tcg);
route!(Tt => Tai: tt_tai);
route!(Tt => Tdb: tt_tdb);
route!(Tt => Tcb: tt_tdb, tdb_tcb);
route!(Tt => Tcg: tt_tcg);
route!(Tdb => Tai: tdb_tt, tt_tai);
route!(Tdb => Tt: tdb_tt);
route!(Tdb => Tcb: tdb_tcb);
route!(Tdb => Tcg: tdb_tt, tt_tcg);
route!(Tcb => Tai: tcb_tdb, tdb_tt, tt_tai);
route!(Tcb => Tt: tcb_tdb, tdb_tt);
route!(Tcb => Tdb: tcb_tdb);
route!(Tcb => Tcg: tcb_tdb, tdb_tt, tt_tcg);
route!(Tcg => Tai: tcg_tt, tt_tai);
route!(Tcg => Tt: tcg_tt);
route!(Tcg => Tdb: tcg_tt, tt_tdb);
route!(Tcg => Tcb: tcg_tt, tt_tdb, tdb_tcb);

/// An epoch from a proleptic Gregorian date and the fraction of the day on scale `S`, as the
/// two-part date `(JD at 0 h, fraction)` (ERFA `cal2jd`).
///
/// # Errors
/// [`TimeError::Calendar`] (`FA-E0800`–`FA-E0802`) for an invalid date, and
/// [`TimeError::Conversion`] (`FA-E0805`) if `fraction` is not finite.
pub fn from_calendar<S: TimeScale, R: Real>(
    year: i32,
    month: i32,
    day: i32,
    fraction: R,
) -> Result<Epoch<S, R>, TimeError> {
    let (djm0, djm) = cal2jd::<R>(year, month, day)?;
    epoch((djm0 + djm, fraction), "calendar date -> epoch")
}

/// The proleptic Gregorian date and fraction of day of an epoch on its own scale (ERFA
/// `jd2cal`).
///
/// # Errors
/// [`TimeError::Calendar`] (`FA-E0803`) outside −68569.5 ..= 1e9 d.
pub fn to_calendar<S: TimeScale, R: Real>(e: Epoch<S, R>) -> Result<Calendar<R>, TimeError> {
    let (d1, d2) = e.jd_parts();
    Ok(jd2cal(d1, d2)?)
}

/// TDB seconds since J2000.0 (JD 2451545.0 TDB), the propagation variable.
///
/// In `f64` the resolution is ulp of the result, e.g. 3.8e-6 s at 3e10 s (950 years from J2000);
/// use a double-double backend when that matters.
#[must_use]
pub fn tdb_seconds_since_j2000<R: Real>(e: Epoch<Tdb, R>) -> TimeSpan<R> {
    let (d1, d2) = e.jd_parts();
    let days = (d1 - R::from_f64(J2000_JD)) + d2;
    TimeSpan::from_seconds(days * R::from_f64(SECONDS_PER_DAY)).unwrap_or_else(|_| TimeSpan::zero())
}

/// The TDB epoch `(J2000.0, seconds / 86400)` from TDB seconds since J2000.0.
///
/// # Errors
/// [`TimeError::Conversion`] (`FA-E0805`) if the result is not finite.
pub fn tdb_from_seconds_since_j2000<R: Real>(s: TimeSpan<R>) -> Result<Epoch<Tdb, R>, TimeError> {
    epoch(
        (
            R::from_f64(J2000_JD),
            s.seconds() / R::from_f64(SECONDS_PER_DAY),
        ),
        "TDB seconds -> epoch",
    )
}

/// The signed difference `a − b` in seconds of two epochs on the same scale, computed part by
/// part so that a "day + fraction" split keeps its resolution.
#[must_use]
pub fn seconds_between<S: TimeScale, R: Real>(a: Epoch<S, R>, b: Epoch<S, R>) -> R {
    let (a1, a2) = a.jd_parts();
    let (b1, b2) = b.jd_parts();
    ((a1 - b1) + (a2 - b2)) * R::from_f64(SECONDS_PER_DAY)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ERFA status strings in `ferroastro-erfa` equal the core `ErrorCode` strings.
    /// Verifies: API-002, DEP-009
    #[test]
    fn erfa_codes_match_core_codes() {
        let cases = [
            ErfaError::BadYear { year: -5000 }, // lint: allow-literal(year before ERFA IYMIN)
            ErfaError::BadMonth { month: 13 },
            ErfaError::BadDay { day: 32, mjd: 0 },
            ErfaError::DateOutOfRange { jd: f64::NAN },
        ];
        for e in cases {
            assert_eq!(e.code(), erfa_code(&e).as_str());
            let t = TimeError::from(e);
            assert_eq!(t.code(), erfa_code(&e));
            assert!(t.to_string().starts_with(e.code()));
        }
    }

    /// Verifies: API-002
    #[test]
    fn calendar_errors_carry_codes() {
        let e = from_calendar::<Tt, f64>(2001, 2, 29, 0.0).unwrap_err(); // lint: allow-literal(calendar fixture)
        assert_eq!(e.code(), ErrorCode::CalendarDay);
        let e = from_calendar::<Tt, f64>(2001, 2, 28, f64::NAN).unwrap_err(); // lint: allow-literal(calendar fixture)
        assert_eq!(e.code(), ErrorCode::TimeConversion);
        assert!(e.to_string().contains("FA-E0805"));
    }
}
