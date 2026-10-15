// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `Real` for IEEE binary128 (`f128`), nightly toolchains only (feature `f128`, DEP-007).
//!
//! Compiled only when the `f128` feature is on **and** build.rs found a nightly compiler. The
//! elementary functions are the nightly `std` `f128` methods; whether they exist for every target
//! (notably wasm32) is open (NUM-006 notes), so this backend is native-only for now.

use crate::Real;
use core::cmp::Ordering;

impl Real for f128 {
    const NAME: &'static str = "f128";
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const HALF: Self = 0.5;
    const PI: Self = core::f128::consts::PI;
    const TAU: Self = core::f128::consts::TAU;
    const FRAC_PI_2: Self = core::f128::consts::FRAC_PI_2;
    const LN_2: Self = core::f128::consts::LN_2;
    const EPSILON: Self = f128::EPSILON;

    fn from_f64(x: f64) -> Self {
        f128::from(x)
    }
    fn from_parts(hi: f64, lo: f64) -> Self {
        f128::from(hi) + f128::from(lo)
    }
    fn from_i32(n: i32) -> Self {
        f128::from(n)
    }
    #[expect(clippy::cast_possible_truncation, reason = "nearest f64 by definition")]
    fn to_f64(self) -> f64 {
        self as f64
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "leading word, then the remainder"
    )]
    fn to_parts(self) -> (f64, f64) {
        let hi = self as f64;
        (hi, (self - f128::from(hi)) as f64)
    }
    fn mul_add(self, a: Self, b: Self) -> Self {
        f128::mul_add(self, a, b)
    }
    fn scale(self, k: f64) -> Self {
        self * f128::from(k)
    }
    fn sqrt(self) -> Self {
        f128::sqrt(self)
    }
    fn recip(self) -> Self {
        1.0 / self
    }
    fn abs(self) -> Self {
        f128::abs(self)
    }
    fn max(self, other: Self) -> Self {
        f128::max(self, other)
    }
    fn min(self, other: Self) -> Self {
        f128::min(self, other)
    }
    fn is_finite(self) -> bool {
        f128::is_finite(self)
    }
    fn is_nan(self) -> bool {
        f128::is_nan(self)
    }
    fn total_cmp(&self, other: &Self) -> Ordering {
        f128::total_cmp(self, other)
    }
    fn floor(self) -> Self {
        f128::floor(self)
    }
    fn round(self) -> Self {
        f128::round(self)
    }
    fn rem_euclid(self, rhs: Self) -> Self {
        f128::rem_euclid(self, rhs)
    }
    fn powi(self, n: i32) -> Self {
        crate::dd::powi_by_squaring(self, n)
    }
    fn sin(self) -> Self {
        f128::sin(self)
    }
    fn cos(self) -> Self {
        f128::cos(self)
    }
    fn sin_cos(self) -> (Self, Self) {
        (f128::sin(self), f128::cos(self))
    }
    fn atan2(self, x: Self) -> Self {
        f128::atan2(self, x)
    }
    fn acos(self) -> Self {
        f128::acos(self)
    }
    fn asin(self) -> Self {
        f128::asin(self)
    }
    fn exp(self) -> Self {
        f128::exp(self)
    }
    fn exp_m1(self) -> Self {
        f128::exp_m1(self)
    }
    fn ln(self) -> Self {
        f128::ln(self)
    }
    fn ln_1p(self) -> Self {
        f128::ln_1p(self)
    }
    fn powf(self, y: Self) -> Self {
        f128::powf(self, y)
    }
}
