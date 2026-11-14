// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-abm
//!
//! Fixed-step Adams–Bashforth–Moulton integration (NUM-002), the "ephemeris mode" integrator of
//! FerroAstro (ADR-0008: IAS15 stays the default).
//!
//! * [`Abm`]: PECE and PECEC (P(EC)²) modes, order 8–14 (default 13), a DP8 start-up at `h/8`,
//!   Kahan-compensated or plain state update, dense output and event location (NUM-010), and
//!   bitwise checkpoints ([`AbmSnapshot`], CI-010). Generic over the scalar backend
//!   `R: Real` (DEP-007): coefficients, history sums and the state are all in `R`.
//! * [`rational`]: the Adams coefficients as exact rationals, computed (not transcribed) by
//!   integrating Lagrange basis polynomials over integer nodes.
//!
//! Sources (clean room, DEP-008): Hairer, Nørsett & Wanner 1993, *Solving Ordinary Differential
//! Equations I*, §III.1 and §II.10; Subbotin, Kodukov & Pavlov 2023, `CeMDA`,
//! doi:10.1007/s10569-023-10139-2 (configuration only).
//!
//! ```
//! use ferroastro_abm::{Abm, AbmConfig};
//! use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
//!
//! /// Unit harmonic oscillator ẍ = −x.
//! struct Oscillator;
//! impl SecondOrderSystem for Oscillator {
//!     fn dimension(&self) -> usize { 1 }
//!     fn acceleration(&self, _t: f64, x: &[f64], _v: &[f64], a: &mut [f64])
//!         -> Result<(), IntegratorError> {
//!         a[0] = -x[0];
//!         Ok(())
//!     }
//! }
//! # fn main() -> Result<(), IntegratorError> {
//! let steps = 200_u32;
//! let h = std::f64::consts::TAU / f64::from(steps);
//! let mut abm = Abm::new(1, 0.0, &[1.0], &[0.0], h, AbmConfig::default())?;
//! for _ in 0..steps {
//!     abm.step(&Oscillator)?;
//! }
//! assert!((abm.positions()[0] - 1.0).abs() < 1e-12);
//! # Ok(())
//! # }
//! ```

mod abm;
mod dp8;
#[expect(
    clippy::unreadable_literal,
    reason = "generated table of binary64 literals (tools/abm_dp8.py)"
)]
mod dp8_table;
pub mod rational;

pub use abm::{
    Abm, AbmConfig, AbmSnapshot, AbmStats, AbmStepReport, Corrector, DEFAULT_ORDER,
    DEFAULT_STARTUP_SUBSTEPS, MAX_ORDER, MIN_ORDER,
};
