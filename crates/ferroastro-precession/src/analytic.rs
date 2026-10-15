// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Analytic Schwarzschild (1PN) perihelion advance, rung GR-SCHW analytic (PHY-004).
//!
//! `Δϖ = 6πGM / (c² a (1 − e²))` per orbit (Will 2014, Living Rev. Relativ. 17, 4, Eq. 65 with
//! β = γ = 1, test-particle limit). The orbital period that converts it to a rate is the
//! Keplerian `P = 2π √(a³/μ)`, where `μ` may differ from the `GM` of the advance (PHY-004 uses
//! `GM☉` for the advance and `GM☉ + GM_Mercury` for the period; caveat C7).

use crate::PrecessionError;
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, SECONDS_PER_DAY};
use ferroastro_core::units::{Angle, AngularRate, GravParam, Length, TimeSpan, Velocity};
use std::f64::consts::{PI, TAU};

/// The analytic advance and the numbers it was built from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchwarzschildAdvance {
    /// Advance per orbit.
    pub per_orbit: Angle,
    /// Keplerian period.
    pub period: TimeSpan,
    /// Orbits per Julian century (36 525 d of 86 400 s).
    pub orbits_per_century: f64,
    /// Secular rate.
    pub rate: AngularRate,
}

impl SchwarzschildAdvance {
    /// The rate in arcseconds per Julian century.
    #[must_use]
    pub fn arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.rate)
    }
}

/// Convert an angular rate to arcseconds per Julian century.
#[must_use]
pub fn rate_arcsec_per_century(rate: AngularRate) -> f64 {
    let rad = rate.rad_per_s() * SECONDS_PER_DAY * DAYS_PER_JULIAN_CENTURY;
    Angle::from_radians(rad).map_or(f64::NAN, Angle::arcsec)
}

/// Evaluate the Schwarzschild advance.
///
/// * `gm_advance`: the `GM` in `6πGM/(c²a(1−e²))`;
/// * `c`: speed of light; `a`, `e`: semi-major axis and eccentricity;
/// * `mu_period`: the gravitational parameter of the Keplerian period.
///
/// # Errors
/// `FA-E0600` for `e ∉ [0, 1)` or non-positive `a`, `c`, `GM` or `μ`.
pub fn schwarzschild_advance(
    gm_advance: GravParam,
    c: Velocity,
    a: Length,
    e: f64,
    mu_period: GravParam,
) -> Result<SchwarzschildAdvance, PrecessionError> {
    let dom = |what, value| PrecessionError::Domain { what, value };
    let (gm, cc, aa, mu) = (
        gm_advance.km3_per_s2(),
        c.km_per_s(),
        a.km(),
        mu_period.km3_per_s2(),
    );
    if !(0.0..1.0).contains(&e) {
        return Err(dom("eccentricity", e));
    }
    for (what, v) in [
        ("GM (km^3/s^2)", gm),
        ("c (km/s)", cc),
        ("a (km)", aa),
        ("mu (km^3/s^2)", mu),
    ] {
        if !(v > 0.0 && v.is_finite()) {
            return Err(dom(what, v));
        }
    }
    let per_orbit = 6.0 * PI * gm / (cc * cc * aa * e.mul_add(-e, 1.0));
    let period = TAU * (aa * aa * aa / mu).sqrt();
    let century = SECONDS_PER_DAY * DAYS_PER_JULIAN_CENTURY;
    let orbits_per_century = century / period;
    let ang = Angle::from_radians(per_orbit).map_err(|_| dom("advance per orbit", per_orbit))?;
    let per = TimeSpan::from_seconds(period).map_err(|_| dom("period (s)", period))?;
    Ok(SchwarzschildAdvance {
        per_orbit: ang,
        period: per,
        orbits_per_century,
        rate: ang / per,
    })
}
