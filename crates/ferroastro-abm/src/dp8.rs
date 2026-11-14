// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! DP8: the 12-stage, 8th-order explicit Runge–Kutta formula of DOP853 (Dormand & Prince;
//! Hairer, Nørsett & Wanner 1993, *Solving ODEs I*, §II.10), used at a fixed step to start the
//! Adams–Bashforth–Moulton history (NUM-002: "DP8 at h/8"; Subbotin, Kodukov & Pavlov 2023).
//!
//! Only the 8th-order solution is used (no error estimate, no step control, no dense output). The
//! coefficients are the published 30-digit values as binary64 `(hi, lo)` pairs
//! ([`crate::dp8_table`], generated and order-checked by `tools/abm_dp8.py`), so a double-double
//! start-up uses them to ~1e-30.

use crate::dp8_table::{A, B, C, Pair, STAGES};
use ferroastro_integrators::IntegratorError;
use ferroastro_real::Real;

/// The DP8 tableau in backend `R`.
#[derive(Debug, Clone)]
pub(crate) struct Dp8<R: Real> {
    c: [R; STAGES],
    b: [R; STAGES],
    a: [[R; STAGES]; STAGES],
    /// Stage derivatives (scratch).
    k: Vec<Vec<R>>,
    /// Stage state (scratch).
    ys: Vec<R>,
}

fn conv<R: Real>(p: Pair) -> R {
    R::from_parts(p.0, p.1)
}

impl<R: Real> Dp8<R> {
    /// The tableau for systems of `m` first-order components.
    pub(crate) fn new(m: usize) -> Self {
        Self {
            c: C.map(conv),
            b: B.map(conv),
            a: A.map(|row| row.map(conv)),
            k: vec![vec![R::ZERO; m]; STAGES],
            ys: vec![R::ZERO; m],
        }
    }

    /// One step `y ← y(t + h)` of `y' = f(t, y)`. Sums run over the stages in index order;
    /// zero coefficients are skipped (structural zeros of the tableau).
    ///
    /// # Errors
    /// The first error of `f`.
    pub(crate) fn step<F>(
        &mut self,
        f: &mut F,
        t: R,
        h: R,
        y: &mut [R],
    ) -> Result<(), IntegratorError>
    where
        F: FnMut(R, &[R], &mut [R]) -> Result<(), IntegratorError>,
    {
        for s in 0..STAGES {
            let (done, rest) = self.k.split_at_mut(s);
            let Some(ks) = rest.first_mut() else {
                break;
            };
            let row = self.a.get(s).copied().unwrap_or([R::ZERO; STAGES]);
            for (i, (yi, zi)) in y.iter().zip(self.ys.iter_mut()).enumerate() {
                let mut acc = R::ZERO;
                for (kj, &aij) in done.iter().zip(&row) {
                    if aij != R::ZERO {
                        acc = kj.get(i).copied().unwrap_or(R::ZERO).mul_add(aij, acc);
                    }
                }
                *zi = h.mul_add(acc, *yi);
            }
            let cs = self.c.get(s).copied().unwrap_or(R::ZERO);
            f(cs.mul_add(h, t), &self.ys, ks)?;
        }
        for (i, yi) in y.iter_mut().enumerate() {
            let mut acc = R::ZERO;
            for (kj, &bj) in self.k.iter().zip(&self.b) {
                if bj != R::ZERO {
                    acc = kj.get(i).copied().unwrap_or(R::ZERO).mul_add(bj, acc);
                }
            }
            *yi = h.mul_add(acc, *yi);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_real::DoubleDouble;
    use ferroastro_vectors::VectorSet;

    /// Steps per period of the order check: chosen so that the error at h/2 stays >= 1e3 times
    /// the double-double round-off floor (WP24 plan check A2).
    const STEPS: [u32; 2] = [16, 32];

    /// Global error of DP8 on the unit harmonic oscillator over one period with `steps` steps.
    fn oscillator_error(steps: u32) -> f64 {
        type D = DoubleDouble;
        let mut dp = Dp8::<D>::new(2);
        let h = D::TAU / D::from_f64(f64::from(steps));
        let mut f = |_t: D, y: &[D], out: &mut [D]| -> Result<(), IntegratorError> {
            out[0] = y[1];
            out[1] = -y[0];
            Ok(())
        };
        let mut y = vec![D::ONE, D::ZERO];
        for i in 0..steps {
            dp.step(&mut f, D::from_f64(f64::from(i)) * h, h, &mut y)
                .unwrap();
        }
        let e0 = y[0] - D::ONE;
        (e0 * e0 + y[1] * y[1]).sqrt().to_f64()
    }

    /// A2: the DP8 start-up formula is of order 8 (`ABM-DP8-ORDER-SLOPE`).
    ///
    /// Verifies: NUM-002
    #[test]
    fn dp8_is_eighth_order() {
        let tol = VectorSet::load_canonical()
            .unwrap()
            .get("ABM-DP8-ORDER-SLOPE")
            .unwrap()
            .clone();
        let [s1, s2] = STEPS;
        let (e1, e2) = (oscillator_error(s1), oscillator_error(s2));
        let slope = (e1 / e2).log2();
        println!(
            "A2: DP8 harmonic oscillator, one period: {s1} steps err {e1:.3e}, {s2} steps err \
             {e2:.3e}, observed order {slope:.3}"
        );
        tol.check(slope - 8.0).unwrap();
    }
}
