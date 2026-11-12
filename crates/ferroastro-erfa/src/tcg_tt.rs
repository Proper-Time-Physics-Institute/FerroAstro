// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! TT ↔ TCG: ports of ERFA 2.0.1 `eraTttcg` (`src/tttcg.c`) and `eraTcgtt` (`src/tcgtt.c`).
//!
//! `TT = TCG − L_G (JD_TCG − T_0) × 86400 s`, with `L_G` = 6.969290134e-10 (IAU 2000 Resolution
//! B1.9) and `T_0` = 1977 January 1, 00:00:32.184 TT. References (as cited by ERFA): IERS
//! Conventions (2003), IERS Technical Note No. 32; IAU 2000 Resolution B1.9.

use crate::consts::{DAYSEC, DJM0, DJM77, elg, ttmtai_seconds};
use ferroastro_real::Real;

/// 1977 Jan 1 00:00:32.184 TT as an MJD.
fn t77t<R: Real>() -> R {
    R::from_f64(DJM77) + ttmtai_seconds::<R>() / R::from_f64(DAYSEC)
}

/// TT to TCG, both two-part Julian dates (ERFA `eraTttcg`; status always 0).
#[must_use]
pub fn tttcg<R: Real>(tt1: R, tt2: R) -> (R, R) {
    let t77t = t77t::<R>();
    let djm0 = R::from_f64(DJM0);
    // TT to TCG rate.
    let elgg = elg::<R>() / (R::ONE - elg::<R>());

    // Result, safeguarding precision.
    if tt1.abs() > tt2.abs() {
        (tt1, tt2 + ((tt1 - djm0) + (tt2 - t77t)) * elgg)
    } else {
        (tt1 + ((tt2 - djm0) + (tt1 - t77t)) * elgg, tt2)
    }
}

/// TCG to TT, both two-part Julian dates (ERFA `eraTcgtt`; status always 0).
#[must_use]
pub fn tcgtt<R: Real>(tcg1: R, tcg2: R) -> (R, R) {
    let t77t = t77t::<R>();
    let djm0 = R::from_f64(DJM0);
    let elg = elg::<R>();

    // Result, safeguarding precision.
    if tcg1.abs() > tcg2.abs() {
        (tcg1, tcg2 - ((tcg1 - djm0) + (tcg2 - t77t)) * elg)
    } else {
        (tcg1 - ((tcg2 - djm0) + (tcg1 - t77t)) * elg, tcg2)
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
