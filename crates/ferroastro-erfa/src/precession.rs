// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! IAU 2006 precession and ecliptic: ports of ERFA 2.0.1 `eraObl06` (`src/obl06.c`), `eraP06e`
//! (`src/p06e.c`), `eraPfw06` (`src/pfw06.c`), `eraFw2m` (`src/fw2m.c`), `eraPmat06`
//! (`src/pmat06.c`), `eraEcm06` (`src/ecm06.c`), `eraEqec06` (`src/eqec06.c`) and `eraEceq06`
//! (`src/eceq06.c`).
//!
//! The precession theory is Capitaine, Wallace & Chapront 2003 ("P03", A&A 412, 567), adopted by
//! the IAU in 2006; the angles are those of Hilton et al. 2006 (`CeMDA` 94, 351), Table 1, and the
//! Fukushima-Williams bias-precession angles of Capitaine & Wallace 2006 (A&A 450, 855), all as
//! coded in ERFA. Dates are two-part **TT** Julian dates, apportioned in any convenient way.
//!
//! The series coefficients are binary64 literals in every backend: the P03 model, not the
//! arithmetic, limits these angles (ERFA notes agreement between parameterisations at the
//! 1 µas level in the present era). In double-double, the conversion factor arcseconds →
//! radians is π/648000 to 106 bits; in `f64` it is ERFA's `ERFA_DAS2R` bit for bit.

use crate::consts::{DJ00, DJC};
use crate::vecmat::{Matrix3, anp, anpm, c2s, ir, rx, rxp, rxr, rz, s2c, trxp};
use ferroastro_real::Real;

/// Arcseconds to radians (`ERFA_DAS2R` in `f64`; π/648000 to the backend's precision).
#[must_use]
pub fn das2r<R: Real>() -> R {
    R::PI / R::from_f64(648_000.0)
}

/// Julian centuries of TT since J2000.0: `((date1 − DJ00) + date2) / DJC`, as ERFA forms it.
fn centuries<R: Real>(date1: R, date2: R) -> R {
    ((date1 - R::from_f64(DJ00)) + date2) / R::from_f64(DJC)
}

/// Horner evaluation `c[0] + (c[1] + (… + c[n]·t)·t)·t`, as ERFA writes the series.
fn horner<R: Real>(coefficients: &[f64], t: R) -> R {
    let mut acc = R::ZERO;
    let mut first = true;
    for &c in coefficients.iter().rev() {
        acc = if first {
            R::from_f64(c)
        } else {
            R::from_f64(c) + acc * t
        };
        first = false;
    }
    acc
}

/// Mean obliquity of the ecliptic, IAU 2006 precession model (ERFA `eraObl06`), radians. The
/// angle between the ecliptic and the mean equator of date.
#[must_use]
pub fn obl06<R: Real>(date1: R, date2: R) -> R {
    let t = centuries(date1, date2);
    horner(
        &[
            84_381.406,
            -46.836_769,
            -0.000_183_1,
            0.002_003_40,
            -0.000_000_576,
            -0.000_000_043_4,
        ],
        t,
    ) * das2r()
}

/// The IAU 2006 (P03) equinox-based precession angles of ERFA `eraP06e`, all in radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct P06e<R: Real = f64> {
    /// `ε₀`: obliquity at J2000.0.
    pub eps0: R,
    /// `ψ_A`: luni-solar precession.
    pub psia: R,
    /// `ω_A`: inclination of the mean equator with respect to the J2000.0 ecliptic.
    pub oma: R,
    /// `P_A`: ecliptic pole x, J2000.0 ecliptic triad.
    pub bpa: R,
    /// `Q_A`: ecliptic pole −y, J2000.0 ecliptic triad.
    pub bqa: R,
    /// `π_A`: angle between the moving and the J2000.0 ecliptics.
    pub pia: R,
    /// `Π_A`: longitude of the ascending node of the moving ecliptic.
    pub bpia: R,
    /// `ε_A`: mean obliquity of the ecliptic.
    pub epsa: R,
    /// `χ_A`: planetary precession.
    pub chia: R,
    /// `z_A`: equatorial precession, minus the third of the 323 Euler angles.
    pub za: R,
    /// `ζ_A`: equatorial precession, minus the first of the 323 Euler angles.
    pub zetaa: R,
    /// `θ_A`: equatorial precession, the second of the 323 Euler angles.
    pub thetaa: R,
    /// `p_A`: general precession in longitude (accumulated precession of the equinox along the
    /// moving ecliptic).
    pub pa: R,
    /// `γ_J2000`: J2000.0 right-ascension difference of the ecliptic poles.
    pub gam: R,
    /// `φ_J2000`: J2000.0 codeclination of the ecliptic pole.
    pub phi: R,
    /// `ψ_J2000`: longitude difference of the equator poles, J2000.0.
    pub psi: R,
}

/// Precession angles, IAU 2006, equinox based (ERFA `eraP06e`) at the TT date `date1 + date2`.
#[must_use]
#[expect(
    clippy::too_many_lines,
    reason = "a transcription of ERFA's 16 series, kept in ERFA's order for line-by-line review"
)]
pub fn p06e<R: Real>(date1: R, date2: R) -> P06e<R> {
    let t = centuries(date1, date2);
    let s = das2r::<R>();
    let eps0 = R::from_f64(84_381.406) * s;
    // Series without a constant term: (c1 + (c2 + …)·t)·t.
    let tn = |c: &[f64]| horner(c, t) * t * s;
    let psia = tn(&[
        5_038.481_507,
        -1.079_006_9,
        -0.001_140_45,
        0.000_132_851,
        -0.000_000_095_1,
    ]);
    let oma = eps0
        + tn(&[
            -0.025_754,
            0.051_262_3,
            -0.007_725_03,
            -0.000_000_467,
            0.000_000_333_7,
        ]);
    let bpa = tn(&[
        4.199_094,
        0.193_987_3,
        -0.000_224_66,
        -0.000_000_912,
        0.000_000_012_0,
    ]);
    let bqa = tn(&[
        -46.811_015,
        0.051_028_3,
        0.000_524_13,
        -0.000_000_646,
        -0.000_000_017_2,
    ]);
    let pia = tn(&[
        46.998_973,
        -0.033_492_6,
        -0.000_125_59,
        0.000_000_113,
        -0.000_000_002_2,
    ]);
    let bpia = horner(
        &[
            629_546.793_6,
            -867.957_58,
            0.157_992,
            -0.000_537_1,
            -0.000_047_97,
            0.000_000_072,
        ],
        t,
    ) * s;
    let epsa = obl06(date1, date2);
    let chia = tn(&[
        10.556_403,
        -2.381_429_2,
        -0.001_211_97,
        0.000_170_663,
        -0.000_000_056_0,
    ]);
    let za = horner(
        &[
            -2.650_545,
            2_306.077_181,
            1.092_734_8,
            0.018_268_37,
            -0.000_028_596,
            -0.000_000_290_4,
        ],
        t,
    ) * s;
    let zetaa = horner(
        &[
            2.650_545,
            2_306.083_227,
            0.298_849_9,
            0.018_018_28,
            -0.000_005_971,
            -0.000_000_317_3,
        ],
        t,
    ) * s;
    let thetaa = tn(&[
        2_004.191_903,
        -0.429_493_4,
        -0.041_822_64,
        -0.000_007_089,
        -0.000_000_127_4,
    ]);
    let pa = tn(&[
        5_028.796_195,
        1.105_434_8,
        0.000_079_64,
        -0.000_023_857,
        -0.000_000_038_3,
    ]);
    let gam = tn(&[
        10.556_403,
        0.493_204_4,
        -0.000_312_38,
        -0.000_002_788,
        0.000_000_026_0,
    ]);
    let phi = eps0
        + tn(&[
            -46.811_015,
            0.051_126_9,
            0.000_532_89,
            -0.000_000_440,
            -0.000_000_017_6,
        ]);
    let psi = tn(&[
        5_038.481_507,
        1.558_417_6,
        -0.000_185_22,
        -0.000_026_452,
        -0.000_000_014_8,
    ]);
    P06e {
        eps0,
        psia,
        oma,
        bpa,
        bqa,
        pia,
        bpia,
        epsa,
        chia,
        za,
        zetaa,
        thetaa,
        pa,
        gam,
        phi,
        psi,
    }
}

/// The Fukushima-Williams bias-precession angles of ERFA `eraPfw06`, radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pfw06<R: Real = f64> {
    /// `γ̄`: F-W angle.
    pub gamb: R,
    /// `φ̄`: F-W angle.
    pub phib: R,
    /// `ψ̄`: F-W angle.
    pub psib: R,
    /// `ε_A`: mean obliquity of the ecliptic ([`obl06`]).
    pub epsa: R,
}

/// Precession angles, IAU 2006 (Fukushima-Williams 4-angle formulation, referred to the GCRS
/// pole, i.e. **including frame bias**) (ERFA `eraPfw06`) at the TT date `date1 + date2`.
#[must_use]
pub fn pfw06<R: Real>(date1: R, date2: R) -> Pfw06<R> {
    let t = centuries(date1, date2);
    let s = das2r::<R>();
    let gamb = horner(
        &[
            -0.052_928,
            10.556_378,
            0.493_204_4,
            -0.000_312_38,
            -0.000_002_788,
            0.000_000_026_0,
        ],
        t,
    ) * s;
    let phib = horner(
        &[
            84_381.412_819,
            -46.811_016,
            0.051_126_8,
            0.000_532_89,
            -0.000_000_440,
            -0.000_000_017_6,
        ],
        t,
    ) * s;
    let psib = horner(
        &[
            -0.041_775,
            5_038.481_484,
            1.558_417_5,
            -0.000_185_22,
            -0.000_026_452,
            -0.000_000_014_8,
        ],
        t,
    ) * s;
    Pfw06 {
        gamb,
        phib,
        psib,
        epsa: obl06(date1, date2),
    }
}

/// Form a rotation matrix from Fukushima-Williams angles (ERFA `eraFw2m`):
/// `R_x(−ε) · R_z(−ψ) · R_x(φ) · R_z(γ)`.
#[must_use]
pub fn fw2m<R: Real>(gamb: R, phib: R, psi: R, eps: R) -> Matrix3<R> {
    let r = rz(gamb, ir());
    let r = rx(phib, r);
    let r = rz(-psi, r);
    rx(-eps, r)
}

/// Precession matrix (including frame bias) from GCRS to the mean equator and equinox of the TT
/// date, IAU 2006 (ERFA `eraPmat06`).
#[must_use]
pub fn pmat06<R: Real>(date1: R, date2: R) -> Matrix3<R> {
    let a = pfw06(date1, date2);
    fw2m(a.gamb, a.phib, a.psib, a.epsa)
}

/// ICRS equatorial to ecliptic rotation matrix, IAU 2006 (ERFA `eraEcm06`): to the mean ecliptic
/// and equinox of the TT date, `R_x(ε_A) · pmat06`. Frame bias is included.
#[must_use]
pub fn ecm06<R: Real>(date1: R, date2: R) -> Matrix3<R> {
    let ob = obl06(date1, date2);
    let bp = pmat06(date1, date2);
    let e = rx(ob, ir());
    rxr(&e, &bp)
}

/// ICRS right ascension and declination to ecliptic longitude and latitude (mean equinox and
/// ecliptic of the TT date), IAU 2006 (ERFA `eraEqec06`). Angles in radians; the longitude is
/// returned in `0 ≤ dl < 2π`, the latitude in `±π/2`.
#[must_use]
pub fn eqec06<R: Real>(date1: R, date2: R, dr: R, dd: R) -> (R, R) {
    let v1 = s2c(dr, dd);
    let rm = ecm06(date1, date2);
    let v2 = rxp(&rm, v1);
    let (a, b) = c2s(v2);
    (anp(a), anpm(b))
}

/// Ecliptic longitude and latitude (mean equinox and ecliptic of the TT date) to ICRS right
/// ascension and declination, IAU 2006 (ERFA `eraEceq06`). Angles in radians; the right
/// ascension is returned in `0 ≤ dr < 2π`, the declination in `±π/2`.
#[must_use]
pub fn eceq06<R: Real>(date1: R, date2: R, dl: R, db: R) -> (R, R) {
    let v1 = s2c(dl, db);
    let rm = ecm06(date1, date2);
    let v2 = trxp(&rm, v1);
    let (a, b) = c2s(v2);
    (anp(a), anpm(b))
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
