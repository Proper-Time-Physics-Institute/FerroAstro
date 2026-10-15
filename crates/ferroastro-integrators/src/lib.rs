// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro numerical integrators (NUM-001, NUM-023).
//!
//! * [`Ias15`]: IAS15-class adaptive 15th-order Gauss–Radau integrator with compensated summation,
//!   clean-room from Everhart 1985 and Rein & Spiegel 2015 (DEP-008). It is the default for all
//!   rungs (ADR-0008) and accepts velocity-dependent forces.
//! * [`diagnostics`]: least-squares slope fits used for the Brouwer-law round-off diagnostics
//!   (NUM-023) and by the precession estimators.
//! * [`compensated`]: Kahan summation.
//!
//! Every integrator type is generic over the scalar backend `R: Real` (`ferroastro-real`; DEP-007,
//! NUM-006), with `R = f64` as the default type parameter: `Ias15` is `Ias15<f64>`, and
//! `Ias15<DoubleDouble>` integrates the same system in double-double.
//!
//! ```
//! use ferroastro_integrators::{Ias15, Ias15Config, IntegratorError, SecondOrderSystem};
//!
//! /// Unit harmonic oscillator ẍ = −x.
//! struct Oscillator;
//! impl SecondOrderSystem for Oscillator {
//!     fn dimension(&self) -> usize { 1 }
//!     fn acceleration(&self, _t: f64, x: &[f64], _v: &[f64], a: &mut [f64])
//!         -> Result<(), IntegratorError> {
//!         for (ai, xi) in a.iter_mut().zip(x) { *ai = -xi; }
//!         Ok(())
//!     }
//! }
//! # fn main() -> Result<(), IntegratorError> {
//! let mut ias = Ias15::new(1, 0.0, &[1.0], &[0.0], 0.1, Ias15Config::default())?;
//! let t_end = std::f64::consts::TAU;
//! ias.integrate_to(&Oscillator, t_end, |_, _| Ok(()))?;
//! assert!((ias.positions()[0] - 1.0).abs() < 1e-13);
//! # Ok(())
//! # }
//! ```

pub mod compensated;
pub mod diagnostics;
mod error;
mod ias15;
mod radau;

pub use error::IntegratorError;
pub use ias15::{Ias15, Ias15Config, Ias15Stats, SecondOrderSystem, StepReport};
