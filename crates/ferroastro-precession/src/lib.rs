// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Orbital elements, perihelion estimators and analytic precession rates (ARCHITECTURE
//! `ferroastro-precession`; PHY-001, PHY-004, ADR-0011).
//!
//! * [`kepler`]: osculating elements ↔ Cartesian states.
//! * [`lrl`]: eccentricity-vector perihelion longitude for single-planet runs.
//! * [`fit`]: secular-rate fit with mandatory metadata (frame, epoch, span, fit model).
//! * [`analytic`]: the Schwarzschild advance `6πGM/(c²a(1−e²))` (rung GR-SCHW analytic) and the
//!   single-body oracles of NUM-015 (SR exact, PPN scaling, J2 planar/inclined, Lense–Thirring).
//! * [`passages`]: perihelion passages (`r·v = 0`) and the per-orbit advance fitted from them.
//! * [`report`]: [`report::RateReport`], a rate that cannot exist without its metadata (PHY-015).
//! * [`park2017`]: the canonical Park-2017 estimator (NUM-011, ADR-0011) for **sampled**
//!   trajectories such as a JPL ephemeris: perihelion passages, mean-orbit frame, quadratic +
//!   periodic least-squares fit, formal and jack-knife σ.
//! * [`lsq`], [`spectral`]: Householder least squares and a radix-2 FFT used by the fit.
//!
//! The LRL estimator is exact for two-body runs, where the eccentricity vector is a constant of
//! motion.

pub mod analytic;
mod error;
pub mod fit;
pub mod kepler;
pub mod lrl;
pub mod lsq;
pub mod park2017;
pub mod passages;
pub mod report;
pub mod spectral;

pub use error::PrecessionError;
