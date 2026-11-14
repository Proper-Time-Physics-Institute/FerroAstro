// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! FerroAstro ephemeris constants (PHY-002, PHY-003, REF-002).
//!
//! * [`header440`]: parser for JPL `header.4xx` GROUP 1010/1040/1041, used at run time to load
//!   initial conditions and constants straight from the JPL file (REF-002).
//! * [`table`]: the committed DE440 constants **metadata** (names, units, citations and the
//!   pinned SHA-256 of `header.440`; no values, #22), joined at run time with the header.
//! * [`De440`]: the header's constants as typed quantities for the precision rungs. Every
//!   constructor verifies the header's SHA-256 against the pin (FA-E0304).
//! * [`nominal`]: IAU 2015 B3 nominal conversion constants, quarantined (PHY-003). Ladder code
//!   must not use them; a test enforces it.
//!
//! # Features
//!
//! * `fs` (default): `De440::load_cached` reads the pinned `header.440` from the FerroAstro
//!   data cache through `ferroastro-testdata` (FA-E0303 when it is absent). Off for wasm32,
//!   where only [`De440::from_header_bytes`] is available.
//! * `embed-header` (opt-in): `build.rs` includes a fetched `header.440` from `$FA_DATA_DIR` and
//!   `De440::embedded` reads it. The file is never committed and the feature is never enabled
//!   for published builds; the build fails with FA-E0303 when the file is absent.
//! * `unchecked-header` (tests only): `De440::from_header_bytes_unchecked` for synthetic
//!   headers.

pub mod de440;
mod error;
pub mod header440;
pub mod nominal;
pub mod table;

pub use de440::{Body, De440};
pub use error::ConstantsError;
