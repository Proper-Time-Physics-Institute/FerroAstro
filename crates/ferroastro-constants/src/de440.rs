// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Typed DE440 constants for the precision rungs (PHY-002, REF-002).
//!
//! The values are read **at run time** from the JPL `header.440` file; the repository commits
//! only metadata (names, units, citations and the pinned SHA-256, [`crate::table`]), never a
//! value (maintainer decision of 2026-11-14, #22). Every constructor checks the SHA-256 of the
//! header against the pin of `data/de440-metadata.toml`, which a test keeps equal to
//! `tests/data/MANIFEST.toml` (FA-E0304 on a mismatch):
//!
//! * [`De440::from_header_bytes`]: the bytes of the file, from anywhere (core API; also wasm32,
//!   e.g. bytes handed over from JavaScript);
//! * [`De440::from_header`]: an already parsed [`Header`] (its [`Header::sha256`] is checked);
//! * `De440::load_cached` (default feature `fs`): the pinned file in the FerroAstro data cache
//!   (`$FA_DATA_DIR`, filled by `tools/fetch_jpl.py`; FA-E0303 when absent);
//! * `De440::embedded` (opt-in feature `embed-header`): the file included at build time from
//!   `$FA_DATA_DIR`; never enabled for published builds.
//!
//! The unit of every exposed entry is checked, and the values are converted to the
//! [`ferroastro_core`] newtypes. GMs are stored in the header in au³/d² and are converted to
//! km³/s² with the header's own `AU` and the exact 86 400 s Julian day. Entries tied to an open
//! item (GMSDOT, OI-04; CENTER, OI-23) are not exposed here.
//!
//! ```
//! use ferroastro_constants::De440;
//! use ferroastro_core::ErrorCode;
//! # fn main() -> Result<(), ferroastro_constants::ConstantsError> {
//! // Anything but the pinned JPL file is refused before it is parsed.
//! let err = De440::from_header_bytes(b"GROUP   1040\n").unwrap_err();
//! assert_eq!(err.code(), ErrorCode::HeaderChecksum);
//! # #[cfg(feature = "fs")]
//! # {
//! // With the pinned header in the data cache (`tools/fetch_jpl.py`):
//! if let Ok(de) = De440::load_cached() {
//!     assert!(de.speed_of_light().km_per_s() > 0.0);
//! }
//! # }
//! # Ok(())
//! # }
//! ```

use crate::ConstantsError;
use crate::header440::{Header, sha256_hex};
use crate::table::{Metadata, Table};
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{AngularRate, GravParam, Length, Velocity};

/// Environment variable that turns a missing header into an error for
/// `De440::load_cached_or_skip` (also implied by `CI=true`).
pub const REQUIRE_DATA_ENV: &str = "FA_REQUIRE_DATA";

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
    /// The constants of the pinned `header.440`, from the file's bytes.
    ///
    /// # Errors
    /// [`ConstantsError::HeaderChecksum`] unless the bytes are the pinned file;
    /// [`ConstantsError::HeaderParse`] if they are not UTF-8 or not a header;
    /// [`ConstantsError::Table`] or [`ConstantsError::ConstantMissing`] if the metadata does not
    /// fit the header.
    pub fn from_header_bytes(bytes: &[u8]) -> Result<Self, ConstantsError> {
        let metadata = Metadata::de440()?;
        metadata.verify_sha256(&sha256_hex(bytes))?;
        let header = parse_bytes(bytes)?;
        Self::from_table(Table::from_header(&metadata, &header)?)
    }

    /// The constants of an already parsed `header.440`; its [`Header::sha256`] must be the pin.
    ///
    /// # Errors
    /// As [`De440::from_header_bytes`].
    pub fn from_header(header: &Header) -> Result<Self, ConstantsError> {
        Self::from_table(Table::from_header(&Metadata::de440()?, header)?)
    }

    /// **Tests only.** [`De440::from_header_bytes`] without the SHA-256 check, for synthetic
    /// headers. Never use it for physics: its values are whatever the bytes say.
    ///
    /// # Errors
    /// As [`De440::from_header_bytes`], except [`ConstantsError::HeaderChecksum`].
    #[cfg(any(test, feature = "unchecked-header"))]
    pub fn from_header_bytes_unchecked(bytes: &[u8]) -> Result<Self, ConstantsError> {
        let header = parse_bytes(bytes)?;
        Self::from_table(Table::from_header_unverified(&Metadata::de440()?, &header)?)
    }

    /// The constants of the pinned `header.440` in the FerroAstro data cache (`$FA_DATA_DIR`,
    /// else `$CARGO_TARGET_DIR/fa-data`, else `<workspace>/target/fa-data`; filled by
    /// `(cd tools && uv run python fetch_jpl.py de440-header)`).
    ///
    /// # Errors
    /// [`ConstantsError::HeaderNotFound`] (FA-E0303) if the file is absent or unreadable;
    /// [`ConstantsError::HeaderChecksum`] (FA-E0304) if it is not the pinned file; otherwise as
    /// [`De440::from_header_bytes`].
    #[cfg(feature = "fs")]
    pub fn load_cached() -> Result<Self, ConstantsError> {
        let (path, bytes) = cached_header_bytes()?;
        Self::from_header_bytes(&bytes).map_err(|e| match e {
            ConstantsError::HeaderChecksum { expected, got } => ConstantsError::HeaderChecksum {
                expected,
                got: format!("{got} ({path})"),
            },
            other => other,
        })
    }

    /// **Test gate.** `De440::load_cached`, but a missing header (FA-E0303) prints
    /// `SKIP <context>: …` and gives `Ok(None)`, so a test can return early on a machine without
    /// the data cache. When `CI=true` or [`REQUIRE_DATA_ENV`] is set (and not `0`/`false`), a
    /// missing header is an error instead: CI never passes a test it did not run. A header with
    /// the wrong SHA-256 is always an error.
    ///
    /// # Errors
    /// As `De440::load_cached`, except a missing header outside CI.
    #[cfg(feature = "fs")]
    pub fn load_cached_or_skip(context: &str) -> Result<Option<Self>, ConstantsError> {
        match Self::load_cached() {
            Ok(de) => Ok(Some(de)),
            Err(e @ ConstantsError::HeaderNotFound { .. }) if !data_required() => {
                println!("SKIP {context}: {e}");
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// The constants of the `header.440` included at build time (feature `embed-header`).
    ///
    /// # Errors
    /// As [`De440::from_header_bytes`].
    #[cfg(feature = "embed-header")]
    pub fn embedded() -> Result<Self, ConstantsError> {
        Self::from_header_bytes(crate::header440::EMBEDDED)
    }

    fn from_table(table: Table) -> Result<Self, ConstantsError> {
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

    /// The underlying table (run-time tokens, metadata, citations, provenance).
    #[must_use]
    pub const fn table(&self) -> &Table {
        &self.table
    }
}

fn parse_bytes(bytes: &[u8]) -> Result<Header, ConstantsError> {
    let text = std::str::from_utf8(bytes).map_err(|e| ConstantsError::HeaderParse {
        problem: format!("not UTF-8: {e}"),
    })?;
    Header::parse(text)
}

/// True when a missing header must fail rather than skip (`CI=true` or `FA_REQUIRE_DATA`).
#[cfg(feature = "fs")]
fn data_required() -> bool {
    let set = |name: &str| {
        std::env::var(name).is_ok_and(|v| {
            let v = v.trim();
            !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false")
        })
    };
    set(REQUIRE_DATA_ENV) || set("CI")
}

/// Path and bytes of the cached `de440-header` product (no network, no checksum yet).
#[cfg(feature = "fs")]
fn cached_header_bytes() -> Result<(String, Vec<u8>), ConstantsError> {
    let product = ferroastro_testdata::product("de440-header").map_err(|e| {
        ConstantsError::HeaderNotFound {
            path: "tests/data/MANIFEST.toml".to_owned(),
            problem: e.to_string(),
        }
    })?;
    let path = ferroastro_testdata::data_dir().join(&product.file);
    let shown = path.display().to_string();
    let bytes = std::fs::read(&path).map_err(|e| ConstantsError::HeaderNotFound {
        path: shown.clone(),
        problem: e.to_string(),
    })?;
    Ok((shown, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::tests::synthetic_header;
    use ferroastro_core::ErrorCode;

    /// Verifies: PHY-002, REF-002
    #[test]
    fn every_constructor_checks_the_pin() {
        let text = synthetic_header(&[]);
        let err = De440::from_header_bytes(text.as_bytes()).unwrap_err();
        assert_eq!(err.code(), ErrorCode::HeaderChecksum, "{err}");
        assert!(err.to_string().starts_with("FA-E0304"));
        let h = Header::parse(&text).unwrap();
        let err = De440::from_header(&h).unwrap_err();
        assert_eq!(err.code(), ErrorCode::HeaderChecksum, "{err}");
        // Not UTF-8: refused by the checksum before parsing, and by the parser when unchecked.
        let err = De440::from_header_bytes(&[0xff, 0xfe]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::HeaderChecksum, "{err}");
        let err = De440::from_header_bytes_unchecked(&[0xff, 0xfe]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::HeaderParse, "{err}");
    }

    /// The unit conversions on a synthetic header (structural: every selected token is 2.5).
    ///
    /// Verifies: PHY-002
    #[test]
    fn conversions_follow_the_metadata_units() {
        let text = synthetic_header(&[]);
        let de = De440::from_header_bytes_unchecked(text.as_bytes()).unwrap();
        let x = 2.5_f64; // lint: allow-literal(the synthetic header token, not physics)
        assert_eq!(de.speed_of_light().km_per_s().to_bits(), x.to_bits());
        assert_eq!(de.au().km().to_bits(), x.to_bits());
        assert_eq!(
            de.sun_rotation_rate().rad_per_s().to_bits(),
            (x / SECONDS_PER_DAY).to_bits()
        );
        let scale = x * x * x / (SECONDS_PER_DAY * SECONDS_PER_DAY);
        assert_eq!(de.gm_sun().km3_per_s2().to_bits(), (x * scale).to_bits());
        assert_eq!(
            de.gm(Body::Moon).km3_per_s2().to_bits(),
            (x * (1.0 / (1.0 + x)) * scale).to_bits()
        );
        assert_eq!(de.table().title(), "Synthetic Ephemeris");
    }
}
