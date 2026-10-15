// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Quantity newtypes (ADR-0001). Each type stores one canonical SI-derived unit (km, s, km/s,
//! km³/s², rad), so conversions happen only at construction and read-out.
//!
//! Only dimensionally meaningful arithmetic is implemented: same-type `+`/`-`, scaling by a
//! dimensionless `f64`, same-type ratio (dimensionless) and `Length / TimeSpan = Velocity`.
//! `Length + Velocity` therefore does not compile.

use crate::error::{Error, require_finite};
use std::ops::{Add, Div, Mul, Neg, Sub};

macro_rules! quantity {
    (
        $(#[$meta:meta])*
        $name:ident, $unit:literal, $ctor:ident, $get:ident, $what:literal
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
        pub struct $name(f64);

        impl $name {
            #[doc = concat!("The zero ", $what, ".")]
            pub const ZERO: Self = Self(0.0);

            #[doc = concat!("Construct from a value in ", $unit, ".")]
            ///
            /// # Errors
            /// [`Error::NonFinite`] (`FA-E0001`) if the value is NaN or infinite.
            pub fn $ctor(value: f64) -> Result<Self, Error> {
                require_finite($what, value, $unit).map(Self)
            }

            #[doc = concat!("The value in ", $unit, ".")]
            #[must_use]
            pub const fn $get(self) -> f64 {
                self.0
            }

            /// Absolute value.
            #[must_use]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }
        }

        impl Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl Neg for $name {
            type Output = Self;
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl Mul<f64> for $name {
            type Output = Self;
            fn mul(self, rhs: f64) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl Div<f64> for $name {
            type Output = Self;
            fn div(self, rhs: f64) -> Self {
                Self(self.0 / rhs)
            }
        }

        /// Same-quantity ratio is dimensionless.
        impl Div for $name {
            type Output = f64;
            fn div(self, rhs: Self) -> f64 {
                self.0 / rhs.0
            }
        }
    };
}

quantity!(
    /// A length, stored in kilometres.
    Length, "km", from_km, km, "length"
);
quantity!(
    /// A duration (time interval), stored in SI seconds.
    TimeSpan, "s", from_seconds, seconds, "time span"
);
quantity!(
    /// A speed or velocity component, stored in km/s.
    Velocity, "km/s", from_km_per_s, km_per_s, "velocity"
);
quantity!(
    /// A gravitational parameter GM, stored in km³/s².
    GravParam, "km^3/s^2", from_km3_per_s2, km3_per_s2, "gravitational parameter"
);
quantity!(
    /// A plane angle, stored in radians.
    Angle, "rad", from_radians, radians, "angle"
);

impl Div<TimeSpan> for Length {
    type Output = Velocity;
    fn div(self, rhs: TimeSpan) -> Velocity {
        Velocity(self.0 / rhs.0)
    }
}

impl Mul<TimeSpan> for Velocity {
    type Output = Length;
    fn mul(self, rhs: TimeSpan) -> Length {
        Length(self.0 * rhs.0)
    }
}

/// Arcseconds per radian, 180·3600/π (a mathematical, not physical, conversion).
const ARCSEC_PER_RAD: f64 = 180.0 * 3600.0 / std::f64::consts::PI;

impl Angle {
    /// Construct from arcseconds.
    ///
    /// # Errors
    /// [`Error::NonFinite`] (`FA-E0001`) if the value is NaN or infinite.
    pub fn from_arcsec(value: f64) -> Result<Self, Error> {
        require_finite("angle", value, "arcsec").map(|v| Self(v / ARCSEC_PER_RAD))
    }

    /// The angle in arcseconds.
    #[must_use]
    pub fn arcsec(self) -> f64 {
        self.0 * ARCSEC_PER_RAD
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: API-001, API-002
    #[test]
    fn constructors_reject_non_finite() {
        assert!(Length::from_km(f64::INFINITY).is_err());
        assert!(TimeSpan::from_seconds(f64::NAN).is_err());
        assert!(Velocity::from_km_per_s(f64::NEG_INFINITY).is_err());
        assert!(GravParam::from_km3_per_s2(f64::NAN).is_err());
        assert!(Angle::from_arcsec(f64::NAN).is_err());
    }

    /// Verifies: API-001
    #[test]
    fn dimensional_arithmetic() -> Result<(), Error> {
        let d = Length::from_km(6.0)?;
        let t = TimeSpan::from_seconds(3.0)?;
        let v = d / t;
        assert!((v.km_per_s() - 2.0).abs() < f64::EPSILON);
        assert!(((v * t).km() - d.km()).abs() < f64::EPSILON);
        assert!(((d + d - d).km() - d.km()).abs() < f64::EPSILON);
        assert!((d / d - 1.0).abs() < f64::EPSILON);
        assert!(((-d).abs().km() - d.km()).abs() < f64::EPSILON);
        Ok(())
    }

    /// Verifies: API-001
    #[test]
    fn arcsec_round_trip() -> Result<(), Error> {
        let a = Angle::from_radians(std::f64::consts::PI)?;
        let back = Angle::from_arcsec(a.arcsec())?;
        assert!((back.radians() - a.radians()).abs() < 4.0 * f64::EPSILON);
        Ok(())
    }
}
