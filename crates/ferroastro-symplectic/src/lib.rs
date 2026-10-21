// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-symplectic
//!
//! The Wisdom–Holman / `WHFast`-type symplectic integrator (NUM-004, WP29): the map in Jacobi
//! coordinates (Wisdom & Holman 1991), Kepler drifts in universal variables
//! (`ferroastro_analytic::drift`), interaction kicks from the inertial accelerations of a
//! `ferroastro-forces` model, and symplectic correctors of order 3, 5, 7 and 11 whose
//! coefficients are derived in `tools/symplectic_correctors.py` (IMPLEMENTATION-NOTES WP29).
//!
//! * [`Whfast`]: fixed step, f64 or double-double, dense output and event location (NUM-010),
//!   bitwise checkpoints ([`WhSnapshot`], CI-010).
//! * [`check_model`]: refuses models the map cannot integrate.
//!
//! **Supported dynamics:** Newtonian point masses, solar J2 with reaction, and the
//! velocity-independent GR effective potential (`ferroastro_forces::GrPotential`, Nobili &
//! Roxburgh 1986): the Newtonian and GR-potential rungs. **Full EIH is not supported under the
//! Wisdom–Holman map** (NUM-004), nor any other velocity-dependent term (1PN central, solar
//! Lense–Thirring): with a velocity-dependent kick the map is neither symplectic nor of its
//! stated order. The Saha & Tremaine 1994 pseudo-velocity 1PN extension (NUM-005) is not
//! implemented: its `H_PN` coefficients are unverified (OI-34).
//!
//! Clean-room (DEP-008): from Wisdom & Holman 1991, Rein & Tamayo 2015 (arXiv:1506.01084),
//! Danby 1988 and Conway 1986 only; no REBOUND, `WHFast`, Swiftest or Mercury6 source was
//! consulted.

#[rustfmt::skip]
#[expect(
    clippy::unreadable_literal,
    reason = "generated table of exact rationals (tools/symplectic_correctors.py)"
)]
mod corrector_table;
mod wh;

pub use wh::{
    CorrectorOrder, SUPPORTED_TERMS, WhConfig, WhSnapshot, WhStats, WhStepReport, Whfast,
    check_model,
};
