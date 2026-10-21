// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The scenario spec (CI-002): types, TOML parsing, validation and the JSON Schema.

use crate::ScenarioError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The only scenario schema version this crate reads.
pub const SCHEMA_VERSION: u32 = 1;

/// Julian year in days (the span unit).
pub const DAYS_PER_JULIAN_YEAR: f64 = 365.25;

/// How the bodies are integrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Every listed body moves and attracts every other (`ferroastro-forces` N-body model).
    NBody,
    /// Exactly two bodies: `central_body` fixed at the origin, the other a massless test
    /// particle in its field (the single-body rungs, μ = GM of the central body).
    TestParticle,
    /// Oracle stage B (REF-006, WP17): the first body other than `central_body` is a massless
    /// particle integrated in the central body's frame (μ = GM of the central body) and pulled by
    /// every other listed body, whose positions come from the DE440 SPK (third-body terms with
    /// the indirect part; the field bodies are not integrated). Initial conditions `de440-bsp`.
    EphemerisField,
}

impl Mode {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Mode::NBody => "n-body",
            Mode::TestParticle => "test-particle",
            Mode::EphemerisField => "ephemeris-field",
        }
    }
}

/// A body a scenario may name. Planets beyond Venus are their system barycentres, as in DE440
/// (`GM4…GM9` are system GMs); `EMB` is the Earth–Moon barycentre as one body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum BodyName {
    /// The Sun (NAIF 10).
    Sun,
    /// Mercury (NAIF 1; the barycentre is the planet).
    Mercury,
    /// Venus (NAIF 2; the barycentre is the planet).
    Venus,
    /// The Earth (NAIF 399).
    Earth,
    /// The Moon (NAIF 301).
    Moon,
    /// The Earth–Moon barycentre as one body (NAIF 3).
    #[serde(rename = "EMB")]
    Emb,
    /// Mars system barycentre (NAIF 4).
    Mars,
    /// Jupiter system barycentre (NAIF 5).
    Jupiter,
    /// Saturn system barycentre (NAIF 6).
    Saturn,
    /// Uranus system barycentre (NAIF 7).
    Uranus,
    /// Neptune system barycentre (NAIF 8).
    Neptune,
    /// Pluto system barycentre (NAIF 9).
    Pluto,
}

impl BodyName {
    /// The NAIF id.
    #[must_use]
    pub const fn naif(self) -> i32 {
        match self {
            BodyName::Sun => 10,
            BodyName::Mercury => 1,
            BodyName::Venus => 2,
            BodyName::Earth => 399,
            BodyName::Moon => 301,
            BodyName::Emb => 3,
            BodyName::Mars => 4,
            BodyName::Jupiter => 5,
            BodyName::Saturn => 6,
            BodyName::Uranus => 7,
            BodyName::Neptune => 8,
            BodyName::Pluto => 9,
        }
    }

    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            BodyName::Sun => "Sun",
            BodyName::Mercury => "Mercury",
            BodyName::Venus => "Venus",
            BodyName::Earth => "Earth",
            BodyName::Moon => "Moon",
            BodyName::Emb => "EMB",
            BodyName::Mars => "Mars",
            BodyName::Jupiter => "Jupiter",
            BodyName::Saturn => "Saturn",
            BodyName::Uranus => "Uranus",
            BodyName::Neptune => "Neptune",
            BodyName::Pluto => "Pluto",
        }
    }
}

/// Where the initial conditions come from (CI-002: `de440-header | de440-bsp@JD | horizons@JD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IcSource {
    /// The DE440 initial conditions in `header.440` (GROUP 1041, at `JDEPOC`).
    De440Header,
    /// `de440.bsp` evaluated at `epoch_jd_tdb`.
    De440Bsp,
    /// JPL Horizons vectors at `epoch_jd_tdb` (supplied to the runner; runners do no network I/O).
    Horizons,
}

impl IcSource {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            IcSource::De440Header => "de440-header",
            IcSource::De440Bsp => "de440-bsp",
            IcSource::Horizons => "horizons",
        }
    }
}

/// `[initial_conditions]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitialConditionsSpec {
    /// The source.
    pub source: IcSource,
    /// Epoch (TDB Julian date): required for `de440-bsp` and `horizons`; for `de440-header`,
    /// absent or equal to the header's `JDEPOC`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch_jd_tdb: Option<f64>,
    /// Re-barycentre the assembled initial conditions (n-body mode; absent: use the source's
    /// states as given). `relativistic` moves the origin to the relativistic SSB with the μᵢ* of
    /// Park et al. 2021 Eqs. 1–2, computed in the run's backend (NUM-018, WP25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub barycentre: Option<IcBarycentre>,
}

/// How the initial conditions are re-barycentred (`initial_conditions.barycentre`, WP25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IcBarycentre {
    /// Σ μᵢ* rᵢ = 0 and Σ (μ̇ᵢ* rᵢ + μᵢ* vᵢ) = 0 (Subbotin et al. 2023 Eq. 5), with
    /// μᵢ* = μᵢ [1 + vᵢ²/(2c²) − Σⱼ μⱼ/(2c² rᵢⱼ)] (Park et al. 2021 Eq. 2) and μ̇ᵢ* from the
    /// Newtonian accelerations. Every body is translated by the same position and velocity.
    Relativistic,
}

impl IcBarycentre {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            IcBarycentre::Relativistic => "relativistic",
        }
    }
}

/// Which numbered asteroids are Major bodies (`forces.major_asteroids`, NUM-009, WP25). The
/// others of `forces.asteroids` stay Minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MajorAsteroids {
    /// None: every asteroid is Minor (Newtonian from the Majors only).
    #[default]
    None,
    /// (1) Ceres, (2) Pallas, (4) Vesta, (7) Iris and (324) Bamberga, the five asteroids that
    /// obey the EIH equations in the EPM2021 model of Subbotin et al. 2023 §2 (with the Sun, the
    /// planets, the Moon and Pluto: their 16 Major bodies).
    Epm2021,
    /// Every asteroid of `forces.asteroids`: the all-mutual reference of the tiering check.
    All,
}

impl MajorAsteroids {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            MajorAsteroids::None => "none",
            MajorAsteroids::Epm2021 => "epm2021",
            MajorAsteroids::All => "all",
        }
    }

    /// True for [`MajorAsteroids::None`] (the serde skip test).
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == MajorAsteroids::None
    }

    /// The asteroid numbers of [`MajorAsteroids::Epm2021`].
    pub const EPM2021: [u32; 5] = [1, 2, 4, 7, 324];
}

/// The GM set (CI-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GmSet {
    /// DE440 (`header.440` GROUP 1041, as tabulated in `ferroastro-constants`).
    De440,
}

/// `[constants]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConstantsSpec {
    /// The GM set.
    pub gm_set: GmSet,
}

/// PPN parameters of the relativistic terms (`ppn{beta,gamma}`, CI-002).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Ppn {
    /// PPN β.
    pub beta: f64,
    /// PPN γ.
    pub gamma: f64,
}

/// `[forces]`: exactly the CI-002 switches. Every switch but `newton` defaults to off.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the CI-002 switches are independent on/off flags, spelled as in the spec"
)]
pub struct Forces {
    /// Newtonian point-mass gravity.
    pub newton: bool,
    /// Solar J2 (DE440 `J2SUN`, `ASUN`; with the reaction on the Sun in N-body mode).
    #[serde(default)]
    pub sun_j2: bool,
    /// 1PN Schwarzschild field of the central body (test-particle mode).
    #[serde(default)]
    pub gr_1pn_central: bool,
    /// 1PN Einstein–Infeld–Hoffmann N-body equations.
    #[serde(default)]
    pub gr_eih: bool,
    /// PPN β, γ of the relativistic terms (default: the DE440 values β = γ = 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppn: Option<Ppn>,
    /// Lense–Thirring field of the spinning central body (DE440 solar spin).
    #[serde(default)]
    pub lense_thirring: bool,
    /// Number of numbered asteroids added as Minor bodies: the first N numbered asteroids of
    /// header.440 in header order (N <= 343), states from the REF-003 sampled file.
    #[serde(default)]
    pub asteroids: u32,
    /// The identified Kuiper-belt objects of header.440 as Minor bodies (REF-003, provisional
    /// while OI-24 is open; not a CI-002 switch name, WP15).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub kbos: bool,
    /// The DE440 KBO ring: 36 fixed Ring masses at 44 au in the ecliptic (provisional, OI-24;
    /// not a CI-002 switch name, WP15).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub kbo_ring: bool,
    /// Which of the `asteroids` are Major bodies instead of Minor (NUM-009; not a CI-002 switch
    /// name, WP25). Default `none`.
    #[serde(default, skip_serializing_if = "MajorAsteroids::is_none")]
    pub major_asteroids: MajorAsteroids,
    /// The velocity-independent GR effective potential of the Sun, `−3(GM☉)²/(c²r²)` per unit
    /// mass (Nobili & Roxburgh 1986), with the reaction on the Sun: the GR-potential rung of the
    /// Wisdom–Holman integrator (n-body mode; not a CI-002 switch name, WP29). It gives the 1PN
    /// perihelion advance, but the mean motion is off by O(GM/(ac²)).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gr_potential: bool,
}

/// The force switches of CI-002, as written under `[forces]`.
pub const SWITCHES: [&str; 11] = [
    "newton",
    "sun_j2",
    "gr_1pn_central",
    "gr_eih",
    "ppn",
    "lense_thirring",
    "asteroids",
    "kbos",
    "kbo_ring",
    "major_asteroids",
    "gr_potential",
];

impl Forces {
    /// The switches this scenario turns on (`ppn` counts when present), in [`SWITCHES`] order.
    #[must_use]
    pub fn requested(&self) -> Vec<&'static str> {
        let on = [
            self.newton,
            self.sun_j2,
            self.gr_1pn_central,
            self.gr_eih,
            self.ppn.is_some(),
            self.lense_thirring,
            self.asteroids > 0,
            self.kbos,
            self.kbo_ring,
            self.major_asteroids != MajorAsteroids::None,
            self.gr_potential,
        ];
        SWITCHES
            .iter()
            .zip(on)
            .filter_map(|(&s, on)| on.then_some(s))
            .collect()
    }
}

/// Integrator names known to the scenario spec (ADR-0008 and the M4/M5 integrators).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IntegratorName {
    /// IAS15-class 15th-order Gauss–Radau (default, ADR-0008).
    Ias15,
    /// Adams–Bashforth–Moulton (WP24).
    Abm,
    /// Wisdom–Holman / `WHFast` (WP29).
    Whfast,
    /// Gauss–Legendre implicit Runge–Kutta, order 16 (WP29).
    Irk16,
}

impl IntegratorName {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            IntegratorName::Ias15 => "ias15",
            IntegratorName::Abm => "abm",
            IntegratorName::Whfast => "whfast",
            IntegratorName::Irk16 => "irk16",
        }
    }
}

/// Arithmetic of the run (DEP-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Precision {
    /// IEEE-754 binary64.
    F64,
    /// Double-double (`ferroastro-real`).
    Dd,
    /// The NUM-007 ephemeris precision mode (Subbotin et al. 2023 "mixed"): state, integrator
    /// sums and coefficients in double-double, forces in f64 (n-body mode, WP25).
    Mixed,
}

impl Precision {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Precision::F64 => "f64",
            Precision::Dd => "dd",
            Precision::Mixed => "mixed",
        }
    }
}

/// Predictor–corrector mode of the Adams–Bashforth–Moulton integrator (WP24, NUM-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AbmCorrector {
    /// P, E, C, E.
    Pece,
    /// P(EC)²: P, E, C, E, C (Subbotin et al. 2023; the default).
    Pecec,
}

impl AbmCorrector {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            AbmCorrector::Pece => "pece",
            AbmCorrector::Pecec => "pecec",
        }
    }
}

/// `[integrator]`. Unset settings take the integrator's defaults. `epsilon`, `pc_tolerance` and
/// `dt0_days` configure IAS15 only; `order`, `corrector`, `step_days` and `compensated` configure
/// ABM only (a key of the other integrator is refused).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IntegratorSpec {
    /// The integrator.
    pub name: IntegratorName,
    /// The arithmetic.
    pub precision: Precision,
    /// IAS15 step-control precision ε (default 1e-9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epsilon: Option<f64>,
    /// IAS15 predictor–corrector convergence threshold (default 1e-16).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pc_tolerance: Option<f64>,
    /// First trial step in days (N-body default 0.1 d; the controller adapts it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dt0_days: Option<f64>,
    /// ABM order, 8 … 14 (default 13).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 8, max = 14))]
    pub order: Option<u32>,
    /// ABM predictor–corrector mode (default `pecec`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrector: Option<AbmCorrector>,
    /// Fixed step in days of ABM (default 1/16 d), `WHFast` and IRK16 (default 1 d); its sign
    /// follows the span.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_days: Option<f64>,
    /// Kahan-compensated state update of ABM and IRK16 (default true; false gives the plain
    /// sums).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensated: Option<bool>,
    /// `WHFast` symplectic-corrector order: 0 (none), 3, 5, 7 or 11 (default 11; WP29).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wh_corrector: Option<u32>,
    /// The N-body pair kernels (NUM-021, WP26): `pairwise` (default) or `soa`. Not a force
    /// switch: it selects FerroAstro's arithmetic, not the physics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub force_kernel: Option<ForceKernel>,
}

/// The N-body pair kernels (`integrator.force_kernel`, NUM-021, WP26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ForceKernel {
    /// Each pair visited once (`i < j`), fused `r²`, `1/(r²√r²)` (the M3/M4 kernels).
    #[default]
    Pairwise,
    /// Structure-of-arrays kernels (`ferroastro_forces::soa`): exact `1/r³`, no FMA, sources of
    /// each target summed in storage order; scalar and SIMD lanes are bitwise identical.
    Soa,
}

impl ForceKernel {
    /// The spelling in scenario files.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ForceKernel::Pairwise => "pairwise",
            ForceKernel::Soa => "soa",
        }
    }
}

/// `[span]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Span {
    /// Length in Julian years of 365.25 d TDB from the IC epoch; negative integrates backwards.
    pub years: f64,
    /// Stop earlier, once this many perihelion passages have been found (the single-body rungs
    /// use orbits + 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_after_passages: Option<u32>,
}

/// `[output]`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OutputSpec {
    /// Sample every body's state every this many days (from dense output; absent: no samples).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cadence_days: Option<f64>,
    /// Record only these bodies in each sample (absent: every body). Keeps the output of runs
    /// with hundreds of small bodies small (WP15).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_bodies: Option<Vec<BodyName>>,
    /// Bodies whose perihelion passages relative to `central_body` are located (NUM-010).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub perihelion: Vec<BodyName>,
}

/// A scenario file (CI-002).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "FerroAstro scenario (CI-002)",
    description = "Tool-agnostic scenario spec read by FerroAstro and every oracle runner. \
                   Generated from ferroastro-scenario; do not edit by hand."
)]
pub struct ScenarioSpec {
    /// Always 1.
    #[schemars(range(min = 1, max = 1))]
    pub schema_version: u32,
    /// Short identifier.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// Integration mode.
    pub mode: Mode,
    /// The bodies, in storage order.
    pub bodies: Vec<BodyName>,
    /// The body that test particles orbit and that perihelia are measured from.
    pub central_body: BodyName,
    /// Initial conditions.
    pub initial_conditions: InitialConditionsSpec,
    /// Constants.
    pub constants: ConstantsSpec,
    /// Force switches.
    pub forces: Forces,
    /// Integrator.
    pub integrator: IntegratorSpec,
    /// Span.
    pub span: Span,
    /// Outputs.
    #[serde(default)]
    pub output: OutputSpec,
}

/// The JSON Schema of [`ScenarioSpec`] (draft 2020-12), pretty-printed with a final newline. The
/// committed copy is `ci/scenarios/scenario.schema.json`.
#[must_use]
pub fn json_schema() -> String {
    let schema = schemars::schema_for!(ScenarioSpec);
    let mut s = serde_json::to_string_pretty(&schema).unwrap_or_default();
    s.push('\n');
    s
}

fn value_err(
    name: &str,
    field: &'static str,
    value: &dyn ToString,
    problem: &str,
) -> ScenarioError {
    ScenarioError::Value {
        name: name.to_owned(),
        field,
        value: value.to_string(),
        problem: problem.to_owned(),
    }
}

impl ScenarioSpec {
    /// Parse and validate a scenario file.
    ///
    /// # Errors
    /// `FA-E1200` for invalid TOML or a schema violation, `FA-E1202` for an unknown key under
    /// `[forces]` (named), `FA-E1203` for invalid or inconsistent values.
    pub fn parse(text: &str) -> Result<Self, ScenarioError> {
        let table: toml::Table = toml::from_str(text).map_err(|e| ScenarioError::Malformed {
            name: "?".into(),
            problem: e.to_string(),
        })?;
        let name = table
            .get("name")
            .and_then(toml::Value::as_str)
            .unwrap_or("?")
            .to_owned();
        // Nested `if let` (not a let-chain): the workspace MSRV is 1.85.
        if let Some(forces) = table.get("forces").and_then(toml::Value::as_table) {
            if let Some(unknown) = forces.keys().find(|k| !SWITCHES.contains(&k.as_str())) {
                return Err(ScenarioError::UnknownSwitch {
                    name,
                    switch: unknown.clone(),
                    valid: SWITCHES.join(", "),
                });
            }
        }
        let spec: Self =
            table
                .try_into()
                .map_err(|e: toml::de::Error| ScenarioError::Malformed {
                    name: name.clone(),
                    problem: e.to_string(),
                })?;
        spec.validate()?;
        Ok(spec)
    }

    /// Check values and their consistency (independent of any runner's capabilities).
    ///
    /// # Errors
    /// `FA-E1203` naming the field.
    #[expect(
        clippy::too_many_lines,
        reason = "one flat list of independent checks reads better than split helpers"
    )]
    pub fn validate(&self) -> Result<(), ScenarioError> {
        let n = self.name.as_str();
        let err = |field, value: &dyn ToString, problem| {
            Err(value_err(n, field, &value.to_string(), problem))
        };
        if self.schema_version != SCHEMA_VERSION {
            return err("schema_version", &self.schema_version, "only 1 is defined");
        }
        if self.name.trim().is_empty() {
            return err("name", &"\"\"", "must not be empty");
        }
        if self.bodies.is_empty() {
            return err("bodies", &"[]", "at least one body");
        }
        for (i, b) in self.bodies.iter().enumerate() {
            if self.bodies.iter().skip(i + 1).any(|c| c == b) {
                return err("bodies", &b.label(), "listed twice");
            }
        }
        let has = |b: BodyName| self.bodies.contains(&b);
        if has(BodyName::Emb) && (has(BodyName::Earth) || has(BodyName::Moon)) {
            return err(
                "bodies",
                &"EMB with Earth or Moon",
                "the Earth-Moon barycentre replaces the Earth and the Moon",
            );
        }
        if !has(self.central_body) {
            return err(
                "central_body",
                &self.central_body.label(),
                "must be one of `bodies`",
            );
        }
        if self.mode == Mode::TestParticle && self.bodies.len() != 2 {
            return err(
                "bodies",
                &self.bodies.len(),
                "test-particle mode takes exactly two bodies: the central body and the particle",
            );
        }
        if self.mode == Mode::EphemerisField {
            if self.bodies.len() < 3 {
                return err(
                    "bodies",
                    &self.bodies.len(),
                    "ephemeris-field mode takes the central body, the particle and >= 1 field body",
                );
            }
            if self.initial_conditions.source != IcSource::De440Bsp {
                return err(
                    "initial_conditions.source",
                    &self.initial_conditions.source.label(),
                    "ephemeris-field mode reads the field and the initial state from de440-bsp",
                );
            }
        }
        let ic = self.initial_conditions;
        match (ic.source, ic.epoch_jd_tdb) {
            (IcSource::De440Bsp | IcSource::Horizons, None) => {
                return err(
                    "initial_conditions.epoch_jd_tdb",
                    &"absent",
                    "required for this source",
                );
            }
            (_, Some(jd)) if !jd.is_finite() => {
                return err("initial_conditions.epoch_jd_tdb", &jd, "must be finite");
            }
            _ => {}
        }
        if self.forces.major_asteroids != MajorAsteroids::None && self.forces.asteroids == 0 {
            return err(
                "forces.major_asteroids",
                &self.forces.major_asteroids.label(),
                "selects Major bodies among forces.asteroids, which is 0",
            );
        }
        if self.mode != Mode::NBody {
            if let Some(b) = self.initial_conditions.barycentre {
                return err(
                    "initial_conditions.barycentre",
                    &b.label(),
                    "re-barycentring applies to n-body initial conditions only",
                );
            }
        }
        if let Some(p) = self.forces.ppn {
            if !(p.beta.is_finite() && p.gamma.is_finite()) {
                return err(
                    "forces.ppn",
                    &format!("{{beta = {}, gamma = {}}}", p.beta, p.gamma),
                    "must be finite",
                );
            }
            if !(self.forces.gr_1pn_central || self.forces.gr_eih || self.forces.lense_thirring) {
                return err(
                    "forces.ppn",
                    &"set",
                    "ppn needs a relativistic term (gr_1pn_central, gr_eih or lense_thirring)",
                );
            }
        }
        if !(self.span.years.is_finite() && self.span.years != 0.0) {
            return err("span.years", &self.span.years, "finite and non-zero");
        }
        if self.span.stop_after_passages == Some(0) {
            return err("span.stop_after_passages", &0, "at least 1");
        }
        if let Some(c) = self.output.cadence_days {
            if !(c.is_finite() && c > 0.0) {
                return err("output.cadence_days", &c, "finite and > 0");
            }
        }
        for b in &self.output.perihelion {
            if !has(*b) || *b == self.central_body {
                return err(
                    "output.perihelion",
                    &b.label(),
                    "must be one of `bodies` other than `central_body`",
                );
            }
        }
        if self.span.stop_after_passages.is_some() && self.output.perihelion.len() != 1 {
            return err(
                "span.stop_after_passages",
                &self.output.perihelion.len(),
                "needs exactly one body in output.perihelion",
            );
        }
        let ig = &self.integrator;
        let owners: [(&'static str, bool, &[IntegratorName], &str); 8] = [
            (
                "integrator.epsilon",
                ig.epsilon.is_some(),
                &[IntegratorName::Ias15],
                "IAS15",
            ),
            (
                "integrator.pc_tolerance",
                ig.pc_tolerance.is_some(),
                &[IntegratorName::Ias15],
                "IAS15",
            ),
            (
                "integrator.dt0_days",
                ig.dt0_days.is_some(),
                &[IntegratorName::Ias15],
                "IAS15",
            ),
            (
                "integrator.order",
                ig.order.is_some(),
                &[IntegratorName::Abm],
                "ABM",
            ),
            (
                "integrator.corrector",
                ig.corrector.is_some(),
                &[IntegratorName::Abm],
                "ABM",
            ),
            (
                "integrator.step_days",
                ig.step_days.is_some(),
                &[
                    IntegratorName::Abm,
                    IntegratorName::Whfast,
                    IntegratorName::Irk16,
                ],
                "the fixed-step integrators (ABM, whfast, irk16)",
            ),
            (
                "integrator.compensated",
                ig.compensated.is_some(),
                &[IntegratorName::Abm, IntegratorName::Irk16],
                "ABM and IRK16",
            ),
            (
                "integrator.wh_corrector",
                ig.wh_corrector.is_some(),
                &[IntegratorName::Whfast],
                "WHFast",
            ),
        ];
        if let Some((field, _, _, owner)) = owners
            .iter()
            .find(|(_, set, who, _)| *set && !who.contains(&ig.name))
        {
            return err(
                field,
                &ig.name.label(),
                &format!(
                    "configures {owner} only, not integrator {}",
                    ig.name.label()
                ),
            );
        }
        if let Some(c) = ig.wh_corrector {
            if ![0, 3, 5, 7, 11].contains(&c) {
                return err("integrator.wh_corrector", &c, "one of 0, 3, 5, 7, 11");
            }
        }
        if self.forces.gr_potential && (self.forces.gr_eih || self.forces.gr_1pn_central) {
            return err(
                "forces.gr_potential",
                &true,
                "the GR effective potential replaces gr_eih / gr_1pn_central; do not combine them",
            );
        }
        if ig.force_kernel == Some(ForceKernel::Soa) {
            if self.mode != Mode::NBody {
                return err(
                    "integrator.force_kernel",
                    &"soa",
                    "the SoA kernels are N-body pair kernels (mode = \"n-body\" only)",
                );
            }
            if ig.precision == Precision::Mixed {
                return err(
                    "integrator.force_kernel",
                    &"soa",
                    "not with precision = \"mixed\": the NUM-007 mixed mode compensates every pair \
                     contribution (WP25), the SoA kernel sums each body's sources in one lane",
                );
            }
        }
        if let Some(o) = ig.order {
            if !(8..=14).contains(&o) {
                return err("integrator.order", &o, "8 <= order <= 14 (NUM-002)");
            }
        }
        for (field, v) in [
            ("integrator.epsilon", self.integrator.epsilon),
            ("integrator.pc_tolerance", self.integrator.pc_tolerance),
            ("integrator.dt0_days", self.integrator.dt0_days),
            ("integrator.step_days", self.integrator.step_days),
        ] {
            if let Some(v) = v {
                if !(v.is_finite() && v > 0.0) {
                    return err(field, &v, "finite and > 0");
                }
            }
        }
        Ok(())
    }

    /// Span in TDB seconds (signed).
    #[must_use]
    pub fn span_seconds(&self) -> f64 {
        self.span.years * DAYS_PER_JULIAN_YEAR * ferroastro_core::time::SECONDS_PER_DAY
    }

    /// Refuse every requested switch, integrator or option that `supports` rejects, naming it
    /// (REF-011). `supports(switch, mode)` gets the [`SWITCHES`] spelling, or
    /// `integrator.<name>` / `integrator.precision.<p>` / `initial_conditions.source.<s>`.
    ///
    /// # Errors
    /// `FA-E1201` naming the first refused item, with `runner` and the reason `supports`
    /// returned.
    pub fn refuse_unsupported(
        &self,
        runner: &str,
        supports: impl Fn(&str, Mode) -> Result<(), String>,
    ) -> Result<(), ScenarioError> {
        let mut items: Vec<String> = Vec::new();
        if !self.forces.newton {
            items.push("newton=false".into());
        }
        items.extend(self.forces.requested().iter().map(|s| (*s).to_owned()));
        items.push(format!("integrator.{}", self.integrator.name.label()));
        items.push(format!(
            "integrator.precision.{}",
            self.integrator.precision.label()
        ));
        items.push(format!(
            "initial_conditions.source.{}",
            self.initial_conditions.source.label()
        ));
        for item in items {
            if let Err(reason) = supports(&item, self.mode) {
                let switch = if SWITCHES.contains(&item.as_str()) || item == "newton=false" {
                    format!("forces.{item}")
                } else {
                    item
                };
                return Err(ScenarioError::Unsupported {
                    name: self.name.clone(),
                    switch,
                    runner: runner.to_owned(),
                    reason,
                });
            }
        }
        Ok(())
    }

    /// SHA-256 (hex) of the scenario's canonical JSON form (field order fixed by the type), used
    /// to tie a checkpoint to its scenario.
    #[must_use]
    pub fn digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let json = serde_json::to_string(self).unwrap_or_default();
        let d = Sha256::digest(json.as_bytes());
        d.iter().fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
    }
}
