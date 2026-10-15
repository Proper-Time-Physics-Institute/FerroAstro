// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro ephemeris constants (PHY-002, PHY-003, REF-002).
//!
//! * [`header440`]: parser for JPL `header.4xx` GROUP 1010/1040/1041, used at run time to load
//!   initial conditions and constants straight from the JPL file (REF-002).
//! * [`table`]: the versioned DE440 constants table, generated from `header.440` with a citation
//!   on every entry and the header's SHA-256 (PHY-002). It is embedded at build time.
//! * [`De440`]: the table's constants as typed quantities for the precision rungs.

pub mod de440;
mod error;
pub mod header440;
pub mod table;

pub use de440::{Body, De440};
pub use error::ConstantsError;
