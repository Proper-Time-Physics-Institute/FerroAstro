// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Double-double ("double-word") arithmetic.
//!
//! A [`DoubleDouble`] is the unevaluated sum `hi + lo` of two binary64 numbers with
//! `hi = RN(hi + lo)`. Algorithms and their proved relative error bounds, with `u = 2⁻⁵³`:
//!
//! | operation | algorithm | bound |
//! |---|---|---|
//! | `+`, `−` | `AccurateDWPlusDW`, JMP 2017 Alg. 6 | `3u² + 13u³` (Thm 3.1) |
//! | `×` | `DWTimesDW3` (FMA), JMP 2017 Alg. 12 | `5u²` (Thm 5.4) |
//! | `×` binary64 ([`Real::scale`]) | `DWTimesFP3` (FMA), JMP 2017 Alg. 9 | `2u²` (Thm 4.3) |
//! | `÷`, [`Real::recip`] | `DWDivDW3` (FMA), JMP 2017 Alg. 18 | `9.8u²` (Thm 7.2) |
//! | [`Real::sqrt`] | `SQRTDWtoDW`, Lefèvre et al. 2023 Alg. 8 | `3.125u²` (Thm 3.6) |
//!
//! JMP 2017 = Joldes, Muller & Popescu, "Tight and rigorous error bounds for basic building blocks
//! of double-word arithmetic", ACM TOMS 44(2), 15res (HAL hal-01351529v3). Lefèvre, Louvet,
//! Muller, Picot & Rideau 2023, "Accurate calculation of Euclidean norms using double-word
//! arithmetic", ACM TOMS 49(1) (HAL hal-03482567v2). The bounds hold barring overflow and
//! underflow. Elementary functions (`exp`, `ln`, `sin`, …) are FerroAstro's own and carry the
//! engineering bounds documented in `dd_math` and pre-registered in `tests/vectors`.
//!
//! Non-finite values: an overflow or NaN anywhere makes the leading word non-finite, which is
//! what [`Real::is_finite`] tests; the trailing word is then meaningless.

use crate::eft::{fast_two_sum, two_prod, two_sum};
use crate::{Real, dd_math, f64_math as m};
use core::cmp::Ordering;
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A double-double number `hi + lo` (≈ 106-bit significand, the binary64 exponent range).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DoubleDouble {
    hi: f64,
    lo: f64,
}

impl DoubleDouble {
    /// Build from a pair that is already normalised (`hi = RN(hi + lo)`); used for constants.
    pub(crate) const fn from_normalized(hi: f64, lo: f64) -> Self {
        Self { hi, lo }
    }

    /// The leading word (the nearest binary64 to the value).
    #[must_use]
    pub const fn hi(self) -> f64 {
        self.hi
    }

    /// The trailing word.
    #[must_use]
    pub const fn lo(self) -> f64 {
        self.lo
    }

    /// Multiply by `2^n` (exact barring overflow and underflow).
    #[must_use]
    pub fn ldexp(self, n: i32) -> Self {
        Self {
            hi: m::ldexp(self.hi, n),
            lo: m::ldexp(self.lo, n),
        }
    }

    /// `DWPlusFP` (JMP 2017 Alg. 4, bound `2u²`).
    #[must_use]
    pub(crate) fn add_f64(self, y: f64) -> Self {
        let (sh, sl) = two_sum(self.hi, y);
        let v = self.lo + sl;
        let (hi, lo) = fast_two_sum(sh, v);
        Self { hi, lo }
    }

    /// `DWTimesFP3` (JMP 2017 Alg. 9, bound `2u²`).
    #[must_use]
    pub(crate) fn mul_f64(self, y: f64) -> Self {
        let (ch, cl1) = two_prod(self.hi, y);
        let cl3 = m::fma(self.lo, y, cl1);
        let (hi, lo) = fast_two_sum(ch, cl3);
        Self { hi, lo }
    }
}

impl Add for DoubleDouble {
    type Output = Self;
    /// `AccurateDWPlusDW` (JMP 2017 Alg. 6).
    #[inline]
    fn add(self, y: Self) -> Self {
        let (sh, sl) = two_sum(self.hi, y.hi);
        let (th, tl) = two_sum(self.lo, y.lo);
        let c = sl + th;
        let (vh, vl) = fast_two_sum(sh, c);
        let w = tl + vl;
        let (hi, lo) = fast_two_sum(vh, w);
        Self { hi, lo }
    }
}

impl Neg for DoubleDouble {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}

impl Sub for DoubleDouble {
    type Output = Self;
    #[inline]
    fn sub(self, y: Self) -> Self {
        self + (-y)
    }
}

impl Mul for DoubleDouble {
    type Output = Self;
    /// `DWTimesDW3` (JMP 2017 Alg. 12).
    #[inline]
    fn mul(self, y: Self) -> Self {
        let (ch, cl1) = two_prod(self.hi, y.hi);
        let tl0 = self.lo * y.lo;
        let tl1 = m::fma(self.hi, y.lo, tl0);
        let cl2 = m::fma(self.lo, y.hi, tl1);
        let cl3 = cl1 + cl2;
        let (hi, lo) = fast_two_sum(ch, cl3);
        Self { hi, lo }
    }
}

impl Div for DoubleDouble {
    type Output = Self;
    /// `DWDivDW3` (JMP 2017 Alg. 18): the reciprocal of lines 1–6 times the dividend (line 7).
    #[inline]
    #[expect(
        clippy::suspicious_arithmetic_impl,
        reason = "Alg. 18 is a product with a Newton-refined reciprocal"
    )]
    fn div(self, y: Self) -> Self {
        self * y.recip_dd()
    }
}

impl DoubleDouble {
    /// Lines 1–6 of `DWDivDW3` (JMP 2017 Alg. 18): a double-word approximation to `1/y`; the
    /// division multiplies it by the dividend (line 7).
    #[inline]
    fn recip_dd(self) -> Self {
        let th = 1.0 / self.hi;
        let rh = m::fma(-self.hi, th, 1.0); // exact (line 2)
        let rl = -(self.lo * th);
        let (eh, el) = fast_two_sum(rh, rl);
        let d = Self { hi: eh, lo: el }.mul_f64(th);
        d.add_f64(th)
    }
}

macro_rules! assign_ops {
    ($($tr:ident $f:ident $op:tt;)*) => {$(
        impl $tr for DoubleDouble {
            #[inline]
            fn $f(&mut self, rhs: Self) {
                *self = *self $op rhs;
            }
        }
    )*};
}
assign_ops! {
    AddAssign add_assign +;
    SubAssign sub_assign -;
    MulAssign mul_assign *;
    DivAssign div_assign /;
}

impl PartialOrd for DoubleDouble {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.hi.partial_cmp(&other.hi)? {
            Ordering::Equal => self.lo.partial_cmp(&other.lo),
            o => Some(o),
        }
    }
}

/// The unbiased binary exponent of a finite non-zero normal `x` (`x = ±1.m · 2^e`); subnormals
/// return −1022, which only makes the hypot scaling less tight.
fn binary_exponent(x: f64) -> i32 {
    let biased = i32::try_from((x.to_bits() >> 52) & 0x7ff).unwrap_or(1023);
    biased.max(1) - 1023
}

/// `x^n` by binary powering (shared by the f64 and double-double backends).
pub(crate) fn powi_by_squaring<R: Real>(x: R, n: i32) -> R {
    let mut base = x;
    let mut k = n.unsigned_abs();
    let mut acc = R::ONE;
    while k > 0 {
        if k & 1 == 1 {
            acc *= base;
        }
        k >>= 1;
        if k > 0 {
            base *= base;
        }
    }
    if n < 0 { acc.recip() } else { acc }
}

/// `π/2` to three binary64 words (for argument reduction).
pub(crate) const FRAC_PI_2_3: [f64; 3] = [
    core::f64::consts::FRAC_PI_2,
    6.123_233_995_736_766e-17,
    -1.497_384_904_859_169_8e-33,
];
/// `ln 2` to three binary64 words (for argument reduction).
pub(crate) const LN_2_3: [f64; 3] = [
    core::f64::consts::LN_2,
    2.319_046_813_846_299_6e-17,
    5.707_708_438_416_212e-34,
];

impl Real for DoubleDouble {
    const NAME: &'static str = "double-double";
    const ZERO: Self = Self::from_normalized(0.0, 0.0);
    const ONE: Self = Self::from_normalized(1.0, 0.0);
    const TWO: Self = Self::from_normalized(2.0, 0.0);
    const HALF: Self = Self::from_normalized(0.5, 0.0);
    const PI: Self = Self::from_normalized(core::f64::consts::PI, 1.224_646_799_147_353_2e-16);
    const TAU: Self = Self::from_normalized(core::f64::consts::TAU, 2.449_293_598_294_706_4e-16);
    const FRAC_PI_2: Self = Self::from_normalized(FRAC_PI_2_3[0], FRAC_PI_2_3[1]);
    const LN_2: Self = Self::from_normalized(LN_2_3[0], LN_2_3[1]);
    // 2^-104
    const EPSILON: Self = Self::from_normalized(f64::EPSILON * f64::EPSILON, 0.0);

    #[inline]
    fn from_f64(x: f64) -> Self {
        Self { hi: x, lo: 0.0 }
    }
    #[inline]
    fn from_parts(hi: f64, lo: f64) -> Self {
        let (hi, lo) = two_sum(hi, lo);
        Self { hi, lo }
    }
    #[inline]
    fn from_i32(n: i32) -> Self {
        Self::from_f64(f64::from(n))
    }
    #[inline]
    fn to_f64(self) -> f64 {
        self.hi + self.lo
    }
    #[inline]
    fn to_parts(self) -> (f64, f64) {
        (self.hi, self.lo)
    }
    #[inline]
    fn mul_add(self, a: Self, b: Self) -> Self {
        self * a + b
    }
    #[inline]
    fn scale(self, k: f64) -> Self {
        self.mul_f64(k)
    }
    /// `SQRTDWtoDW` (Lefèvre et al. 2023 Alg. 8).
    #[inline]
    fn sqrt(self) -> Self {
        if self.hi == 0.0 {
            return Self::ZERO;
        }
        let sh = m::sqrt(self.hi);
        let rho1 = m::fma(-sh, sh, self.hi);
        let rho2 = self.lo + rho1;
        let sl = rho2 / (2.0 * sh);
        let (hi, lo) = fast_two_sum(sh, sl);
        Self { hi, lo }
    }
    /// `√(x'² + y'²)·2ᵏ` with `x' = x·2⁻ᵏ`, `y' = y·2⁻ᵏ` scaled exactly so the larger is near 1.
    fn hypot(self, other: Self) -> Self {
        let (a, b) = (self.abs(), other.abs());
        let big = if a > b { a } else { b };
        if big.hi == 0.0 || !big.is_finite() || b.is_nan() || a.is_nan() {
            return if a.is_nan() || b.is_nan() { a + b } else { big };
        }
        let k = binary_exponent(big.hi);
        let (x, y) = (a.ldexp(-k), b.ldexp(-k));
        (x * x + y * y).sqrt().ldexp(k)
    }
    #[inline]
    fn recip(self) -> Self {
        self.recip_dd()
    }
    #[inline]
    fn abs(self) -> Self {
        if self.hi < 0.0 || (self.hi == 0.0 && self.lo < 0.0) {
            -self
        } else {
            self
        }
    }
    #[inline]
    fn max(self, other: Self) -> Self {
        if self.is_nan() || other > self {
            other
        } else {
            self
        }
    }
    #[inline]
    fn min(self, other: Self) -> Self {
        if self.is_nan() || other < self {
            other
        } else {
            self
        }
    }
    #[inline]
    fn is_finite(self) -> bool {
        self.hi.is_finite() && self.lo.is_finite()
    }
    #[inline]
    fn is_nan(self) -> bool {
        self.hi.is_nan() || self.lo.is_nan()
    }
    fn total_cmp(&self, other: &Self) -> Ordering {
        self.hi
            .total_cmp(&other.hi)
            .then_with(|| self.lo.total_cmp(&other.lo))
    }
    #[expect(
        clippy::float_cmp,
        reason = "exact integrality and half-integer tests on binary64 words"
    )]
    fn floor(self) -> Self {
        let fh = m::floor(self.hi);
        if fh == self.hi {
            let (hi, lo) = fast_two_sum(fh, m::floor(self.lo));
            Self { hi, lo }
        } else {
            Self::from_f64(fh)
        }
    }
    #[expect(
        clippy::float_cmp,
        reason = "exact integrality and half-integer tests on binary64 words"
    )]
    fn round(self) -> Self {
        let rh = m::round(self.hi);
        if rh == self.hi {
            // hi is an integer: the trailing word decides (|lo| <= ulp(hi)/2).
            let half_up = self.lo.abs() == 0.5 && (self.lo > 0.0) == (self.hi >= 0.0);
            let rl = if self.lo.abs() == 0.5 && !half_up {
                0.0
            } else {
                m::round(self.lo)
            };
            let (hi, lo) = fast_two_sum(rh, rl);
            Self { hi, lo }
        } else if (rh - self.hi).abs() == 0.5 {
            // hi is a half-integer: lo breaks the apparent tie.
            let d = rh - self.hi;
            let r = if d > 0.0 && self.lo < 0.0 {
                rh - 1.0
            } else if d < 0.0 && self.lo > 0.0 {
                rh + 1.0
            } else {
                rh
            };
            Self::from_f64(r)
        } else {
            Self::from_f64(rh)
        }
    }
    fn rem_euclid(self, rhs: Self) -> Self {
        let mut r = self - rhs * (self / rhs).floor();
        if r < Self::ZERO {
            r += rhs.abs();
        }
        if r >= rhs.abs() {
            r -= rhs.abs();
        }
        r
    }
    fn powi(self, n: i32) -> Self {
        powi_by_squaring(self, n)
    }
    fn sin(self) -> Self {
        dd_math::sin_cos(self).0
    }
    fn cos(self) -> Self {
        dd_math::sin_cos(self).1
    }
    fn sin_cos(self) -> (Self, Self) {
        dd_math::sin_cos(self)
    }
    fn atan2(self, x: Self) -> Self {
        dd_math::atan2(self, x)
    }
    fn acos(self) -> Self {
        dd_math::acos(self)
    }
    fn asin(self) -> Self {
        dd_math::asin(self)
    }
    fn exp(self) -> Self {
        dd_math::exp(self)
    }
    fn exp_m1(self) -> Self {
        dd_math::exp_m1(self)
    }
    fn ln(self) -> Self {
        dd_math::ln(self)
    }
    fn ln_1p(self) -> Self {
        dd_math::ln_1p(self)
    }
    fn powf(self, y: Self) -> Self {
        (y * self.ln()).exp()
    }
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "error-free transformations and special values have exact expected results"
)]
mod tests {
    use super::*;

    /// Constants are normalised and the arithmetic round-trips simple identities exactly.
    ///
    /// Verifies: NUM-006
    #[test]
    fn constants_are_normalised_and_identities_hold() {
        for c in [
            DoubleDouble::PI,
            DoubleDouble::TAU,
            DoubleDouble::FRAC_PI_2,
            DoubleDouble::LN_2,
        ] {
            assert_eq!(c.hi + c.lo, c.hi, "{c:?} not normalised");
        }
        let three = DoubleDouble::from_f64(3.0);
        let third = DoubleDouble::ONE / three;
        assert!(third.lo != 0.0);
        let back = third * three - DoubleDouble::ONE;
        assert!(back.abs() <= DoubleDouble::EPSILON);
        assert_eq!(DoubleDouble::TAU, DoubleDouble::PI * DoubleDouble::TWO);
        assert_eq!(
            DoubleDouble::from_parts(1.0, f64::EPSILON / 4.0).to_parts(),
            (1.0, f64::EPSILON / 4.0)
        );
        assert_eq!(DoubleDouble::from_f64(2.0).powi(-2).to_f64(), 0.25);
        assert_eq!(DoubleDouble::from_f64(-2.5).round().to_f64(), -3.0); // lint: allow-literal(a half-integer tie, not a physical value)
        assert_eq!(DoubleDouble::from_parts(2.5, -1e-20).round().to_f64(), 2.0); // lint: allow-literal(sub-ulp perturbation of a tie)
        assert_eq!(DoubleDouble::from_f64(-0.5).floor().to_f64(), -1.0);
        let r = DoubleDouble::from_f64(-1.0).rem_euclid(DoubleDouble::TAU);
        assert!((r - (DoubleDouble::TAU - DoubleDouble::ONE)).abs() <= DoubleDouble::EPSILON);
        assert!(!DoubleDouble::from_f64(f64::NAN).is_finite());
        assert!(DoubleDouble::from_f64(-1.0).sqrt().is_nan());
    }
}
