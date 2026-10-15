// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-system
//!
//! The N-body system model (CRATE-PLAN L5; NUM-009, REF-002, NUM-018 SSB part):
//!
//! * [`Catalogue`]: bodies keyed by NAIF id with GMs from a constants set (the DE440 table, or a
//!   `header.440` read at run time including the asteroid masses `MAnnnn`) and a [`Tier`]
//!   (Major / Minor / Ring) per body;
//! * [`SystemState`]: structure-of-arrays states generic over [`ferroastro_real::Real`], typed by
//!   frame and centre; bodies go in and out as typed [`ferroastro_core::frames::State`]s;
//! * [`EarthMoon`]: the geocentric-Moon variables (Earth–Moon barycentre + Moon − Earth);
//! * [`barycentre`] / [`rebarycentre`]: Newtonian and relativistic (μ*, DE440) SSB conditions;
//! * [`InitialConditions`]: assembly from `header.440`, from an ephemeris at an epoch, or from
//!   supplied barycentric vectors (e.g. JPL Horizons);
//! * [`SmallBodyStates`], [`KBO_IDENTIFICATION`], [`KBO_RING`]: the REF-003 asteroid/KBO initial
//!   states sampled from `sb441-n373s.bsp` at JDEPOC, the identified KBO masses and the DE440 KBO
//!   ring (provisional, OI-24).
//!
//! ```
//! use ferroastro_constants::De440;
//! use ferroastro_system::{Catalogue, Tier};
//! use ferroastro_ephem::NaifId;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut cat = Catalogue::de440_major(&De440::load()?);
//! assert_eq!(cat.ids_in(Tier::Major).count(), 11);
//! cat.set_tier(NaifId::PLUTO_BARYCENTER, Tier::Ring)?;
//! assert_eq!(cat.get(NaifId::PLUTO_BARYCENTER)?.tier(), Tier::Ring);
//! # Ok(())
//! # }
//! ```

mod barycentre;
mod catalogue;
mod earth_moon;
mod error;
mod ic;
mod small_bodies;
mod state;
mod vec3;

pub use barycentre::{BarycentreOffset, MassModel, barycentre, mass_weights, rebarycentre};
pub use catalogue::{
    ASTEROID_NAIF_BASE, BodyEntry, Catalogue, DE440_MAJOR, MA_UNIDENTIFIED_FROM, Tier,
    UnidentifiedMass,
};
pub use earth_moon::EarthMoon;
pub use error::SystemError;
pub use ic::{HEADER_STATE_INTERPRETATION, HEADER_STATE_SUFFIXES, InitialConditions};
pub use small_bodies::{
    KBO_IDENTIFICATION, KBO_RING, KboIdentification, KboRing, SmallBodyRecord, SmallBodyStates,
    kbo_ring_states, place_small_bodies,
};
pub use state::{BodyState, SystemState};
