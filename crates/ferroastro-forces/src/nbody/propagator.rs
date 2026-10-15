// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! [`NBodyPropagator`]: an [`NBodyModel`] advanced by IAS15 from a TDB epoch, typed in and out.

use super::model::NBodyModel;
use crate::ForceError;
use ferroastro_core::frames::{Barycentric, Frame};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::TimeSpan;
use ferroastro_integrators::{Ias15, Ias15Config, Ias15Stats, IntegratorError, StepReport};
use ferroastro_real::Real;
use ferroastro_system::{EarthMoon, SystemState};

/// IAS15 hooked to an N-body model. The integrator's independent variable is TDB seconds since
/// the start epoch, in backend `R`.
#[derive(Debug)]
pub struct NBodyPropagator<F: Frame, R: Real = f64> {
    model: NBodyModel<F, R>,
    ias: Ias15<R>,
    start: Epoch<Tdb, R>,
}

impl<F: Frame, R: Real> NBodyPropagator<F, R> {
    /// Start at `epoch` from barycentric `states` (and, for a geocentric model, optionally the
    /// Earth–Moon pair in DE variables), with first trial step `dt0` and IAS15 settings `cfg`.
    ///
    /// # Errors
    /// `FA-E0500` for a body missing from `states`; the IAS15 errors (`FA-E040x`) for invalid
    /// settings or initial values.
    pub fn new(
        model: NBodyModel<F, R>,
        states: &SystemState<F, Barycentric, R>,
        earth_moon: Option<&EarthMoon<F, R>>,
        epoch: Epoch<Tdb, R>,
        dt0: TimeSpan<R>,
        cfg: Ias15Config,
    ) -> Result<Self, IntegratorError> {
        let (x, v) = model.pack(states, earth_moon)?;
        let n = x.len();
        let ias = Ias15::new(n, R::ZERO, &x, &v, dt0.seconds(), cfg)?;
        Ok(Self {
            model,
            ias,
            start: epoch,
        })
    }

    /// The model.
    #[must_use]
    pub const fn model(&self) -> &NBodyModel<F, R> {
        &self.model
    }

    /// The integrator (flat state, statistics, dense output).
    #[must_use]
    pub const fn integrator(&self) -> &Ias15<R> {
        &self.ias
    }

    /// The start epoch.
    #[must_use]
    pub const fn start(&self) -> Epoch<Tdb, R> {
        self.start
    }

    /// The current epoch.
    #[must_use]
    pub fn epoch(&self) -> Epoch<Tdb, R> {
        self.start + TimeSpan::from_seconds(self.ias.time()).unwrap_or_else(|_| TimeSpan::zero())
    }

    /// Integrator statistics.
    #[must_use]
    pub const fn stats(&self) -> Ias15Stats {
        self.ias.stats()
    }

    /// Advance (or go back) to `epoch`, calling `observer` after every accepted step.
    ///
    /// # Errors
    /// The integrator's or the model's errors, or the observer's.
    pub fn advance_to_with<O>(
        &mut self,
        epoch: Epoch<Tdb, R>,
        observer: O,
    ) -> Result<(), IntegratorError>
    where
        O: FnMut(&Ias15<R>, &StepReport<R>) -> Result<(), IntegratorError>,
    {
        let t = (epoch - self.start).seconds();
        self.ias.integrate_to(&self.model, t, observer)
    }

    /// Advance (or go back) to `epoch`.
    ///
    /// # Errors
    /// The integrator's or the model's errors.
    pub fn advance_to(&mut self, epoch: Epoch<Tdb, R>) -> Result<(), IntegratorError> {
        self.advance_to_with(epoch, |_, _| Ok(()))
    }

    /// The current barycentric system (physical Earth and Moon).
    ///
    /// # Errors
    /// `FA-E0500` for a non-finite component.
    pub fn state(&self) -> Result<SystemState<F, Barycentric, R>, ForceError> {
        self.model
            .unpack(self.ias.positions(), self.ias.velocities())
    }

    /// The current Earth–Moon pair in DE variables (geocentric models only).
    #[must_use]
    pub fn earth_moon(&self) -> Option<EarthMoon<F, R>> {
        self.model
            .earth_moon(self.ias.positions(), self.ias.velocities())
    }

    /// Current total energy / G (see [`NBodyModel::energy_over_g`]).
    #[must_use]
    pub fn energy_over_g(&self) -> Option<R> {
        self.model
            .energy_over_g(self.ias.positions(), self.ias.velocities())
    }

    /// Current momentum / G and its scale (see [`NBodyModel::momentum_over_g`]).
    #[must_use]
    pub fn momentum_over_g(&self) -> ([R; 3], R) {
        self.model
            .momentum_over_g(self.ias.positions(), self.ias.velocities())
    }
}
