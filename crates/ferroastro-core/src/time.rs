// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Time scales as type markers and a two-part Julian date epoch (ADR-0001, ADR-0003).
//!
//! `Epoch<Tdb> - Epoch<Tdb>` is a [`TimeSpan`]; subtracting epochs on different scales does not
//! compile. Conversions between scales arrive with `ferroastro-time` (M1, hifitime + ERFA port).
//! UTC is deliberately absent: it is not a uniform scale and is handled by `hifitime`.

use crate::error::{Error, require_finite};
use crate::units::TimeSpan;
use ferroastro_real::Real;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Sub};

/// Seconds per day in the Julian-date convention (exactly 86 400 SI seconds by definition).
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// Days per Julian year (exactly 365.25 by definition; IAU Style Manual, Wilkins 1989).
pub const DAYS_PER_JULIAN_YEAR: f64 = 365.25;

/// Days per Julian century (exactly 36 525; the century of the ″/cy precession rates).
pub const DAYS_PER_JULIAN_CENTURY: f64 = 36_525.0;

/// Julian date of the standard epoch J2000.0 (2000 January 1, 12h), by definition (IAU 1976).
pub const J2000_JD: f64 = 2_451_545.0;

mod sealed {
    pub trait Sealed {}
}

/// A uniform astronomical time scale.
pub trait TimeScale: sealed::Sealed + Copy + Debug + Default + 'static {
    /// Short name used in reports, e.g. `"TDB"`.
    const NAME: &'static str;
}

macro_rules! scale {
    ($(#[$meta:meta])* $name:ident, $label:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;
        impl sealed::Sealed for $name {}
        impl TimeScale for $name {
            const NAME: &'static str = $label;
        }
    };
}

scale!(
    /// Terrestrial Time.
    Tt, "TT"
);
scale!(
    /// Barycentric Dynamical Time (the JPL DE ephemeris argument).
    Tdb, "TDB"
);
scale!(
    /// Barycentric Coordinate Time.
    Tcb, "TCB"
);
scale!(
    /// Geocentric Coordinate Time.
    Tcg, "TCG"
);

/// An instant on time scale `S`, stored as a two-part Julian date `day + fraction` to keep
/// sub-microsecond resolution over millennia (the ERFA/SOFA two-part convention). Both parts are
/// in backend `R` (default `f64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Epoch<S: TimeScale, R: Real = f64> {
    day: R,
    frac: R,
    _scale: PhantomData<S>,
}

impl<S: TimeScale> Epoch<S, f64> {
    /// The epoch J2000.0 (JD 2451545.0) on this time scale (binary64 parts).
    #[must_use]
    pub const fn j2000() -> Self {
        Self {
            day: J2000_JD,
            frac: 0.0,
            _scale: PhantomData,
        }
    }
}

impl<S: TimeScale, R: Real> Epoch<S, R> {
    /// Build from a two-part Julian date (days). Any split is accepted; `day + frac` is the date.
    ///
    /// # Errors
    /// [`Error::NonFinite`] (`FA-E0001`) if either part is NaN or infinite.
    pub fn from_jd_parts(day: R, frac: R) -> Result<Self, Error> {
        let day = require_finite("Julian date (day part)", day, "d")?;
        let frac = require_finite("Julian date (fraction part)", frac, "d")?;
        Ok(Self {
            day,
            frac,
            _scale: PhantomData,
        })
    }

    /// The two parts `(day, fraction)` as given or as produced by arithmetic.
    #[must_use]
    pub const fn jd_parts(self) -> (R, R) {
        (self.day, self.frac)
    }

    /// The Julian date as a single number (in f64 this loses resolution; for display only).
    #[must_use]
    pub fn jd(self) -> R {
        self.day + self.frac
    }

    /// The time-scale label.
    #[must_use]
    pub const fn scale_name(self) -> &'static str {
        S::NAME
    }

    /// The same epoch in another backend (exact when widening).
    #[must_use]
    pub fn cast<T: Real>(self) -> Epoch<S, T> {
        let (dh, dl) = self.day.to_parts();
        let (fh, fl) = self.frac.to_parts();
        Epoch {
            day: T::from_parts(dh, dl),
            frac: T::from_parts(fh, fl),
            _scale: PhantomData,
        }
    }
}

impl<S: TimeScale, R: Real> Sub for Epoch<S, R> {
    type Output = TimeSpan<R>;
    fn sub(self, rhs: Self) -> TimeSpan<R> {
        let days = (self.day - rhs.day) + (self.frac - rhs.frac);
        TimeSpan::from_seconds(days * R::from_f64(SECONDS_PER_DAY))
            .unwrap_or_else(|_| TimeSpan::zero())
    }
}

impl<S: TimeScale, R: Real> Add<TimeSpan<R>> for Epoch<S, R> {
    type Output = Self;
    fn add(self, rhs: TimeSpan<R>) -> Self {
        Self {
            day: self.day,
            frac: self.frac + rhs.seconds() / R::from_f64(SECONDS_PER_DAY),
            _scale: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: API-001
    #[test]
    fn same_scale_difference_is_a_time_span() -> Result<(), Error> {
        let a = Epoch::<Tdb>::from_jd_parts(2.0, 0.5)?;
        let b = Epoch::<Tdb>::from_jd_parts(2.0, 0.0)?;
        let dt = a - b;
        assert!((dt.seconds() - SECONDS_PER_DAY / 2.0).abs() < f64::EPSILON);
        let c = b + dt;
        assert!((c.jd() - a.jd()).abs() < f64::EPSILON);
        assert_eq!(a.scale_name(), "TDB");
        assert!(Epoch::<Tt>::from_jd_parts(f64::NAN, 0.0).is_err());
        Ok(())
    }
}
