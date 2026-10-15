// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Pole-defined frames: Mercury's mean-orbit frame (Park et al. 2017) and solar-equator frames
//! with a configurable pole (PHY-010).

use crate::rotation::{cross, unit};
use crate::{FrameError, Rotation};
use ferroastro_core::frames::{Icrf, MercuryMeanOrbit, SolarEquator, SunEquatorIau};
use ferroastro_core::units::Angle;
use ferroastro_real::Real;

/// Right ascension of Mercury's mean-orbit pole, degrees, ICRF (Park et al. 2017, AJ 153, 121,
/// §4: the mean orbital angular momentum over 1000 January 1 – 3000 January 1).
pub const PARK2017_POLE_RA_DEG: f64 = 280.9876;
/// Declination of Mercury's mean-orbit pole, degrees, ICRF (Park et al. 2017 §4).
pub const PARK2017_POLE_DEC_DEG: f64 = 61.4481;

/// Right ascension α₀ of the Sun's north pole, degrees, ICRF: IAU WGCCRE 2015 (Archinal et al.
/// 2018, `CeMDA` 130, 22), as in NAIF `pck00011.tpc` (`BODY10_POLE_RA`). Constant in time.
pub const SOLAR_POLE_IAU_RA_DEG: f64 = 286.13;
/// Declination δ₀ of the Sun's north pole, degrees, ICRF (same source, `BODY10_POLE_DEC`).
pub const SOLAR_POLE_IAU_DEC_DEG: f64 = 63.87;
/// Source of [`SOLAR_POLE_IAU_RA_DEG`] and [`SOLAR_POLE_IAU_DEC_DEG`].
pub const SOLAR_POLE_IAU_SOURCE: &str =
    "IAU WGCCRE 2015 (Archinal et al. 2018, CeMDA 130, 22); NAIF pck00011.tpc BODY10_POLE_RA/DEC";

/// A pole direction in ICRF.
#[derive(Debug, Clone, PartialEq)]
pub struct Pole<R: Real = f64> {
    /// Right ascension.
    pub ra: Angle<R>,
    /// Declination.
    pub dec: Angle<R>,
    /// Where the pole comes from (recorded in every rotation built from it).
    pub source: String,
}

fn degrees<R: Real>(deg: f64) -> Result<Angle<R>, FrameError> {
    Angle::from_radians(R::from_f64(deg) * R::PI / R::from_f64(180.0)).map_err(|source| {
        FrameError::Transform {
            quantity: "pole angle",
            source,
        }
    })
}

impl<R: Real> Pole<R> {
    /// A pole from right ascension and declination in degrees.
    ///
    /// # Errors
    /// `FA-E0901` if an angle is not finite.
    pub fn from_degrees(
        ra_deg: f64,
        dec_deg: f64,
        source: impl Into<String>,
    ) -> Result<Self, FrameError> {
        Ok(Self {
            ra: degrees(ra_deg)?,
            dec: degrees(dec_deg)?,
            source: source.into(),
        })
    }

    /// Mercury's mean-orbit pole of Park et al. 2017 (α 280.9876°, δ 61.4481°).
    ///
    /// # Errors
    /// Never for the built-in values; the `Result` mirrors [`Pole::from_degrees`].
    pub fn park2017_mercury_mean_orbit() -> Result<Self, FrameError> {
        Self::from_degrees(
            PARK2017_POLE_RA_DEG,
            PARK2017_POLE_DEC_DEG,
            "Park et al. 2017 (AJ 153, 121) section 4, Mercury mean orbital pole 1000-3000",
        )
    }

    /// The IAU (WGCCRE 2015) solar north pole, α₀ 286.13°, δ₀ 63.87°.
    ///
    /// # Errors
    /// Never for the built-in values; the `Result` mirrors [`Pole::from_degrees`].
    pub fn iau_sun() -> Result<Self, FrameError> {
        Self::from_degrees(
            SOLAR_POLE_IAU_RA_DEG,
            SOLAR_POLE_IAU_DEC_DEG,
            SOLAR_POLE_IAU_SOURCE,
        )
    }

    /// The unit vector of the pole in ICRF.
    #[must_use]
    pub fn unit_vector(&self) -> [R; 3] {
        let (sa, ca) = self.ra.radians().sin_cos();
        let (sd, cd) = self.dec.radians().sin_cos();
        [cd * ca, cd * sa, sd]
    }

    fn describe(&self) -> String {
        let deg = |a: Angle<R>| a.radians().to_f64().to_degrees();
        format!(
            "pole RA {:.6} deg, Dec {:.6} deg (ICRF) from {}",
            deg(self.ra),
            deg(self.dec),
            self.source
        )
    }
}

fn degenerate(what: &'static str, value: f64) -> FrameError {
    FrameError::Definition {
        what,
        value,
        valid: "a pole not parallel to the reference axis",
        fix: "check the pole right ascension and declination",
    }
}

/// ICRF → Mercury's mean-orbit frame with Park et al. 2017's axes (§4): z = `pole`,
/// y = z × `x̂_ICRF` (normalised), x = y × z.
///
/// # Errors
/// `FA-E0900` if the pole lies along the ICRF x-axis.
pub fn icrf_to_mean_orbit<R: Real>(
    pole: &Pole<R>,
) -> Result<Rotation<Icrf, MercuryMeanOrbit, R>, FrameError> {
    let z = pole.unit_vector();
    let y = unit(cross(z, [R::ONE, R::ZERO, R::ZERO]))
        .ok_or_else(|| degenerate("mean-orbit pole along the ICRF x-axis", 0.0))?;
    let x = cross(y, z);
    Ok(Rotation::from_parts_unchecked(
        [x, y, z],
        format!(
            "Mercury mean-orbit frame (Park et al. 2017 axes: y = z x X_ICRF, x = y x z), {}",
            pole.describe()
        ),
    ))
}

/// Body-equator axes of a pole: z = pole, x = `ẑ_ICRF` × pole (the ascending node of the equator on
/// the ICRF equator), y = z × x.
fn equator_axes<R: Real>(pole: &Pole<R>) -> Result<[[R; 3]; 3], FrameError> {
    let z = pole.unit_vector();
    let x = unit(cross([R::ZERO, R::ZERO, R::ONE], z))
        .ok_or_else(|| degenerate("equator pole along the ICRF z-axis (node undefined)", 0.0))?;
    Ok([x, cross(z, x), z])
}

/// ICRF → a solar-equator frame with a configurable pole (PHY-010): z = `pole`, x = the
/// ascending node of the solar equator on the ICRF equator. The pole and its source are recorded
/// in [`Rotation::definition`].
///
/// # Errors
/// `FA-E0900` if the pole lies along the ICRF z-axis.
pub fn icrf_to_solar_equator<R: Real>(
    pole: &Pole<R>,
) -> Result<Rotation<Icrf, SolarEquator, R>, FrameError> {
    Ok(Rotation::from_parts_unchecked(
        equator_axes(pole)?,
        format!(
            "solar equator (z = pole, x = node on the ICRF equator), {}",
            pole.describe()
        ),
    ))
}

/// ICRF → `IAU_SUN` (the Sun's IAU body-equator frame, as JPL Horizons realises it): the
/// [`icrf_to_solar_equator`] axes of the IAU pole [`Pole::iau_sun`].
///
/// # Errors
/// Never for the built-in pole.
pub fn icrf_to_sun_equator_iau<R: Real>() -> Result<Rotation<Icrf, SunEquatorIau, R>, FrameError> {
    let pole = Pole::iau_sun()?;
    Ok(Rotation::from_parts_unchecked(
        equator_axes(&pole)?,
        format!(
            "IAU_SUN (z = IAU solar pole, x = node on the ICRF equator), {}",
            pole.describe()
        ),
    ))
}
