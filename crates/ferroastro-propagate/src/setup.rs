// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Data sources, the capabilities of this runner (REF-011), constants and IC assembly.

use crate::PropagateError;
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::{Barycentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Length, SpinGm, Velocity};
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use ferroastro_real::Real;
use ferroastro_scenario::{IcSource, Mode, ScenarioSpec};
use ferroastro_system::{EarthMoon, InitialConditions, SmallBodyStates, SystemState};

/// Name of this runner in refusals (REF-011).
pub const RUNNER: &str = "FerroAstro (ferroastro-propagate)";

/// A barycentric ICRF state supplied by the caller: NAIF id, position (km), velocity (km/s).
pub type SuppliedState = (i32, [f64; 3], [f64; 3]);

/// The data a run may need. The driver performs no I/O: the caller loads and verifies the
/// products (e.g. with `ferroastro-testdata`) and passes them with their provenance.
#[derive(Debug, Clone, Copy, Default)]
pub struct DataSources<'a> {
    header: Option<(&'a Header, &'a str)>,
    spk: Option<(&'a Spk<'a>, &'a str)>,
    horizons: Option<(&'a [SuppliedState], &'a str)>,
    small_bodies: Option<&'a SmallBodyStates>,
    solar: Option<&'a SolarOverrides>,
}

/// Values that replace the DE440 solar parameters of the `sun_j2` and `lense_thirring` terms
/// (NUM-019's configurable pole; Park17's J2 = 2.25e-7 for the PHY-010 rung and the Park17
/// comparison of WP20). They are not part of the scenario file (CI-002's switch list is fixed),
/// so they are written into [`crate::RunOutput::model`] and into every [`crate::Checkpoint`],
/// and a checkpoint refuses to resume under different overrides (CI-010). Overrides change no
/// initial condition, so the IC hash does not depend on them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SolarOverrides {
    /// Solar J2 (unnormalised) instead of DE440 `J2SUN`.
    pub j2: Option<f64>,
    /// Solar pole (ICRF components, any non-zero length; normalised) instead of the WGCCRE 2015
    /// pole, for J2 and Lense–Thirring.
    pub pole: Option<[f64; 3]>,
    /// `G·S` of the Sun (km⁵/s³) instead of DE440 `GMS · MOISUN · ASUN² · OMGSUN`.
    pub spin_gm: Option<f64>,
    /// Where the values come from (e.g. a vector id), recorded with them.
    pub provenance: String,
}

impl SolarOverrides {
    /// True if no value is overridden.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.j2.is_none() && self.pole.is_none() && self.spin_gm.is_none()
    }

    /// A canonical one-line description (empty string when nothing is overridden). Recorded in
    /// the run's model string and its checkpoints; values are printed with full precision.
    #[must_use]
    pub fn describe(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut parts = Vec::new();
        if let Some(j2) = self.j2 {
            parts.push(format!("J2 = {j2:e}"));
        }
        if let Some([x, y, z]) = self.pole {
            parts.push(format!("pole = [{x:e}, {y:e}, {z:e}]"));
        }
        if let Some(gs) = self.spin_gm {
            parts.push(format!("G S = {gs:e} km^5/s^3"));
        }
        format!(
            "solar overrides: {} (source: {})",
            parts.join(", "),
            self.provenance
        )
    }
}

impl<'a> DataSources<'a> {
    /// No data.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// The SPK and its provenance line, if supplied.
    pub(crate) const fn spk(&self) -> Option<(&'a Spk<'a>, &'a str)> {
        self.spk
    }

    /// With a parsed `header.440` and its provenance line (URL, SHA-256, retrieval date).
    #[must_use]
    pub const fn with_header(mut self, header: &'a Header, provenance: &'a str) -> Self {
        self.header = Some((header, provenance));
        self
    }

    /// With `de440.bsp` (or another SPK) and its provenance line.
    #[must_use]
    pub const fn with_spk(mut self, spk: &'a Spk<'a>, provenance: &'a str) -> Self {
        self.spk = Some((spk, provenance));
        self
    }

    /// With the REF-003 sampled asteroid/KBO states (`tests/goldens/sb441-n373s-jdepoc-states.csv`
    /// parsed by [`SmallBodyStates::parse`]; its provenance is recorded in the run output). Needed
    /// by the `asteroids` and `kbos` switches.
    #[must_use]
    pub const fn with_small_bodies(mut self, states: &'a SmallBodyStates) -> Self {
        self.small_bodies = Some(states);
        self
    }

    /// With values that replace the DE440 solar J2, pole or spin ([`SolarOverrides`]).
    #[must_use]
    pub const fn with_solar_overrides(mut self, overrides: &'a SolarOverrides) -> Self {
        self.solar = Some(overrides);
        self
    }

    /// The solar overrides, if supplied.
    #[must_use]
    pub const fn solar_overrides(&self) -> Option<&'a SolarOverrides> {
        self.solar
    }

    /// [`SolarOverrides::describe`] of the supplied overrides (empty if none).
    #[must_use]
    pub fn solar_overrides_tag(&self) -> String {
        self.solar
            .map_or_else(String::new, SolarOverrides::describe)
    }

    /// The small-body states, if supplied.
    #[must_use]
    pub const fn small_bodies(&self) -> Option<&'a SmallBodyStates> {
        self.small_bodies
    }

    /// The header and its provenance, if supplied.
    #[must_use]
    pub const fn header(&self) -> Option<(&'a Header, &'a str)> {
        self.header
    }

    /// With barycentric ICRF states at the scenario epoch (e.g. JPL Horizons, centre `500@0`,
    /// TDB) and their provenance line.
    #[must_use]
    pub const fn with_states(mut self, states: &'a [SuppliedState], provenance: &'a str) -> Self {
        self.horizons = Some((states, provenance));
        self
    }
}

/// What this runner implements, per item of [`ScenarioSpec::refuse_unsupported`].
///
/// # Errors
/// The reason, for an item this runner refuses.
pub fn supports(item: &str, mode: Mode) -> Result<(), String> {
    const ALWAYS: [&str; 9] = [
        "newton",
        "sun_j2",
        "ppn",
        "integrator.ias15",
        "integrator.precision.f64",
        "integrator.precision.dd",
        "initial_conditions.source.de440-header",
        "initial_conditions.source.de440-bsp",
        "initial_conditions.source.horizons",
    ];
    // gr_1pn_central: the R3 field in test-particle mode, the heliocentric Sun-only rung in n-body
    // mode (WP19). lense_thirring: the R3b field in test-particle mode, the DE440 Eq. 37-39 term
    // on every non-Sun body in n-body mode (WP20).
    let central_term = matches!(item, "gr_1pn_central" | "lense_thirring");
    let nbody_relativity = matches!(item, "gr_eih" | "gr_1pn_central" | "lense_thirring");
    let small_bodies = matches!(item, "asteroids" | "kbos" | "kbo_ring");
    if mode == Mode::EphemerisField {
        const FIELD: [&str; 4] = [
            "newton",
            "integrator.ias15",
            "integrator.precision.f64",
            "initial_conditions.source.de440-bsp",
        ];
        return if FIELD.contains(&item) {
            Ok(())
        } else {
            Err(
                "ephemeris-field mode (oracle stage B) implements Newtonian point masses from \
                 de440-bsp in f64 only"
                    .into(),
            )
        };
    }
    if ALWAYS.contains(&item)
        || (item == "integrator.abm" && mode == Mode::NBody)
        || (central_term && mode == Mode::TestParticle)
        || (nbody_relativity && mode == Mode::NBody)
        || (small_bodies && mode == Mode::NBody)
    {
        return Ok(());
    }
    Err(match item {
        "newton=false" => "every FerroAstro model contains Newtonian gravity".into(),
        "gr_eih" => {
            "the EIH equations need n-body mode (a test particle has no mutual 1PN terms; use \
             gr_1pn_central)"
                .into()
        }
        _ if small_bodies => "small bodies (Minor/Ring tiers) need n-body mode".into(),
        "integrator.abm" => {
            "ABM is implemented for n-body runs (WP24); test-particle runs use IAS15".into()
        }
        other if other.starts_with("integrator.") => {
            "only IAS15 and (n-body) ABM are implemented (WHFast/IRK16: WP29)".into()
        }
        _ => "not implemented".into(),
    })
}

/// Refuse what this runner does not implement (REF-011), naming it.
///
/// # Errors
/// `FA-E1201`.
pub fn refuse_unsupported(spec: &ScenarioSpec) -> Result<(), PropagateError> {
    Ok(spec.refuse_unsupported(RUNNER, supports)?)
}

/// The constants version that enters the IC hash: the DE440 table and the header it came from.
#[must_use]
pub fn constants_version(de: &De440) -> String {
    let s = de.table().source();
    format!(
        "DE440 constants table from {} sha256 {}",
        s.product, s.sha256
    )
}

/// `model` followed by the solar overrides of `data`, if any (the run's model string).
pub(crate) fn tag_overrides(model: String, data: &DataSources<'_>) -> String {
    let tag = data.solar_overrides_tag();
    if tag.is_empty() {
        model
    } else {
        format!("{model}; {tag}")
    }
}

/// The solar J2 of a run: the override if supplied, else DE440 `J2SUN`.
pub(crate) fn solar_j2(de: &De440, data: &DataSources<'_>) -> f64 {
    data.solar_overrides()
        .and_then(|o| o.j2)
        .unwrap_or_else(|| de.sun_j2())
}

/// The solar pole of a run (ICRF unit vector): the override if supplied, else WGCCRE 2015.
///
/// # Errors
/// `FA-E0500` (wrapped) for a zero or non-finite override.
pub(crate) fn solar_pole<R: Real>(
    data: &DataSources<'_>,
) -> Result<ferroastro_forces::Pole<Icrf, R>, PropagateError> {
    let k = match data.solar_overrides().and_then(|o| o.pole) {
        Some(p) => p.map(R::from_f64),
        None => ferroastro_frames::Pole::<R>::sun_wgccre2015()?.unit_vector(),
    };
    Ok(ferroastro_forces::Pole::from_components(k)?)
}

/// `G·S` of a run: the override if supplied, else [`de440_solar_spin`].
///
/// # Errors
/// As [`de440_solar_spin`]; `FA-E0001` (wrapped) for an invalid override.
pub(crate) fn solar_spin(
    de: &De440,
    data: &DataSources<'_>,
    scenario: &str,
) -> Result<SpinGm, PropagateError> {
    match data.solar_overrides().and_then(|o| o.spin_gm) {
        Some(gs) => SpinGm::from_km5_per_s3(gs)
            .map_err(|e| setup(scenario, e.to_string(), "check the solar overrides")),
        None => de440_solar_spin(de, scenario),
    }
}

/// `G·S` of the Sun from DE440: `GM☉ · MOISUN · ASUN² · OMGSUN` (as the single-body rungs).
///
/// # Errors
/// `FA-E0001` (wrapped) if the product is not finite.
pub fn de440_solar_spin(de: &De440, scenario: &str) -> Result<SpinGm, PropagateError> {
    let r = de.sun_radius().km();
    let gs = de.gm_sun().km3_per_s2()
        * de.sun_moment_of_inertia()
        * r
        * r
        * de.sun_rotation_rate().rad_per_s();
    SpinGm::from_km5_per_s3(gs).map_err(|e| setup(scenario, e.to_string(), "check the DE440 table"))
}

pub(crate) fn setup(
    scenario: &str,
    problem: impl Into<String>,
    fix: &'static str,
) -> PropagateError {
    PropagateError::Setup {
        scenario: scenario.to_owned(),
        problem: problem.into(),
        fix,
    }
}

/// The `header.440` state-block suffix of a body (GROUP 1041).
const fn header_suffix(naif: i32) -> Option<&'static str> {
    Some(match naif {
        10 => "S",
        1 => "1",
        2 => "2",
        3 => "B",
        4 => "4",
        5 => "5",
        6 => "6",
        7 => "7",
        8 => "8",
        9 => "9",
        _ => return None,
    })
}

/// The scenario epoch as `Epoch<Tdb, R>` and its JD, checked against the header for
/// `de440-header`.
pub(crate) fn epoch<R: Real>(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<(Epoch<Tdb, R>, f64), PropagateError> {
    let n = spec.name.as_str();
    let jd = match spec.initial_conditions.source {
        IcSource::De440Header => {
            let (h, _) = data.header.ok_or_else(|| {
                setup(
                    n,
                    "IC source de440-header but no header.440 supplied",
                    "pass DataSources::with_header",
                )
            })?;
            let jdepoc = h.value("JDEPOC")?;
            // Nested `if let` (not a let-chain): the workspace MSRV is 1.85.
            if let Some(jd) = spec.initial_conditions.epoch_jd_tdb {
                if jd.to_bits() != jdepoc.to_bits() {
                    return Err(ferroastro_scenario::ScenarioError::Value {
                        name: n.to_owned(),
                        field: "initial_conditions.epoch_jd_tdb",
                        value: jd.to_string(),
                        problem: format!("de440-header ICs are at JDEPOC = {jdepoc}"),
                    }
                    .into());
                }
            }
            jdepoc
        }
        IcSource::De440Bsp | IcSource::Horizons => spec
            .initial_conditions
            .epoch_jd_tdb
            .ok_or_else(|| setup(n, "no epoch_jd_tdb", "set initial_conditions.epoch_jd_tdb"))?,
    };
    let e = Epoch::from_jd_parts(R::from_f64(jd), R::ZERO)
        .map_err(|e| setup(n, e.to_string(), "use a finite TDB Julian date"))?;
    Ok((e, jd))
}

/// The test particle's state relative to the central body (km, km/s), assembled in f64 exactly
/// as the single-body rungs do (for the header: the difference of the header values in au and
/// au/day, then scaled, which is WP7's D5 arithmetic), and a provenance line.
pub(crate) fn relative_state(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    jd: f64,
) -> Result<([f64; 3], [f64; 3], String), PropagateError> {
    let n = spec.name.as_str();
    let central = spec.central_body.naif();
    let particle = spec
        .bodies
        .iter()
        .map(|b| b.naif())
        .find(|&id| id != central)
        .ok_or_else(|| setup(n, "no test particle", "list two bodies"))?;
    match spec.initial_conditions.source {
        IcSource::De440Header => {
            let (h, prov) = data
                .header
                .ok_or_else(|| setup(n, "no header.440", "pass DataSources::with_header"))?;
            let (Some(sp), Some(sc)) = (header_suffix(particle), header_suffix(central)) else {
                return Err(setup(
                    n,
                    "header.440 holds no separate Earth or Moon barycentric state",
                    "use EMB or another body",
                ));
            };
            let v = |name: String| h.value(&name);
            let au = v("AU".into())?;
            let mut r0 = [0.0; 3];
            let mut v0 = [0.0; 3];
            for (k, (c, cd)) in [("X", "XD"), ("Y", "YD"), ("Z", "ZD")]
                .into_iter()
                .enumerate()
            {
                if let (Some(r), Some(w)) = (r0.get_mut(k), v0.get_mut(k)) {
                    *r = (v(format!("{c}{sp}"))? - v(format!("{c}{sc}"))?) * au;
                    *w = (v(format!("{cd}{sp}"))? - v(format!("{cd}{sc}"))?) * au / SECONDS_PER_DAY;
                }
            }
            Ok((
                r0,
                v0,
                format!(
                    "header.440 GROUP 1041 X{sp}..ZD{sp} minus X{sc}..ZD{sc} at JDEPOC ({prov}; \
                     {})",
                    ferroastro_system::HEADER_STATE_INTERPRETATION
                ),
            ))
        }
        IcSource::De440Bsp => {
            let (spk, prov) = data
                .spk
                .ok_or_else(|| setup(n, "no SPK", "pass DataSources::with_spk"))?;
            let e: Epoch<Tdb> = Epoch::from_jd_parts(jd, 0.0)
                .map_err(|e| setup(n, e.to_string(), "use a finite TDB Julian date"))?;
            let p = spk
                .state::<Barycentric, f64>(NaifId(particle), e)
                .map_err(ferroastro_system::SystemError::from)?;
            let c = spk
                .state::<Barycentric, f64>(NaifId(central), e)
                .map_err(ferroastro_system::SystemError::from)?;
            let (rp, vp) = raw(&p);
            let (rc, vc) = raw(&c);
            Ok((
                sub(rp, rc),
                sub(vp, vc),
                format!("SPK at JD {jd} TDB, {particle} minus {central} ({prov})"),
            ))
        }
        IcSource::Horizons => {
            let (states, prov) = data
                .horizons
                .ok_or_else(|| setup(n, "no supplied states", "pass DataSources::with_states"))?;
            let get = |id: i32| {
                states
                    .iter()
                    .find(|s| s.0 == id)
                    .map(|s| (s.1, s.2))
                    .ok_or_else(|| {
                        setup(
                            n,
                            format!("no supplied state for NAIF {id}"),
                            "supply every body",
                        )
                    })
            };
            let (rp, vp) = get(particle)?;
            let (rc, vc) = get(central)?;
            Ok((
                sub(rp, rc),
                sub(vp, vc),
                format!("supplied states at JD {jd} TDB ({prov})"),
            ))
        }
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn raw<C: ferroastro_core::frames::Center, R: Real>(
    s: &State<Icrf, C, R>,
) -> ([R; 3], [R; 3]) {
    (
        s.position().components().map(Length::km),
        s.velocity().map(Velocity::km_per_s),
    )
}

fn typed<R: Real>(
    n: &str,
    p: [f64; 3],
    w: [f64; 3],
) -> Result<State<Icrf, Barycentric, R>, PropagateError> {
    let bad = |e: ferroastro_core::Error| setup(n, e.to_string(), "supply finite states");
    let l = |z: f64| Length::from_km(R::from_f64(z)).map_err(bad);
    let s = |z: f64| Velocity::from_km_per_s(R::from_f64(z)).map_err(bad);
    Ok(State::new(
        Position::new(l(p[0])?, l(p[1])?, l(p[2])?),
        [s(w[0])?, s(w[1])?, s(w[2])?],
    ))
}

/// N-body ICs: barycentric states of the scenario's bodies (EMB from the Earth–Moon pair) and,
/// when both Earth and Moon are listed, the pair in DE variables; plus provenance.
pub(crate) type NBodyIcs<R> = (
    SystemState<Icrf, Barycentric, R>,
    Option<EarthMoon<Icrf, R>>,
    String,
);

pub(crate) fn nbody_ics<R: Real>(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    epoch: Epoch<Tdb, R>,
) -> Result<NBodyIcs<R>, PropagateError> {
    let n = spec.name.as_str();
    let ids: Vec<NaifId> = spec.bodies.iter().map(|b| NaifId(b.naif())).collect();
    let ic: InitialConditions<R> = match spec.initial_conditions.source {
        IcSource::De440Header => {
            let (h, prov) = data
                .header
                .ok_or_else(|| setup(n, "no header.440", "pass DataSources::with_header"))?;
            InitialConditions::from_header(h, prov)?
        }
        IcSource::De440Bsp => {
            let (spk, prov) = data
                .spk
                .ok_or_else(|| setup(n, "no SPK", "pass DataSources::with_spk"))?;
            InitialConditions::from_ephemeris(spk, epoch, &ids, prov)?
        }
        IcSource::Horizons => {
            let (states, prov) = data
                .horizons
                .ok_or_else(|| setup(n, "no supplied states", "pass DataSources::with_states"))?;
            let typed_states = states
                .iter()
                .map(|&(id, p, w)| Ok((NaifId(id), typed::<R>(n, p, w)?)))
                .collect::<Result<Vec<_>, PropagateError>>()?;
            InitialConditions::from_states(epoch, typed_states, prov)?
        }
    };
    let mut out = SystemState::new();
    for &id in &ids {
        let st = if id == NaifId::EARTH_MOON_BARYCENTER && ic.states().index_of(id).is_none() {
            ic.earth_moon().map(|em| em.emb).ok_or_else(|| {
                setup(
                    n,
                    "no Earth-Moon barycentre in the ICs",
                    "use a source that provides it",
                )
            })?
        } else {
            ic.states().state(id)?
        };
        out.push(id, &st)?;
    }
    let pair = if ids.contains(&NaifId::EARTH) && ids.contains(&NaifId::MOON) {
        ic.earth_moon().copied()
    } else {
        None
    };
    Ok((out, pair, ic.source().to_owned()))
}
