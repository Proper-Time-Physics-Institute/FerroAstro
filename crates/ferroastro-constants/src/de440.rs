// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Typed DE440 constants for the precision rungs (PHY-002).
//!
//! [`De440::load`] reads the committed table ([`crate::table`]) once, checks the unit of every
//! entry it exposes, and converts to the [`ferroastro_core`] newtypes. GMs are stored in the
//! header in au³/d² and are converted to km³/s² with the table's own `AU` and the exact
//! 86 400 s Julian day. Entries tied to an open item (GMSDOT, OI-04; CENTER, OI-23) are not
//! exposed here.
//!
//! ```
//! use ferroastro_constants::De440;
//! # fn main() -> Result<(), ferroastro_constants::ConstantsError> {
//! let de = De440::load()?;
//! let c = de.speed_of_light();
//! assert!(c.km_per_s() > 0.0);
//! # Ok(())
//! # }
//! ```

use crate::ConstantsError;
use crate::table::Table;
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{AngularRate, GravParam, Length, Velocity};

/// A body whose GM the DE440 table provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Body {
    /// The Sun (`GMS`).
    Sun,
    /// Mercury (`GM1`).
    Mercury,
    /// Venus (`GM2`).
    Venus,
    /// The Earth-Moon system (`GMB`).
    EarthMoon,
    /// Earth alone, `GMB·EMRAT/(1+EMRAT)`.
    Earth,
    /// The Moon alone, `GMB/(1+EMRAT)`.
    Moon,
    /// The Mars system (`GM4`).
    Mars,
    /// The Jupiter system (`GM5`).
    Jupiter,
    /// The Saturn system (`GM6`).
    Saturn,
    /// The Uranus system (`GM7`).
    Uranus,
    /// The Neptune system (`GM8`).
    Neptune,
    /// The Pluto system (`GM9`).
    Pluto,
}

impl Body {
    /// Every body, Sun first.
    pub const ALL: [Body; 12] = [
        Body::Sun,
        Body::Mercury,
        Body::Venus,
        Body::EarthMoon,
        Body::Earth,
        Body::Moon,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Uranus,
        Body::Neptune,
        Body::Pluto,
    ];

    /// The header name of a directly tabulated GM, or `None` for Earth and Moon (derived).
    #[must_use]
    pub const fn header_name(self) -> Option<&'static str> {
        match self {
            Body::Sun => Some("GMS"),
            Body::Mercury => Some("GM1"),
            Body::Venus => Some("GM2"),
            Body::EarthMoon => Some("GMB"),
            Body::Mars => Some("GM4"),
            Body::Jupiter => Some("GM5"),
            Body::Saturn => Some("GM6"),
            Body::Uranus => Some("GM7"),
            Body::Neptune => Some("GM8"),
            Body::Pluto => Some("GM9"),
            Body::Earth | Body::Moon => None,
        }
    }
}

/// The DE440 constants used by the precision rungs, as typed quantities.
#[derive(Debug, Clone)]
pub struct De440 {
    table: Table,
    jd_epoch: f64,
    clight: Velocity,
    au: Length,
    beta: f64,
    gamma: f64,
    emrat: f64,
    asun: Length,
    j2sun: f64,
    moisun: f64,
    omgsun: AngularRate,
    /// GMs indexed by `Body as usize` (the order of [`Body::ALL`]).
    gms: [GravParam; 12],
}

fn value_in(table: &Table, name: &str, unit: &str) -> Result<f64, ConstantsError> {
    let e = table.get(name)?;
    if e.open_item.is_some() {
        return Err(ConstantsError::Table {
            problem: format!(
                "{name} is tied to open item {:?} and must not be used",
                e.open_item
            ),
        });
    }
    if e.unit == unit {
        Ok(e.value())
    } else {
        Err(ConstantsError::Table {
            problem: format!("{name} has unit {:?}, expected {unit:?}", e.unit),
        })
    }
}

fn core_err(name: &str, e: &ferroastro_core::Error) -> ConstantsError {
    ConstantsError::Table {
        problem: format!("{name}: {e}"),
    }
}

impl De440 {
    /// Load the committed DE440 table and check the units of every exposed entry.
    ///
    /// # Errors
    /// [`ConstantsError::Table`] if the table is malformed or a unit is not the expected one,
    /// [`ConstantsError::ConstantMissing`] if an entry is absent.
    pub fn load() -> Result<Self, ConstantsError> {
        Self::from_table(Table::de440()?)
    }

    /// Build from an already parsed table.
    ///
    /// # Errors
    /// As [`De440::load`].
    pub fn from_table(table: Table) -> Result<Self, ConstantsError> {
        let clight = Velocity::from_km_per_s(value_in(&table, "CLIGHT", "km/s")?)
            .map_err(|e| core_err("CLIGHT", &e))?;
        let au = Length::from_km(value_in(&table, "AU", "km")?).map_err(|e| core_err("AU", &e))?;
        let asun =
            Length::from_km(value_in(&table, "ASUN", "km")?).map_err(|e| core_err("ASUN", &e))?;
        let omgsun =
            AngularRate::from_rad_per_s(value_in(&table, "OMGSUN", "rad/d")? / SECONDS_PER_DAY)
                .map_err(|e| core_err("OMGSUN", &e))?;
        let de = Self {
            jd_epoch: value_in(&table, "JDEPOC", "d")?,
            clight,
            au,
            beta: value_in(&table, "BETA", "1")?,
            gamma: value_in(&table, "GAMMA", "1")?,
            emrat: value_in(&table, "EMRAT", "1")?,
            asun,
            j2sun: value_in(&table, "J2SUN", "1")?,
            moisun: value_in(&table, "MOISUN", "1")?,
            omgsun,
            gms: [GravParam::ZERO; 12],
            table,
        };
        let mut gms = [GravParam::ZERO; 12];
        for (slot, body) in gms.iter_mut().zip(Body::ALL) {
            *slot = de.convert_gm(body)?;
        }
        Ok(Self { gms, ..de })
    }

    fn convert_gm(&self, body: Body) -> Result<GravParam, ConstantsError> {
        let gm = |name: &str| value_in(&self.table, name, "au^3/d^2");
        let au = self.au.km();
        let scale = au * au * au / (SECONDS_PER_DAY * SECONDS_PER_DAY);
        let au3d2 = if let Some(name) = body.header_name() {
            gm(name)?
        } else {
            let frac = if body == Body::Earth {
                self.emrat / (1.0 + self.emrat)
            } else {
                1.0 / (1.0 + self.emrat)
            };
            gm("GMB")? * frac
        };
        GravParam::from_km3_per_s2(au3d2 * scale).map_err(|e| core_err("GM", &e))
    }

    /// GM of `body` in km³/s².
    #[must_use]
    #[expect(
        clippy::indexing_slicing,
        reason = "Body discriminants are 0..12, the order of Body::ALL and of `gms`"
    )]
    pub fn gm(&self, body: Body) -> GravParam {
        self.gms[body as usize]
    }

    /// GM of the Sun (`GMS`).
    #[must_use]
    pub fn gm_sun(&self) -> GravParam {
        self.gm(Body::Sun)
    }

    /// Epoch of the initial conditions (`JDEPOC`) on the TDB scale.
    ///
    /// # Errors
    /// Never in practice (the value is finite); [`ConstantsError::Table`] otherwise.
    pub fn initial_epoch(&self) -> Result<Epoch<Tdb>, ConstantsError> {
        Epoch::from_jd_parts(self.jd_epoch, 0.0).map_err(|e| core_err("JDEPOC", &e))
    }

    /// Speed of light (`CLIGHT`).
    #[must_use]
    pub const fn speed_of_light(&self) -> Velocity {
        self.clight
    }

    /// Astronomical unit (`AU`).
    #[must_use]
    pub const fn au(&self) -> Length {
        self.au
    }

    /// PPN β used to build DE440 (`BETA`).
    #[must_use]
    pub const fn ppn_beta(&self) -> f64 {
        self.beta
    }

    /// PPN γ used to build DE440 (`GAMMA`).
    #[must_use]
    pub const fn ppn_gamma(&self) -> f64 {
        self.gamma
    }

    /// Earth/Moon mass ratio (`EMRAT`).
    #[must_use]
    pub const fn earth_moon_mass_ratio(&self) -> f64 {
        self.emrat
    }

    /// Solar equatorial radius used as the J2 reference radius (`ASUN`).
    #[must_use]
    pub const fn sun_radius(&self) -> Length {
        self.asun
    }

    /// Solar J2 (`J2SUN`).
    #[must_use]
    pub const fn sun_j2(&self) -> f64 {
        self.j2sun
    }

    /// Solar polar moment of inertia factor C/(MR²) (`MOISUN`).
    #[must_use]
    pub const fn sun_moment_of_inertia(&self) -> f64 {
        self.moisun
    }

    /// Solar rotation rate (`OMGSUN`).
    #[must_use]
    pub const fn sun_rotation_rate(&self) -> AngularRate {
        self.omgsun
    }

    /// The underlying table (raw tokens, citations, provenance).
    #[must_use]
    pub const fn table(&self) -> &Table {
        &self.table
    }
}
