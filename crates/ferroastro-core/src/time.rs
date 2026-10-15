// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Time scales as type markers and a two-part Julian date epoch (ADR-0001, ADR-0003).
//!
//! `Epoch<Tdb> - Epoch<Tdb>` is a [`TimeSpan`]; subtracting epochs on different scales does not
//! compile. Conversions between scales arrive with `ferroastro-time` (M1, hifitime + ERFA port).
//! UTC is deliberately absent: it is not a uniform scale and is handled by `hifitime`.

use crate::error::{Error, require_finite};
use crate::units::TimeSpan;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Sub};

/// Seconds per day in the Julian-date convention (exactly 86 400 SI seconds by definition).
const SECONDS_PER_DAY: f64 = 86_400.0;

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
/// sub-microsecond resolution over millennia (the ERFA/SOFA two-part convention).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Epoch<S: TimeScale> {
    day: f64,
    frac: f64,
    _scale: PhantomData<S>,
}

impl<S: TimeScale> Epoch<S> {
    /// Build from a two-part Julian date (days). Any split is accepted; `day + frac` is the date.
    ///
    /// # Errors
    /// [`Error::NonFinite`] (`FA-E0001`) if either part is NaN or infinite.
    pub fn from_jd_parts(day: f64, frac: f64) -> Result<Self, Error> {
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
    pub const fn jd_parts(self) -> (f64, f64) {
        (self.day, self.frac)
    }

    /// The Julian date as a single `f64` (loses resolution; for display only).
    #[must_use]
    pub fn jd(self) -> f64 {
        self.day + self.frac
    }

    /// The time-scale label.
    #[must_use]
    pub const fn scale_name(self) -> &'static str {
        S::NAME
    }
}

impl<S: TimeScale> Sub for Epoch<S> {
    type Output = TimeSpan;
    fn sub(self, rhs: Self) -> TimeSpan {
        let days = (self.day - rhs.day) + (self.frac - rhs.frac);
        TimeSpan::from_seconds(days * SECONDS_PER_DAY).unwrap_or(TimeSpan::ZERO)
    }
}

impl<S: TimeScale> Add<TimeSpan> for Epoch<S> {
    type Output = Self;
    fn add(self, rhs: TimeSpan) -> Self {
        Self {
            day: self.day,
            frac: self.frac + rhs.seconds() / SECONDS_PER_DAY,
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
