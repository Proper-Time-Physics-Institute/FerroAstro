// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Proper Time (Gill) **comparison model** (ADR-0016 decision D6, PHY-021) for a test
//! particle: [`ProperTimeCentral`], Gill's flat-space force law about a fixed central body (the
//! Rung-lab and analytic-check form of the N-body [`crate::ProperTimeKernel`]).
//!
//! # Physics (relativistic-dynamics#88, research packages of 2026-11-14)
//! Gill's interaction Hamiltonian `H₁ = √(c²π² + (mc² + V)²)` and its proper-time generator
//! `K = H₁²/(2mc²) + mc²/2 = π²/2m + mc² + V + V²/(2mc²)` give, in Gill's parameter `τ_K`,
//!
//! ```text
//! d²x/dτ_K² = −(∇V/m)(1 + V/(mc²)),      V = −GMm/r  ⇒  a = −(GM/r²) r̂ + (GM)²/(c² r³) r̂.
//! ```
//!
//! Bridge A: `H₁` is exactly the energy of a free particle on a geodesic of the conformally flat
//! metric `(1 + V/(mc²))² η = (1 − GM/(rc²))² η`, whose PPN parameters are β = 1/2, γ = −1. The
//! perihelion advance is therefore `(2 + 2γ − β)/3 = −1/6` of GR's at first order:
//! `Δϖ = −πGM/(c²a(1−e²))` per orbit, −7.163 ″/cy for Mercury against GR's +42.98 ″/cy.
//! [`crate::SchwarzschildPpn`] at (1/2, −1) is the independent, velocity-dependent route to the
//! same number (PHY-021 acceptance).
//!
//! `dt/dτ_K = E/(mc²)` is constant on each orbit, so integrating the force law in coordinate time
//! gives the same orbit and the same advance per orbit; only the period is rescaled by a constant
//! `1 + O(GM/(ac²))`.

use crate::central::{CentralTerm, dot};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_core::units::{GravParam, Velocity};
use ferroastro_real::Real;
use std::marker::PhantomData;

/// Gill's flat-space force law about a fixed central body of gravitational parameter `μ`, minus
/// the Newtonian part that [`crate::TestParticle`] already carries: `δa = +(μ²/c²) r/r⁴`
/// (comparison model, ADR-0016, PHY-021). Velocity independent; potential
/// `Φ = +μ²/(2c²r²)` per unit mass. Gauge [`Gauge::ProperTimeKernel`].
#[derive(Debug, Clone, Copy)]
pub struct ProperTimeCentral<F: Frame, R: Real = f64> {
    mu: R,
    c: R,
    /// `μ²/c²` (km⁴/s²).
    k: R,
    _frame: PhantomData<fn() -> F>,
}

impl<F: Frame, R: Real> ProperTimeCentral<F, R> {
    /// The term for the central `μ` and the speed of light `c`.
    ///
    /// # Errors
    /// `FA-E0500` if `μ` is not finite and positive or `c` is not finite and positive.
    pub fn new(mu: GravParam<R>, c: Velocity<R>) -> Result<Self, ForceError> {
        let m = mu.km3_per_s2();
        if !(m.is_finite() && m > R::ZERO) {
            return Err(ForceError::Invalid {
                name: "GM (km^3/s^2)",
                value: m.to_f64(),
                valid: "finite and > 0",
            });
        }
        let c = crate::conformal::speed_of_light(c)?;
        Ok(Self {
            mu: m,
            c,
            k: m * m / (c * c),
            _frame: PhantomData,
        })
    }

    /// The potential per unit mass (km²/s²) at heliocentric position `r`: `Φ = μ²/(2c²|r|²)`.
    #[must_use]
    pub fn potential(&self, r: [R; 3]) -> R {
        R::HALF * self.k / dot(r, r)
    }
}

impl<F: Frame, R: Real> CentralTerm<F, R> for ProperTimeCentral<F, R> {
    fn add_acceleration(&self, r: [R; 3], _v: [R; 3], a: &mut [R; 3]) {
        let r2 = dot(r, r);
        let s = self.k / (r2 * r2);
        for (ai, ri) in a.iter_mut().zip(r) {
            *ai += s * ri;
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::ProperTimeKernel
    }
    fn describe(&self) -> String {
        format!(
            "Proper Time (Gill) kernel [comparison model, ADR-0016, PHY-021] \
             a = -(mu/r^2)(1 - mu/(r c^2)) r_hat, mu = {} km^3/s^2, c = {} km/s",
            self.mu.to_f64(),
            self.c.to_f64()
        )
    }
}
