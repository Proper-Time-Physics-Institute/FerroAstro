// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Einstein–Infeld–Hoffmann (EIH) 1PN N-body accelerations with PPN β and γ (M4, WP19).
//!
//! **Clean-room (DEP-008, AGENTS.md rule 7).** Everything here is transcribed from the typeset PDF
//! of Park, Folkner, Williams & Boggs 2021, *The JPL Planetary and Lunar Ephemerides DE440 and
//! DE441*, AJ 161, 105: Eq. 27 (§3.1, the point-mass acceleration, "Will & Nordtvedt 1972; Moyer
//! 2003") and Eqs. 1–2 (§2.2, the solar-system barycentre). No REBOUND, `REBOUNDx`, ASSIST,
//! Swiftest or Mercury6 source was read. The term-by-term transcription for the independent
//! second check (NUM-020, open item OI-33) is `TRANSCRIPTION.md` in this crate; it was signed off
//! by the maintainer on 2026-10-21 (OI-33 closed, pinned `m4-eih` `78d5a4c`). WP22 changed only the
//! sets and the order of Eq. 27's sums in `eih.rs` (decision D1), not its terms; the NUM-020 notes
//! require the affected rows to be re-checked against that diff.
//!
//! # Contents
//! * [`Eih`]: the c⁻² part of Eq. 27 as an N-body [`ferroastro_forces::ForceTerm`], for every
//!   moving body over the tiered interaction graph (WP22, decision D1). The Newtonian
//!   part stays in [`ferroastro_forces::NewtonianMutual`], so an EIH model is `NewtonianMutual` +
//!   `Eih` (scenario switch `gr_eih`). The right-hand-side accelerations `a_j` are Newtonian by
//!   default, or one fixed-point iteration ([`RhsAcceleration`], NUM-018).
//! * [`HeliocentricSchwarzschild`]: the Sun-only 1PN rung in N-body mode (scenario switch
//!   `gr_1pn_central` with `mode = "n-body"`): the R3 test-particle correction of the Sun's field
//!   applied to every other Major body, without reaction.
//! * [`mu_star`] and [`relativistic_barycentre`]: Eqs. 1–2 (NUM-018); [`mu_star_rate`] and
//!   [`relativistic_barycentre_with_rate`]: the barycentre velocity condition with dμ*/dt
//!   (DE430 Eq. 3, Subbotin et al. 2023 Eq. 5; OI-75 decision of 2026-10-21), taken over from
//!   `m5-performance` (WP25) and re-verified in WP22.
//!
//! # Gauge, time and units (NUM-017, AGENTS.md rule 4)
//! Both terms declare [`ferroastro_forces::Gauge::PpnIsotropicHarmonic`]. The time argument is
//! TDB and the GMs are the DE440 (TDB-compatible) values. Units are the N-body model's: km, km/s,
//! TDB seconds, km³/s².

mod barycentre;
mod central;
mod eih;

pub use barycentre::{
    BarycentreBody, mu_star, mu_star_rate, relativistic_barycentre,
    relativistic_barycentre_with_rate,
};
pub use central::HeliocentricSchwarzschild;
pub use eih::{Eih, RhsAcceleration};
