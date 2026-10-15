// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The body catalogue: NAIF id, name, GM from a constants set, and the tier (NUM-009).
//!
//! GMs come from a constants set, never from source literals (REF-002):
//! [`Catalogue::de440_major`] from the committed DE440 table ([`De440`]), and
//! [`Catalogue::from_header`] from a parsed `header.440` at run time, which adds the asteroid
//! masses (`MAnnnn`). Both convert au³/d² to km³/s² with the header `AU` and the exact 86 400 s
//! day, and split the Earth–Moon GM with `EMRAT` exactly as [`De440`] does, so the two sets agree
//! bit for bit on the bodies they share.

use crate::SystemError;
use ferroastro_constants::header440::Header;
use ferroastro_constants::{Body, De440};
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::GravParam;
use ferroastro_ephem::NaifId;

/// How a body interacts (NUM-009, Subbotin et al. 2023; Folkner et al. 2014).
///
/// * `Major`: attracts and is attracted by every other Major body (EIH in the precision rungs);
/// * `Minor`: attracted by the Majors only (Newtonian) and does not interact with other Minor
///   bodies (NUM-009). NUM-009 does not say whether a Minor body attracts the Majors; the
///   asteroid row of the ladder (N-AST in PHY-013) is the asteroids' pull on Mercury, so the
///   force model (WP13/WP15) must keep that reaction. Not decided here (no forces in this crate);
/// * `Ring`: a fixed point mass (attracts, does not move).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Mutually interacting body.
    Major,
    /// Test-particle-like body attracted by the Majors only.
    Minor,
    /// Fixed point mass.
    Ring,
}

/// One body of a catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyEntry {
    id: NaifId,
    name: String,
    gm: GravParam,
    tier: Tier,
    gm_source: String,
}

impl BodyEntry {
    pub(crate) fn new(id: NaifId, name: &str, gm: GravParam, tier: Tier, gm_source: &str) -> Self {
        Self {
            id,
            name: name.to_owned(),
            gm,
            tier,
            gm_source: gm_source.to_owned(),
        }
    }

    /// NAIF id.
    #[must_use]
    pub const fn id(&self) -> NaifId {
        self.id
    }

    /// Display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// GM in km³/s².
    #[must_use]
    pub const fn gm(&self) -> GravParam {
        self.gm
    }

    /// Tier.
    #[must_use]
    pub const fn tier(&self) -> Tier {
        self.tier
    }

    /// Where the GM comes from (header symbol or derivation), for reports.
    #[must_use]
    pub fn gm_source(&self) -> &str {
        &self.gm_source
    }
}

/// A mass in the constants set that no NAIF id is attached to here (e.g. header.440
/// `MA8001…MA8236`; their identity is an open item, OI-24). Kept for reports; never placed unless
/// promoted by [`Catalogue::with_identified_kbos`] or [`Catalogue::with_kbo_ring`].
#[derive(Debug, Clone, PartialEq)]
pub struct UnidentifiedMass {
    /// Header symbol.
    pub symbol: String,
    /// GM in km³/s².
    pub gm: GravParam,
}

/// NAIF ids of the DE440 major bodies, in the catalogue's order, with names. The planet ids are
/// system barycentres (1, 2, 4–9): DE integrates the planetary systems as points and GM1…GM9 are
/// system GMs; for Mercury and Venus the barycentre is the planet.
pub const DE440_MAJOR: [(NaifId, &str); 11] = [
    (NaifId::SUN, "Sun"),
    (NaifId::MERCURY_BARYCENTER, "Mercury"),
    (NaifId::VENUS_BARYCENTER, "Venus"),
    (NaifId::EARTH, "Earth"),
    (NaifId::MOON, "Moon"),
    (NaifId::MARS_BARYCENTER, "Mars system"),
    (NaifId::JUPITER_BARYCENTER, "Jupiter system"),
    (NaifId::SATURN_BARYCENTER, "Saturn system"),
    (NaifId::URANUS_BARYCENTER, "Uranus system"),
    (NaifId::NEPTUNE_BARYCENTER, "Neptune system"),
    (NaifId::PLUTO_BARYCENTER, "Pluto system"),
];

/// The [`Body`] of the DE440 table for a major-body NAIF id.
fn table_body(id: NaifId) -> Option<Body> {
    Some(match id.0 {
        10 => Body::Sun,
        1 => Body::Mercury,
        2 => Body::Venus,
        399 => Body::Earth,
        301 => Body::Moon,
        4 => Body::Mars,
        5 => Body::Jupiter,
        6 => Body::Saturn,
        7 => Body::Uranus,
        8 => Body::Neptune,
        9 => Body::Pluto,
        _ => return None,
    })
}

/// NAIF ids of numbered asteroids start here (`2 000 000 + number`).
pub const ASTEROID_NAIF_BASE: i32 = 2_000_000;

/// Header `MAnnnn` numbers from here on are not asteroid numbers that this crate can identify.
pub const MA_UNIDENTIFIED_FROM: i32 = 8000;

/// A body catalogue tied to one constants set.
#[derive(Debug, Clone, PartialEq)]
pub struct Catalogue {
    pub(crate) bodies: Vec<BodyEntry>,
    pub(crate) unidentified: Vec<UnidentifiedMass>,
    constants: String,
}

/// `GM[au³/d²] → km³/s²`, in the operation order of [`De440`] (bit-identical results).
fn au3d2_to_km3s2(au3d2: f64, au_km: f64) -> f64 {
    let scale = au_km * au_km * au_km / (SECONDS_PER_DAY * SECONDS_PER_DAY);
    au3d2 * scale
}

fn gm_value(name: &str, km3s2: f64) -> Result<GravParam, SystemError> {
    GravParam::from_km3_per_s2(km3s2).map_err(|_| SystemError::non_finite(name, km3s2))
}

impl Catalogue {
    /// The 11 DE440 major bodies (Sun, Mercury, Venus, Earth, Moon, Mars…Pluto systems), GMs
    /// from the committed DE440 table, all [`Tier::Major`].
    #[must_use]
    pub fn de440_major(de: &De440) -> Self {
        let bodies = DE440_MAJOR
            .iter()
            .filter_map(|&(id, name)| {
                let body = table_body(id)?;
                Some(BodyEntry {
                    id,
                    name: name.to_owned(),
                    gm: de.gm(body),
                    tier: Tier::Major,
                    gm_source: body.header_name().map_or_else(
                        || {
                            if body == Body::Earth {
                                "GMB*EMRAT/(1+EMRAT)".to_owned()
                            } else {
                                "GMB/(1+EMRAT)".to_owned()
                            }
                        },
                        str::to_owned,
                    ),
                })
            })
            .collect();
        Self {
            bodies,
            unidentified: Vec::new(),
            constants: "DE440 committed table (ferroastro-constants, generated from header.440)"
                .to_owned(),
        }
    }

    /// The major bodies plus every asteroid mass of a parsed `header.440` (`MAnnnn`, nnnn below
    /// [`MA_UNIDENTIFIED_FROM`], as NAIF `2 000 000 + nnnn`, [`Tier::Minor`]). Entries from
    /// [`MA_UNIDENTIFIED_FROM`] on are kept as [`UnidentifiedMass`]es. `provenance` describes the
    /// header (e.g. its SHA-256) and is recorded in [`Catalogue::constants`].
    ///
    /// # Errors
    /// [`SystemError::Constants`] if a symbol is missing; [`SystemError::InitialConditions`] for a
    /// non-finite GM or a malformed `MA` symbol.
    pub fn from_header(header: &Header, provenance: &str) -> Result<Self, SystemError> {
        let au = header.value("AU")?;
        let emrat = header.value("EMRAT")?;
        let mut bodies = Vec::new();
        for &(id, name) in &DE440_MAJOR {
            let (au3d2, source) = match id {
                NaifId::EARTH => (
                    header.value("GMB")? * (emrat / (1.0 + emrat)),
                    "GMB*EMRAT/(1+EMRAT)".to_owned(),
                ),
                NaifId::MOON => (
                    header.value("GMB")? * (1.0 / (1.0 + emrat)),
                    "GMB/(1+EMRAT)".to_owned(),
                ),
                NaifId::SUN => (header.value("GMS")?, "GMS".to_owned()),
                NaifId(n) => {
                    let symbol = format!("GM{n}");
                    (header.value(&symbol)?, symbol)
                }
            };
            bodies.push(BodyEntry {
                id,
                name: name.to_owned(),
                gm: gm_value(&source, au3d2_to_km3s2(au3d2, au))?,
                tier: Tier::Major,
                gm_source: source,
            });
        }
        let mut unidentified = Vec::new();
        for c in header
            .constants()
            .iter()
            .filter(|c| c.name.starts_with("MA"))
        {
            let number: i32 = c.name[2..]
                .parse()
                .map_err(|_| SystemError::InitialConditions {
                    quantity: c.name.clone(),
                    value: c.token.clone(),
                    valid: "MAnnnn with a decimal asteroid number",
                    fix: "use the unmodified JPL header.440",
                })?;
            let gm = gm_value(&c.name, au3d2_to_km3s2(c.value, au))?;
            if number >= MA_UNIDENTIFIED_FROM {
                unidentified.push(UnidentifiedMass {
                    symbol: c.name.clone(),
                    gm,
                });
            } else {
                bodies.push(BodyEntry {
                    id: NaifId(ASTEROID_NAIF_BASE + number),
                    name: format!("({number})"),
                    gm,
                    tier: Tier::Minor,
                    gm_source: c.name.clone(),
                });
            }
        }
        Ok(Self {
            bodies,
            unidentified,
            constants: format!("header.440 GROUP 1041 at run time ({provenance})"),
        })
    }

    /// Every body, in catalogue order.
    #[must_use]
    pub fn bodies(&self) -> &[BodyEntry] {
        &self.bodies
    }

    /// Masses without a NAIF identification (never placed in a state).
    #[must_use]
    pub fn unidentified_masses(&self) -> &[UnidentifiedMass] {
        &self.unidentified
    }

    /// The constants set the GMs come from.
    #[must_use]
    pub fn constants(&self) -> &str {
        &self.constants
    }

    /// The entry of `id`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is not in the catalogue.
    pub fn get(&self, id: NaifId) -> Result<&BodyEntry, SystemError> {
        self.bodies
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| SystemError::unknown(id.0))
    }

    /// GM of `id`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is not in the catalogue.
    pub fn gm(&self, id: NaifId) -> Result<GravParam, SystemError> {
        self.get(id).map(BodyEntry::gm)
    }

    /// Assign `tier` to body `id`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is not in the catalogue.
    pub fn set_tier(&mut self, id: NaifId, tier: Tier) -> Result<(), SystemError> {
        let b = self
            .bodies
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or_else(|| SystemError::unknown(id.0))?;
        b.tier = tier;
        Ok(())
    }

    /// The ids of the bodies in `tier`, in catalogue order.
    pub fn ids_in(&self, tier: Tier) -> impl Iterator<Item = NaifId> + '_ {
        self.bodies
            .iter()
            .filter(move |b| b.tier == tier)
            .map(BodyEntry::id)
    }

    /// Keep only the bodies for which `keep` is true.
    pub fn retain(&mut self, mut keep: impl FnMut(&BodyEntry) -> bool) {
        self.bodies.retain(|b| keep(b));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: NUM-009
    #[test]
    fn de440_major_bodies_are_major_and_tiers_are_assignable() {
        let de = De440::load().unwrap();
        let mut cat = Catalogue::de440_major(&de);
        assert_eq!(cat.bodies().len(), DE440_MAJOR.len());
        assert_eq!(cat.ids_in(Tier::Major).count(), DE440_MAJOR.len());
        assert_eq!(cat.gm(NaifId::SUN).unwrap(), de.gm_sun());
        cat.set_tier(NaifId::PLUTO_BARYCENTER, Tier::Ring).unwrap();
        assert_eq!(
            cat.ids_in(Tier::Ring).collect::<Vec<_>>(),
            [NaifId::PLUTO_BARYCENTER]
        );
        let err = cat.set_tier(NaifId(2_000_001), Tier::Minor).unwrap_err(); // lint: allow-literal(NAIF id of Ceres)
        assert_eq!(err.code(), ferroastro_core::ErrorCode::SystemBody);
        assert!(err.to_string().starts_with("FA-E1100"));
    }
}
