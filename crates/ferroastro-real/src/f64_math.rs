// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! binary64 elementary functions from `std` (feature `std`, the default) or from the pure-Rust
//! `libm` crate (feature `libm`, for `no_std` builds). With `std` the calls are the inherent
//! `f64` methods, so the f64 backend compiles to exactly the code the non-generic M0 crates used
//! (NUM-006 "zero-cost f64").

#[cfg(not(any(feature = "std", feature = "libm")))]
compile_error!("ferroastro-real needs the `std` feature (default) or the `libm` feature");

macro_rules! unary {
    ($($(#[$m:meta])* $name:ident => $libm:ident;)*) => {$(
        $(#[$m])*
        #[inline]
        #[must_use]
        pub fn $name(x: f64) -> f64 {
            #[cfg(feature = "std")]
            {
                x.$name()
            }
            #[cfg(not(feature = "std"))]
            {
                libm::$libm(x)
            }
        }
    )*};
}

unary! {
    /// Square root (correctly rounded, IEEE 754 §5.4.1).
    sqrt => sqrt;
    /// Sine.
    sin => sin;
    /// Cosine.
    cos => cos;
    /// Arc cosine.
    acos => acos;
    /// Arc sine.
    asin => asin;
    /// Exponential.
    exp => exp;
    /// `exp(x) − 1`.
    exp_m1 => expm1;
    /// Natural logarithm.
    ln => log;
    /// `ln(1 + x)`.
    ln_1p => log1p;
    /// Largest integer ≤ x.
    floor => floor;
    /// Nearest integer, ties away from zero.
    round => round;
}

/// Fused multiply-add `a·b + c` with a single rounding (IEEE 754 §5.4.1).
#[inline]
#[must_use]
pub fn fma(a: f64, b: f64, c: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        a.mul_add(b, c)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::fma(a, b, c)
    }
}

/// Four-quadrant arc tangent of `y/x`.
#[inline]
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        y.atan2(x)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::atan2(y, x)
    }
}

/// `√(x² + y²)` without undue overflow.
#[inline]
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.hypot(y)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::hypot(x, y)
    }
}

/// `x^y`.
#[inline]
#[must_use]
pub fn powf(x: f64, y: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        x.powf(y)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::pow(x, y)
    }
}

/// `x · 2^n` (exact barring overflow and underflow).
#[inline]
#[must_use]
pub fn ldexp(x: f64, n: i32) -> f64 {
    #[cfg(feature = "std")]
    {
        // 2^n is exact for |n| <= 1023; split larger shifts in two.
        let half = n / 2;
        x * 2.0_f64.powi(half) * 2.0_f64.powi(n - half)
    }
    #[cfg(not(feature = "std"))]
    {
        libm::ldexp(x, n)
    }
}
