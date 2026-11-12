// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The crate-owned scalar trait [`Real`] and its backends (ADR-0002, NUM-006, DEP-007, DEP-012).
//!
//! * `f64` (the default type parameter everywhere in FerroAstro): IEEE binary64, every operation
//!   is the inherent `f64` method, so generic code monomorphised at `f64` is the code the
//!   non-generic M0 crates ran (NUM-006 "zero-cost f64").
//! * [`DoubleDouble`]: an unevaluated sum `hi + lo` of two binary64 numbers (≈ 106-bit
//!   significand), built on FMA error-free transformations. Addition, multiplication, division and
//!   square root follow the algorithms whose relative error bounds are proved in Joldes, Muller &
//!   Popescu 2017 (ACM TOMS 44(2), 15res) and Lefèvre et al. 2023 (ACM TOMS 49(1)); the bounds are
//!   the oracles of the DEP-012 property tests (`tests/vectors/real-backends.toml`).
//! * `f128` (feature `f128`, nightly toolchains only): IEEE binary128. On a stable compiler the
//!   feature compiles to nothing ([`F128_AVAILABLE`] is `false`).
//!
//! Precision is a type parameter resolved by monomorphisation, never a runtime switch, and nothing
//! uses x87 80-bit arithmetic (NUM-006).
//!
//! [`Neumaier`] is a compensated accumulator generic over [`Real`].
//!
//! The crate is `no_std`: with the default `std` feature it uses the platform's `f64` math, and
//! without it the `libm` feature supplies pure-Rust elementary functions (wasm32 builds either way).
//!
//! ```
//! use ferroastro_real::{DoubleDouble, Real};
//!
//! /// One generic function, two monomorphised backends.
//! fn third<R: Real>() -> R {
//!     R::ONE / R::from_f64(3.0)
//! }
//! let f: f64 = third();
//! let dd: DoubleDouble = third();
//! // The double-double keeps the binary64 rounding error of 1/3 in its low word.
//! assert_eq!(dd.hi(), f);
//! assert!((dd * DoubleDouble::from_f64(3.0) - DoubleDouble::ONE).abs().to_f64() < 1e-31);
//! ```

#![no_std]
#![cfg_attr(all(feature = "f128", fa_nightly), feature(f128))]

#[cfg(feature = "std")]
extern crate std;

mod compensated;
mod dd;
mod dd_math;
pub mod eft;
#[cfg(all(feature = "f128", fa_nightly))]
mod f128_impl;
mod f64_impl;
pub mod f64_math;

pub use compensated::Neumaier;
pub use dd::DoubleDouble;

use core::cmp::Ordering;
use core::fmt::Debug;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// True when this build contains the nightly-only `f128` backend (feature `f128` on a nightly
/// compiler).
pub const F128_AVAILABLE: bool = cfg!(all(feature = "f128", fa_nightly));

/// A real scalar backend.
///
/// Arithmetic is the usual operator set. Elementary functions follow the `f64` method names. For
/// [`DoubleDouble`] each documented operation has a relative error bound in units of
/// `u² = 2⁻¹⁰⁶` (see the type's documentation); for `f64` the basic operations are correctly
/// rounded and the elementary functions are those of the platform.
pub trait Real:
    Copy
    + Debug
    + Default
    + PartialEq
    + PartialOrd
    + Send
    + Sync
    + 'static
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
{
    /// Short backend name for reports (`"f64"`, `"double-double"`, `"f128"`).
    const NAME: &'static str;
    /// 0.
    const ZERO: Self;
    /// 1.
    const ONE: Self;
    /// 2.
    const TWO: Self;
    /// ½.
    const HALF: Self;
    /// π, correctly rounded to the backend's precision.
    const PI: Self;
    /// 2π.
    const TAU: Self;
    /// π/2.
    const FRAC_PI_2: Self;
    /// ln 2.
    const LN_2: Self;
    /// The backend's machine epsilon (`2⁻⁵²` for f64, `2⁻¹⁰⁴` for double-double).
    const EPSILON: Self;

    /// The nearest value of this backend to `x` (exact for every backend at least as precise as f64).
    fn from_f64(x: f64) -> Self;
    /// The value `hi + lo` (exact for double-double and wider, rounded once for f64).
    fn from_parts(hi: f64, lo: f64) -> Self;
    /// Exact conversion of an integer.
    fn from_i32(n: i32) -> Self;
    /// The nearest `f64`.
    fn to_f64(self) -> f64;
    /// A leading/trailing binary64 pair whose exact sum approximates `self` (`(x, 0)` for f64).
    fn to_parts(self) -> (f64, f64);

    /// `self · a + b` (one rounding for f64; for double-double the product and sum each carry
    /// their own bound).
    #[must_use]
    fn mul_add(self, a: Self, b: Self) -> Self;
    /// Multiply by a binary64 number (cheaper than a full product for double-double).
    #[must_use]
    fn scale(self, k: f64) -> Self;
    /// Square root.
    #[must_use]
    fn sqrt(self) -> Self;
    /// `√(self² + other²)` without undue overflow or underflow.
    #[must_use]
    fn hypot(self, other: Self) -> Self;
    /// `1/self`.
    #[must_use]
    fn recip(self) -> Self;
    /// Absolute value.
    #[must_use]
    fn abs(self) -> Self;
    /// The larger of two values (NaN-ignoring, like `f64::max`).
    #[must_use]
    fn max(self, other: Self) -> Self;
    /// The smaller of two values (NaN-ignoring, like `f64::min`).
    #[must_use]
    fn min(self, other: Self) -> Self;
    /// True unless NaN or infinite.
    fn is_finite(self) -> bool;
    /// True for NaN.
    fn is_nan(self) -> bool;
    /// IEEE 754 `totalOrder` (on the leading word, then the trailing word).
    fn total_cmp(&self, other: &Self) -> Ordering;
    /// Largest integer ≤ `self`.
    #[must_use]
    fn floor(self) -> Self;
    /// Nearest integer, ties away from zero.
    #[must_use]
    fn round(self) -> Self;
    /// Least non-negative remainder of `self` modulo `rhs` (`rhs > 0`).
    #[must_use]
    fn rem_euclid(self, rhs: Self) -> Self;
    /// Integer power by repeated squaring.
    #[must_use]
    fn powi(self, n: i32) -> Self;

    /// Sine.
    #[must_use]
    fn sin(self) -> Self;
    /// Cosine.
    #[must_use]
    fn cos(self) -> Self;
    /// `(sin, cos)`.
    fn sin_cos(self) -> (Self, Self);
    /// Four-quadrant arc tangent of `self / x`.
    #[must_use]
    fn atan2(self, x: Self) -> Self;
    /// Arc cosine.
    #[must_use]
    fn acos(self) -> Self;
    /// Arc sine.
    #[must_use]
    fn asin(self) -> Self;
    /// Exponential.
    #[must_use]
    fn exp(self) -> Self;
    /// `exp(self) − 1`, accurate for small arguments.
    #[must_use]
    fn exp_m1(self) -> Self;
    /// Natural logarithm.
    #[must_use]
    fn ln(self) -> Self;
    /// `ln(1 + self)`, accurate for small arguments.
    #[must_use]
    fn ln_1p(self) -> Self;
    /// `self^y` for `self > 0`.
    #[must_use]
    fn powf(self, y: Self) -> Self;
}
