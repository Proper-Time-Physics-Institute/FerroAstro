// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Exact rational Adams coefficients (NUM-002).
//!
//! The Adams formulas integrate the polynomial that interpolates the derivative at equally spaced
//! nodes (Hairer, Nørsett & Wanner 1993, *Solving ODEs I*, §III.1). With the step as the unit
//! and the nodes at integer offsets `d_j` from the start of the step, the weight of node `j` over
//! `[0, θ]` is `W_j(θ) = ∫₀^θ L_j(s) ds`, where `L_j` is the Lagrange basis polynomial. The basis
//! polynomials have integer coefficients over an integer denominator, so every weight is an exact
//! rational number. They are computed here in `i128` arithmetic with every operation checked,
//! instead of being transcribed from tables:
//!
//! * Adams–Bashforth (p steps, order p): nodes `0, −1, …, −(p−1)`, `θ = 1`;
//! * Adams–Moulton (p nodes, order p): nodes `1, 0, …, −(p−2)`, `θ = 1`;
//! * dense output: the same nodes as the corrector, as polynomials in `θ`.
//!
//! A rational is converted to a backend value once, by a double-double division rounded to the
//! backend ([`Rational::to_real`]), so f64 coefficients are correctly rounded (to within the
//! double-double error) and double-double coefficients are accurate to ~2⁻¹⁰⁴.

use ferroastro_real::{DoubleDouble, Real};

/// An exact rational `num / den` in lowest terms with `den > 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rational {
    num: i128,
    den: i128,
}

const fn gcd(mut a: i128, mut b: i128) -> i128 {
    if a < 0 {
        a = -a;
    }
    if b < 0 {
        b = -b;
    }
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

impl Rational {
    /// Zero.
    pub const ZERO: Self = Self { num: 0, den: 1 };

    /// `num / den` in lowest terms; `None` if `den = 0` or a value overflows.
    #[must_use]
    pub fn new(num: i128, den: i128) -> Option<Self> {
        if den == 0 {
            return None;
        }
        let g = gcd(num, den).max(1);
        let (mut num, mut den) = (num / g, den / g);
        if den < 0 {
            num = num.checked_neg()?;
            den = den.checked_neg()?;
        }
        Some(Self { num, den })
    }

    /// An integer.
    #[must_use]
    pub const fn integer(n: i128) -> Self {
        Self { num: n, den: 1 }
    }

    /// Numerator (lowest terms; carries the sign).
    #[must_use]
    pub const fn numer(self) -> i128 {
        self.num
    }

    /// Denominator (lowest terms; positive).
    #[must_use]
    pub const fn denom(self) -> i128 {
        self.den
    }

    /// `self + other`, `None` on overflow.
    #[must_use]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        let g = gcd(self.den, other.den).max(1);
        let l = (self.den / g).checked_mul(other.den)?;
        let a = self.num.checked_mul(l / self.den)?;
        let b = other.num.checked_mul(l / other.den)?;
        Self::new(a.checked_add(b)?, l)
    }

    /// `self · other`, `None` on overflow.
    #[must_use]
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        let g1 = gcd(self.num, other.den).max(1);
        let g2 = gcd(other.num, self.den).max(1);
        let num = (self.num / g1).checked_mul(other.num / g2)?;
        let den = (self.den / g2).checked_mul(other.den / g1)?;
        Self::new(num, den)
    }

    /// `self / other`, `None` for a zero divisor or on overflow.
    #[must_use]
    pub fn checked_div(self, other: Self) -> Option<Self> {
        self.checked_mul(Self::new(other.den, other.num)?)
    }

    /// The backend value nearest to `num / den`, through one double-double division.
    /// `None` if `|num|` or `den` is 2¹⁰⁶ or more (not exactly representable in double-double).
    #[must_use]
    pub fn to_real<R: Real>(self) -> Option<R> {
        let q = exact_dd(self.num)? / exact_dd(self.den)?;
        Some(R::from_parts(q.hi(), q.lo()))
    }
}

/// The integer `n` exactly as a double-double (`|n| < 2¹⁰⁶`).
fn exact_dd(n: i128) -> Option<DoubleDouble> {
    const LIMIT: i128 = 1 << 106;
    const SPLIT: u32 = 53;
    if n <= -LIMIT || n >= LIMIT {
        return None;
    }
    let hi = n >> SPLIT; // |hi| < 2^53, exact in binary64
    let lo = n - (hi << SPLIT); // 0 <= lo < 2^53, exact in binary64
    #[expect(
        clippy::cast_precision_loss,
        reason = "both halves are below 2^53 in magnitude (checked above), so the casts are exact"
    )]
    let (hf, lf) = (hi as f64, lo as f64);
    // 2^53 as a scale factor: exact.
    let scale = f64::from(1_u32 << 26) * f64::from(1_u32 << 27);
    Some(DoubleDouble::from_parts(hf * scale, lf))
}

/// Integer coefficients (constant term first) of `Π_{i≠j} (s − d_i)` and the integer
/// `Π_{i≠j} (d_j − d_i)`; `None` on overflow or repeated nodes.
fn basis(offsets: &[i64], j: usize) -> Option<(Vec<i128>, i128)> {
    let dj = i128::from(*offsets.get(j)?);
    let mut poly: Vec<i128> = vec![1];
    let mut den: i128 = 1;
    for (i, &di) in offsets.iter().enumerate() {
        if i == j {
            continue;
        }
        let di = i128::from(di);
        // poly · (s − d_i)
        let mut next = vec![0_i128; poly.len() + 1];
        for (m, &c) in poly.iter().enumerate() {
            let up = next.get_mut(m + 1)?;
            *up = up.checked_add(c)?;
            let here = next.get_mut(m)?;
            *here = here.checked_sub(c.checked_mul(di)?)?;
        }
        poly = next;
        let diff = dj.checked_sub(di)?;
        if diff == 0 {
            return None;
        }
        den = den.checked_mul(diff)?;
    }
    Some((poly, den))
}

/// The weight polynomials `W_j(θ) = ∫₀^θ L_j(s) ds` over integer `offsets`, each as the
/// coefficients `w[m]` of `θ^{m+1}`, `m = 0 … len−1` (so `W_j(θ) = Σ_m w[m] θ^{m+1}`).
/// `None` on overflow or repeated nodes.
#[must_use]
pub fn weight_polynomials(offsets: &[i64]) -> Option<Vec<Vec<Rational>>> {
    (0..offsets.len())
        .map(|j| {
            let (poly, den) = basis(offsets, j)?;
            poly.iter()
                .enumerate()
                .map(|(m, &c)| {
                    let m1 = i128::try_from(m + 1).ok()?;
                    Rational::new(c, den.checked_mul(m1)?)
                })
                .collect()
        })
        .collect()
}

/// `W_j(θ)` for every node at a rational `θ`.
///
/// `None` on overflow or repeated nodes.
#[must_use]
pub fn weights_at(offsets: &[i64], theta: Rational) -> Option<Vec<Rational>> {
    weight_polynomials(offsets)?
        .iter()
        .map(|w| {
            // Horner in θ, then one more factor θ.
            let mut acc = Rational::ZERO;
            for &c in w.iter().rev() {
                acc = acc.checked_mul(theta)?.checked_add(c)?;
            }
            acc.checked_mul(theta)
        })
        .collect()
}

fn offsets_from(first: i64, count: usize) -> Option<Vec<i64>> {
    (0..count)
        .map(|j| first.checked_sub(i64::try_from(j).ok()?))
        .collect()
}

/// Node offsets of the p-step Adams–Bashforth formula: `0, −1, …, −(p−1)` (node `j` is `f_{n−j}`).
///
/// `None` if `p` does not fit in `i64`.
#[must_use]
pub fn bashforth_offsets(p: usize) -> Option<Vec<i64>> {
    offsets_from(0, p)
}

/// Node offsets of the order-p Adams–Moulton formula: `1, 0, …, −(p−2)` (node `j` is `f_{n+1−j}`).
///
/// `None` if `p` does not fit in `i64`.
#[must_use]
pub fn moulton_offsets(p: usize) -> Option<Vec<i64>> {
    offsets_from(1, p)
}

/// Coefficients `β_j` of the p-step Adams–Bashforth formula
/// `y_{n+1} = y_n + h Σ_j β_j f_{n−j}`, `j = 0 … p−1` (order p).
///
/// `None` on overflow (not for `p ≤ 16`).
#[must_use]
pub fn adams_bashforth(p: usize) -> Option<Vec<Rational>> {
    weights_at(&bashforth_offsets(p)?, Rational::integer(1))
}

/// Coefficients `α_j` of the order-p Adams–Moulton formula
/// `y_{n+1} = y_n + h Σ_j α_j f_{n+1−j}`, `j = 0 … p−1` (`α_0` multiplies the new derivative).
///
/// `None` on overflow (not for `p ≤ 16`).
#[must_use]
pub fn adams_moulton(p: usize) -> Option<Vec<Rational>> {
    weights_at(&moulton_offsets(p)?, Rational::integer(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i128, d: i128) -> Rational {
        Rational::new(n, d).unwrap()
    }

    /// HNW I, Tables III.1.1 and III.1.2: the 4-step Adams–Bashforth formula is
    /// (55, −59, 37, −9)/24, and the order-4 Adams–Moulton formula is (9, 19, −5, 1)/24.
    ///
    /// Verifies: NUM-002
    #[test]
    fn textbook_order_four_coefficients() {
        let ab = adams_bashforth(4).unwrap();
        assert_eq!(ab, vec![q(55, 24), q(-59, 24), q(37, 24), q(-9, 24)]);
        let am = adams_moulton(4).unwrap();
        assert_eq!(am, vec![q(9, 24), q(19, 24), q(-5, 24), q(1, 24)]);
        // Trapezoidal rule and the explicit Euler/AB2 cases.
        assert_eq!(adams_moulton(2).unwrap(), vec![q(1, 2), q(1, 2)]);
        assert_eq!(adams_bashforth(2).unwrap(), vec![q(3, 2), q(-1, 2)]);
    }

    /// Every Adams formula of order p integrates `s^m`, `m < p`, exactly over the step:
    /// `Σ_j w_j d_j^m = 1/(m+1)`, for p = 1 … 14 (exact rational identities).
    ///
    /// Verifies: NUM-002
    #[test]
    fn coefficients_integrate_polynomials_of_degree_below_the_order_exactly() {
        for p in 1..=14_usize {
            for (offsets, w) in [
                (bashforth_offsets(p).unwrap(), adams_bashforth(p).unwrap()),
                (moulton_offsets(p).unwrap(), adams_moulton(p).unwrap()),
            ] {
                for m in 0..p {
                    let mut sum = Rational::ZERO;
                    for (&d, &c) in offsets.iter().zip(&w) {
                        let mut pw = Rational::integer(1);
                        for _ in 0..m {
                            pw = pw.checked_mul(Rational::integer(i128::from(d))).unwrap();
                        }
                        sum = sum.checked_add(c.checked_mul(pw).unwrap()).unwrap();
                    }
                    let want = q(1, i128::try_from(m + 1).unwrap());
                    assert_eq!(sum, want, "p = {p}, m = {m}, offsets {offsets:?}");
                }
            }
        }
    }

    /// The dense weights at θ = 1 are the corrector coefficients, and at θ = 0 they vanish.
    ///
    /// Verifies: NUM-010
    #[test]
    fn dense_weights_end_on_the_corrector() {
        for p in 8..=14 {
            let off = moulton_offsets(p).unwrap();
            assert_eq!(
                weights_at(&off, Rational::integer(1)).unwrap(),
                adams_moulton(p).unwrap()
            );
            assert!(
                weights_at(&off, Rational::ZERO)
                    .unwrap()
                    .iter()
                    .all(|w| *w == Rational::ZERO)
            );
        }
    }

    /// Conversion: exact for representable values, and DD-accurate for 1/3.
    ///
    /// Verifies: DEP-007
    #[test]
    fn conversion_to_backends() {
        assert_eq!(
            q(-9, 24).to_real::<f64>().unwrap().to_bits(),
            (-0.375_f64).to_bits() // lint: allow-literal(exact value of -9/24)
        );
        let third: DoubleDouble = q(1, 3).to_real().unwrap();
        let err = (third * DoubleDouble::from_f64(3.0) - DoubleDouble::ONE).abs();
        assert!(err.to_f64() < 4.0 * DoubleDouble::EPSILON.to_f64());
        let big = Rational::integer((1_i128 << 100) + 1);
        let b: DoubleDouble = big.to_real().unwrap();
        assert_eq!(b.lo().to_bits(), 1.0_f64.to_bits());
        assert!(Rational::integer(1_i128 << 110).to_real::<f64>().is_none());
    }
}
