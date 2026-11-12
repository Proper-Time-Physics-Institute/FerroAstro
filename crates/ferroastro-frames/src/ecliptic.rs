// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Ecliptic frames: the J2000 ecliptic of JPL/NAIF (`ECLIPJ2000`) and the IAU 2006 mean ecliptic
//! and equinox of date (ERFA `ecm06`), plus the IAU 2006 ecliptic precession angles (`p06e`).

use crate::Rotation;
use ferroastro_core::frames::{EclipticJ2000, EclipticOfDate, Icrf};
use ferroastro_core::time::{Epoch, Tt};
use ferroastro_core::units::Angle;
use ferroastro_erfa::{das2r, ecm06, ir, p06e, rx};
use ferroastro_real::Real;

/// Obliquity (arcseconds) that defines [`EclipticJ2000`]: the IAU 1976 value at J2000 (Lieske et
/// al. 1977, A&A 58, 1), which JPL Horizons and NAIF use for `ECLIPJ2000`: "produced by rotating
/// around the ICRF x-axis by a standard fixed obliquity angle (epsilon) of 84381.448 arcseconds"
/// (JPL Horizons manual, "Ecliptic of Standard Epoch").
pub const ECLIPJ2000_OBLIQUITY_ARCSEC: f64 = 84_381.448;

/// Definition string of [`icrf_to_ecliptic_j2000`].
pub const ECLIPJ2000_DEFINITION: &str = "ECLIPJ2000 (JPL Horizons / NAIF): R_x(84381.448 arcsec) \
     applied to ICRF axes; no frame bias (differs from the IAU 2006 J2000 ecliptic of ERFA ecm06 by \
     about 0.04 arcsec)";

/// ICRF → [`EclipticJ2000`]: `R_x(ε)` with ε = [`ECLIPJ2000_OBLIQUITY_ARCSEC`].
///
/// This is the frame of JPL Horizons' `ECLIPTIC` tables (verified against them in the data tier).
/// It is **not** the IAU 2006 mean ecliptic of J2000, which includes the frame bias and
/// ε₀ = 84381.406″; use [`icrf_to_ecliptic_of_date`] at J2000 for that.
#[must_use]
pub fn icrf_to_ecliptic_j2000<R: Real>() -> Rotation<Icrf, EclipticJ2000, R> {
    let eps = R::from_f64(ECLIPJ2000_OBLIQUITY_ARCSEC) * das2r::<R>();
    Rotation::from_parts_unchecked(rx(eps, ir()), ECLIPJ2000_DEFINITION.to_owned())
}

/// ICRF → IAU 2006 mean ecliptic and equinox of the TT date `tt` (ERFA `ecm06`, frame bias
/// included). For the PHY-017 historical comparison and NUM-014 only: rates are reported in
/// inertial ICRF by default.
#[must_use]
pub fn icrf_to_ecliptic_of_date<R: Real>(tt: Epoch<Tt, R>) -> Rotation<Icrf, EclipticOfDate, R> {
    let (d1, d2) = tt.jd_parts();
    Rotation::from_parts_unchecked(
        ecm06(d1, d2),
        format!(
            "IAU 2006 mean ecliptic and equinox of date (ERFA 2.0.1 ecm06 port), TT JD {:.9}",
            tt.jd().to_f64()
        ),
    )
}

/// The IAU 2006 (P03) ecliptic precession angles at a TT date (ERFA `p06e`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EclipticPrecession<R: Real = f64> {
    /// `π_A`: angle between the moving ecliptic and the J2000 ecliptic.
    pub pi_a: Angle<R>,
    /// `Π_A`: longitude of the ascending node of the moving ecliptic on the J2000 ecliptic.
    pub big_pi_a: Angle<R>,
    /// `p_A`: general precession in longitude.
    pub p_a: Angle<R>,
}

/// IAU 2006 ecliptic precession angles `π_A`, `Π_A`, `p_A` at the TT date `tt` (ERFA `p06e`).
///
/// # Errors
/// `FA-E0001` if the date is not finite.
pub fn ecliptic_precession<R: Real>(
    tt: Epoch<Tt, R>,
) -> Result<EclipticPrecession<R>, ferroastro_core::Error> {
    let (d1, d2) = tt.jd_parts();
    let a = p06e(d1, d2);
    Ok(EclipticPrecession {
        pi_a: Angle::from_radians(a.pia)?,
        big_pi_a: Angle::from_radians(a.bpia)?,
        p_a: Angle::from_radians(a.pa)?,
    })
}
