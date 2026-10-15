// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Quantity newtypes (ADR-0001). Each type stores one canonical SI-derived unit (km, s, km/s,
//! km³/s², rad, rad/s), so conversions happen only at construction and read-out.
//!
//! Only dimensionally meaningful arithmetic is implemented: same-type `+`/`-`, scaling by a
//! dimensionless number of the same backend, same-type ratio (dimensionless),
//! `Length / TimeSpan = Velocity` and `Angle / TimeSpan = AngularRate`. Every type is generic
//! over the scalar backend ([`ferroastro_real::Real`], default `f64`), and mixing backends
//! (`Length<f64> + Length<DoubleDouble>`) does not compile either.
//! `Length + Velocity` therefore does not compile.

use crate::error::{Error, require_finite};
use ferroastro_real::Real;
use std::ops::{Add, Div, Mul, Neg, Sub};

macro_rules! quantity {
    (
        $(#[$meta:meta])*
        $name:ident, $unit:literal, $ctor:ident, $get:ident, $what:literal
    ) => {
        $(#[$meta])*
        ///
        #[doc = concat!(
            "Generic over the scalar backend `R` ([`Real`]); `", stringify!($name),
            "` alone means `", stringify!($name), "<f64>`."
        )]
        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
        pub struct $name<R: Real = f64>(R);

        impl $name<f64> {
            #[doc = concat!("The zero ", $what, " (binary64).")]
            pub const ZERO: Self = Self(0.0);
        }

        impl<R: Real> $name<R> {
            #[doc = concat!("The zero ", $what, " in any backend.")]
            #[must_use]
            pub fn zero() -> Self {
                Self(R::ZERO)
            }

            #[doc = concat!("Construct from a value in ", $unit, ".")]
            ///
            /// # Errors
            /// [`Error::NonFinite`] (`FA-E0001`) if the value is NaN or infinite.
            pub fn $ctor(value: R) -> Result<Self, Error> {
                require_finite($what, value, $unit).map(Self)
            }

            #[doc = concat!("The value in ", $unit, ".")]
            #[must_use]
            pub const fn $get(self) -> R {
                self.0
            }

            /// Absolute value.
            #[must_use]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }

            /// The same quantity in another backend (exact when widening, e.g. f64 → double-double;
            /// rounded once when narrowing).
            #[must_use]
            pub fn cast<S: Real>(self) -> $name<S> {
                let (hi, lo) = self.0.to_parts();
                $name(S::from_parts(hi, lo))
            }
        }

        impl<R: Real> Add for $name<R> {
            type Output = Self;
            fn add(self, rhs: Self) -> Self {
                Self(self.0 + rhs.0)
            }
        }

        impl<R: Real> Sub for $name<R> {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self {
                Self(self.0 - rhs.0)
            }
        }

        impl<R: Real> Neg for $name<R> {
            type Output = Self;
            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        /// Scaling by a dimensionless number of the same backend.
        impl<R: Real> Mul<R> for $name<R> {
            type Output = Self;
            fn mul(self, rhs: R) -> Self {
                Self(self.0 * rhs)
            }
        }

        impl<R: Real> Div<R> for $name<R> {
            type Output = Self;
            fn div(self, rhs: R) -> Self {
                Self(self.0 / rhs)
            }
        }

        /// Same-quantity ratio is dimensionless.
        impl<R: Real> Div for $name<R> {
            type Output = R;
            fn div(self, rhs: Self) -> R {
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

quantity!(
    /// An angular rate (e.g. a rotation rate), stored in rad/s.
    AngularRate, "rad/s", from_rad_per_s, rad_per_s, "angular rate"
);

quantity!(
    /// The product `G·S` of the gravitational constant and a body's spin angular momentum
    /// (the Lense–Thirring source strength), stored in km⁵/s³. It equals `GM · C/(MR²) · R² · Ω`,
    /// so it never needs `G` on its own.
    SpinGm, "km^5/s^3", from_km5_per_s3, km5_per_s3, "G times spin angular momentum"
);

impl<R: Real> Div<TimeSpan<R>> for Angle<R> {
    type Output = AngularRate<R>;
    fn div(self, rhs: TimeSpan<R>) -> AngularRate<R> {
        AngularRate(self.0 / rhs.0)
    }
}

impl<R: Real> Div<TimeSpan<R>> for Length<R> {
    type Output = Velocity<R>;
    fn div(self, rhs: TimeSpan<R>) -> Velocity<R> {
        Velocity(self.0 / rhs.0)
    }
}

impl<R: Real> Mul<TimeSpan<R>> for Velocity<R> {
    type Output = Length<R>;
    fn mul(self, rhs: TimeSpan<R>) -> Length<R> {
        Length(self.0 * rhs.0)
    }
}

/// Arcseconds per radian, `180·3600/π = 648000/π` (a mathematical, not physical, conversion),
/// in the backend's precision.
fn arcsec_per_rad<R: Real>() -> R {
    R::from_f64(180.0 * 3600.0) / R::PI
}

impl<R: Real> Angle<R> {
    /// Construct from arcseconds.
    ///
    /// # Errors
    /// [`Error::NonFinite`] (`FA-E0001`) if the value is NaN or infinite.
    pub fn from_arcsec(value: R) -> Result<Self, Error> {
        require_finite("angle", value, "arcsec").map(|v| Self(v / arcsec_per_rad::<R>()))
    }

    /// The angle in arcseconds.
    #[must_use]
    pub fn arcsec(self) -> R {
        self.0 * arcsec_per_rad::<R>()
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
        assert!(AngularRate::from_rad_per_s(f64::NAN).is_err());
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
        let w = Angle::from_radians(6.0)? / t;
        assert!((w.rad_per_s() - 2.0).abs() < f64::EPSILON);
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
