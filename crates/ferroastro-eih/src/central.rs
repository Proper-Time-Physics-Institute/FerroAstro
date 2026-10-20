// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Sun-only 1PN rung in N-body mode (scenario switch `gr_1pn_central`, `mode = "n-body"`).
//!
//! For every Major body `i` other than the Sun, with `r = r_i − r_☉`, `v = v_i − v_☉` and
//! `μ = GM_☉`, it adds the R3 Schwarzschild correction in PPN isotropic/harmonic gauge (the same
//! form as `ferroastro_forces::SchwarzschildPpn`, Journal #4 R3):
//!
//! `δa_i = μ/(c² r³) [(2(β+γ) μ/r − γ v²) r + 2(1+γ)(r·v) v]`.
//!
//! This is also Eq. 27 of Park et al. 2021 restricted to the Sun as the only source, with the
//! Sun's velocity and acceleration set to zero in the relative frame. The Sun receives **no**
//! reaction, so the 1PN momentum is not conserved: it is a ladder rung, not a production
//! equation of motion (interpretation of "Sun-only 1PN rung", NEEDS HUMAN REVIEW).

use ferroastro_core::frames::Frame;
use ferroastro_core::units::Velocity;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{Accelerations, ForceError, ForceTerm, Gauge, Layout, TermParameter, View};
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// The Sun's 1PN Schwarzschild field acting on every other Major body (no reaction).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeliocentricSchwarzschild<R: Real = f64> {
    c2: R,
    beta: R,
    gamma: R,
    sun: Option<usize>,
}

impl<R: Real> HeliocentricSchwarzschild<R> {
    /// The term with the speed of light and PPN β, γ. GM☉ is the layout's.
    ///
    /// # Errors
    /// `FA-E0500` if `c` is not positive and finite or β, γ are not finite.
    pub fn new(c: Velocity<R>, beta: R, gamma: R) -> Result<Self, ForceError> {
        let cc = c.km_per_s();
        let bad = |name, value: R, valid| ForceError::Invalid {
            name,
            value: value.to_f64(),
            valid,
        };
        if !(cc.is_finite() && cc > R::ZERO) {
            return Err(bad("c (km/s)", cc, "finite and > 0"));
        }
        if !beta.is_finite() {
            return Err(bad("PPN beta", beta, "finite"));
        }
        if !gamma.is_finite() {
            return Err(bad("PPN gamma", gamma, "finite"));
        }
        Ok(Self {
            c2: cc * cc,
            beta,
            gamma,
            sun: None,
        })
    }
}

fn dot<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

impl<F: Frame, R: Real> ForceTerm<F, R> for HeliocentricSchwarzschild<R> {
    fn name(&self) -> &'static str {
        "gr-1pn-central (heliocentric R3, Sun-only, no reaction)"
    }
    fn gauge(&self) -> Gauge {
        Gauge::PpnIsotropicHarmonic
    }
    fn parameters(&self) -> Vec<TermParameter> {
        vec![
            TermParameter {
                name: "PPN beta",
                value: self.beta.to_f64(),
                unit: "1",
            },
            TermParameter {
                name: "PPN gamma",
                value: self.gamma.to_f64(),
                unit: "1",
            },
            TermParameter {
                name: "c",
                value: self.c2.sqrt().to_f64(),
                unit: "km/s",
            },
            TermParameter {
                name: "time argument TDB (NUM-017)",
                value: 1.0,
                unit: "1",
            },
        ]
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major]
    }
    fn bind(&mut self, layout: &Layout<R>) -> Result<(), ForceError> {
        let sun = layout.index_of(NaifId::SUN).ok_or(ForceError::Invalid {
            name: "Sun (NAIF 10) among the bodies",
            value: 0.0,
            valid: "present for the heliocentric 1PN term",
        })?;
        if layout.tiers().get(sun) != Some(&Tier::Major) {
            return Err(ForceError::Invalid {
                name: "tier of the Sun",
                value: 0.0,
                valid: "Major",
            });
        }
        self.sun = Some(sun);
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let Some(sun) = self.sun else {
            return Err(ForceError::Invalid {
                name: "heliocentric 1PN term bound to a layout",
                value: 0.0,
                valid: "bound (NBodyModel::new binds every term)",
            });
        };
        let layout = view.layout();
        let mu = layout.gm(sun);
        let (ps, ws) = (view.position(sun), view.velocity(sun));
        for &i in layout.indices(Tier::Major) {
            if i == sun {
                continue;
            }
            let (pi, wi) = (view.position(i), view.velocity(i));
            let r = [pi[0] - ps[0], pi[1] - ps[1], pi[2] - ps[2]];
            let v = [wi[0] - ws[0], wi[1] - ws[1], wi[2] - ws[2]];
            let r2 = dot(r, r);
            if r2 == R::ZERO {
                return Err(ForceError::Singular {
                    i: sun,
                    j: i,
                    t: t.to_f64(),
                });
            }
            let rn = r2.sqrt();
            let k = mu / (self.c2 * r2 * rn);
            let radial =
                (R::TWO * (self.beta + self.gamma)).mul_add(mu / rn, -self.gamma * dot(v, v));
            let along = R::TWO * (R::ONE + self.gamma) * dot(r, v);
            acc.add(
                i,
                [
                    k * radial.mul_add(r[0], along * v[0]),
                    k * radial.mul_add(r[1], along * v[1]),
                    k * radial.mul_add(r[2], along * v[2]),
                ],
            );
        }
        Ok(())
    }
    fn potential_over_g(&self, _view: &View<'_, R>) -> Option<R> {
        None
    }
}
