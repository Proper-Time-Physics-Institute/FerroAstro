// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Initial-condition assembly (REF-002): from `header.440` GROUP 1041, from an ephemeris sampled
//! at an epoch, or from externally supplied barycentric vectors (e.g. JPL Horizons).
//!
//! Every result is an ICRF, SSB-centred [`SystemState`] with its epoch and a provenance line that
//! names the source and, for the header, the state interpretation it relies on
//! ([`HEADER_STATE_INTERPRETATION`]).

use crate::vec3::zip3;
use crate::{EarthMoon, SystemError, SystemState};
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Geocentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::{Ephemeris, NaifId};
use ferroastro_real::Real;

/// How this crate reads the header.440 state block (open item OI-23; NEEDS HUMAN REVIEW for its
/// closure). It is checked against de440.bsp at JDEPOC by the PR-tier test
/// `ref002_header_initial_conditions_equal_de440_at_jdepoc`.
pub const HEADER_STATE_INTERPRETATION: &str = "header.440 CENTER = 0: X/Y/Z (au) and XD/YD/ZD \
     (au/day, day = 86400 s TDB) are SSB-centred states in the ICRF-aligned DE frame at JDEPOC \
     (TDB); suffixes 1, 2, 4-9 are planetary-system barycentres, B the Earth-Moon barycentre, S \
     the Sun, M the Moon relative to the Earth; AU = header AU (km); the XC block is not used";

/// Header suffixes of the SSB-centred states and their NAIF ids (Earth and Moon come from the
/// `B` and `M` blocks).
pub const HEADER_STATE_SUFFIXES: [(&str, NaifId); 9] = [
    ("S", NaifId::SUN),
    ("1", NaifId::MERCURY_BARYCENTER),
    ("2", NaifId::VENUS_BARYCENTER),
    ("4", NaifId::MARS_BARYCENTER),
    ("5", NaifId::JUPITER_BARYCENTER),
    ("6", NaifId::SATURN_BARYCENTER),
    ("7", NaifId::URANUS_BARYCENTER),
    ("8", NaifId::NEPTUNE_BARYCENTER),
    ("9", NaifId::PLUTO_BARYCENTER),
];

/// Initial conditions: epoch, barycentric ICRF states, the geocentric Moon pair when known, and
/// provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct InitialConditions<R: Real = f64> {
    epoch: Epoch<Tdb, R>,
    states: SystemState<Icrf, Barycentric, R>,
    earth_moon: Option<EarthMoon<Icrf, R>>,
    source: String,
}

fn typed<C: ferroastro_core::frames::Center, R: Real>(
    what: &str,
    p: [R; 3],
    v: [R; 3],
) -> Result<State<Icrf, C, R>, SystemError> {
    let km = |x: R| {
        Length::from_km(x)
            .map_err(|_| SystemError::non_finite(format!("{what} position (km)"), x.to_f64()))
    };
    let kms = |x: R| {
        Velocity::from_km_per_s(x)
            .map_err(|_| SystemError::non_finite(format!("{what} velocity (km/s)"), x.to_f64()))
    };
    Ok(State::new(
        Position::new(km(p[0])?, km(p[1])?, km(p[2])?),
        [kms(v[0])?, kms(v[1])?, kms(v[2])?],
    ))
}

/// The six header values of state block `suffix`, converted to km and km/s in `R`.
fn header_block<R: Real>(h: &Header, suffix: &str, au: R) -> Result<([R; 3], [R; 3]), SystemError> {
    let day = R::from_f64(SECONDS_PER_DAY);
    let get = |name: String| -> Result<R, SystemError> { Ok(R::from_f64(h.value(&name)?)) };
    let p = [
        get(format!("X{suffix}"))? * au,
        get(format!("Y{suffix}"))? * au,
        get(format!("Z{suffix}"))? * au,
    ];
    let v = [
        get(format!("XD{suffix}"))? * au / day,
        get(format!("YD{suffix}"))? * au / day,
        get(format!("ZD{suffix}"))? * au / day,
    ];
    Ok((p, v))
}

impl<R: Real> InitialConditions<R> {
    /// The DE ICs of a parsed `header.440` (REF-002 run-time path) under
    /// [`HEADER_STATE_INTERPRETATION`]: Sun, Mercury … Pluto systems, and Earth and Moon split
    /// from `XB` and `XM` with `EMRAT`. `provenance` describes the header (e.g. its SHA-256).
    ///
    /// # Errors
    /// `FA-E1101` if `CENTER` is not 0 (the interpretation would not apply) or a value is not
    /// finite; `FA-E0301` if a symbol is missing.
    pub fn from_header(h: &Header, provenance: &str) -> Result<Self, SystemError> {
        let center = h.value("CENTER")?;
        if center != 0.0 {
            return Err(SystemError::InitialConditions {
                quantity: "header.440 CENTER".into(),
                value: format!("{center}"),
                valid: "0 (the only CENTER this crate's interpretation covers, OI-23)",
                fix: "extend HEADER_STATE_INTERPRETATION for this header after checking it against its SPK",
            });
        }
        let au = R::from_f64(h.value("AU")?);
        let emrat = R::from_f64(h.value("EMRAT")?);
        let epoch = Epoch::from_jd_parts(R::from_f64(h.value("JDEPOC")?), R::ZERO)
            .map_err(|_| SystemError::non_finite("JDEPOC", f64::NAN))?;
        let mut states = SystemState::new();
        for (suffix, id) in HEADER_STATE_SUFFIXES {
            let (p, v) = header_block(h, suffix, au)?;
            states.push(id, &typed(suffix, p, v)?)?;
        }
        let (bp, bv) = header_block(h, "B", au)?;
        let (mp, mv) = header_block(h, "M", au)?;
        let pair = EarthMoon {
            emb: typed("B", bp, bv)?,
            moon: typed::<Geocentric, R>("M", mp, mv)?,
        };
        let (earth, moon) = pair.split(emrat);
        states.push(NaifId::EARTH, &earth)?;
        states.push(NaifId::MOON, &moon)?;
        Ok(Self {
            epoch,
            states,
            earth_moon: Some(pair),
            source: format!("{provenance}; {HEADER_STATE_INTERPRETATION}"),
        })
    }

    /// Barycentric states of `ids` sampled from an ephemeris at `epoch`. If both the Earth and
    /// the Moon are requested, the geocentric Moon pair is sampled too (Moon relative to Earth
    /// directly, not as a difference of barycentric states).
    ///
    /// # Errors
    /// The ephemeris errors (`FA-E10xx`), `FA-E1100` for a duplicated id.
    pub fn from_ephemeris<E: Ephemeris>(
        eph: &E,
        epoch: Epoch<Tdb, R>,
        ids: &[NaifId],
        description: &str,
    ) -> Result<Self, SystemError> {
        let mut states = SystemState::new();
        for &id in ids {
            states.push(id, &eph.state::<Barycentric, R>(id, epoch)?)?;
        }
        let earth_moon = if ids.contains(&NaifId::EARTH) && ids.contains(&NaifId::MOON) {
            Some(EarthMoon {
                emb: eph.state::<Barycentric, R>(NaifId::EARTH_MOON_BARYCENTER, epoch)?,
                moon: eph.state::<Geocentric, R>(NaifId::MOON, epoch)?,
            })
        } else {
            None
        };
        Ok(Self {
            epoch,
            states,
            earth_moon,
            source: format!(
                "ephemeris sample at JD {} TDB: {description}",
                epoch.jd().to_f64()
            ),
        })
    }

    /// Initial conditions from barycentric ICRF states supplied by the caller (e.g. JPL Horizons
    /// vectors with centre `500@0`, `REF_SYSTEM=ICRF`, TDB).
    ///
    /// # Errors
    /// `FA-E1100` for a duplicated id.
    pub fn from_states(
        epoch: Epoch<Tdb, R>,
        states: impl IntoIterator<Item = (NaifId, State<Icrf, Barycentric, R>)>,
        description: &str,
    ) -> Result<Self, SystemError> {
        let mut s = SystemState::new();
        for (id, st) in states {
            s.push(id, &st)?;
        }
        Ok(Self {
            epoch,
            states: s,
            earth_moon: None,
            source: description.to_owned(),
        })
    }

    /// The epoch (TDB).
    #[must_use]
    pub const fn epoch(&self) -> Epoch<Tdb, R> {
        self.epoch
    }

    /// The barycentric ICRF states.
    #[must_use]
    pub const fn states(&self) -> &SystemState<Icrf, Barycentric, R> {
        &self.states
    }

    /// Mutable states (e.g. for [`crate::rebarycentre`]).
    pub fn states_mut(&mut self) -> &mut SystemState<Icrf, Barycentric, R> {
        &mut self.states
    }

    /// The Earth–Moon pair in DE's variables, if the source provided it.
    #[must_use]
    pub const fn earth_moon(&self) -> Option<&EarthMoon<Icrf, R>> {
        self.earth_moon.as_ref()
    }

    /// The geocentric Moon: from the source's pair if present, else Moon − Earth of the states.
    ///
    /// # Errors
    /// `FA-E1100` if neither the pair nor both bodies are present.
    pub fn geocentric_moon(&self) -> Result<State<Icrf, Geocentric, R>, SystemError> {
        if let Some(pair) = &self.earth_moon {
            return Ok(pair.moon);
        }
        let e = self.states.state(NaifId::EARTH)?;
        let m = self.states.state(NaifId::MOON)?;
        let [x, y, z] = (m.position() - e.position()).components();
        Ok(State::new(
            Position::new(x, y, z),
            zip3(m.velocity(), e.velocity(), |a, b| a - b),
        ))
    }

    /// Provenance: the source and, for the header, the interpretation used.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}
