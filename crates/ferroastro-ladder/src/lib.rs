// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Mercury perihelion ladder (ARCHITECTURE `ferroastro-ladder`; ORG-002).
//!
//! M2 scope (WP4): the single-body relativistic rungs. [`Scenario`] holds the cited inputs,
//! [`single_body::single_body_rungs`] is the catalogue that `ferroastro ladder --rungs
//! single-body` runs, and [`single_body::SingleBody::run`] produces a
//! [`single_body::RungResult`] with the numerical rate, the analytic oracle and the PHY-015
//! metadata.
//!
//! **Not an observable:** the split of the 1PN advance into "velocity-dependent" (SR-b) and
//! other terms depends on the coordinate gauge (7/6 of GR in isotropic/harmonic coordinates,
//! 5/6 in areal Schwarzschild coordinates). Rung SR-b is pedagogical and is never "the SR part of
//! GR"; only the full 1PN sum `(2 + 2γ − β)/3 × 6πGM/(c²p)` is coordinate independent (PHY-008).
//! Likewise the SR-a fraction (1/6) belongs to one model, `H = γmc² − μm/r`; other couplings of
//! special relativity to Newtonian gravity give other fractions (NUM-016).

mod error;
pub mod passage_run;
mod scenario;
pub mod single_body;

pub use error::LadderError;
pub use scenario::{MERCURY_SINGLE_BODY, MeanElements, Scenario};
