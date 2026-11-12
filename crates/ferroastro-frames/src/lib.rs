// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Typed frame rotations (NUM-014, PHY-010, PHY-017).
//!
//! | Frame (`ferroastro_core::frames`) | Definition | Constructor |
//! |---|---|---|
//! | `Icrf` | ICRF axes: the **default output frame** of every rate | — |
//! | `EclipticJ2000` | JPL/NAIF `ECLIPJ2000`: `R_x(84381.448″)` of ICRF, no frame bias | [`icrf_to_ecliptic_j2000`] |
//! | `EclipticOfDate` | IAU 2006 mean ecliptic and equinox of date (ERFA `ecm06`, bias included) | [`icrf_to_ecliptic_of_date`] |
//! | `MercuryMeanOrbit` | Park et al. 2017 §4: z = mean orbital pole, y = z × `x̂_ICRF`, x = y × z | [`icrf_to_mean_orbit`] |
//! | `SolarEquator` | z = a configurable solar pole, x = node on the ICRF equator | [`icrf_to_solar_equator`] |
//! | `SunEquatorIau` | the same with the IAU (WGCCRE 2015) pole, as Horizons' `IAU_SUN` | [`icrf_to_sun_equator_iau`] |
//!
//! A [`Rotation<F, T>`] can only be applied to values tagged with `F`, and produces values tagged
//! with `T`, so frames cannot be mixed silently. Every rotation carries its definition (source,
//! pole, epoch) for output metadata (PHY-015).
//!
//! ```
//! use ferroastro_core::frames::{EclipticJ2000, Heliocentric, Icrf, Position};
//! use ferroastro_core::units::Length;
//! use ferroastro_frames::{ECLIPJ2000_OBLIQUITY_ARCSEC, icrf_to_ecliptic_j2000};
//!
//! let to_ecl = icrf_to_ecliptic_j2000::<f64>();
//! // The ICRF z-axis (the celestial pole) seen from the J2000 ecliptic lies ε from its pole.
//! let km = |x: f64| Length::from_km(x).unwrap();
//! let pole = Position::<Icrf, Heliocentric>::new(km(0.0), km(0.0), km(1.0));
//! let p: Position<EclipticJ2000, Heliocentric> = to_ecl.position(&pole)?;
//! let [_, y, z] = p.components();
//! let eps = y.km().atan2(z.km());
//! assert!((eps.to_degrees() * 3600.0 - ECLIPJ2000_OBLIQUITY_ARCSEC).abs() < 1e-9);
//! # Ok::<(), ferroastro_frames::FrameError>(())
//! ```

mod ecliptic;
mod error;
mod pole;
mod rotation;

pub use ecliptic::{
    ECLIPJ2000_DEFINITION, ECLIPJ2000_OBLIQUITY_ARCSEC, EclipticPrecession, ecliptic_precession,
    icrf_to_ecliptic_j2000, icrf_to_ecliptic_of_date,
};
pub use error::FrameError;
pub use pole::{
    PARK2017_POLE_DEC_DEG, PARK2017_POLE_RA_DEG, Pole, SOLAR_POLE_IAU_DEC_DEG,
    SOLAR_POLE_IAU_RA_DEG, SOLAR_POLE_IAU_SOURCE, icrf_to_mean_orbit, icrf_to_solar_equator,
    icrf_to_sun_equator_iau,
};
pub use rotation::Rotation;
