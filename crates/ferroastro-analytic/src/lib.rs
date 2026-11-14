// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Closed-form oracles of the Mercury ladder (CRATE-PLAN `ferroastro-analytic`, layer L4;
//! PHY-004, NUM-015, REF-010).
//!
//! * [`kepler`]: osculating elements ↔ Cartesian states, Kepler's equation.
//! * [`advance`]: the analytic perihelion advances — Schwarzschild `6πGM/(c²a(1−e²))`
//!   (PHY-004), PPN scaling `(2+2γ−β)/3` (PHY-006), the 1PN unit `πμ/(c²p)` (PHY-007/009),
//!   the exact SR-Kepler advance (NUM-015 R2), J2 planar and inclined (PHY-010, Park et al. 2017
//!   Eq. 3) and Lense–Thirring (PHY-011, Park et al. 2017 Eq. 2).
//! * [`vector`]: raw 3-vector helpers and the eccentricity (LRL) vector.
//! * [`drift`]: the exact Kepler flow in universal variables (the Wisdom–Holman drift, NUM-004).
//!
//! Every oracle is generic over the scalar backend `R: ferroastro_real::Real` (default `f64`), so
//! the same formula can be evaluated in double-double when a rung is run in double-double.
//!
//! This crate contains no propagation and no estimator: numerical rungs live in
//! `ferroastro-ladder`, estimators in `ferroastro-precession`. Keeping the oracles apart from
//! the code they check is what makes a rung comparison non-circular.
//!
//! ```
//! use ferroastro_analytic::advance::schwarzschild_advance;
//! use ferroastro_core::units::{GravParam, Length, Velocity};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // A dimensionless check (GM = c = a = 1, e = 0): the advance is 6π per orbit.
//! let one = GravParam::from_km3_per_s2(1.0)?;
//! let adv = schwarzschild_advance(one, Velocity::from_km_per_s(1.0)?, Length::from_km(1.0)?, 0.0, one)?;
//! assert!((adv.per_orbit.radians() - 6.0 * std::f64::consts::PI).abs() < 1e-14);
//! # Ok(())
//! # }
//! ```

pub mod advance;
pub mod drift;
mod error;
pub mod kepler;
pub mod vector;

pub use error::AnalyticError;
