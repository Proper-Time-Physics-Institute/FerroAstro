// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro ephemeris constants (PHY-002, PHY-003, REF-002).
//!
//! * [`header440`]: parser for JPL `header.4xx` GROUP 1010/1040/1041, used at run time to load
//!   initial conditions and constants straight from the JPL file (REF-002).

mod error;
pub mod header440;

pub use error::ConstantsError;
