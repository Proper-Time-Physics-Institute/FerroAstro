// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro force models (ARCHITECTURE `ferroastro-forces`; AGENTS.md rule 4).
//!
//! Every force model implements [`ForceModel`], which states its **gauge** and is a
//! [`SecondOrderSystem`] the integrators can advance. M0 provides
//! [`NewtonianPointMasses`], the force model of rungs N-2B and N-SUM.
//!
//! Internal units: positions in km, velocities in km/s, time in TDB seconds, GM in km³/s². The
//! typed constructors and [`NewtonianPointMasses::pack`] / [`NewtonianPointMasses::unpack`]
//! convert from and to the [`ferroastro_core`] newtypes, so raw `f64` only appears inside the
//! integration loop.

mod error;
mod newtonian;

pub use error::ForceError;
pub use ferroastro_integrators::SecondOrderSystem;
pub use newtonian::NewtonianPointMasses;

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
}

impl Gauge {
    /// A label for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Gauge::Newtonian => "Newtonian (no PN gauge)",
            Gauge::PpnIsotropicHarmonic => "PPN isotropic/harmonic",
        }
    }
}

/// A force model: a second-order system that declares its gauge and a stable name.
pub trait ForceModel: SecondOrderSystem {
    /// The model's gauge.
    fn gauge(&self) -> Gauge;
    /// A short stable name for reports, e.g. `"newtonian-point-masses"`.
    fn name(&self) -> &'static str;
}
