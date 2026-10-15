// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro foundation types (ADR-0001, API-001, API-002).
//!
//! * [`units`]: quantity newtypes (`Length`, `Velocity`, `TimeSpan`, `GravParam`, `Angle`) whose
//!   arithmetic only combines dimensionally compatible values.
//! * [`frames`]: reference-frame and centre markers, and frame/centre-tagged positions.
//! * [`time`]: time-scale markers and the two-part Julian date [`time::Epoch`].
//! * [`error`]: the crate-wide [`Error`] type with stable `FA-Exxxx` codes.
//!
//! Mixing frames, centres, time scales or units is a compile error, which the `trybuild` suite in
//! `tests/compile_fail.rs` proves (API-001):
//!
//! ```
//! use ferroastro_core::units::{Length, TimeSpan, Velocity};
//! # fn main() -> Result<(), ferroastro_core::Error> {
//! let d = Length::from_km(3.0)?;
//! let t = TimeSpan::from_seconds(2.0)?;
//! let v: Velocity = d / t;
//! assert!((v.km_per_s() - 1.5).abs() < 1e-15);
//! # Ok(())
//! # }
//! ```
//!
//! M0 scope: the scalar is `f64`. ADR-0001 asks for newtypes generic over the `Real` trait, which
//! arrives with `ferroastro-real` in M1 (see `docs/strategy/IMPLEMENTATION-NOTES.md`, WP1).

pub mod error;
pub mod frames;
pub mod time;
pub mod units;

pub use error::{Error, ErrorCode};
