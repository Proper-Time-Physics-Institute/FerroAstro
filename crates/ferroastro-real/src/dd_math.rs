// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Double-double elementary functions (FerroAstro's own; no published bound applies).
//!
//! Algorithms (all arithmetic in double-double unless noted):
//!
//! * `exp_m1(r)` for `|r| ≤ ln2/2`: `r` is divided by `2⁶`, the Taylor series
//!   `Σ₁¹⁴ rⁿ/n!` is summed by Horner's rule, and `expm1(2x) = expm1(x)(expm1(x) + 2)` is applied
//!   six times (the doubling keeps the relative accuracy of `expm1` near 0).
//! * `exp(x)`: Cody–Waite reduction `x = k ln2 + r` with a three-word `ln2`, then
//!   `exp(x) = 2ᵏ (1 + expm1(r))`.
//! * `ln(x)`: `x = 2ᵏ m` with `m ∈ [√½, √2)`, `ln x = k ln2 + 2 atanh((m − 1)/(m + 1))`, the
//!   `atanh` series summed to 22 terms (`|z| ≤ 0.1716`, last term < 10⁻³⁴). `ln_1p(x)` uses the
//!   same series on `z = x/(2 + x)` for `|x| < ½`.
//! * `sin`, `cos`: reduction by `π/2` (three-word constant) to `|r| ≤ π/4`, Taylor series to
//!   degree 33/32.
//! * `atan2(y, x)`: one Newton step from the binary64 `atan2` on `sin`/`cos`
//!   (quadratic convergence: 53 → 106 bits). `asin`, `acos` via `atan2` with `√((1−x)(1+x))`.
//!
//! Error bounds are engineering bounds from operation counts, pre-registered (before the first
//! measurement) in `tests/vectors/real-backends.toml` and checked against `astro-float` by the
//! DEP-012 property tests.

use crate::dd::{DoubleDouble, FRAC_PI_2_3, LN_2_3};
use crate::{Real, f64_math as m};

type Dd = DoubleDouble;

/// `1/n!` for `n = 0…33` as normalised double-double pairs (computed with mpmath at 400 bits and
/// split with round-to-nearest; checked against astro-float in the unit tests).
const INV_FACT: [(f64, f64); 34] = [
    (1.0, 0.0),
    (1.0, 0.0),
    (0.5, 0.0),
    (0.166_666_666_666_666_66, 9.251_858_538_542_97e-18),
    (0.041_666_666_666_666_664, 2.312_964_634_635_742_7e-18),
    (0.008_333_333_333_333_333, 1.156_482_317_317_871_4e-19),
    (0.001_388_888_888_888_889, -5.300_543_954_373_577e-20),
    (0.000_198_412_698_412_698_4, 1.720_955_829_342_070_5e-22),
    (2.480_158_730_158_73e-5, 2.151_194_786_677_588_2e-23),
    (2.755_731_922_398_589_3e-6, -1.858_393_274_046_472e-22),
    (2.755_731_922_398_589e-7, 2.376_771_462_225_029_7e-23),
    (2.505_210_838_544_172e-8, -1.448_814_070_935_912e-24),
    (2.087_675_698_786_81e-9, -1.207_345_059_113_26e-25),
    (1.605_904_383_682_161_3e-10, 1.258_529_458_875_209_8e-26),
    (1.147_074_559_772_972_5e-11, 2.065_551_275_283_074_5e-28),
    (7.647_163_731_819_816e-13, 7.038_728_777_334_53e-30),
    (4.779_477_332_387_385e-14, 4.399_205_485_834_081e-31),
    (2.811_457_254_345_520_6e-15, 1.650_884_273_086_143_3e-31),
    (1.561_920_696_858_622_5e-16, 1.191_067_966_027_375_4e-32),
    (8.220_635_246_624_33e-18, 2.214_189_411_960_426_5e-34),
    (4.110_317_623_312_165e-19, 1.441_297_337_865_952_7e-36),
    (1.957_294_106_339_126_3e-20, -1.364_350_383_008_790_8e-36),
    (8.896_791_392_450_574e-22, -7.911_402_614_872_376e-38),
    (3.868_170_170_630_684e-23, -8.843_177_655_482_344e-40),
    (1.611_737_571_096_118_4e-24, -3.684_657_356_450_976_6e-41),
    (6.446_950_284_384_474e-26, -1.933_040_423_370_346_5e-42),
    (2.479_596_263_224_797_6e-27, -1.295_373_096_476_522_9e-43),
    (9.183_689_863_795_546e-29, 1.430_315_039_678_732_2e-45),
    (3.279_889_237_069_838e-30, 1.511_754_274_402_987_9e-46),
    (1.130_996_288_644_771_6e-31, 1.049_801_541_295_950_6e-47),
    (3.769_987_628_815_905_4e-33, 2.587_034_783_275_032_4e-49),
    (1.216_125_041_553_518e-34, 5.586_290_567_888_806e-51),
    (3.800_390_754_854_743_4e-36, 1.745_715_802_465_251_8e-52),
    (1.151_633_562_077_195e-37, -6.099_574_457_884_54e-54),
];

fn inv_fact(n: usize) -> Dd {
    INV_FACT
        .get(n)
        .map_or(Dd::ZERO, |&(h, l)| Dd::from_normalized(h, l))
}

/// Largest argument of `exp` before overflow, and smallest before underflow to zero.
const EXP_MAX: f64 = 709.78;
const EXP_MIN: f64 = -745.2;

/// Index `k` of the Cody–Waite reduction `x − k·c` (a small integer, exact in binary64).
fn reduction_index(x: Dd, c: f64) -> f64 {
    m::round(x.hi() / c)
}

/// `x − k·(c₀ + c₁ + c₂)` with each `k·cᵢ` formed exactly.
fn reduce(x: Dd, k: f64, c: [f64; 3]) -> Dd {
    let mut r = x;
    for ci in c {
        let (p, e) = crate::eft::two_prod(k, ci);
        r -= Dd::from_normalized(p, 0.0) + Dd::from_f64(e);
    }
    r
}

/// `expm1(r)` for `|r| ≤ ln2/2` (about 0.35).
fn expm1_small(r: Dd) -> Dd {
    const HALVINGS: i32 = 6;
    let x = r.ldexp(-HALVINGS);
    // Horner: x·(1 + x/2! + x²/3! + … + x¹³/14!).
    let mut acc = inv_fact(14);
    for n in (1..14).rev() {
        acc = acc * x + inv_fact(n);
    }
    let mut s = acc * x;
    for _ in 0..HALVINGS {
        s = s * (s + Dd::TWO);
    }
    s
}

/// `exp(x)`.
pub(crate) fn exp(x: Dd) -> Dd {
    if x.is_nan() {
        return x;
    }
    if x.hi() > EXP_MAX {
        return Dd::from_f64(f64::INFINITY);
    }
    if x.hi() < EXP_MIN {
        return Dd::ZERO;
    }
    let k = reduction_index(x, LN_2_3[0]);
    let r = reduce(x, k, LN_2_3);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "|k| <= 1076 after the range checks above"
    )]
    let ki = k as i32;
    (expm1_small(r) + Dd::ONE).ldexp(ki)
}

/// `exp(x) − 1`.
pub(crate) fn exp_m1(x: Dd) -> Dd {
    if x.abs().hi() <= 0.5 * LN_2_3[0] {
        expm1_small(x)
    } else {
        exp(x) - Dd::ONE
    }
}

/// `2 atanh(z) = 2(z + z³/3 + z⁵/5 + …)` for `|z| ≤ 0.1716` (22 terms).
fn two_atanh_small(z: Dd) -> Dd {
    const TERMS: i32 = 22;
    let w = z * z;
    let mut acc = Dd::ONE / Dd::from_i32(2 * TERMS + 1);
    for j in (0..TERMS).rev() {
        acc = acc * w + Dd::ONE / Dd::from_i32(2 * j + 1);
    }
    (acc * z).scale(2.0)
}

/// `ln(x)`.
pub(crate) fn ln(x: Dd) -> Dd {
    if x.is_nan() || x.hi() < 0.0 {
        return Dd::from_f64(f64::NAN);
    }
    if x.hi() == 0.0 {
        return Dd::from_f64(f64::NEG_INFINITY);
    }
    if x.hi() == f64::INFINITY {
        return x;
    }
    // x = 2^k · mant with mant in [sqrt(1/2), sqrt(2)).
    let mut k = 0_i32;
    let mut mant = x;
    while mant.hi() >= core::f64::consts::SQRT_2 {
        mant = mant.ldexp(-1);
        k += 1;
    }
    while mant.hi() < core::f64::consts::FRAC_1_SQRT_2 {
        mant = mant.ldexp(1);
        k -= 1;
    }
    let z = (mant - Dd::ONE) / (mant + Dd::ONE);
    let series = two_atanh_small(z);
    if k == 0 {
        series
    } else {
        let kf = f64::from(k);
        let mut kln2 = Dd::ZERO;
        for c in LN_2_3 {
            let (p, e) = crate::eft::two_prod(kf, c);
            kln2 += Dd::from_normalized(p, 0.0) + Dd::from_f64(e);
        }
        kln2 + series
    }
}

/// `ln(1 + x)`.
pub(crate) fn ln_1p(x: Dd) -> Dd {
    if x.abs().hi() < 0.5 {
        two_atanh_small(x / (Dd::TWO + x))
    } else {
        ln(Dd::ONE + x)
    }
}

/// `(sin r, cos r)` for `|r| ≤ π/4` by Taylor series (degrees 33 and 32).
fn sin_cos_small(r: Dd) -> (Dd, Dd) {
    // With t = −r²: sin r = r Σ tᵏ/(2k+1)!, cos r = Σ tᵏ/(2k)!, k = 0…16 (Horner in t).
    let t = -(r * r);
    let mut s = inv_fact(33);
    let mut c = inv_fact(32);
    for k in (0..16).rev() {
        s = s * t + inv_fact(2 * k + 1);
        c = c * t + inv_fact(2 * k);
    }
    (s * r, c)
}

/// `(sin x, cos x)`.
pub(crate) fn sin_cos(x: Dd) -> (Dd, Dd) {
    if !x.is_finite() {
        let nan = Dd::from_f64(f64::NAN);
        return (nan, nan);
    }
    let k = reduction_index(x, FRAC_PI_2_3[0]);
    let r = reduce(x, k, FRAC_PI_2_3);
    let (s, c) = sin_cos_small(r);
    // Quadrant of k (mod 4).
    let q = k - 4.0 * m::floor(k / 4.0);
    if q < 0.5 {
        (s, c)
    } else if q < 1.5 {
        (c, -s)
    } else if q < 2.5 {
        (-s, -c)
    } else {
        (-c, s)
    }
}

/// `atan2(y, x)`.
pub(crate) fn atan2(y: Dd, x: Dd) -> Dd {
    if x.is_nan() || y.is_nan() {
        return Dd::from_f64(f64::NAN);
    }
    if y.hi() == 0.0 && x.hi() == 0.0 {
        return Dd::from_f64(m::atan2(y.hi(), x.hi()));
    }
    let z0 = Dd::from_f64(m::atan2(y.hi(), x.hi()));
    let r = (x * x + y * y).sqrt();
    let (xx, yy) = (x / r, y / r);
    let (s, c) = sin_cos(z0);
    if xx.abs().hi() > yy.abs().hi() {
        z0 + (yy - s) / c
    } else {
        z0 - (xx - c) / s
    }
}

/// `√((1 − x)(1 + x))`, accurate near `|x| = 1`.
fn cofunction(x: Dd) -> Dd {
    ((Dd::ONE - x) * (Dd::ONE + x)).sqrt()
}

/// `asin(x)` for `|x| ≤ 1`.
pub(crate) fn asin(x: Dd) -> Dd {
    if x.abs() > Dd::ONE {
        return Dd::from_f64(f64::NAN);
    }
    atan2(x, cofunction(x))
}

/// `acos(x)` for `|x| ≤ 1`.
pub(crate) fn acos(x: Dd) -> Dd {
    if x.abs() > Dd::ONE {
        return Dd::from_f64(f64::NAN);
    }
    atan2(cofunction(x), x)
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "error-free transformations and special values have exact expected results"
)]
mod tests {
    use super::*;

    /// Special values and exact identities.
    ///
    /// Verifies: DEP-012
    #[test]
    fn special_values() {
        assert_eq!(exp(Dd::ZERO), Dd::ONE);
        assert_eq!(ln(Dd::ONE), Dd::ZERO);
        assert!(ln(Dd::from_f64(-1.0)).is_nan());
        assert_eq!(ln(Dd::ZERO).hi(), f64::NEG_INFINITY);
        assert_eq!(exp(Dd::from_f64(1.0e3)).hi(), f64::INFINITY); // lint: allow-literal(beyond the overflow threshold)
        assert_eq!(sin_cos(Dd::ZERO), (Dd::ZERO, Dd::ONE));
        assert!(asin(Dd::TWO).is_nan());
        let pi = atan2(Dd::ZERO, Dd::from_f64(-1.0));
        assert!((pi - Dd::PI).abs() <= Dd::EPSILON);
        let two = exp(Dd::LN_2);
        assert!((two - Dd::TWO).abs() <= Dd::EPSILON.scale(4.0));
    }
}
