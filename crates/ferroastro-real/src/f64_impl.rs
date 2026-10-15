// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! `Real` for IEEE binary64: every method is the inherent `f64` operation (std) or its `libm`
//! equivalent (`no_std`), so monomorphising at `f64` adds no cost (NUM-006).

use crate::Real;
use crate::f64_math as m;
use core::cmp::Ordering;

impl Real for f64 {
    const NAME: &'static str = "f64";
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const HALF: Self = 0.5;
    const PI: Self = core::f64::consts::PI;
    const TAU: Self = core::f64::consts::TAU;
    const FRAC_PI_2: Self = core::f64::consts::FRAC_PI_2;
    const LN_2: Self = core::f64::consts::LN_2;
    const EPSILON: Self = f64::EPSILON;

    #[inline]
    fn from_f64(x: f64) -> Self {
        x
    }
    #[inline]
    fn from_parts(hi: f64, lo: f64) -> Self {
        hi + lo
    }
    #[inline]
    fn from_i32(n: i32) -> Self {
        f64::from(n)
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self
    }
    #[inline]
    fn to_parts(self) -> (f64, f64) {
        (self, 0.0)
    }
    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        m::fma(self, a, b)
    }
    #[inline]
    fn scale(self, k: f64) -> Self {
        self * k
    }
    #[inline]
    fn sqrt(self) -> Self {
        m::sqrt(self)
    }
    #[inline]
    fn recip(self) -> Self {
        1.0 / self
    }
    #[inline]
    fn abs(self) -> Self {
        f64::abs(self)
    }
    #[inline]
    fn max(self, other: Self) -> Self {
        f64::max(self, other)
    }
    #[inline]
    fn min(self, other: Self) -> Self {
        f64::min(self, other)
    }
    #[inline]
    fn is_finite(self) -> bool {
        f64::is_finite(self)
    }
    #[inline]
    fn is_nan(self) -> bool {
        f64::is_nan(self)
    }
    #[inline]
    fn total_cmp(&self, other: &Self) -> Ordering {
        f64::total_cmp(self, other)
    }
    #[inline]
    fn floor(self) -> Self {
        m::floor(self)
    }
    #[inline]
    fn round(self) -> Self {
        m::round(self)
    }
    #[inline]
    fn rem_euclid(self, rhs: Self) -> Self {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }
    #[inline]
    fn powi(self, n: i32) -> Self {
        crate::dd::powi_by_squaring(self, n)
    }
    #[inline]
    fn sin(self) -> Self {
        m::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        m::cos(self)
    }
    #[inline]
    fn sin_cos(self) -> (Self, Self) {
        (m::sin(self), m::cos(self))
    }
    #[inline]
    fn atan2(self, x: Self) -> Self {
        m::atan2(self, x)
    }
    #[inline]
    fn acos(self) -> Self {
        m::acos(self)
    }
    #[inline]
    fn asin(self) -> Self {
        m::asin(self)
    }
    #[inline]
    fn exp(self) -> Self {
        m::exp(self)
    }
    #[inline]
    fn exp_m1(self) -> Self {
        m::exp_m1(self)
    }
    #[inline]
    fn ln(self) -> Self {
        m::ln(self)
    }
    #[inline]
    fn ln_1p(self) -> Self {
        m::ln_1p(self)
    }
    #[inline]
    fn powf(self, y: Self) -> Self {
        m::powf(self, y)
    }
}
