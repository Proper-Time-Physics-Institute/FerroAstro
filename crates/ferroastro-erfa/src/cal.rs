// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Gregorian calendar ↔ Julian date: ports of ERFA 2.0.1 `eraCal2jd` (`src/cal2jd.c`) and
//! `eraJd2cal` (`src/jd2cal.c`).
//!
//! Reference (as cited by ERFA): Explanatory Supplement to the Astronomical Almanac,
//! P. K. Seidelmann (ed.), University Science Books (1992), Section 12.92 (p. 604); Klein, A.,
//! A Generalized Kahan-Babuska-Summation-Algorithm, Computing 76, 279-293 (2006), Section 3.

use crate::ErfaError;
use crate::consts::DJM0;
use ferroastro_real::Real;

/// A proleptic Gregorian calendar date with the fraction of a day (`eraJd2cal` output).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Calendar<R: Real = f64> {
    /// Year (astronomical numbering: year 0 is 1 BC).
    pub year: i32,
    /// Month, 1–12.
    pub month: i32,
    /// Day of the month.
    pub day: i32,
    /// Fraction of the day, in `[0, 1)`.
    pub fraction: R,
}

/// Gregorian calendar date (0 h) to a two-part Julian date `(DJM0, MJD)` (ERFA `eraCal2jd`).
///
/// The first part is always 2 400 000.5; the second is the Modified Julian Date at 0 h. The
/// calendar is proleptic Gregorian, valid from −4799 January 1; the AD/BC convention is not
/// observed (year 0 exists).
///
/// # Errors
/// [`ErfaError::BadYear`] (`FA-E0800`) before −4799, [`ErfaError::BadMonth`] (`FA-E0801`) and
/// [`ErfaError::BadDay`] (`FA-E0802`, which carries the MJD ERFA computes anyway).
pub fn cal2jd<R: Real>(iy: i32, im: i32, id: i32) -> Result<(R, R), ErfaError> {
    // Earliest year allowed (4800 BC).
    const IYMIN: i32 = -4799;
    // Month lengths in days.
    const MTAB: [i32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    if iy < IYMIN {
        return Err(ErfaError::BadYear { year: iy });
    }
    let Some(&month_len) = usize::try_from(im - 1).ok().and_then(|i| MTAB.get(i)) else {
        return Err(ErfaError::BadMonth { month: im });
    };
    // If February in a leap year, 1, otherwise 0.
    let ly = i32::from(im == 2 && iy % 4 == 0 && (iy % 100 != 0 || iy % 400 == 0));
    let bad_day = id < 1 || id > month_len + ly;

    // Integer arithmetic exactly as ERFA (C `long`, truncating division).
    let my = i64::from((im - 14) / 12);
    let iypmy = i64::from(iy) + my;
    let djm = (1461 * (iypmy + 4800)) / 4 + (367 * (i64::from(im) - 2 - 12 * my)) / 12
        - (3 * ((iypmy + 4900) / 100)) / 4
        + i64::from(id)
        - 2_432_076;
    // |djm| < 2^31 for every year accepted above and any i32 day; checked anyway.
    let djm = i32::try_from(djm).map_err(|_| ErfaError::BadYear { year: iy })?;
    if bad_day {
        return Err(ErfaError::BadDay { day: id, mjd: djm });
    }
    Ok((R::from_f64(DJM0), R::from_i32(djm)))
}

/// Round half away from zero (ERFA `ERFA_DNINT`).
fn dnint<R: Real>(a: R) -> R {
    if a.abs() < R::HALF {
        R::ZERO
    } else {
        a.round()
    }
}

/// An integer-valued `R` (|x| ≤ 2^31) as `i64`.
fn to_i64<R: Real>(x: R) -> i64 {
    let (hi, lo) = x.to_parts();
    // Both words of an integer-valued double-word below 2^53 are integers; the casts are exact.
    #[allow(clippy::cast_possible_truncation)]
    let v = hi as i64 + lo as i64;
    v
}

/// Two-part Julian date to Gregorian year, month, day and fraction of day (ERFA `eraJd2cal`).
///
/// The date may be split in any way between `dj1` and `dj2`. Day and fraction are separated
/// with Klein's (2006) compensated summation, so the fraction keeps the precision of the split.
///
/// # Errors
/// [`ErfaError::DateOutOfRange`] (`FA-E0803`) if `dj1 + dj2` is outside −68569.5 ..= 1e9
/// (−4900 March 1 onwards) or is not finite.
pub fn jd2cal<R: Real>(dj1: R, dj2: R) -> Result<Calendar<R>, ErfaError> {
    // Minimum and maximum allowed JD.
    let djmin = R::from_f64(-68_569.5);
    let djmax = R::from_f64(1e9);

    // Verify date is acceptable.
    let dj = dj1 + dj2;
    if !(dj >= djmin && dj <= djmax) {
        return Err(ErfaError::DateOutOfRange { jd: dj.to_f64() });
    }

    // Separate day and fraction (where -0.5 <= fraction < 0.5).
    let d = dnint(dj1);
    let f1 = dj1 - d;
    let mut jd = to_i64(d);
    let d = dnint(dj2);
    let f2 = dj2 - d;
    jd += to_i64(d);

    // Compute f1+f2+0.5 using compensated summation (Klein 2006).
    let mut s = R::HALF;
    let mut cs = R::ZERO;
    for x in [f1, f2] {
        let t = s + x;
        cs += if s.abs() >= x.abs() {
            (s - t) + x
        } else {
            (x - t) + s
        };
        s = t;
        if s >= R::ONE {
            jd += 1;
            s -= R::ONE;
        }
    }
    let mut f = s + cs;
    cs = f - s;

    // Deal with negative f.
    if f < R::ZERO {
        // Compensated summation: assume that |s| <= 1.0.
        f = s + R::ONE;
        cs += (R::ONE - f) + s;
        s = f;
        f = s + cs;
        cs = f - s;
        jd -= 1;
    }

    // Deal with f that is 1.0 or more (when rounded to the backend's precision).
    let eps = R::EPSILON;
    if (f - R::ONE) >= -eps / R::from_f64(4.0) {
        // Compensated summation: assume that |s| <= 1.0.
        let t = s - R::ONE;
        cs += (s - t) - R::ONE;
        s = t;
        f = s + cs;
        if -eps / R::TWO < f {
            jd += 1;
            f = f.max(R::ZERO);
        }
    }

    // Express day in Gregorian calendar.
    let mut l = jd + 68_569;
    let n = (4 * l) / 146_097;
    l -= (146_097 * n + 3) / 4;
    let i = (4000 * (l + 1)) / 1_461_001;
    l -= (1461 * i) / 4 - 31;
    let k = (80 * l) / 2447;
    let id = l - (2447 * k) / 80;
    l = k / 11;
    let im = k + 2 - 12 * l;
    let iy = 100 * (n - 49) + i + l;

    // All three are small for |JD| <= 1e9 (year < 3e6).
    let out_of_range = || ErfaError::DateOutOfRange { jd: dj.to_f64() };
    Ok(Calendar {
        year: i32::try_from(iy).map_err(|_| out_of_range())?,
        month: i32::try_from(im).map_err(|_| out_of_range())?,
        day: i32::try_from(id).map_err(|_| out_of_range())?,
        fraction: f,
    })
}

// ERFA licence, retained verbatim (BSD-3-Clause condition 1):
//
// Copyright (C) 2013-2023, NumFOCUS Foundation.
// All rights reserved.
//
// This library is derived, with permission, from the International
// Astronomical Union's "Standards of Fundamental Astronomy" library,
// available from http://www.iausofa.org.
//
// The ERFA version is intended to retain identical functionality to
// the SOFA library, but made distinct through different function and
// file names, as set out in the SOFA license conditions.  The SOFA
// original has a role as a reference standard for the IAU and IERS,
// and consequently redistribution is permitted only in its unaltered
// state.  The ERFA version is not subject to this restriction and
// therefore can be included in distributions which do not support the
// concept of "read only" software.
//
// Although the intent is to replicate the SOFA API (other than
// replacement of prefix names) and results (with the exception of
// bugs;  any that are discovered will be fixed), SOFA is not
// responsible for any errors found in this version of the library.
//
// If you wish to acknowledge the SOFA heritage, please acknowledge
// that you are using a library derived from SOFA, rather than SOFA
// itself.
//
//
// TERMS AND CONDITIONS
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions
// are met:
//
// 1 Redistributions of source code must retain the above copyright
//   notice, this list of conditions and the following disclaimer.
//
// 2 Redistributions in binary form must reproduce the above copyright
//   notice, this list of conditions and the following disclaimer in
//   the documentation and/or other materials provided with the
//   distribution.
//
// 3 Neither the name of the Standards Of Fundamental Astronomy Board,
//   the International Astronomical Union nor the names of its
//   contributors may be used to endorse or promote products derived
//   from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
// FOR A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE
// COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
// INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
// BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
// LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
// LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN
// ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
// POSSIBILITY OF SUCH DAMAGE.
