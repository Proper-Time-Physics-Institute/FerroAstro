// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Scenario files: the cited physical inputs of a set of rungs (ARCHITECTURE: "Scenario TOML →
//! runs → structured results").

use crate::LadderError;
use ferroastro_analytic::kepler::Elements;
use ferroastro_core::units::{Angle, Length};
use serde::Deserialize;

/// The committed single-body scenario (`data/mercury-single-body.toml`).
pub const MERCURY_SINGLE_BODY: &str = include_str!("../data/mercury-single-body.toml");

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    schema_version: i64,
    name: String,
    description: String,
    orbits: u32,
    elements: RawElements,
    park17: RawPark17,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawElements {
    source: String,
    frame: String,
    a_au: String,
    e: String,
    i_deg: String,
    mean_longitude_deg: String,
    long_perihelion_deg: String,
    node_deg: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPark17 {
    source: String,
    j2: String,
    solar_equator_tilt_deg: String,
}

/// Mean elements as given (angles in degrees, `a` in au).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeanElements {
    /// Semi-major axis (au).
    pub a_au: f64,
    /// Eccentricity.
    pub e: f64,
    /// Inclination (deg).
    pub i_deg: f64,
    /// Mean longitude (deg).
    pub mean_longitude_deg: f64,
    /// Longitude of perihelion (deg).
    pub long_perihelion_deg: f64,
    /// Longitude of the ascending node (deg).
    pub node_deg: f64,
}

/// A parsed single-body scenario.
#[derive(Debug, Clone, PartialEq)]
pub struct Scenario {
    /// Scenario name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Orbits per run.
    pub orbits: u32,
    /// Initial mean elements.
    pub elements: MeanElements,
    /// Citation of the elements.
    pub elements_source: String,
    /// Frame of the elements.
    pub elements_frame: String,
    /// Park et al. 2017 solar J2.
    pub park17_j2: f64,
    /// Park et al. 2017 tilt of the solar equator to Mercury's mean orbit plane (deg).
    pub park17_tilt_deg: f64,
    /// Citation of the Park et al. 2017 values.
    pub park17_source: String,
}

impl Scenario {
    /// Parse a scenario file.
    ///
    /// # Errors
    /// `FA-E0701` for malformed TOML, an unknown schema version, unparsable numbers or no orbits.
    pub fn parse(text: &str) -> Result<Self, LadderError> {
        let raw: Raw = toml::from_str(text).map_err(|e| LadderError::Scenario {
            name: "?".into(),
            problem: e.to_string(),
        })?;
        let name = raw.name.clone();
        let err = |problem: String| LadderError::Scenario {
            name: name.clone(),
            problem,
        };
        if raw.schema_version != 1 {
            return Err(err(format!("schema_version {} != 1", raw.schema_version)));
        }
        if raw.orbits < 2 {
            return Err(err(format!("orbits = {} (at least 2)", raw.orbits)));
        }
        let num = |field: &str, s: &str| -> Result<f64, LadderError> {
            s.trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| err(format!("{field} = {s:?} is not a finite number")))
        };
        let el = &raw.elements;
        Ok(Self {
            elements: MeanElements {
                a_au: num("a_au", &el.a_au)?,
                e: num("e", &el.e)?,
                i_deg: num("i_deg", &el.i_deg)?,
                mean_longitude_deg: num("mean_longitude_deg", &el.mean_longitude_deg)?,
                long_perihelion_deg: num("long_perihelion_deg", &el.long_perihelion_deg)?,
                node_deg: num("node_deg", &el.node_deg)?,
            },
            park17_j2: num("park17.j2", &raw.park17.j2)?,
            park17_tilt_deg: num(
                "park17.solar_equator_tilt_deg",
                &raw.park17.solar_equator_tilt_deg,
            )?,
            park17_source: raw.park17.source,
            elements_source: raw.elements.source,
            elements_frame: raw.elements.frame,
            orbits: raw.orbits,
            description: raw.description,
            name: raw.name,
        })
    }

    /// The committed Mercury single-body scenario.
    ///
    /// # Errors
    /// As [`Scenario::parse`] (never for the committed file, which a test parses).
    pub fn mercury_single_body() -> Result<Self, LadderError> {
        Self::parse(MERCURY_SINGLE_BODY)
    }

    /// The mean elements as [`Elements`], with `a` converted by `au`.
    ///
    /// # Errors
    /// `FA-E0701` if a value cannot be represented.
    pub fn kepler_elements(&self, au: Length) -> Result<Elements, LadderError> {
        let m = self.elements;
        let bad = |what: &str| LadderError::Scenario {
            name: self.name.clone(),
            problem: format!("{what} is not representable"),
        };
        let deg = |d: f64, what: &str| Angle::from_radians(d.to_radians()).map_err(|_| bad(what));
        Ok(Elements::from_longitudes(
            Length::from_km(m.a_au * au.km()).map_err(|_| bad("a"))?,
            m.e,
            deg(m.i_deg, "i")?,
            deg(m.node_deg, "node")?,
            deg(m.long_perihelion_deg, "long. perihelion")?,
            deg(m.mean_longitude_deg, "mean longitude")?,
        ))
    }
}
