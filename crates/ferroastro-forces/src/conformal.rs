// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The conformal-gravity **comparison model** (ADR-0016, PHY-020): Mannheim's weak-field linear
//! and quadratic potentials as a test-particle [`CentralTerm`] ([`ConformalCentral`], for the
//! Rung lab and the analytic checks), plus the shared coefficient conversion of the N-body terms
//! [`crate::ConformalPotential`] and [`crate::ConformalUniformGamma0`].
//!
//! # Physics (ADR-0016)
//! The Mannheim–Kazanas exterior in the form used for orbit work is
//! `B(r) = 1 − 2β/r + γr − κr²`, `β = GM/c²`. To Newtonian order `Φ = c²(B − 1)/2`, so the two
//! extra terms add the potential per unit mass and the acceleration
//!
//! ```text
//! Φ_CG(r) = +(γc²/2) r − (κc²/2) r²,        a_CG = −∇Φ_CG = −(γc²/2) r̂ + κc² r.
//! ```
//!
//! `γ` and `κ` are signed (decision D3): `γ > 0` is a constant attraction (retrograde advance),
//! `κ > 0` a Hooke-like repulsion (prograde advance), matching the `−κr²` term of `B`. The Gauss
//! secular rates per orbit, the PHY-020 oracles, are `Δϖ = −πγa²√(1−e²)/β` and
//! `Δϖ = +3πκa³√(1−e²)/β`.
//!
//! The galactic fits write `γ = N*γ* + γ0` with `N*` the source mass in solar masses and `γ0` a
//! cosmological term whose centre the sources leave open (decision D2, `gamma0_centre`). The fit
//! values (OI-79) are unverified and are never test oracles (D7).
//!
//! # Units
//! The published coefficients are CGS (`γ` in cm⁻¹, `κ` in cm⁻²); the scenario table
//! `[forces.conformal]` takes them in those units. The force terms take **km⁻¹ and km⁻²** (the
//! engine's length unit): `γ[km⁻¹] = γ[cm⁻¹] · 10⁵` and `κ[km⁻²] = κ[cm⁻²] · 10¹⁰`
//! (`ferroastro_scenario::CM_PER_KM` and the `Conformal::*_per_km` accessors convert).
//! With `c` in km/s the linear coefficient `γc²/2` is in km/s² and `κc²` in s⁻².

use crate::central::{CentralTerm, dot};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_core::units::Velocity;
use ferroastro_real::Real;
use std::marker::PhantomData;

/// Check that a coefficient is finite (any sign).
pub(crate) fn finite<R: Real>(name: &'static str, value: R) -> Result<R, ForceError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ForceError::Invalid {
            name,
            value: value.to_f64(),
            valid: "finite (signed)",
        })
    }
}

/// `c` in km/s, checked finite and positive.
pub(crate) fn speed_of_light<R: Real>(c: Velocity<R>) -> Result<R, ForceError> {
    let c = c.km_per_s();
    if c.is_finite() && c > R::ZERO {
        Ok(c)
    } else {
        Err(ForceError::Invalid {
            name: "speed of light (km/s)",
            value: c.to_f64(),
            valid: "finite and > 0",
        })
    }
}

/// `(γc²/2, κc²)` in km/s² and s⁻² from `γ` (km⁻¹), `κ` (km⁻²) and `c` (km/s).
pub(crate) fn coefficients<R: Real>(gamma: R, kappa: R, c: R) -> (R, R) {
    let c2 = c * c;
    (gamma * c2 * R::HALF, kappa * c2)
}

/// Mannheim's weak-field conformal-gravity terms for a test particle about a fixed central body
/// (comparison model, ADR-0016, PHY-020): `a = −(γc²/2) r̂ + κc² r`, `r` the heliocentric position,
/// with `γ` the total linear coefficient of the central body (`N*γ*`, plus `γ0` for the
/// Sun-centred reading) and `κ` the quadratic coefficient, both signed.
///
/// A uniform `γ0` (decision D2, `"uniform"`) accelerates the central body and the particle
/// equally and so has no relative effect; it has no test-particle form and is omitted here (the
/// N-body [`crate::ConformalUniformGamma0`] implements it).
#[derive(Debug, Clone, Copy)]
pub struct ConformalCentral<F: Frame, R: Real = f64> {
    gamma: R,
    kappa: R,
    c: R,
    /// `γc²/2` (km/s²).
    lin: R,
    /// `κc²` (s⁻²).
    quad: R,
    _frame: PhantomData<fn() -> F>,
}

impl<F: Frame, R: Real> ConformalCentral<F, R> {
    /// The term for linear coefficient `gamma` (km⁻¹), quadratic coefficient `kappa` (km⁻²) and
    /// speed of light `c`.
    ///
    /// # Errors
    /// `FA-E0500` if a coefficient is not finite or `c` is not finite and positive.
    pub fn new(gamma: R, kappa: R, c: Velocity<R>) -> Result<Self, ForceError> {
        let gamma = finite("conformal gamma (1/km)", gamma)?;
        let kappa = finite("conformal kappa (1/km^2)", kappa)?;
        let c = speed_of_light(c)?;
        let (lin, quad) = coefficients(gamma, kappa, c);
        Ok(Self {
            gamma,
            kappa,
            c,
            lin,
            quad,
            _frame: PhantomData,
        })
    }

    /// `(γ c²/2, κ c²)` in km/s² and s⁻².
    #[must_use]
    pub const fn coefficients(&self) -> (R, R) {
        (self.lin, self.quad)
    }

    /// The potential per unit mass (km²/s²) at heliocentric position `r`:
    /// `Φ = (γc²/2) |r| − (κc²/2) |r|²`.
    #[must_use]
    pub fn potential(&self, r: [R; 3]) -> R {
        let r2 = dot(r, r);
        self.lin * r2.sqrt() - R::HALF * self.quad * r2
    }
}

impl<F: Frame, R: Real> CentralTerm<F, R> for ConformalCentral<F, R> {
    fn add_acceleration(&self, r: [R; 3], _v: [R; 3], a: &mut [R; 3]) {
        let rn = dot(r, r).sqrt();
        let s = self.quad - self.lin / rn;
        for (ai, ri) in a.iter_mut().zip(r) {
            *ai += s * ri;
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::StaticWeakFieldPotential
    }
    fn describe(&self) -> String {
        format!(
            "conformal gravity [comparison model, ADR-0016] a = -(gamma c^2/2) r_hat + kappa c^2 r, \
             gamma = {:e} 1/km, kappa = {:e} 1/km^2, c = {} km/s",
            self.gamma.to_f64(),
            self.kappa.to_f64(),
            self.c.to_f64()
        )
    }
}
