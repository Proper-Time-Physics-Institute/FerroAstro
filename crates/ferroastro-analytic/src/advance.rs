// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Analytic perihelion advances: rung GR-SCHW (PHY-004) and the single-body oracles of
//! NUM-015 (SR exact, PPN scaling, 1PN unit, J2 planar and inclined, Lense–Thirring).
//!
//! `Δϖ = 6πGM / (c² a (1 − e²))` per orbit (Will 2014, Living Rev. Relativ. 17, 4, Eq. 65 with
//! β = γ = 1, test-particle limit). The orbital period that converts it to a rate is the
//! Keplerian `P = 2π √(a³/μ)`, where `μ` may differ from the `GM` of the advance (PHY-004 uses
//! `GM☉` for the advance and `GM☉ + GM_Mercury` for the period; caveat C7).

use crate::AnalyticError;
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, SECONDS_PER_DAY};
use ferroastro_core::units::{Angle, AngularRate, GravParam, Length, SpinGm, TimeSpan, Velocity};
use ferroastro_real::Real;

/// The analytic advance and the numbers it was built from (backend `R`, default `f64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchwarzschildAdvance<R: Real = f64> {
    /// Advance per orbit.
    pub per_orbit: Angle<R>,
    /// Keplerian period.
    pub period: TimeSpan<R>,
    /// Orbits per Julian century (36 525 d of 86 400 s).
    pub orbits_per_century: R,
    /// Secular rate.
    pub rate: AngularRate<R>,
}

impl<R: Real> SchwarzschildAdvance<R> {
    /// The rate in arcseconds per Julian century.
    #[must_use]
    pub fn arcsec_per_century(&self) -> R {
        rate_arcsec_per_century(self.rate)
    }
}

/// Convert an angular rate to arcseconds per Julian century.
#[must_use]
pub fn rate_arcsec_per_century<R: Real>(rate: AngularRate<R>) -> R {
    let rad =
        rate.rad_per_s() * R::from_f64(SECONDS_PER_DAY) * R::from_f64(DAYS_PER_JULIAN_CENTURY);
    Angle::from_radians(rad).map_or(R::from_f64(f64::NAN), Angle::arcsec)
}

fn dom<R: Real>(what: &'static str, value: R) -> AnalyticError {
    AnalyticError::Domain {
        what,
        value: value.to_f64(),
    }
}

/// Evaluate the Schwarzschild advance.
///
/// * `gm_advance`: the `GM` in `6πGM/(c²a(1−e²))`;
/// * `c`: speed of light; `a`, `e`: semi-major axis and eccentricity;
/// * `mu_period`: the gravitational parameter of the Keplerian period.
///
/// **μ convention used throughout FerroAstro** (caveat C7, WP6 review): the advance takes the
/// central mass alone, `gm_advance = GM☉` (test-particle limit of the Schwarzschild field), and
/// the period — hence the mean motion `n` — takes the two-body `mu_period = GM☉ + GM☿`, which is
/// also the GM with which JPL Horizons computes osculating elements. Using `GM☉` in both places
/// changes Mercury's rate by about 4e-6 ″/cy (7e-6 ″/cy for `GM☉ + GM☿` in both). Harmless
/// against PHY-004's 1e-4, but it matters once rates are differenced at NUM-013's 1e-4 level, so
/// callers must keep the convention fixed across the runs they difference.
///
/// # Errors
/// `FA-E0600` for `e ∉ [0, 1)` or non-positive `a`, `c`, `GM` or `μ`.
pub fn schwarzschild_advance<R: Real>(
    gm_advance: GravParam<R>,
    c: Velocity<R>,
    a: Length<R>,
    e: R,
    mu_period: GravParam<R>,
) -> Result<SchwarzschildAdvance<R>, AnalyticError> {
    let (gm, cc, aa, mu) = (
        gm_advance.km3_per_s2(),
        c.km_per_s(),
        a.km(),
        mu_period.km3_per_s2(),
    );
    if !(e >= R::ZERO && e < R::ONE) {
        return Err(dom("eccentricity", e));
    }
    for (what, v) in [
        ("GM (km^3/s^2)", gm),
        ("c (km/s)", cc),
        ("a (km)", aa),
        ("mu (km^3/s^2)", mu),
    ] {
        if !(v > R::ZERO && v.is_finite()) {
            return Err(dom(what, v));
        }
    }
    let per_orbit = R::from_f64(6.0) * R::PI * gm / (cc * cc * aa * e.mul_add(-e, R::ONE));
    let period = R::TAU * (aa * aa * aa / mu).sqrt();
    let century = R::from_f64(SECONDS_PER_DAY) * R::from_f64(DAYS_PER_JULIAN_CENTURY);
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

// ---------------------------------------------------------------------------------------------
// Single-body analytic oracles (NUM-015; rungs SR-a, SR-b, PPN, J2, LT).
// ---------------------------------------------------------------------------------------------

fn positive<R: Real>(what: &'static str, v: R) -> Result<R, AnalyticError> {
    if v.is_finite() && v > R::ZERO {
        Ok(v)
    } else {
        Err(dom(what, v))
    }
}

fn eccentricity<R: Real>(e: R) -> Result<R, AnalyticError> {
    if e >= R::ZERO && e < R::ONE {
        Ok(e)
    } else {
        Err(dom("eccentricity", e))
    }
}

fn angle<R: Real>(what: &'static str, rad: R) -> Result<Angle<R>, AnalyticError> {
    Angle::from_radians(rad).map_err(|_| dom(what, rad))
}

fn rate<R: Real>(what: &'static str, rad_s: R) -> Result<AngularRate<R>, AnalyticError> {
    AngularRate::from_rad_per_s(rad_s).map_err(|_| dom(what, rad_s))
}

/// PPN scaling of the Schwarzschild advance, `(2 + 2γ − β)/3` (Will 2014 Eq. 65; PHY-006).
#[must_use]
pub fn ppn_scaling<R: Real>(beta: R, gamma: R) -> R {
    R::TWO.mul_add(gamma, R::TWO - beta) / R::from_f64(3.0)
}

/// The 1PN advance unit `πμ/(c²p)` (PHY-009): a single coefficient of the general test-particle
/// Lagrangian advances the perihelion by `8A`, `4B`, `2C` or `2D` of these units per orbit.
///
/// # Errors
/// `FA-E0600` for non-positive `μ`, `c` or `p`.
pub fn pn_unit<R: Real>(
    mu: GravParam<R>,
    c: Velocity<R>,
    p: Length<R>,
) -> Result<Angle<R>, AnalyticError> {
    let m = positive("GM (km^3/s^2)", mu.km3_per_s2())?;
    let cc = positive("c (km/s)", c.km_per_s())?;
    let pp = positive("semi-latus rectum (km)", p.km())?;
    angle("1PN unit", R::PI * m / (cc * cc * pp))
}

/// Exact SR-Kepler advance per orbit (Journal #4 R2):
/// `2π[(1 − μ²/(c²h²))^(−1/2) − 1]` with the conserved `h = γ|r × v|`, evaluated from a state
/// (`r` in km, `v` in km/s) without cancellation as `2π·expm1(−½·ln1p(−μ²/(c²h²)))`.
///
/// # Errors
/// `FA-E0600` for non-positive `μ` or `c`, `|v| ≥ c`, or `μ ≥ c h` (no bound rosette).
pub fn sr_exact_advance<R: Real>(
    mu: GravParam<R>,
    c: Velocity<R>,
    r: [R; 3],
    v: [R; 3],
) -> Result<Angle<R>, AnalyticError> {
    let m = positive("GM (km^3/s^2)", mu.km3_per_s2())?;
    let cc = positive("c (km/s)", c.km_per_s())?;
    let b2 = crate::vector::dot(v, v) / (cc * cc);
    if b2.is_nan() || b2 >= R::ONE {
        return Err(dom("v^2/c^2", b2));
    }
    let h = crate::vector::norm(crate::vector::cross(r, v)) / (R::ONE - b2).sqrt();
    let x = m * m / (cc * cc * h * h);
    if x.is_nan() || x >= R::ONE {
        return Err(dom("mu^2/(c^2 h^2)", x));
    }
    angle(
        "SR exact advance",
        R::TAU * (R::from_f64(-0.5) * (-x).ln_1p()).exp_m1(),
    )
}

/// Keplerian period `2π√(a³/μ)`.
///
/// # Errors
/// `FA-E0600` for non-positive `a` or `μ`.
pub fn keplerian_period<R: Real>(
    mu: GravParam<R>,
    a: Length<R>,
) -> Result<TimeSpan<R>, AnalyticError> {
    let m = positive("GM (km^3/s^2)", mu.km3_per_s2())?;
    let aa = positive("a (km)", a.km())?;
    let p = R::TAU * (aa * aa * aa / m).sqrt();
    TimeSpan::from_seconds(p).map_err(|_| dom("period (s)", p))
}

/// Rung J2, orbit in the central body's equatorial plane: `3πJ₂R²/p²` per orbit
/// (Journal #4 R1; PHY-010).
///
/// # Errors
/// `FA-E0600` for a non-finite `J₂` or non-positive `R` or `p`.
pub fn j2_planar_advance<R: Real>(
    j2: R,
    radius: Length<R>,
    p: Length<R>,
) -> Result<Angle<R>, AnalyticError> {
    if !j2.is_finite() {
        return Err(dom("J2", j2));
    }
    let rr = positive("radius (km)", radius.km())?;
    let pp = positive("semi-latus rectum (km)", p.km())?;
    angle(
        "J2 advance",
        R::from_f64(3.0) * R::PI * j2 * rr * rr / (pp * pp),
    )
}

/// Rung J2 for an orbit inclined by `i` to the central body's equator, the rate of the
/// perihelion longitude **along the orbit plane** (Park et al. 2017, AJ 153, 121, Eq. 3):
/// `ϖ̇ = (3/2) n J₂ (R/a)² (1 − e²)⁻² (1 − (3/2) sin² i)`, with `n = √(μ/a³)`.
/// At `i = 0` it reduces to [`j2_planar_advance`] per Keplerian period.
///
/// # Errors
/// `FA-E0600` for invalid `μ`, `a`, `e`, `R` or a non-finite `J₂`.
pub fn j2_inclined_rate<R: Real>(
    mu: GravParam<R>,
    j2: R,
    radius: Length<R>,
    a: Length<R>,
    e: R,
    inclination: Angle<R>,
) -> Result<AngularRate<R>, AnalyticError> {
    if !j2.is_finite() {
        return Err(dom("J2", j2));
    }
    let m = positive("GM (km^3/s^2)", mu.km3_per_s2())?;
    let aa = positive("a (km)", a.km())?;
    let rr = positive("radius (km)", radius.km())?;
    let one_e2 = eccentricity(e)?.mul_add(-e, R::ONE);
    let n = (m / (aa * aa * aa)).sqrt();
    let s = inclination.radians().sin();
    let three_halves = R::from_f64(1.5);
    let geom = (-three_halves * s).mul_add(s, R::ONE);
    rate(
        "J2 rate",
        three_halves * n * j2 * (rr / aa) * (rr / aa) / (one_e2 * one_e2) * geom,
    )
}

/// Rung LT: the Lense–Thirring rate of the perihelion longitude along the orbit plane
/// (Park et al. 2017, AJ 153, 121, Eq. 2):
/// `ϖ̇ = −2(1 + γ) G S cos i / (c² a³ (1 − e²)^(3/2))`, `i` the inclination of the spin
/// equator to the orbit plane. At `γ = 1`, `i = 0` this is `−4GS/(c²a³(1−e²)^(3/2))` (PHY-011).
///
/// # Errors
/// `FA-E0600` for invalid `c`, `a`, `e`, a negative `G·S` or non-finite `γ`.
pub fn lense_thirring_rate<R: Real>(
    gs: SpinGm<R>,
    c: Velocity<R>,
    gamma: R,
    a: Length<R>,
    e: R,
    inclination: Angle<R>,
) -> Result<AngularRate<R>, AnalyticError> {
    let g = gs.km5_per_s3();
    if !(g.is_finite() && g >= R::ZERO && gamma.is_finite()) {
        return Err(dom("G S (km^5/s^3) or gamma", g));
    }
    let cc = positive("c (km/s)", c.km_per_s())?;
    let aa = positive("a (km)", a.km())?;
    let one_e2 = eccentricity(e)?.mul_add(-e, R::ONE);
    let denom = cc * cc * aa * aa * aa * one_e2 * one_e2.sqrt();
    rate(
        "LT rate",
        -R::TWO * (R::ONE + gamma) * g * inclination.radians().cos() / denom,
    )
}

#[cfg(test)]
mod single_body_tests {
    use super::*;

    fn gm(x: f64) -> GravParam {
        GravParam::from_km3_per_s2(x).unwrap()
    }
    fn len(x: f64) -> Length {
        Length::from_km(x).unwrap()
    }

    /// Internal consistency: Park17 Eq. 3 at i = 0 equals 3πJ₂R²/p² per Keplerian period;
    /// the SR exact formula reduces to πμ/(c²p) at leading order; PPN scaling is 1 at β = γ = 1.
    ///
    /// Verifies: NUM-015, PHY-006, PHY-010
    #[test]
    fn oracles_reduce_to_their_limits() {
        let (mu, a, e, r) = (gm(1.0), len(1.0), 0.5_f64, len(0.1)); // lint: allow-literal(dimensionless test geometry)
        let p = len(a.km() * e.mul_add(-e, 1.0));
        let planar = j2_planar_advance(1.0, r, p).unwrap().radians();
        let incl = j2_inclined_rate(mu, 1.0, r, a, e, Angle::ZERO).unwrap();
        let per = (incl * 1.0).rad_per_s() * keplerian_period(mu, a).unwrap().seconds();
        assert!((per - planar).abs() <= 8.0 * f64::EPSILON * planar);
        assert!((ppn_scaling(1.0, 1.0) - 1.0).abs() <= f64::EPSILON);
        // Circular orbit r = 1, v = sqrt(mu/r) = 1, c = 1000: exact vs leading term.
        let c = Velocity::from_km_per_s(1.0e3).unwrap(); // lint: allow-literal(dimensionless test speed of light)
        let exact = sr_exact_advance(mu, c, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
            .unwrap()
            .radians();
        let unit = pn_unit(mu, c, len(1.0)).unwrap().radians();
        assert!((exact / unit - 1.0).abs() < 1e-5); // lint: allow-literal(O(mu/c^2 r) = 1e-6 next-order term)
        assert!(sr_exact_advance(mu, c, [1.0, 0.0, 0.0], [0.0, 2.0e3, 0.0]).is_err()); // lint: allow-literal(v > c)
        let lt = lense_thirring_rate(
            SpinGm::from_km5_per_s3(1.0).unwrap(),
            c,
            1.0,
            a,
            0.0,
            Angle::ZERO,
        )
        .unwrap()
        .rad_per_s();
        assert!((lt + 4.0e-6).abs() < 1e-20); // lint: allow-literal(-4 GS/(c^2 a^3) with c = 1000)
    }
}
