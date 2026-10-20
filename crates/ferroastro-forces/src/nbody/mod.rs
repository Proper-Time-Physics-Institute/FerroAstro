// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! N-body force models (M3, WP12): tiered Newtonian point masses, the geocentric Moon and solar
//! J2 with its reaction, behind the [`ForceTerm`] trait, and their IAS15 propagation.

mod layout;
mod model;
mod propagator;
mod terms;

pub use layout::{Accelerations, Layout, MoonPair, MoonVariables, NBody, View};
pub use model::NBodyModel;
pub use propagator::NBodyPropagator;
pub use terms::{
    ForceTerm, GeocentricMoon, LtReaction, MOON_TAYLOR_Q_MAX, MOON_TAYLOR_TERMS_DEFAULT,
    MOON_TAYLOR_TERMS_MIN, MinorFromMajors, MinorReaction, NewtonianMutual, RingMasses,
    SolarJ2WithReaction, SolarLenseThirring, TermParameter,
};

#[cfg(test)]
mod tests;
