// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! [`Rotation`]: a proper rotation from frame `F` to frame `T`, carrying its definition.

use crate::FrameError;
use ferroastro_core::frames::{Center, Displacement, Frame, Position, State};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_erfa::{Matrix3, rxp, rxr, tr};
use ferroastro_real::Real;
use std::marker::PhantomData;

/// A rotation taking components in frame `F` to components in frame `T`: `v_T = M · v_F`.
///
/// The rows of `M` are `T`'s unit axes expressed in `F`. Composition and inversion keep the frame
/// types, so a `State<MercuryMeanOrbit, _>` can never reach code that expects ICRF (NUM-014).
/// Every rotation carries a human-readable definition (source routine, pole, epoch) for reports.
#[derive(Debug, Clone, PartialEq)]
pub struct Rotation<F: Frame, T: Frame, R: Real = f64> {
    m: Matrix3<R>,
    definition: String,
    _frames: PhantomData<fn(F) -> T>,
}

/// Largest accepted `max |M·Mᵀ − I|` element, in units of the backend epsilon.
const ORTHONORMAL_EPS: f64 = 64.0;

impl<F: Frame, T: Frame, R: Real> Rotation<F, T, R> {
    /// A rotation from its matrix (rows = `T`'s axes in `F`).
    ///
    /// # Errors
    /// `FA-E0900` unless `M` is a proper rotation to within 64 backend epsilons per element of
    /// `M·Mᵀ − I` and has a positive determinant.
    pub fn from_matrix(m: Matrix3<R>, definition: impl Into<String>) -> Result<Self, FrameError> {
        let mmt = rxr(&m, &tr(&m));
        let mut worst = R::ZERO;
        for (i, row) in mmt.iter().enumerate() {
            for (j, &x) in row.iter().enumerate() {
                let target = if i == j { R::ONE } else { R::ZERO };
                let d = (x - target).abs();
                // NaN propagates to `worst` (and is rejected below).
                if d > worst || d.is_nan() {
                    worst = d;
                }
            }
        }
        let [[a, b, c], [d, e, f], [g, h, k]] = m;
        let det = a * (e * k - f * h) - b * (d * k - f * g) + c * (d * h - e * g);
        let tol = R::EPSILON.scale(ORTHONORMAL_EPS);
        if worst.is_nan() || worst > tol || det.is_nan() || det <= R::ZERO {
            return Err(FrameError::Definition {
                what: "matrix is not a proper rotation (max |M Mt - I| shown)",
                value: worst.to_f64(),
                valid: "orthonormal to 64 epsilon per element, determinant +1",
                fix: "build rotations with the ferroastro-frames constructors",
            });
        }
        Ok(Self {
            m,
            definition: definition.into(),
            _frames: PhantomData,
        })
    }

    /// The matrix (rows = `T`'s axes in `F`).
    #[must_use]
    pub const fn matrix(&self) -> &Matrix3<R> {
        &self.m
    }

    /// The definition (source routine, pole, epoch), for output metadata.
    #[must_use]
    pub fn definition(&self) -> &str {
        &self.definition
    }

    /// The axis `k` (0 = x, 1 = y, 2 = z) of `T`, expressed in `F`.
    #[must_use]
    pub fn axis(&self, k: usize) -> Option<[R; 3]> {
        self.m.get(k).copied()
    }

    /// The inverse rotation, `T → F` (the transpose).
    #[must_use]
    pub fn inverse(&self) -> Rotation<T, F, R> {
        Rotation {
            m: tr(&self.m),
            definition: format!("inverse of [{}]", self.definition),
            _frames: PhantomData,
        }
    }

    /// `self` followed by `next`: `F → T → U`.
    #[must_use]
    pub fn then<U: Frame>(&self, next: &Rotation<T, U, R>) -> Rotation<F, U, R> {
        Rotation {
            m: rxr(&next.m, &self.m),
            definition: format!("[{}] then [{}]", self.definition, next.definition),
            _frames: PhantomData,
        }
    }

    /// Rotate raw components (`F` → `T`).
    #[must_use]
    pub fn apply(&self, v: [R; 3]) -> [R; 3] {
        rxp(&self.m, v)
    }

    fn lengths(&self, xyz: [Length<R>; 3]) -> Result<[Length<R>; 3], FrameError> {
        let [x, y, z] = self.apply(xyz.map(Length::km));
        let l = |v: R| {
            Length::from_km(v).map_err(|source| FrameError::Transform {
                quantity: "position",
                source,
            })
        };
        Ok([l(x)?, l(y)?, l(z)?])
    }

    /// Rotate a position (the centre is unchanged).
    ///
    /// # Errors
    /// `FA-E0901` if a rotated component is not finite.
    pub fn position<C: Center>(
        &self,
        p: &Position<F, C, R>,
    ) -> Result<Position<T, C, R>, FrameError> {
        let [x, y, z] = self.lengths(p.components())?;
        Ok(Position::new(x, y, z))
    }

    /// Rotate a displacement.
    ///
    /// # Errors
    /// `FA-E0901` if a rotated component is not finite.
    pub fn displacement(&self, d: &Displacement<F, R>) -> Result<Displacement<T, R>, FrameError> {
        let [x, y, z] = self.lengths(d.components())?;
        Ok(Displacement::new(x, y, z))
    }

    /// Rotate a state: position and velocity are both rotated with `M`.
    ///
    /// For a time-dependent frame (`EclipticOfDate`) this gives the velocity referred to the
    /// axes at that instant **without** the frame-rotation term `Ṁ·r` (about 1e-5 of Mercury's
    /// velocity for the IAU 2006 precession); positions, and so perihelion longitudes, are exact.
    ///
    /// # Errors
    /// `FA-E0901` if a rotated component is not finite.
    pub fn state<C: Center>(&self, s: &State<F, C, R>) -> Result<State<T, C, R>, FrameError> {
        let p = self.position(&s.position())?;
        let [vx, vy, vz] = self.apply(s.velocity().map(Velocity::km_per_s));
        let v = |x: R| {
            Velocity::from_km_per_s(x).map_err(|source| FrameError::Transform {
                quantity: "velocity",
                source,
            })
        };
        Ok(State::new(p, [v(vx)?, v(vy)?, v(vz)?]))
    }
}

impl<F: Frame, T: Frame, R: Real> Rotation<F, T, R> {
    /// A rotation from a matrix that is a rotation by construction (an ERFA rotation product or
    /// an orthonormal triad built here); not checked.
    pub(crate) const fn from_parts_unchecked(m: Matrix3<R>, definition: String) -> Self {
        Self {
            m,
            definition,
            _frames: PhantomData,
        }
    }
}

#[expect(
    clippy::mismatching_type_param_order,
    reason = "the identity maps a frame onto itself, so both frame parameters are the same"
)]
impl<F: Frame, R: Real> Rotation<F, F, R> {
    /// The identity rotation of a frame onto itself.
    #[must_use]
    pub fn identity() -> Self {
        Self {
            m: ferroastro_erfa::ir(),
            definition: format!("identity ({})", F::NAME),
            _frames: PhantomData,
        }
    }
}

/// Smallest norm [`unit`] accepts: the cross product of two unit vectors less than 1e-12 rad
/// apart defines no direction worth trusting (the same threshold as WP5's reference planes).
const MIN_CROSS_NORM: f64 = 1e-12;

/// Unit vector along `v`, or `None` if `|v|` is below 1e-12 or not finite (`v` is a cross product
/// of unit vectors here).
pub(crate) fn unit<R: Real>(v: [R; 3]) -> Option<[R; 3]> {
    let [x, y, z] = v;
    let n = x.hypot(y).hypot(z);
    if n.is_finite() && n > R::from_f64(MIN_CROSS_NORM) {
        Some(v.map(|c| c / n))
    } else {
        None
    }
}

/// `a × b`.
pub(crate) fn cross<R: Real>(a: [R; 3], b: [R; 3]) -> [R; 3] {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [a1 * b2 - a2 * b1, a2 * b0 - a0 * b2, a0 * b1 - a1 * b0]
}
