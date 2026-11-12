// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-oracle
//!
//! Oracle glue (CRATE-PLAN L11, `publish = false`; CI-003, CI-005, REF-005, LIC-001). The
//! reference codes run in their own pinned containers (`ci/oracles/<tool>/`, driven by
//! `tools/oracles/run_oracles.sh`) and return **plain files** plus a `MANIFEST.json`; this crate
//! only reads those files. Nothing here links, binds or calls a GPL oracle (LIC-001), and no
//! FerroAstro build ever enters an oracle container (red-team L4).
//!
//! * [`Manifest::verify_dir`]: every file of a golden directory has a complete CI-003 entry and
//!   its `output_sha256` matches; [`ManifestEntry::check_input`] ties an input (e.g.
//!   `header.440`) to its pin.
//! * [`GoldenStates`]: the state-vector CSV of the N-body runners.
//! * [`compare_states`]: largest position and velocity differences, per body, between a
//!   FerroAstro run and a golden at the same output times.
//!
//! Tolerances are not decided here: tests load them from `tests/vectors/` (ADR-0004).
//!
//! ```
//! use ferroastro_oracle::{GoldenStates, sha256_hex};
//! let text = "# epoch_jd_tdb: 2440400.5\nt_days,naif,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s\n\
//!             0.0,10,1.0,2.0,3.0,0.1,0.2,0.3\n";
//! let g = GoldenStates::parse("demo.csv", text).unwrap();
//! assert_eq!(g.rows.len(), 1);
//! assert_eq!(g.meta_f64("epoch_jd_tdb").unwrap(), 2_440_400.5);
//! assert_eq!(sha256_hex(b"").len(), 64);
//! ```

mod compare;
mod error;
mod manifest;
mod states;

pub use compare::{BodyDifference, StateComparison, TIME_MATCH_S, compare_states};
pub use error::OracleError;
pub use manifest::{MANIFEST_NAME, MANIFEST_SCHEMA_VERSION, Manifest, ManifestEntry, sha256_hex};
pub use states::{GoldenState, GoldenStates, STATE_COLUMNS};
