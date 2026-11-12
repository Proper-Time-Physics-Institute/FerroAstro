// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! TDB ↔ TCB: ports of ERFA 2.0.1 `eraTdbtcb` (`src/tdbtcb.c`) and `eraTcbtdb`
//! (`src/tcbtdb.c`).
//!
//! The conventional linear relation of IAU 2006 Resolution B3:
//! `TDB = TCB − L_B (JD_TCB − T_0) × 86400 s + TDB_0`, with `T_0` = 1977 January 1.0 TAI
//! (= 1977 January 1, 00:00:32.184 TT/TCB/TCG), `L_B` = 1.550519768e-8 and
//! `TDB_0` = −6.55e-5 s. TDB is essentially JPL's `T_eph`.

use crate::consts::{DAYSEC, DJM0, DJM77, elb, tdb0_seconds, ttmtai_seconds};
use ferroastro_real::Real;

/// 1977 Jan 1 00:00:32.184 TT as a two-part JD `(t77td, t77tf)`.
fn t77<R: Real>() -> (R, R) {
    (
        R::from_f64(DJM0 + DJM77),
        ttmtai_seconds::<R>() / R::from_f64(DAYSEC),
    )
}

/// TDB to TCB, both two-part Julian dates (ERFA `eraTdbtcb`; status always 0).
#[must_use]
pub fn tdbtcb<R: Real>(tdb1: R, tdb2: R) -> (R, R) {
    let (t77td, t77tf) = t77::<R>();
    // TDB (days) at TAI 1977 Jan 1.0.
    let tdb0 = tdb0_seconds::<R>() / R::from_f64(DAYSEC);
    // TDB to TCB rate.
    let elbb = elb::<R>() / (R::ONE - elb::<R>());

    // Result, preserving date format but safeguarding precision.
    if tdb1.abs() > tdb2.abs() {
        let d = t77td - tdb1;
        let f = tdb2 - tdb0;
        (tdb1, f - (d - (f - t77tf)) * elbb)
    } else {
        let d = t77td - tdb2;
        let f = tdb1 - tdb0;
        (f - (d - (f - t77tf)) * elbb, tdb2)
    }
}

/// TCB to TDB, both two-part Julian dates (ERFA `eraTcbtdb`; status always 0).
#[must_use]
pub fn tcbtdb<R: Real>(tcb1: R, tcb2: R) -> (R, R) {
    let (t77td, t77tf) = t77::<R>();
    // TDB (days) at TAI 1977 Jan 1.0.
    let tdb0 = tdb0_seconds::<R>() / R::from_f64(DAYSEC);
    let elb = elb::<R>();

    // Result, safeguarding precision.
    if tcb1.abs() > tcb2.abs() {
        let d = tcb1 - t77td;
        (tcb1, tcb2 + tdb0 - (d + (tcb2 - t77tf)) * elb)
    } else {
        let d = tcb2 - t77td;
        (tcb1 + tdb0 - (d + (tcb1 - t77tf)) * elb, tcb2)
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
