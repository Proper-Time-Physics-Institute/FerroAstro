// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro force models (ARCHITECTURE `ferroastro-forces`; AGENTS.md rule 4).
//!
//! Every force model implements [`ForceModel`], which states its **gauge** and is a
//! [`SecondOrderSystem`] the integrators can advance. M0 provides
//! [`NewtonianPointMasses`], the force model of rungs N-2B and N-SUM, and the single-body
//! relativistic rungs (M2 scope, WP4): a [`TestParticle`] in the field of a fixed central body with
//! composable [`CentralTerm`]s —
//!
//! * [`SrKepler`] (Journal #4 R2): `d(γv)/dt = −μr/r³`, Hamiltonian `H = γmc² − μm/r`;
//! * [`SchwarzschildPpn`] (R3): 1PN Schwarzschild field with PPN β, γ, isotropic/harmonic gauge;
//! * [`PnLagrangian`] (PHY-009): the general test-particle Lagrangian
//!   `L = ½v² + U + c⁻²[A v⁴ + B U v² + C U² + D U (n·v)²]`;
//! * [`SolarJ2`] (R1): the central body's quadrupole with a configurable pole;
//! * [`LenseThirring`] (R3b): the gravitomagnetic field of the central body's spin.
//!
//! M3 (WP12) adds tiered N-body models: an [`NBodyModel`] is a body [`Layout`] (NAIF ids, GMs,
//! [`ferroastro_system::Tier`]s, optionally the geocentric Earth–Moon pair of NUM-008) and an
//! ordered list of [`ForceTerm`]s — [`NewtonianMutual`], [`MinorFromMajors`], [`RingMasses`],
//! [`GeocentricMoon`], [`SolarJ2WithReaction`] and (WP20) [`SolarLenseThirring`] — each declaring its gauge, parameters and
//! tiers. [`NBodyPropagator`] advances a model with IAS15 from a TDB epoch.
//!
//! M5 (WP25) adds the NUM-007 ephemeris precision mode: [`MixedForces`] (f64 force terms,
//! compensated accumulation widened into double-double) and the plain [`F64Forces`] adapter, for
//! an integrator whose state, sums and coefficients are double-double.
//!
//! M5 (WP26) adds the [`soa`] structure-of-arrays kernels (NUM-021): [`NewtonianMutualSoa`], with
//! an exact `1/r³`, no fused multiply-add and a fixed reduction order, whose scalar and SIMD
//! (feature `simd`, the safe `wide` crate) instantiations are bitwise identical.
//!
//! Internal units: positions in km, velocities in km/s, time in TDB seconds, GM in km³/s². The
//! typed constructors and [`NewtonianPointMasses::pack`] / [`NewtonianPointMasses::unpack`]
//! convert from and to the [`ferroastro_core`] newtypes, so raw scalars only appear inside the
//! integration loop. Every model and term is generic over the scalar backend
//! `R: ferroastro_real::Real` with `R = f64` by default (DEP-007): `TestParticle<F>` is the f64
//! model, `TestParticle<F, DoubleDouble>` the same physics in double-double.

mod central;
mod error;
mod mixed;
mod nbody;
mod newtonian;
mod oblateness;
mod relativity;
pub mod soa;

pub use central::{CentralTerm, Pole, TestParticle};
pub use error::ForceError;
pub use ferroastro_integrators::SecondOrderSystem;
pub use mixed::{F64Forces, MixedForces};
pub use nbody::{
    Accelerations, ForceTerm, GeocentricMoon, Layout, LtReaction, MOON_TAYLOR_Q_MAX,
    MOON_TAYLOR_TERMS_DEFAULT, MOON_TAYLOR_TERMS_MIN, MinorFromMajors, MinorReaction, MoonPair,
    MoonVariables, NBody, NBodyModel, NBodyPropagator, NewtonianMutual, RingMasses,
    SolarJ2WithReaction, SolarLenseThirring, TermParameter, View,
};
pub use newtonian::NewtonianPointMasses;
pub use oblateness::SolarJ2;
pub use relativity::{LenseThirring, PnCoefficients, PnLagrangian, SchwarzschildPpn, SrKepler};
pub use soa::{NewtonianMutualSoa, SoaBackend};

/// The coordinate gauge of a force model (AGENTS.md rule 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Gauge {
    /// Newtonian dynamics: no post-Newtonian coordinate gauge applies (the Newtonian limit of
    /// every PPN gauge is the same).
    Newtonian,
    /// PPN isotropic / harmonic coordinates (identical at 1PN for a test particle; the default
    /// for relativistic models, ADR-0011 / NUM-017).
    PpnIsotropicHarmonic,
    /// Standard (areal) Schwarzschild coordinates. **Pedagogical only**: NUM-017 forbids
    /// standard-Schwarzschild-coordinate equations of motion except for labelled oracles; this
    /// gauge exists solely for rung SR-b's areal-gauge variant (PHY-008), whose purpose is to
    /// show that the velocity-dependent share of the advance is gauge dependent.
    SchwarzschildAreal,
    /// Special relativity in an inertial frame with a Newtonian potential (rung SR-a). No
    /// post-Newtonian metric, hence no PN gauge; the SR-a advance is gauge independent because
    /// no 1PN coordinate change alters the `v⁴` coefficient.
    SpecialRelativistic,
    /// A generic 1PN test-particle Lagrangian with freely chosen coefficients (the PHY-009
    /// coefficient study). It is not derived from a metric, so it has no gauge.
    GenericPnLagrangian,
}

impl Gauge {
    /// A label for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Gauge::Newtonian => "Newtonian (no PN gauge)",
            Gauge::PpnIsotropicHarmonic => "PPN isotropic/harmonic",
            Gauge::SchwarzschildAreal => {
                "Schwarzschild areal (standard) coordinates [pedagogical only, NUM-017 exemption]"
            }
            Gauge::SpecialRelativistic => {
                "special relativity, inertial frame (no PN gauge; H = gamma m c^2 - mu m / r)"
            }
            Gauge::GenericPnLagrangian => "generic 1PN Lagrangian coefficients (no metric gauge)",
        }
    }

    /// True for gauges that describe post-Newtonian (or special-relativistic) dynamics, i.e.
    /// every gauge except [`Gauge::Newtonian`]. Terms of two different non-Newtonian gauges
    /// cannot be combined in one model.
    #[must_use]
    pub const fn is_relativistic(self) -> bool {
        !matches!(self, Gauge::Newtonian)
    }
}

/// A force model: a second-order system (in backend `R`, default `f64`) that declares its gauge
/// and a stable name.
pub trait ForceModel<R: ferroastro_real::Real = f64>: SecondOrderSystem<R> {
    /// The model's gauge.
    fn gauge(&self) -> Gauge;
    /// A short stable name for reports, e.g. `"newtonian-point-masses"`.
    fn name(&self) -> &'static str;
}
