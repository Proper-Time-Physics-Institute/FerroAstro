// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro foundation types (ADR-0001, API-001, API-002).
//!
//! * [`units`]: quantity newtypes (`Length`, `Velocity`, `TimeSpan`, `GravParam`, `Angle`, `AngularRate`) whose
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
//! Every newtype is generic over the scalar backend `R: Real` (`ferroastro-real`; ADR-0001,
//! ADR-0002) with `R = f64` as the default type parameter, so `Length` means `Length<f64>` and
//! M0 call sites compile unchanged. The same mismatch classes fail to compile with the
//! double-double backend, and mixing backends is a further mismatch class (API-001):
//!
//! ```
//! use ferroastro_core::units::{Length, TimeSpan, Velocity};
//! use ferroastro_real::{DoubleDouble, Real};
//! # fn main() -> Result<(), ferroastro_core::Error> {
//! let d: Length<DoubleDouble> = Length::from_km(DoubleDouble::from_f64(1.0))?;
//! let t = TimeSpan::from_seconds(DoubleDouble::from_f64(3.0))?;
//! let v: Velocity<DoubleDouble> = d / t; // 1/3 km/s to ~32 digits
//! assert!(v.km_per_s().lo() != 0.0);
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod frames;
pub mod time;
pub mod units;

pub use error::{Error, ErrorCode};
