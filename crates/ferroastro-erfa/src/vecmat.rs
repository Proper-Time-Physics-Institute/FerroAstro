// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Vector and rotation-matrix helpers: ports of ERFA 2.0.1 `eraIr` (`src/ir.c`), `eraRx`
//! (`src/rx.c`), `eraRz` (`src/rz.c`), `eraRxr` (`src/rxr.c`), `eraTr` (`src/tr.c`), `eraRxp`
//! (`src/rxp.c`), `eraTrxp` (`src/trxp.c`), `eraS2c` (`src/s2c.c`), `eraC2s` (`src/c2s.c`),
//! `eraAnp` (`src/anp.c`) and `eraAnpm` (`src/anpm.c`).
//!
//! Matrices are `[[R; 3]; 3]` indexed `[row][column]`, as ERFA's `double r[3][3]`. ERFA's
//! in-place routines (`eraRx`, `eraRz`, `eraIr`) take the matrix by value and return the result
//! here. The operations are ERFA's, in ERFA's order (no fused multiply-add), so in `f64` the
//! results are bitwise those of the C library up to the platform's `sin`, `cos` and `atan2`.

use ferroastro_real::Real;

/// A 3×3 matrix, `[row][column]`.
pub type Matrix3<R> = [[R; 3]; 3];

/// The identity matrix (ERFA `eraIr`).
#[must_use]
pub fn ir<R: Real>() -> Matrix3<R> {
    let (o, z) = (R::ONE, R::ZERO);
    [[o, z, z], [z, o, z], [z, z, o]]
}

/// Rotate an r-matrix about the x-axis by `phi` (ERFA `eraRx`): returns `Rx(phi) · r`. A positive
/// `phi` rotates the axes anticlockwise as seen looking towards the origin from positive x.
#[must_use]
pub fn rx<R: Real>(phi: R, r: Matrix3<R>) -> Matrix3<R> {
    let (s, c) = (phi.sin(), phi.cos());
    let [r0, r1, r2] = r;
    let row1 = [
        c * r1[0] + s * r2[0],
        c * r1[1] + s * r2[1],
        c * r1[2] + s * r2[2],
    ];
    let row2 = [
        -s * r1[0] + c * r2[0],
        -s * r1[1] + c * r2[1],
        -s * r1[2] + c * r2[2],
    ];
    [r0, row1, row2]
}

/// Rotate an r-matrix about the z-axis by `psi` (ERFA `eraRz`): returns `Rz(psi) · r`. A positive
/// `psi` rotates the axes anticlockwise as seen looking towards the origin from positive z.
#[must_use]
pub fn rz<R: Real>(psi: R, r: Matrix3<R>) -> Matrix3<R> {
    let (s, c) = (psi.sin(), psi.cos());
    let [r0, r1, r2] = r;
    let row0 = [
        c * r0[0] + s * r1[0],
        c * r0[1] + s * r1[1],
        c * r0[2] + s * r1[2],
    ];
    let row1 = [
        -s * r0[0] + c * r1[0],
        -s * r0[1] + c * r1[1],
        -s * r0[2] + c * r1[2],
    ];
    [row0, row1, r2]
}

/// `w = 0; w += a[k]·b[k]` for k = 0, 1, 2, ERFA's accumulation order.
fn dot_in_order<R: Real>(a: &[R; 3], b: &[R; 3]) -> R {
    a.iter()
        .zip(b.iter())
        .fold(R::ZERO, |w, (&x, &y)| w + x * y)
}

/// Product of two r-matrices, `a · b` (ERFA `eraRxr`).
#[must_use]
pub fn rxr<R: Real>(a: &Matrix3<R>, b: &Matrix3<R>) -> Matrix3<R> {
    let columns = tr(b);
    a.map(|row| columns.map(|col| dot_in_order(&row, &col)))
}

/// Transpose of an r-matrix (ERFA `eraTr`).
#[must_use]
pub fn tr<R: Real>(r: &Matrix3<R>) -> Matrix3<R> {
    let [[a, b, c], [d, e, f], [g, h, i]] = *r;
    [[a, d, g], [b, e, h], [c, f, i]]
}

/// Multiply a p-vector by an r-matrix, `r · p` (ERFA `eraRxp`).
#[must_use]
pub fn rxp<R: Real>(r: &Matrix3<R>, p: [R; 3]) -> [R; 3] {
    r.map(|row| dot_in_order(&row, &p))
}

/// Multiply a p-vector by the transpose of an r-matrix, `rᵀ · p` (ERFA `eraTrxp`).
#[must_use]
pub fn trxp<R: Real>(r: &Matrix3<R>, p: [R; 3]) -> [R; 3] {
    rxp(&tr(r), p)
}

/// Spherical coordinates `(theta, phi)` (radians) to a unit vector (ERFA `eraS2c`).
#[must_use]
pub fn s2c<R: Real>(theta: R, phi: R) -> [R; 3] {
    let cp = phi.cos();
    [theta.cos() * cp, theta.sin() * cp, phi.sin()]
}

/// A p-vector to spherical coordinates `(theta, phi)` in radians (ERFA `eraC2s`). The vector
/// need not be unit; at a pole `theta` is 0, and for the null vector both are 0.
#[must_use]
pub fn c2s<R: Real>(p: [R; 3]) -> (R, R) {
    let [x, y, z] = p;
    let d2 = x * x + y * y;
    let theta = if d2 == R::ZERO { R::ZERO } else { y.atan2(x) };
    let phi = if z == R::ZERO {
        R::ZERO
    } else {
        z.atan2(d2.sqrt())
    };
    (theta, phi)
}

/// ERFA's `fmod(a, 2π)` (sign of `a`), exact in binary64: `sign(a) · (|a| mod 2π)`.
fn fmod_2pi<R: Real>(a: R) -> R {
    let w = a.abs().rem_euclid(R::TAU);
    if a < R::ZERO { -w } else { w }
}

/// Normalise an angle into the range `0 ≤ a < 2π` (ERFA `eraAnp`).
#[must_use]
pub fn anp<R: Real>(a: R) -> R {
    let mut w = fmod_2pi(a);
    if w < R::ZERO {
        w += R::TAU;
    }
    w
}

/// Normalise an angle into the range `±π` (ERFA `eraAnpm`).
#[must_use]
pub fn anpm<R: Real>(a: R) -> R {
    let mut w = fmod_2pi(a);
    if w.abs() >= R::PI {
        // ERFA_DSIGN(ERFA_D2PI, a)
        if a < R::ZERO {
            w += R::TAU;
        } else {
            w -= R::TAU;
        }
    }
    w
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
