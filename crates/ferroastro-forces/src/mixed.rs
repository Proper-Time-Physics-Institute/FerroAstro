// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The NUM-007 "ephemeris precision mode": an f64 force model behind an integrator that runs in
//! a wider backend (WP25).
//!
//! Subbotin, Kodukov & Pavlov 2023 (`CeMDA`, doi:10.1007/s10569-023-10139-2; arXiv:2304.04458
//! §4.3) keep the state, its time derivative and the integrator's intermediate values and
//! coefficients in double-double, and evaluate the accelerations in double precision. They found
//! that this "mixed" scheme gives round-off close to 80-bit extended precision at about 3× the
//! cost of plain f64 (their Table 1).
//!
//! [`F64Forces`] is that scheme as a type configuration. It wraps any
//! `SecondOrderSystem<f64>` and is a `SecondOrderSystem<R>` for every backend `R`. Each
//! evaluation
//!
//! 1. rounds `t`, `x` and `v` once to the nearest f64 ([`Real::to_f64`]);
//! 2. evaluates the wrapped f64 model;
//! 3. widens each acceleration component exactly ([`Real::from_f64`]).
//!
//! The integrator then forms its sums and its state update in `R`. For `R = f64` the wrapper is
//! the identity: every acceleration is bitwise that of the wrapped model.
//!
//! FerroAstro's integrators keep the whole derivative history in `R`. Subbotin et al. keep the
//! backward differences ∇ʲf in f64 and only the γⱼ in double-double, so this realisation rounds
//! less than theirs, never more.

use crate::NBodyModel;
use ferroastro_core::frames::Frame;
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use ferroastro_real::Real;

/// An f64 force model seen as a system in backend `R` (NUM-007 mixed mode; see the module
/// documentation).
#[derive(Debug, Clone, Copy)]
pub struct F64Forces<'a, S: ?Sized> {
    inner: &'a S,
}

impl<'a, S: SecondOrderSystem<f64> + ?Sized> F64Forces<'a, S> {
    /// Wrap an f64 model.
    #[must_use]
    pub const fn new(inner: &'a S) -> Self {
        Self { inner }
    }

    /// The wrapped f64 model.
    #[must_use]
    pub const fn inner(&self) -> &'a S {
        self.inner
    }
}

impl<R: Real, S: SecondOrderSystem<f64> + ?Sized> SecondOrderSystem<R> for F64Forces<'_, S> {
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn acceleration(&self, t: R, x: &[R], v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let xf: Vec<f64> = x.iter().map(|&z| z.to_f64()).collect();
        let vf: Vec<f64> = v.iter().map(|&z| z.to_f64()).collect();
        let mut af = vec![0.0_f64; a.len()];
        self.inner.acceleration(t.to_f64(), &xf, &vf, &mut af)?;
        for (out, &value) in a.iter_mut().zip(&af) {
            *out = R::from_f64(value);
        }
        Ok(())
    }
}

/// The NUM-007 mixed mode of an f64 [`NBodyModel`] as Subbotin et al. 2023 §4.3 realise it:
/// each force term is evaluated in f64 from the state rounded once to f64, and the
/// contributions are accumulated with Neumaier's compensated sum and widened as the unevaluated
/// sum `(Σ, compensation)` into `R` ([`NBodyModel::acceleration_split`]), so the time derivative
/// `f` is not rounded to f64 (their "x and f are stored in double-double"). For `R = f64` the
/// pair is rounded once.
///
/// WP25 found that [`F64Forces`], which rounds the summed acceleration to f64, is limited by the
/// summation round-off of the hundreds of small contributions to each body (IMPLEMENTATION-NOTES
/// WP25); this adapter is the mixed mode of the run driver.
#[derive(Debug, Clone, Copy)]
pub struct MixedForces<'a, F: Frame> {
    model: &'a NBodyModel<F, f64>,
}

impl<'a, F: Frame> MixedForces<'a, F> {
    /// Wrap an f64 N-body model.
    #[must_use]
    pub const fn new(model: &'a NBodyModel<F, f64>) -> Self {
        Self { model }
    }
}

impl<R: Real, F: Frame> SecondOrderSystem<R> for MixedForces<'_, F> {
    fn dimension(&self) -> usize {
        SecondOrderSystem::<f64>::dimension(self.model)
    }

    fn acceleration(&self, t: R, x: &[R], v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        let xf: Vec<f64> = x.iter().map(|&z| z.to_f64()).collect();
        let vf: Vec<f64> = v.iter().map(|&z| z.to_f64()).collect();
        let mut hi = vec![0.0_f64; a.len()];
        let mut lo = vec![0.0_f64; a.len()];
        self.model
            .acceleration_split(t.to_f64(), &xf, &vf, &mut hi, &mut lo)?;
        for ((out, &h), &l) in a.iter_mut().zip(&hi).zip(&lo) {
            *out = R::from_parts(h, l);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_real::DoubleDouble;

    /// A velocity-dependent toy system, `a = −x·|x|⁻³ + 1e-3 v`.
    struct Toy;
    impl SecondOrderSystem<f64> for Toy {
        fn dimension(&self) -> usize {
            3
        }
        fn acceleration(
            &self,
            t: f64,
            x: &[f64],
            v: &[f64],
            a: &mut [f64],
        ) -> Result<(), IntegratorError> {
            let r = x.iter().map(|z| z * z).sum::<f64>().sqrt();
            for k in 0..3 {
                a[k] = 1e-9_f64.mul_add(t, 1e-3_f64.mul_add(v[k], -x[k] / (r * r * r))); // lint: allow-literal(dimensionless toy system)
            }
            Ok(())
        }
    }

    /// Verifies: NUM-007, DEP-007
    #[test]
    fn f64_backend_is_the_identity_and_dd_rounds_the_inputs_once() {
        let x = [0.3_f64, -1.1, 0.7]; // lint: allow-literal(dimensionless toy state)
        let v = [0.01_f64, 0.2, -0.3]; // lint: allow-literal(dimensionless toy state)
        let t = 2.5; // lint: allow-literal(dimensionless toy time)
        let mut want = [0.0; 3];
        Toy.acceleration(t, &x, &v, &mut want).unwrap();
        let mut got = [0.0; 3];
        F64Forces::new(&Toy)
            .acceleration(t, &x, &v, &mut got)
            .unwrap();
        assert_eq!(got.map(f64::to_bits), want.map(f64::to_bits));

        // DD inputs with a low word: the f64 model sees their nearest f64 values.
        let lo = 1e-20; // lint: allow-literal(a low word below half an ulp of every input)
        let xd = x.map(|z| DoubleDouble::from_parts(z, lo));
        let vd = v.map(|z| DoubleDouble::from_parts(z, -lo));
        let mut ad = [DoubleDouble::ZERO; 3];
        F64Forces::new(&Toy)
            .acceleration(DoubleDouble::from_parts(t, lo), &xd, &vd, &mut ad)
            .unwrap();
        for k in 0..3 {
            assert_eq!(ad[k].to_parts(), (want[k], 0.0));
        }
        assert_eq!(
            SecondOrderSystem::<DoubleDouble>::dimension(&F64Forces::new(&Toy)),
            3
        );
    }
}
