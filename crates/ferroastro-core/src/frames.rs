// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Reference frames and centres as zero-sized type markers (ADR-0001).
//!
//! A [`Position<F, C>`] is tagged with its frame `F` and centre `C`. Subtracting two positions
//! yields a centre-free [`Displacement<F>`], and only when frame **and** centre agree; mixing
//! them is a compile error. Frame/centre conversions will be explicit functions citing their
//! source routine (ARCHITECTURE design rule 2); none exist in M0.

use crate::units::{Length, Velocity};
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
/// Solar-system barycentre.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Barycentric;
/// Centre of the Sun.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Heliocentric;

impl sealed::Sealed for Icrf {}
impl sealed::Sealed for EclipticJ2000 {}
impl sealed::Sealed for SunEquatorIau {}
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
impl Center for Barycentric {
    const NAME: &'static str = "SSB";
}
impl Center for Heliocentric {
    const NAME: &'static str = "SUN";
}

/// A position vector in frame `F` relative to centre `C`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position<F: Frame, C: Center> {
    xyz: [Length; 3],
    _tag: PhantomData<(F, C)>,
}

/// A centre-free displacement vector in frame `F`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Displacement<F: Frame> {
    xyz: [Length; 3],
    _tag: PhantomData<F>,
}

impl<F: Frame, C: Center> Position<F, C> {
    /// Build a position from its three components.
    #[must_use]
    pub const fn new(x: Length, y: Length, z: Length) -> Self {
        Self {
            xyz: [x, y, z],
            _tag: PhantomData,
        }
    }

    /// The components `[x, y, z]`.
    #[must_use]
    pub const fn components(&self) -> [Length; 3] {
        self.xyz
    }
}

impl<F: Frame> Displacement<F> {
    /// Build a displacement from its three components.
    #[must_use]
    pub const fn new(x: Length, y: Length, z: Length) -> Self {
        Self {
            xyz: [x, y, z],
            _tag: PhantomData,
        }
    }

    /// The components `[x, y, z]`.
    #[must_use]
    pub const fn components(&self) -> [Length; 3] {
        self.xyz
    }

    /// Euclidean norm, computed with `hypot` to avoid overflow.
    #[must_use]
    pub fn norm(&self) -> Length {
        let [x, y, z] = self.xyz;
        let r = x.km().hypot(y.km()).hypot(z.km());
        // A hypot of finite values is finite, so this cannot fail; fall back to ZERO defensively.
        Length::from_km(r).unwrap_or(Length::ZERO)
    }
}

impl<F: Frame, C: Center> Sub for Position<F, C> {
    type Output = Displacement<F>;
    fn sub(self, rhs: Self) -> Displacement<F> {
        let [a, b, c] = self.xyz;
        let [d, e, f] = rhs.xyz;
        Displacement::new(a - d, b - e, c - f)
    }
}

impl<F: Frame, C: Center> Add<Displacement<F>> for Position<F, C> {
    type Output = Self;
    fn add(self, rhs: Displacement<F>) -> Self {
        let [a, b, c] = self.xyz;
        let [d, e, f] = rhs.xyz;
        Self::new(a + d, b + e, c + f)
    }
}

/// A position and velocity in frame `F` relative to centre `C` (a Cartesian state vector).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State<F: Frame, C: Center> {
    position: Position<F, C>,
    velocity: [Velocity; 3],
}

impl<F: Frame, C: Center> State<F, C> {
    /// Build a state from a position and its velocity components (same frame and centre).
    #[must_use]
    pub const fn new(position: Position<F, C>, velocity: [Velocity; 3]) -> Self {
        Self { position, velocity }
    }

    /// The position.
    #[must_use]
    pub const fn position(&self) -> Position<F, C> {
        self.position
    }

    /// The velocity components `[vx, vy, vz]`.
    #[must_use]
    pub const fn velocity(&self) -> [Velocity; 3] {
        self.velocity
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
