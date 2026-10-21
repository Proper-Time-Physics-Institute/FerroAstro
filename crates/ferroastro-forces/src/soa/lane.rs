// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The [`Lane`] abstraction: a kernel is written once, generic over the lane type, and runs with
//! [`Scalar`] (width 1, every [`Real`] backend) or, with the `simd` feature, `F64x4` (four
//! binary64 lanes, the safe `wide` crate).
//!
//! **Bitwise contract (TST-005, NUM-021).** Every operation of the trait is either one IEEE-754
//! binary64 operation rounded to nearest (`+ − × ÷ √`), applied lane by lane, or an exact lane
//! select. There is no fused multiply-add and no negation: `wide`'s `mul_add` is fused only when
//! the `fma` target feature is on, and its `Neg` is `0 − x` (which differs from `−x` for `+0`). A
//! kernel written against this trait therefore produces, in every lane, exactly the bits the
//! [`Scalar`] instantiation produces for that lane, on every target (x86-64 with or without AVX,
//! aarch64, wasm32).

use core::fmt::Debug;
use core::ops::{Add, Div, Mul, Sub};
use ferroastro_real::Real;

/// A vector of [`Lane::WIDTH`] values of [`Lane::Elem`] with lane-wise correctly rounded
/// arithmetic.
pub trait Lane:
    Copy + Debug + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self>
{
    /// The element type.
    type Elem: Real;
    /// The number of lanes.
    const WIDTH: usize;
    /// A short label for reports.
    const LABEL: &'static str;
    /// Every lane equal to `x`.
    fn splat(x: Self::Elem) -> Self;
    /// The lanes from `src[..WIDTH]` (missing elements are zero).
    fn load(src: &[Self::Elem]) -> Self;
    /// Write the lanes to `dst[..WIDTH]` (as many as fit).
    fn store(self, dst: &mut [Self::Elem]);
    /// Lane-wise correctly rounded square root.
    #[must_use]
    fn sqrt(self) -> Self;
    /// Lane-wise `if a == b { t } else { f }` (IEEE equality: NaN is never equal).
    #[must_use]
    fn select_eq(a: Self, b: Self, t: Self, f: Self) -> Self;
}

/// One value of any [`Real`] backend as a width-1 lane: the scalar kernel.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Scalar<R: Real>(pub R);

impl<R: Real> Add for Scalar<R> {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl<R: Real> Sub for Scalar<R> {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl<R: Real> Mul for Scalar<R> {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl<R: Real> Div for Scalar<R> {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl<R: Real> Lane for Scalar<R> {
    type Elem = R;
    const WIDTH: usize = 1;
    const LABEL: &'static str = "scalar";
    #[inline]
    fn splat(x: R) -> Self {
        Self(x)
    }
    #[inline]
    fn load(src: &[R]) -> Self {
        Self(src.first().copied().unwrap_or(R::ZERO))
    }
    #[inline]
    fn store(self, dst: &mut [R]) {
        if let Some(d) = dst.first_mut() {
            *d = self.0;
        }
    }
    #[inline]
    fn sqrt(self) -> Self {
        Self(self.0.sqrt())
    }
    #[inline]
    fn select_eq(a: Self, b: Self, t: Self, f: Self) -> Self {
        if a.0 == b.0 { t } else { f }
    }
}

/// Four binary64 lanes (`wide::f64x4`: AVX with `target-feature=+avx`, two SSE2 halves on the
/// x86-64 baseline, NEON on aarch64, `simd128` or per-lane `f64` on wasm32).
#[cfg(feature = "simd")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct F64x4(wide::f64x4);

#[cfg(feature = "simd")]
mod f64x4_impl {
    use super::{F64x4, Lane};
    use core::ops::{Add, Div, Mul, Sub};
    use wide::{CmpEq, f64x4};

    impl Add for F64x4 {
        type Output = Self;
        #[inline]
        fn add(self, rhs: Self) -> Self {
            Self(self.0 + rhs.0)
        }
    }

    impl Sub for F64x4 {
        type Output = Self;
        #[inline]
        fn sub(self, rhs: Self) -> Self {
            Self(self.0 - rhs.0)
        }
    }

    impl Mul for F64x4 {
        type Output = Self;
        #[inline]
        fn mul(self, rhs: Self) -> Self {
            Self(self.0 * rhs.0)
        }
    }

    impl Div for F64x4 {
        type Output = Self;
        #[inline]
        fn div(self, rhs: Self) -> Self {
            Self(self.0 / rhs.0)
        }
    }

    impl Lane for F64x4 {
        type Elem = f64;
        const WIDTH: usize = 4;
        const LABEL: &'static str = "wide f64x4";
        #[inline]
        fn splat(x: f64) -> Self {
            Self(f64x4::splat(x))
        }
        #[inline]
        fn load(src: &[f64]) -> Self {
            let mut a = [0.0; 4];
            for (d, s) in a.iter_mut().zip(src) {
                *d = *s;
            }
            Self(f64x4::from(a))
        }
        #[inline]
        fn store(self, dst: &mut [f64]) {
            let a: [f64; 4] = self.0.into();
            for (d, s) in dst.iter_mut().zip(a) {
                *d = s;
            }
        }
        #[inline]
        fn sqrt(self) -> Self {
            Self(self.0.sqrt())
        }
        #[inline]
        fn select_eq(a: Self, b: Self, t: Self, f: Self) -> Self {
            Self(a.0.cmp_eq(b.0).blend(t.0, f.0))
        }
    }
}
