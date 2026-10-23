// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! TDB − TT for an observer on the Earth: port of ERFA 2.0.1 `eraDtdb` (`src/dtdb.c`).
//!
//! The geocentric part is the full Fairhead & Bretagnon (1990) series (787 terms, table in
//! `dtdb_table.rs`) with ERFA's adjustment to JPL planetary masses. The topocentric part is
//! Moyer (1981) and Murray (1983), with fundamental arguments from Simon et al. (1994); it
//! approximates `(v/c)·(r/c)` and vanishes for `u = v = 0`.
//!
//! Accuracy (ERFA note 7): better than ±3 ns over 1950–2050 against time ephemerides integrated
//! from JPL DE405. It is a model; the definitive TT–TDB relation comes from integrating the
//! solar-system ephemeris (ERFA note 8).
//!
//! References (as cited by ERFA): Fairhead, L. & Bretagnon, P., A&A 229, 240-247 (1990); IAU 2006
//! Resolution 3; IERS Conventions (2003), IERS TN 32; Moyer, T. D., Cel. Mech. 23, 33 (1981);
//! Murray, C. A., Vectorial Astrometry, Adam Hilger (1983); Seidelmann, P. K. et al.,
//! Explanatory Supplement to the Astronomical Almanac, Ch. 2 (1992); Simon, J. L. et al.,
//! A&A 282, 663-683 (1994).

use crate::consts::{DJ00, DJM};
use crate::dtdb_table::FAIRHD;
use ferroastro_real::Real;

/// C `fmod`: the remainder of `x / y` with the sign of `x` (exact in binary64).
fn fmod<R: Real>(x: R, y: R) -> R {
    if x < R::ZERO {
        -(-x).rem_euclid(y)
    } else {
        x.rem_euclid(y)
    }
}

/// Sum `amplitude · sin(frequency · t + phase)` over `rows`, last row first (ERFA's order).
fn series<R: Real>(rows: &[[f64; 3]], t: R) -> R {
    rows.iter().rev().fold(R::ZERO, |w, &[a, f, p]| {
        w + R::from_f64(a) * (R::from_f64(f) * t + R::from_f64(p)).sin()
    })
}

/// TDB − TT in seconds (ERFA `eraDtdb`).
///
/// * `date1 + date2`: the date as a two-part Julian date, formally TDB; TT may be used "with no
///   practical effect on the accuracy" (ERFA note 1). The J2000 split gives the best resolution.
/// * `ut`: UT1 as a fraction of one day (topocentric terms only).
/// * `elong`: east longitude of the observer (rad).
/// * `u`: distance of the observer from the Earth's spin axis (km).
/// * `v`: distance of the observer north of the equatorial plane (km).
///
/// With `u = v = 0` the result is the geocentric Fairhead & Bretagnon value.
#[must_use]
pub fn dtdb<R: Real>(date1: R, date2: R, ut: R, elong: R, u: R, v: R) -> R {
    let c = R::from_f64;

    // Time since J2000.0 in Julian millennia.
    let t = ((date1 - c(DJ00)) + date2) / c(DJM);

    // ================= Topocentric terms =================

    // Convert UT to local solar time in radians.
    let tsol = fmod(ut, R::ONE) * R::TAU + elong;

    // FUNDAMENTAL ARGUMENTS: Simon et al. 1994.
    // Combine time argument (millennia) with deg/arcsec factor.
    let w = t / c(3600.0);
    let dd2r = R::PI / c(180.0);
    let deg360 = c(360.0);
    // Sun Mean Longitude.
    let elsun = fmod(c(280.466_456_83) + c(1_296_027_711.034_29) * w, deg360) * dd2r;
    // Sun Mean Anomaly.
    let emsun = fmod(c(357.529_109_18) + c(1_295_965_810.481) * w, deg360) * dd2r;
    // Mean Elongation of Moon from Sun.
    let d = fmod(c(297.850_195_47) + c(16_029_616_012.090) * w, deg360) * dd2r;
    // Mean Longitude of Jupiter.
    let elj = fmod(c(34.351_518_74) + c(109_306_899.894_53) * w, deg360) * dd2r;
    // Mean Longitude of Saturn.
    let els = fmod(c(50.077_444_30) + c(44_046_398.470_38) * w, deg360) * dd2r;

    // TOPOCENTRIC TERMS: Moyer 1981 and Murray 1983 (ERFA's evaluation order).
    let two = R::TWO;
    let wt = c(0.000_29e-10) * u * (tsol + elsun - els).sin()
        + c(0.001_00e-10) * u * (tsol - two * emsun).sin()
        + c(0.001_33e-10) * u * (tsol - d).sin()
        + c(0.001_33e-10) * u * (tsol + elsun - elj).sin()
        - c(0.002_29e-10) * u * (tsol + two * elsun + emsun).sin()
        - c(0.022_00e-10) * v * (elsun + emsun).cos()
        + c(0.053_12e-10) * u * (tsol - emsun).sin()
        - c(0.136_77e-10) * u * (tsol + two * elsun).sin()
        - c(1.318_40e-10) * v * elsun.cos()
        + c(3.176_79e-10) * u * tsol.sin();

    // ================= Fairhead et al. model =================
    // Rows 0-473: T^0, 474-678: T^1, 679-763: T^2, 764-783: T^3, 784-786: T^4.
    let (w0, w1, w2, w3, w4) = match (
        FAIRHD.get(0..474),
        FAIRHD.get(474..679),
        FAIRHD.get(679..764),
        FAIRHD.get(764..784),
        FAIRHD.get(784..787),
    ) {
        (Some(r0), Some(r1), Some(r2), Some(r3), Some(r4)) => (
            series(r0, t),
            series(r1, t),
            series(r2, t),
            series(r3, t),
            series(r4, t),
        ),
        // The table has exactly 787 rows (a compile-time array length); unreachable.
        _ => (R::ZERO, R::ZERO, R::ZERO, R::ZERO, R::ZERO),
    };

    // Multiply by powers of T and combine.
    let wf = t * (t * (t * (t * w4 + w3) + w2) + w1) + w0;

    // Adjustments to use JPL planetary masses instead of IAU.
    let wj = c(0.000_65e-6) * (c(6_069.776_754) * t + c(4.021_194)).sin()
        + c(0.000_33e-6) * (c(213.299_095) * t + c(5.543_132)).sin()
        + (-c(0.001_96e-6) * (c(6_208.294_251) * t + c(5.696_701)).sin())
        + (-c(0.001_73e-6) * (c(74.781_599) * t + c(2.435_900)).sin())
        + c(0.036_38e-6) * t * t;

    // TDB-TT in seconds.
    wt + wf + wj
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
