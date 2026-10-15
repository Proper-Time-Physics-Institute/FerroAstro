// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Reference frames and centres as zero-sized type markers (ADR-0001).
//!
//! A [`Position<F, C>`] is tagged with its frame `F` and centre `C`. Subtracting two positions
//! yields a centre-free [`Displacement<F>`], and only when frame **and** centre agree; mixing
//! them is a compile error. Frame conversions are explicit, typed rotations in
//! `ferroastro-frames`, each citing its definition (ARCHITECTURE design rule 2); ICRF is the
//! default output frame (NUM-014).

use crate::units::{Length, Velocity};
use ferroastro_real::Real;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::ops::{Add, Sub};

mod sealed {
    pub trait Sealed {}
}

/// A reference frame (orientation of the axes).
pub trait Frame: sealed::Sealed + Copy + Debug + Default + 'static {
    /// Short name used in reports, e.g. `"ICRF"`.
    const NAME: &'static str;
}

/// An origin (centre) of coordinates.
pub trait Center: sealed::Sealed + Copy + Debug + Default + 'static {
    /// Short name used in reports, e.g. `"SSB"`.
    const NAME: &'static str;
}

/// International Celestial Reference Frame axes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Icrf;
/// Mean ecliptic and equinox of J2000 axes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EclipticJ2000;
/// The Sun's IAU (WGCCRE) body-equator frame as JPL Horizons realises it (`IAU_SUN`): z along
/// the IAU solar north pole, x along the ascending node of the solar equator on the ICRF
/// equator. The IAU solar pole has no precession term, so "of date" equals fixed axes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SunEquatorIau;
/// IAU 2006 mean ecliptic and equinox **of date** (ERFA `ecm06`). The date is not part of the
/// type: a value in this frame is only meaningful together with the epoch of the rotation that
/// produced it. For PHY-017 historical comparisons and NUM-014 only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EclipticOfDate;
/// Mercury's mean-orbit frame (Park et al. 2017 §4): z along the mean orbital pole, y = z × `x̂_ICRF`
/// (normalised), x = y × z. Fixed axes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MercuryMeanOrbit;
/// A solar-equator frame with a **configurable** pole (PHY-010): z along the pole, x along the
/// ascending node of the solar equator on the ICRF equator. The pole travels with the rotation
/// as metadata; [`SunEquatorIau`] is the special case of the IAU (WGCCRE) pole.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SolarEquator;
/// Solar-system barycentre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Barycentric;
/// Centre of the Sun.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Heliocentric;

impl sealed::Sealed for Icrf {}
impl sealed::Sealed for EclipticJ2000 {}
impl sealed::Sealed for SunEquatorIau {}
impl sealed::Sealed for EclipticOfDate {}
impl sealed::Sealed for MercuryMeanOrbit {}
impl sealed::Sealed for SolarEquator {}
impl sealed::Sealed for Barycentric {}
impl sealed::Sealed for Heliocentric {}

impl Frame for Icrf {
    const NAME: &'static str = "ICRF";
}
impl Frame for EclipticJ2000 {
    const NAME: &'static str = "ECLIPJ2000";
}
impl Frame for SunEquatorIau {
    const NAME: &'static str = "IAU_SUN";
}
impl Frame for EclipticOfDate {
    const NAME: &'static str = "ECLIPDATE-IAU2006";
}
impl Frame for MercuryMeanOrbit {
    const NAME: &'static str = "MERCURY-MEAN-ORBIT";
}
impl Frame for SolarEquator {
    const NAME: &'static str = "SOLAR-EQUATOR";
}
impl Center for Barycentric {
    const NAME: &'static str = "SSB";
}
impl Center for Heliocentric {
    const NAME: &'static str = "SUN";
}

/// A position vector in frame `F` relative to centre `C`, components in backend `R`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position<F: Frame, C: Center, R: Real = f64> {
    xyz: [Length<R>; 3],
    _tag: PhantomData<(F, C)>,
}

/// A centre-free displacement vector in frame `F`, components in backend `R`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Displacement<F: Frame, R: Real = f64> {
    xyz: [Length<R>; 3],
    _tag: PhantomData<F>,
}

impl<F: Frame, C: Center, R: Real> Position<F, C, R> {
    /// Build a position from its three components.
    #[must_use]
    pub const fn new(x: Length<R>, y: Length<R>, z: Length<R>) -> Self {
        Self {
            xyz: [x, y, z],
            _tag: PhantomData,
        }
    }

    /// The components `[x, y, z]`.
    #[must_use]
    pub const fn components(&self) -> [Length<R>; 3] {
        self.xyz
    }

    /// The same position in another backend (exact when widening).
    #[must_use]
    pub fn cast<S: Real>(&self) -> Position<F, C, S> {
        let [x, y, z] = self.xyz;
        Position::new(x.cast(), y.cast(), z.cast())
    }
}

impl<F: Frame, R: Real> Displacement<F, R> {
    /// Build a displacement from its three components.
    #[must_use]
    pub const fn new(x: Length<R>, y: Length<R>, z: Length<R>) -> Self {
        Self {
            xyz: [x, y, z],
            _tag: PhantomData,
        }
    }

    /// The components `[x, y, z]`.
    #[must_use]
    pub const fn components(&self) -> [Length<R>; 3] {
        self.xyz
    }

    /// Euclidean norm, computed with `hypot` to avoid overflow.
    #[must_use]
    pub fn norm(&self) -> Length<R> {
        let [x, y, z] = self.xyz;
        let r = x.km().hypot(y.km()).hypot(z.km());
        // A hypot of finite values is finite, so this cannot fail; fall back to zero defensively.
        Length::from_km(r).unwrap_or_else(|_| Length::zero())
    }
}

impl<F: Frame, C: Center, R: Real> Sub for Position<F, C, R> {
    type Output = Displacement<F, R>;
    fn sub(self, rhs: Self) -> Displacement<F, R> {
        let [a, b, c] = self.xyz;
        let [d, e, f] = rhs.xyz;
        Displacement::new(a - d, b - e, c - f)
    }
}

impl<F: Frame, C: Center, R: Real> Add<Displacement<F, R>> for Position<F, C, R> {
    type Output = Self;
    fn add(self, rhs: Displacement<F, R>) -> Self {
        let [a, b, c] = self.xyz;
        let [d, e, f] = rhs.xyz;
        Self::new(a + d, b + e, c + f)
    }
}

/// A position and velocity in frame `F` relative to centre `C` (a Cartesian state vector),
/// components in backend `R`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State<F: Frame, C: Center, R: Real = f64> {
    position: Position<F, C, R>,
    velocity: [Velocity<R>; 3],
}

impl<F: Frame, C: Center, R: Real> State<F, C, R> {
    /// Build a state from a position and its velocity components (same frame and centre).
    #[must_use]
    pub const fn new(position: Position<F, C, R>, velocity: [Velocity<R>; 3]) -> Self {
        Self { position, velocity }
    }

    /// The position.
    #[must_use]
    pub const fn position(&self) -> Position<F, C, R> {
        self.position
    }

    /// The velocity components `[vx, vy, vz]`.
    #[must_use]
    pub const fn velocity(&self) -> [Velocity<R>; 3] {
        self.velocity
    }

    /// The same state in another backend (exact when widening, e.g. f64 → double-double).
    #[must_use]
    pub fn cast<S: Real>(&self) -> State<F, C, S> {
        State::new(self.position.cast(), self.velocity.map(Velocity::cast))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    /// Verifies: API-001
    #[test]
    fn same_frame_and_centre_subtract() -> Result<(), Error> {
        let p = Position::<Icrf, Barycentric>::new(
            Length::from_km(3.0)?,
            Length::from_km(4.0)?,
            Length::ZERO,
        );
        let o = Position::<Icrf, Barycentric>::new(Length::ZERO, Length::ZERO, Length::ZERO);
        let d = p - o;
        assert!((d.norm().km() - 5.0).abs() < f64::EPSILON);
        assert_eq!((o + d).components(), p.components());
        assert_eq!(<Icrf as Frame>::NAME, "ICRF");
        assert_eq!(<Heliocentric as Center>::NAME, "SUN");
        Ok(())
    }
}
