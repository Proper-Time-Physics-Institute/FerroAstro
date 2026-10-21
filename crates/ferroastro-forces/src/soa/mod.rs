// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Structure-of-arrays pair kernels (NUM-021, WP26): exact `1/r³`, no fused multiply-add, a fixed
//! reduction order, and bitwise-identical scalar and SIMD instantiations.
//!
//! # Layout and reduction order
//! [`SoaBodies`] holds one array per component (positions, velocities, GMs and a group label),
//! padded to a multiple of [`PAD`]. A kernel is vectorised **across targets**: each lane is one
//! body `i`, and the sources `j` are a scalar loop, broadcast to every lane. Body `i`'s sum
//! therefore runs over `j = 0, 1, …, m − 1` in storage order whatever the lane width; there is no
//! cross-lane reduction. Pairs whose group labels are equal (the body itself and, for the
//! Newtonian mutual term with a geocentric Moon, the Earth–Moon pair) are removed by a lane
//! select, never by arithmetic. All `m²` ordered pairs are evaluated.
//!
//! # FMA policy
//! The SoA kernels contain **no fused multiply-add** and no negation (see [`Lane`]); every
//! operation is one correctly rounded binary64 `+ − × ÷ √` or an exact select. The [`Scalar`] and
//! `F64x4` (feature `simd`) instantiations are therefore bitwise identical on every target, and
//! a run with the feature on reproduces the run with it off bit for bit. The production pairwise
//! kernels ([`crate::NewtonianMutual`], `ferroastro_eih::Eih`) keep their explicit `mul_add`
//! (always fused) and are unchanged.
//!
//! # Selection
//! [`NewtonianMutualSoa`] is the [`crate::NewtonianMutual`] physics on this kernel. Its
//! [`SoaBackend`] picks the lane type for `f64`; other backends (double-double) always use
//! [`Scalar`].

mod inv_r3;
mod lane;
mod newton;

pub use inv_r3::{inv_r3_exact, inv_r3_exact_scalar};
#[cfg(feature = "simd")]
pub use lane::F64x4;
pub use lane::{Lane, Scalar};
pub use newton::{NewtonianMutualSoa, coinciding, newtonian_soa};

use crate::nbody::View;
use ferroastro_real::Real;

/// Arrays are padded to a multiple of this (the widest lane type).
pub const PAD: usize = 4;

/// The lane type of the SoA kernels for `f64` (other backends always use [`Scalar`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SoaBackend {
    /// One value at a time ([`Scalar`]).
    Scalar,
    /// Four binary64 lanes (`F64x4`, the `wide` crate).
    #[cfg(feature = "simd")]
    Simd,
}

impl SoaBackend {
    /// `SoaBackend::Simd` when the `simd` feature is on, [`SoaBackend::Scalar`] otherwise. The
    /// two give identical bits.
    #[must_use]
    pub const fn preferred() -> Self {
        #[cfg(feature = "simd")]
        {
            SoaBackend::Simd
        }
        #[cfg(not(feature = "simd"))]
        {
            SoaBackend::Scalar
        }
    }

    /// A label for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            SoaBackend::Scalar => "scalar",
            #[cfg(feature = "simd")]
            SoaBackend::Simd => "wide f64x4",
        }
    }
}

impl Default for SoaBackend {
    fn default() -> Self {
        Self::preferred()
    }
}

/// Bodies in structure-of-arrays form: `n` real bodies, arrays padded to a multiple of [`PAD`].
/// Padding lanes have zero GM, distinct far-away positions and negative group labels; their
/// results are never read.
#[derive(Debug, Clone, PartialEq)]
pub struct SoaBodies<R: Real> {
    /// The layout index of each real body.
    pub index: Vec<usize>,
    /// Positions (km).
    pub pos: [Vec<R>; 3],
    /// Velocities (km/s).
    pub vel: [Vec<R>; 3],
    /// GM (km³/s²).
    pub gm: Vec<R>,
    /// Group label (a small non-negative integer for real bodies): pairs with equal labels do not
    /// interact.
    pub group: Vec<R>,
}

/// The padded length of `n` elements.
#[must_use]
pub const fn padded(n: usize) -> usize {
    n.div_ceil(PAD) * PAD
}

impl<R: Real> SoaBodies<R> {
    /// Gather the bodies `index` (layout indices) from a view; `group(k)` labels the `k`-th one.
    #[must_use]
    pub fn gather(view: &View<'_, R>, index: &[usize], group: impl Fn(usize) -> usize) -> Self {
        let pos: Vec<[R; 3]> = index.iter().map(|&i| view.position(i)).collect();
        let vel: Vec<[R; 3]> = index.iter().map(|&i| view.velocity(i)).collect();
        let gm: Vec<R> = index.iter().map(|&i| view.layout().gm(i)).collect();
        let groups: Vec<usize> = (0..index.len()).map(group).collect();
        let mut b = Self::from_bodies(&pos, &vel, &gm, &groups);
        b.index = index.to_vec();
        b
    }

    /// Bodies from per-body positions (km), velocities (km/s), GMs (km³/s²) and group labels
    /// (the shortest of the four slices sets the count; `index` is `0, 1, …`).
    #[must_use]
    pub fn from_bodies(pos: &[[R; 3]], vel: &[[R; 3]], gm: &[R], group: &[usize]) -> Self {
        let n = pos.len().min(vel.len()).min(gm.len()).min(group.len());
        let len = padded(n);
        let comp = |v: &[[R; 3]], c: usize| -> Vec<R> {
            let mut out: Vec<R> = v
                .iter()
                .take(n)
                .map(|p| p.get(c).copied().unwrap_or(R::ZERO))
                .collect();
            out.resize(len, R::ZERO);
            out
        };
        let mut pos_c: [Vec<R>; 3] = core::array::from_fn(|c| comp(pos, c));
        let vel_c: [Vec<R>; 3] = core::array::from_fn(|c| comp(vel, c));
        // Padding: far from every body (1e15 km apart), massless, each in its own group.
        for (k, x) in pos_c[0].iter_mut().enumerate().skip(n) {
            *x = R::from_f64(1.0e15 * count(k + 1));
        }
        let mut gm_v: Vec<R> = gm.iter().take(n).copied().collect();
        gm_v.resize(len, R::ZERO);
        let labels: Vec<R> = (0..len)
            .map(|k| match group.get(k) {
                Some(&g) if k < n => R::from_f64(count(g)),
                _ => -R::from_f64(count(k + 1)),
            })
            .collect();
        Self {
            index: (0..n).collect(),
            pos: pos_c,
            vel: vel_c,
            gm: gm_v,
            group: labels,
        }
    }

    /// The number of real bodies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// True if there are no bodies.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

/// A small count as f64 (exact below 2⁵³).
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// `a · b` without FMA, in the order `(a₀b₀ + a₁b₁) + a₂b₂`.
#[inline]
#[must_use]
pub fn dot3<L: Lane>(a: [L; 3], b: [L; 3]) -> L {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    (a0 * b0 + a1 * b1) + a2 * b2
}

/// The lanes `k..k + L::WIDTH` of three component arrays.
#[inline]
#[must_use]
pub fn load3<L: Lane>(v: &[Vec<L::Elem>; 3], k: usize) -> [L; 3] {
    core::array::from_fn(|c| L::load(v.get(c).and_then(|x| x.get(k..)).unwrap_or(&[])))
}

/// Body `j` of three component arrays, broadcast.
#[inline]
#[must_use]
pub fn splat3<L: Lane>(v: &[Vec<L::Elem>; 3], j: usize) -> [L; 3] {
    core::array::from_fn(|c| {
        L::splat(
            v.get(c)
                .and_then(|x| x.get(j))
                .copied()
                .unwrap_or(L::Elem::ZERO),
        )
    })
}
