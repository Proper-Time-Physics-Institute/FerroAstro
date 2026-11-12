// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! In-crate JPL DAF/SPK reader (DEP-011, REF-004, DEP-007).
//!
//! * [`Daf`]: the NAIF Double Precision Array File container, both byte orders, parsed from a
//!   byte slice — no mmap, no `unsafe`, every read bounds-checked (API-002).
//! * [`Spk`]: SPK segments; Chebyshev **type 2** (position, velocity by differentiation) and
//!   **type 3** (position and velocity) evaluated by the Clenshaw recurrence, generic over
//!   [`ferroastro_real::Real`] (f64, double-double, f128). The two-part TDB date is split into
//!   record and offset like jplephem 2.24, the REF-004 reference reader.
//! * [`Ephemeris`]: typed states (`State<Icrf, C, R>`) of any body relative to any centre,
//!   chaining segments through their lowest common ancestor.
//! * [`parse_testpo`]: JPL `testpo.4xx` test points (REF-001).
//!
//! Data are supplied by the caller (fetch-on-demand, DEP-017); nothing is bundled.
//!
//! ```
//! use ferroastro_ephem::{Ephemeris, NaifId, Spk};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Bytes from a file the caller fetched and verified (here: not a DAF at all).
//! let bytes = vec![0u8; 2048];
//! let err = Spk::parse(&bytes).unwrap_err();
//! assert!(err.to_string().starts_with("FA-E1000"));
//! # let _ = NaifId::MERCURY;
//! # Ok(()) }
//! ```

mod cheb;
mod daf;
mod ephemeris;
mod error;
mod spk;
mod testpo;

pub use cheb::{clenshaw, clenshaw_with_derivative};
pub use daf::{Daf, Endian, FTPSTR, RECORD_BYTES, Summary, WORD_BYTES};
pub use ephemeris::{Ephemeris, NAIF_FRAME_J2000, NaifCenter, NaifId};
pub use error::EphemError;
pub use spk::{
    ChebyshevLayout, J2000_JD, SECONDS_PER_DAY, Segment, Spk, SpkType, StateKm, et_seconds,
};
pub use testpo::{TestPoint, naif_id_of_testeph_code, parse_testpo};
