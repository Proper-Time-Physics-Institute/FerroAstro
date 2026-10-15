// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Perihelion-rate estimators (CRATE-PLAN `ferroastro-precession`, layer L5; NUM-011, NUM-012,
//! PHY-015, ADR-0011).
//!
//! Estimators operate on **sampled states or perihelion passages**; this crate contains no
//! propagation (it does not depend on `ferroastro-integrators`). The runs that produce the
//! samples live in `ferroastro-ladder`; the analytic oracles in `ferroastro-analytic`.
//!
//! * [`park2017`]: the canonical Park-2017 estimator (NUM-011) for sampled trajectories such as
//!   a JPL ephemeris: perihelion passages, mean-orbit frame, quadratic + periodic least-squares
//!   fit, formal and jack-knife σ.
//! * [`passages`]: the per-orbit advance fitted from perihelion passages of a single-body run.
//! * [`lrl`]: eccentricity-vector (LRL) perihelion longitude (NUM-012 alternative estimator).
//! * [`fit`]: straight-line fits and the secular-rate fit with mandatory metadata.
//! * [`report`]: [`report::RateReport`], a rate that cannot exist without its metadata (PHY-015).
//! * [`lsq`], [`spectral`]: Householder least squares and a radix-2 FFT used by the fit.
//!
//! The estimator definitions differ (NUM-012): on the same trajectory they can disagree by
//! periodic and gauge terms, which is why every report names its estimator.

mod error;
pub mod fit;
pub mod lrl;
pub mod lsq;
pub mod park2017;
pub mod passages;
pub mod report;
pub mod spectral;

pub use error::PrecessionError;
