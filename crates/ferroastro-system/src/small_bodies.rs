// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Asteroids, Kuiper-belt objects and the KBO ring of DE440 (REF-003, provisional while OI-24 is
//! open; NUM-009 tiers).
//!
//! * [`SmallBodyStates`]: the REF-003 sampled initial-state file
//!   (`tests/goldens/sb441-n373s-jdepoc-states.csv`, written by `tools/sample_sb441.py`): the
//!   Sun-centred ICRF state of every target of the pinned `sb441-n373s.bsp` at header.440
//!   `JDEPOC`, with the file's provenance (source file name, SHA-256, JDEPOC).
//! * [`KBO_IDENTIFICATION`]: the header.440 KBO masses (`MA8001…MA8030`, unnamed by JPL) that a
//!   documented rule ties to a NAIF id; the others stay [`crate::UnidentifiedMass`]es.
//! * [`KBO_RING`]: the 36-point ring of Park et al. 2021 §2.2 as fixed [`crate::Tier::Ring`]
//!   masses.
//!
//! Small-body barycentric states are the Sun-centred states plus the Sun's barycentric state of
//! the same initial conditions ([`place_small_bodies`]); the major bodies are never
//! rebarycentred here, so their records stay identical in every configuration (NUM-013).

use crate::catalogue::{ASTEROID_NAIF_BASE, BodyEntry};
use crate::{Catalogue, SystemError, SystemState, Tier};
use ferroastro_core::frames::{Barycentric, Heliocentric, Icrf, Position, State};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_real::Real;

/// One KBO identification: header symbol, NAIF id, name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KboIdentification {
    /// header.440 GM symbol (`MA80nn`).
    pub symbol: &'static str,
    /// NAIF id (`2 000 000 + MPC number`).
    pub id: NaifId,
    /// Name.
    pub name: &'static str,
}

/// The header.440 KBO masses identified by the WP15 rule (IMPLEMENTATION-NOTES WP15, OI-24
/// evidence, **not** a closure): an object of Pitjeva & Pitjev 2018 (Astron. Lett. 44, 554)
/// Table 1 is identified with `MA80nn` iff `MA80nn` is the only header KBO GM inside the
/// object's ±1σ mass interval and lies in no other object's interval. The test
/// `kbo_identification_follows_from_the_rule` re-derives this table from the vectors
/// (`PP2018-T1-*`) and the header at run time. Haumea and Makemake fail the second clause.
pub const KBO_IDENTIFICATION: [KboIdentification; 3] = [
    KboIdentification {
        symbol: "MA8001",
        id: NaifId(ASTEROID_NAIF_BASE + 136_199),
        name: "Eris",
    },
    KboIdentification {
        symbol: "MA8007",
        id: NaifId(ASTEROID_NAIF_BASE + 90_482),
        name: "Orcus",
    },
    KboIdentification {
        symbol: "MA8009",
        id: NaifId(ASTEROID_NAIF_BASE + 55_637),
        name: "2002 UX25",
    },
];

/// The KBO ring of DE440 (Park et al. 2021, AJ 161, 105, §2.2: "36 point masses with equal mass
/// located in the ecliptic plane with a semimajor axis of 44 au"). The phase (point k at ecliptic
/// longitude k·360°/36) and the choice of fixed points (NUM-009: Ring bodies are fixed point
/// masses) are FerroAstro's and UNVERIFIED (OI-24); the plane is JPL's `ECLIPJ2000`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KboRing {
    /// Number of points.
    pub points: u32,
    /// Radius in au.
    pub radius_au: f64,
    /// header.440 number of the first point's GM symbol (`MA8201`).
    pub first_symbol: u32,
    /// Pseudo-id of point k is `pseudo_id_base + first_symbol + k` (FerroAstro-local, not NAIF).
    pub pseudo_id_base: i32,
}

/// The DE440 ring (see [`KboRing`]).
pub const KBO_RING: KboRing = KboRing {
    points: 36,
    radius_au: 44.0,
    first_symbol: 8201,
    pseudo_id_base: 9_000_000,
};

impl KboRing {
    /// The header symbol and pseudo-id of point `k`.
    #[must_use]
    pub fn point(&self, k: u32) -> (String, NaifId) {
        let n = self.first_symbol + k;
        (
            format!("MA{n}"),
            NaifId(self.pseudo_id_base.saturating_add_unsigned(n)),
        )
    }
}

/// One row of the sampled file.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallBodyRecord {
    /// NAIF id.
    pub id: NaifId,
    /// header.440 symbol of a numbered asteroid (`MAnnnn`), if the file names one.
    pub symbol: Option<String>,
    /// Sun-centred ICRF position, km.
    pub position_km: [f64; 3],
    /// Sun-centred ICRF velocity, km/s.
    pub velocity_km_s: [f64; 3],
}

/// The REF-003 sampled initial-state file.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallBodyStates {
    records: Vec<SmallBodyRecord>,
    meta: Vec<(String, String)>,
}

fn bad(quantity: impl Into<String>, value: impl Into<String>, valid: &'static str) -> SystemError {
    SystemError::InitialConditions {
        quantity: quantity.into(),
        value: value.into(),
        valid,
        fix: "regenerate the file with tools/sample_sb441.py from the pinned sb441-n373s.bsp",
    }
}

fn parse_f64(line: usize, s: &str) -> Result<f64, SystemError> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
        .ok_or_else(|| {
            bad(
                format!("sampled-state file line {line}"),
                s,
                "a finite number",
            )
        })
}

impl SmallBodyStates {
    /// Parse the file text (`#` provenance lines `key: value`, a column line
    /// `naif,symbol,x,y,z,vx,vy,vz`, then one row per target).
    ///
    /// # Errors
    /// `FA-E1101` for a missing column line, a malformed row, a non-finite number, a missing
    /// `source_sha256`/`jdepoc_tdb` line or a duplicated id.
    pub fn parse(text: &str) -> Result<Self, SystemError> {
        let mut meta = Vec::new();
        let mut records: Vec<SmallBodyRecord> = Vec::new();
        let mut columns = false;
        for (n, raw) in text.lines().enumerate() {
            let line = n + 1;
            if let Some(m) = raw.strip_prefix("# ") {
                if let Some((k, v)) = m.split_once(": ") {
                    meta.push((k.trim().to_owned(), v.trim().to_owned()));
                }
                continue;
            }
            if raw.trim().is_empty() {
                continue;
            }
            if !columns {
                if raw.trim() != "naif,symbol,x,y,z,vx,vy,vz" {
                    return Err(bad("column line", raw, "naif,symbol,x,y,z,vx,vy,vz"));
                }
                columns = true;
                continue;
            }
            let f: Vec<&str> = raw.split(',').collect();
            let [id, sym, x, y, z, vx, vy, vz] = f.as_slice() else {
                return Err(bad(format!("line {line}"), raw, "8 comma-separated fields"));
            };
            let id = NaifId(
                id.trim()
                    .parse()
                    .map_err(|_| bad(format!("line {line} naif"), *id, "an integer NAIF id"))?,
            );
            if records.iter().any(|r| r.id == id) {
                return Err(SystemError::duplicate(id.0));
            }
            records.push(SmallBodyRecord {
                id,
                symbol: (!sym.trim().is_empty()).then(|| sym.trim().to_owned()),
                position_km: [
                    parse_f64(line, x)?,
                    parse_f64(line, y)?,
                    parse_f64(line, z)?,
                ],
                velocity_km_s: [
                    parse_f64(line, vx)?,
                    parse_f64(line, vy)?,
                    parse_f64(line, vz)?,
                ],
            });
        }
        if !columns {
            return Err(bad("column line", "<none>", "naif,symbol,x,y,z,vx,vy,vz"));
        }
        let s = Self { records, meta };
        s.jdepoc()?;
        s.required("source_sha256")?;
        Ok(s)
    }

    fn required(&self, key: &str) -> Result<&str, SystemError> {
        self.meta(key)
            .ok_or_else(|| bad(format!("provenance line '{key}'"), "<missing>", "present"))
    }

    /// The value of provenance line `key`, if present.
    #[must_use]
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// The epoch of the states (TDB Julian date), from the `jdepoc_tdb` line.
    ///
    /// # Errors
    /// `FA-E1101` if the line is missing or not a finite number.
    pub fn jdepoc(&self) -> Result<f64, SystemError> {
        parse_f64(0, self.required("jdepoc_tdb")?)
    }

    /// SHA-256 of the SPK the states were sampled from.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        self.meta("source_sha256").unwrap_or_default()
    }

    /// One-line provenance: file, URL, SHA-256, retrieval date, JDEPOC, generator.
    #[must_use]
    pub fn provenance(&self) -> String {
        let g = |k: &str| self.meta(k).unwrap_or("?");
        format!(
            "REF-003 states from {} ({}, sha256 {}, retrieved {}) at JDEPOC {} TDB, Sun-centred \
             ICRF; {}",
            g("source_file"),
            g("source_url"),
            g("source_sha256"),
            g("source_retrieved"),
            g("jdepoc_tdb"),
            g("generated_by")
        )
    }

    /// Every record, in file order (ascending NAIF id).
    #[must_use]
    pub fn records(&self) -> &[SmallBodyRecord] {
        &self.records
    }

    /// The record of `id`.
    ///
    /// # Errors
    /// `FA-E1100` if the file has no state for `id`.
    pub fn get(&self, id: NaifId) -> Result<&SmallBodyRecord, SystemError> {
        self.records
            .iter()
            .find(|r| r.id == id)
            .ok_or_else(|| SystemError::unknown(id.0))
    }

    /// The Sun-centred typed state of `id`.
    ///
    /// # Errors
    /// `FA-E1100` if absent.
    pub fn heliocentric<R: Real>(
        &self,
        id: NaifId,
    ) -> Result<State<Icrf, Heliocentric, R>, SystemError> {
        let r = self.get(id)?;
        let l = |x: f64| {
            Length::from_km(R::from_f64(x)).map_err(|_| SystemError::non_finite("position", x))
        };
        let v = |x: f64| {
            Velocity::from_km_per_s(R::from_f64(x))
                .map_err(|_| SystemError::non_finite("velocity", x))
        };
        let [px, py, pz] = r.position_km;
        let [vx, vy, vz] = r.velocity_km_s;
        Ok(State::new(
            Position::new(l(px)?, l(py)?, l(pz)?),
            [v(vx)?, v(vy)?, v(vz)?],
        ))
    }
}

/// Barycentric states of `ids` from the sampled file: Sun-centred state + `sun` (the Sun's
/// barycentric state of the same initial conditions, at the file's epoch).
///
/// # Errors
/// `FA-E1100` if an id has no state in the file or appears twice.
pub fn place_small_bodies<R: Real>(
    file: &SmallBodyStates,
    ids: &[NaifId],
    sun: &State<Icrf, Barycentric, R>,
) -> Result<SystemState<Icrf, Barycentric, R>, SystemError> {
    let mut helio = SystemState::<Icrf, Heliocentric, R>::new();
    for &id in ids {
        helio.push(id, &file.heliocentric::<R>(id)?)?;
    }
    Ok(helio.barycentric(sun))
}

/// The ring points as fixed barycentric states (zero velocity) in the ICRF: radius
/// [`KboRing::radius_au`] × `au_km`, in the `ECLIPJ2000` plane, point k at ecliptic longitude
/// k·2π/points, centred on the SSB.
///
/// # Errors
/// `FA-E1101` for a non-finite radius.
pub fn kbo_ring_states<R: Real>(
    ring: &KboRing,
    au_km: f64,
) -> Result<SystemState<Icrf, Barycentric, R>, SystemError> {
    let to_icrf = ferroastro_frames::icrf_to_ecliptic_j2000::<R>().inverse();
    let radius = R::from_f64(ring.radius_au * au_km);
    let mut out = SystemState::new();
    let zero = Velocity::from_km_per_s(R::ZERO)
        .map_err(|_| SystemError::non_finite("ring velocity", 0.0))?;
    for k in 0..ring.points {
        let lon = R::from_f64(core::f64::consts::TAU) * R::from_f64(f64::from(k))
            / R::from_f64(f64::from(ring.points));
        let p = to_icrf.apply([radius * lon.cos(), radius * lon.sin(), R::ZERO]);
        let l = |x: R| {
            Length::from_km(x).map_err(|_| SystemError::non_finite("ring radius", x.to_f64()))
        };
        out.push(
            ring.point(k).1,
            &State::new(Position::new(l(p[0])?, l(p[1])?, l(p[2])?), [zero; 3]),
        )?;
    }
    Ok(out)
}

impl Catalogue {
    /// Move the [`KBO_IDENTIFICATION`] masses from the unidentified list into the catalogue as
    /// [`Tier::Minor`] bodies (GM source: their header symbol).
    ///
    /// # Errors
    /// `FA-E1100` if an identified symbol is not an unidentified mass of this catalogue (e.g. a
    /// catalogue not built from header.440) or its id is already present.
    pub fn with_identified_kbos(&mut self) -> Result<(), SystemError> {
        for k in &KBO_IDENTIFICATION {
            self.promote(k.symbol, k.id, k.name, Tier::Minor)?;
        }
        Ok(())
    }

    /// Move the [`KBO_RING`] masses (`MA8201…MA8236`) into the catalogue as [`Tier::Ring`]
    /// bodies with their pseudo-ids.
    ///
    /// # Errors
    /// `FA-E1100` as [`Catalogue::with_identified_kbos`].
    pub fn with_kbo_ring(&mut self) -> Result<(), SystemError> {
        for k in 0..KBO_RING.points {
            let (symbol, id) = KBO_RING.point(k);
            self.promote(&symbol, id, &format!("KBO ring point {k}"), Tier::Ring)?;
        }
        Ok(())
    }

    fn promote(
        &mut self,
        symbol: &str,
        id: NaifId,
        name: &str,
        tier: Tier,
    ) -> Result<(), SystemError> {
        if self.get(id).is_ok() {
            return Err(SystemError::duplicate(id.0));
        }
        let pos = self
            .unidentified
            .iter()
            .position(|u| u.symbol == symbol)
            .ok_or(SystemError::Body {
                id: id.0,
                problem: "its header symbol is not an unidentified mass of this catalogue",
                valid: "a catalogue from header.440 that still lists the symbol",
                fix: "build the catalogue with Catalogue::from_header and promote each body once",
            })?;
        let u = self.unidentified.remove(pos);
        self.bodies
            .push(BodyEntry::new(id, name, u.gm, tier, &u.symbol));
        Ok(())
    }
}
