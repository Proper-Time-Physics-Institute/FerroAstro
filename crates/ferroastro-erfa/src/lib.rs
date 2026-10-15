// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Port of the ERFA calendar, time-scale and IAU 2006 precession/ecliptic routines, generic over
//! [`Real`] (DEP-009, LIC-004).
//!
//! # Provenance and licence
//! The routines are ported from **ERFA 2.0.1** (Essential Routines for Fundamental Astronomy,
//! BSD-3-Clause, Copyright (C) 2013-2023 `NumFOCUS` Foundation; git tag `v2.0.1`, commit
//! `9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3`, release tarball `erfa-2.0.1.tar.xz` SHA-256
//! `45111f0addf6ef69345aaf093bd054705b41ac2db7541dd3f0e105362c8e20e4`). ERFA is derived, with
//! permission, from the IAU SOFA library; **no SOFA file was read or copied** (LIC-004: SOFA's
//! licence only permits renamed derivatives, and FerroAstro takes ERFA's BSD-3 relicensing instead).
//! This is a library derived from ERFA, not ERFA or SOFA itself.
//!
//! Every ported file carries the SPDX licence expression `Apache-2.0 AND BSD-3-Clause`, the ERFA
//! copyright line and, at its end, the ERFA notice, conditions and disclaimer verbatim
//! (BSD-3-Clause condition 1). The repository `NOTICE` reproduces them for binary distributions
//! (condition 2). `LICENSES/BSD-3-Clause.txt` holds the licence template.
//!
//! # Naming
//! No public symbol starts with SOFA's `iau` prefix (DEP-009), nor with ERFA's `era`: the
//! functions use the ERFA name without its prefix, in lower case (`eraTaitt` → [`taitt`]).
//!
//! # Numerics
//! * Dates are two-part Julian dates `(d1, d2)` as in ERFA: any split is accepted, and each
//!   transformation adds its correction to the part with the smaller magnitude so that a
//!   "day + fraction" split keeps sub-nanosecond resolution in binary64.
//! * Monomorphised at `f64`, each routine performs ERFA's operations in ERFA's order, so the results
//!   are those of the C library up to the platform's `sin` (only [`dtdb`] calls one).
//! * With a double-double backend the defined decimal constants (32.184 s, `L_B`, `L_G`,
//!   `TDB0`) are exact to 106 bits; ERFA's own binary64 roundings of them are not inherited.
//!   The Fairhead & Bretagnon series coefficients stay binary64 literals: the model, not the
//!   arithmetic, limits [`dtdb`] (a few ns against numerically integrated ephemerides).
//!
//! # Status codes
//! ERFA's non-zero statuses become [`ErfaError`] values with stable `FA-E08xx` codes, which
//! `ferroastro-core`'s `ErrorCode` lists too.
//!
//! ```
//! use ferroastro_erfa::{taitt, tttdb, dtdb};
//!
//! // TAI 2006 January 15, 21:25:10.5 (ERFA test value), to TT and then TDB.
//! let (tt1, tt2) = taitt(2_453_750.5_f64, 0.892_482_639);
//! let dtr = dtdb(tt1, tt2, 0.0, 0.0, 0.0, 0.0);
//! let (tdb1, tdb2) = tttdb(tt1, tt2, dtr);
//! assert!((((tdb1 - tt1) + (tdb2 - tt2)) * 86_400.0 - dtr).abs() < 1e-9);
//! ```

#![no_std]

mod cal;
mod consts;
mod dtdb;
mod dtdb_table;
mod precession;
mod tai_tt;
mod tcb_tdb;
mod tcg_tt;
mod tdb_tt;
mod vecmat;

pub use cal::{Calendar, cal2jd, jd2cal};
pub use consts::{
    DAYSEC, DJ00, DJC, DJM, DJM0, DJM77, ELB, ELG, TDB0, TTMTAI, elb, elg, tdb0_seconds,
    ttmtai_seconds,
};
pub use dtdb::dtdb;
pub use precession::{P06e, Pfw06, das2r, eceq06, ecm06, eqec06, fw2m, obl06, p06e, pfw06, pmat06};
pub use tai_tt::{taitt, tttai};
pub use tcb_tdb::{tcbtdb, tdbtcb};
pub use tcg_tt::{tcgtt, tttcg};
pub use tdb_tt::{tdbtt, tttdb};
pub use vecmat::{Matrix3, anp, anpm, c2s, ir, rx, rxp, rxr, rz, s2c, tr, trxp};

use core::fmt;

#[cfg(doc)]
use ferroastro_real::Real;

/// A non-zero ERFA status, with its stable FerroAstro code.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum ErfaError {
    /// `FA-E0800`: [`cal2jd`] status −1, the year is before −4799 (4800 BC).
    BadYear {
        /// The rejected year.
        year: i32,
    },
    /// `FA-E0801`: [`cal2jd`] status −2, the month is outside 1–12.
    BadMonth {
        /// The rejected month.
        month: i32,
    },
    /// `FA-E0802`: [`cal2jd`] status −3, the day is outside the month. ERFA still computes the
    /// date; it is returned here so that callers can reproduce ERFA's behaviour.
    BadDay {
        /// The rejected day.
        day: i32,
        /// The Modified Julian Date ERFA computes anyway (0 h of that "day").
        mjd: i32,
    },
    /// `FA-E0803`: [`jd2cal`] status −1, the Julian date is outside −68569.5 ..= 1e9 or not finite.
    DateOutOfRange {
        /// The rejected Julian date (`d1 + d2`, rounded to binary64).
        jd: f64,
    },
}

impl ErfaError {
    /// The stable code, e.g. `"FA-E0800"`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            ErfaError::BadYear { .. } => "FA-E0800",
            ErfaError::BadMonth { .. } => "FA-E0801",
            ErfaError::BadDay { .. } => "FA-E0802",
            ErfaError::DateOutOfRange { .. } => "FA-E0803",
        }
    }

    /// The ERFA status integer this error stands for.
    #[must_use]
    pub const fn erfa_status(&self) -> i32 {
        match self {
            ErfaError::BadYear { .. } | ErfaError::DateOutOfRange { .. } => -1,
            ErfaError::BadMonth { .. } => -2,
            ErfaError::BadDay { .. } => -3,
        }
    }
}

impl fmt::Display for ErfaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code();
        match self {
            ErfaError::BadYear { year } => write!(
                f,
                "{code}: calendar year = {year} is outside the valid range >= -4799 (4800 BC); \
                 fix: use a proleptic Gregorian year of -4799 or later"
            ),
            ErfaError::BadMonth { month } => write!(
                f,
                "{code}: calendar month = {month} is outside the valid range 1..=12; \
                 fix: pass a month number from 1 (January) to 12 (December)"
            ),
            ErfaError::BadDay { day, mjd } => write!(
                f,
                "{code}: calendar day = {day} is outside the month (ERFA status -3; MJD {mjd} \
                 computed anyway); valid range 1..=month length; fix: check the day of month"
            ),
            ErfaError::DateOutOfRange { jd } => write!(
                f,
                "{code}: Julian date = {jd} d is outside the valid range -68569.5..=1e9 d; \
                 fix: check the two-part date for unit or epoch errors"
            ),
        }
    }
}

impl core::error::Error for ErfaError {}
