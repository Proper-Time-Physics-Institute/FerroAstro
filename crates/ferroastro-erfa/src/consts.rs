// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Constants of ERFA 2.0.1 `src/erfam.h` used by the time-scale and precession routines.
//!
//! The binary64 values are ERFA's. The `ttmtai_seconds`, `elb`, `elg` and `tdb0_seconds`
//! functions return the defined decimal values exact to 106 bits in a double-double backend
//! (high word = the binary64 constant, low word = the decimal remainder, computed with Python's
//! `decimal` at 60 digits). In `f64` they return the binary64 constant, as ERFA does.

use ferroastro_real::Real;

/// Seconds per day (`ERFA_DAYSEC`).
pub const DAYSEC: f64 = 86_400.0;
/// Reference epoch J2000.0 as a Julian date (`ERFA_DJ00`).
pub const DJ00: f64 = 2_451_545.0;
/// Days per Julian century (`ERFA_DJC`).
pub const DJC: f64 = 36_525.0;
/// Days per Julian millennium (`ERFA_DJM`).
pub const DJM: f64 = 365_250.0;
/// Julian date of Modified Julian Date zero (`ERFA_DJM0`).
pub const DJM0: f64 = 2_400_000.5;
/// MJD of 1977 January 1.0 (`ERFA_DJM77`).
pub const DJM77: f64 = 43_144.0;
/// TT − TAI in seconds (`ERFA_TTMTAI`), 32.184 s by definition.
pub const TTMTAI: f64 = 32.184;
/// `L_G = 1 − d(TT)/d(TCG)` (`ERFA_ELG`), IAU 2000 Resolution B1.9.
pub const ELG: f64 = 6.969_290_134e-10;
/// `L_B = 1 − d(TDB)/d(TCB)` (`ERFA_ELB`), IAU 2006 Resolution B3.
pub const ELB: f64 = 1.550_519_768e-8;
/// `TDB_0`, TDB − TT at TAI 1977 January 1.0, in seconds (`ERFA_TDB0`), IAU 2006 Resolution B3.
pub const TDB0: f64 = -6.55e-5;

/// [`TTMTAI`] in backend `R`, exact to 106 bits in double-double.
#[must_use]
pub fn ttmtai_seconds<R: Real>() -> R {
    R::from_parts(TTMTAI, 2.501_110_429_875_552_5e-15)
}

/// [`ELB`] in backend `R`, exact to 106 bits in double-double.
#[must_use]
pub fn elb<R: Real>() -> R {
    R::from_parts(ELB, -1.014_359_578_815_210_5e-24)
}

/// [`ELG`] in backend `R`, exact to 106 bits in double-double.
#[must_use]
pub fn elg<R: Real>() -> R {
    R::from_parts(ELG, 3.254_816_309_425_064e-27)
}

/// [`TDB0`] in backend `R`, exact to 106 bits in double-double.
#[must_use]
pub fn tdb0_seconds<R: Real>() -> R {
    R::from_parts(TDB0, 6.459_242_862_799_641e-21)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_real::DoubleDouble;

    /// The f64 backend sees exactly ERFA's binary64 constants; the low words are below half an
    /// ulp of the high words (a normalised double-word).
    /// Verifies: DEP-009
    #[test]
    fn constants_are_erfa_values_in_f64_and_normalised_in_dd() {
        for (f, dd) in [
            (ttmtai_seconds::<f64>(), ttmtai_seconds::<DoubleDouble>()),
            (elb::<f64>(), elb::<DoubleDouble>()),
            (elg::<f64>(), elg::<DoubleDouble>()),
            (tdb0_seconds::<f64>(), tdb0_seconds::<DoubleDouble>()),
        ] {
            let (hi, lo) = dd.to_parts();
            assert_eq!(f.to_bits(), hi.to_bits());
            assert!(lo.abs() <= hi.abs() * f64::EPSILON / 2.0);
            assert!(lo != 0.0, "decimal constant should not be binary64-exact");
        }
    }
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
