// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Toggles and parameters (DESIGN §3.1, §4.3), their validation, and the scenario a
//! configuration builds ([`ViewerConfig::to_scenario`]).

#![forbid(unsafe_code)]

use crate::ViewerError;
use ferroastro_scenario::ScenarioSpec;
use std::fmt::Write as _;

/// Toggle bits (DESIGN §3.1).
pub mod bits {
    /// Venus (Newtonian point mass).
    pub const VENUS: u32 = 1 << 0;
    /// Earth–Moon barycentre as one body.
    pub const EMB: u32 = 1 << 1;
    /// Mars system.
    pub const MARS: u32 = 1 << 2;
    /// Jupiter system.
    pub const JUPITER: u32 = 1 << 3;
    /// Saturn system.
    pub const SATURN: u32 = 1 << 4;
    /// Uranus system.
    pub const URANUS: u32 = 1 << 5;
    /// Neptune system.
    pub const NEPTUNE: u32 = 1 << 6;
    /// Solar J2 (DE440 `J2SUN`, with its reaction on the Sun).
    pub const SUN_J2: u32 = 1 << 7;
    /// 1PN Schwarzschild field of the Sun (IAS15) or the GR effective potential (`WHFast`).
    pub const GR_1PN: u32 = 1 << 8;
    /// Solar Lense–Thirring (IAS15 only).
    pub const LENSE_THIRRING: u32 = 1 << 9;
    /// The first `ASTEROID_COUNT` numbered asteroids of header.440 (IAS15 only).
    pub const ASTEROIDS: u32 = 1 << 10;
    /// 1PN EIH among all bodies (IAS15 only; replaces [`GR_1PN`]).
    pub const EIH: u32 = 1 << 11;
    /// The seven planet bits.
    pub const PLANETS: u32 = 0x7f;
    /// Every defined bit.
    pub const ALL: u32 = 0xfff;
    /// The default mask: bits 0–9 (planets, J2, GR, Lense–Thirring).
    pub const DEFAULT: u32 = 0x3ff;
}

/// Parameter ids of `fav_set_param` / `fav_get_param` (DESIGN §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Param {
    /// 0 System, 1 Rung lab.
    Mode = 0,
    /// 0 AUTO, 1 `WHFast`, 2 IAS15.
    Engine = 1,
    /// PPN β.
    PpnBeta = 2,
    /// PPN γ.
    PpnGamma = 3,
    /// `WHFast` step (days), 0.125 … 4.
    WhStepDays = 4,
    /// IAS15 ε, 1e-12 … 1e-6.
    Ias15Epsilon = 5,
    /// Run span (Julian years), 1 … 20000.
    SpanYears = 6,
    /// Display sample cadence (days), 0.5 … 50.
    DisplayCadenceDays = 7,
    /// Number of asteroids of bit 10, 1 … 343.
    AsteroidCount = 8,
    /// Rung-lab dynamics: 0 Newton, 1 SR-a, 2 SR-b harmonic, 3 SR-b areal, 4 GR PPN.
    Rung = 9,
    /// Solar J2 instead of DE440 `J2SUN` (NaN = DE440), 0 … 1e-3.
    J2Override = 10,
    /// Factor on the DE440 solar `G·S`, 0 … 1e6.
    SpinScale = 11,
    /// 1: fit the Park17 Table 2 terms the span resolves; 0: linear only.
    FitPeriodic = 12,
}

/// Number of parameters.
pub const PARAM_COUNT: usize = 13;

impl Param {
    /// The parameter of an id.
    #[must_use]
    pub const fn from_id(id: u32) -> Option<Self> {
        Some(match id {
            0 => Self::Mode,
            1 => Self::Engine,
            2 => Self::PpnBeta,
            3 => Self::PpnGamma,
            4 => Self::WhStepDays,
            5 => Self::Ias15Epsilon,
            6 => Self::SpanYears,
            7 => Self::DisplayCadenceDays,
            8 => Self::AsteroidCount,
            9 => Self::Rung,
            10 => Self::J2Override,
            11 => Self::SpinScale,
            12 => Self::FitPeriodic,
            _ => return None,
        })
    }

    /// Name in metadata.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mode => "mode",
            Self::Engine => "engine",
            Self::PpnBeta => "beta",
            Self::PpnGamma => "gamma",
            Self::WhStepDays => "wh_step_days",
            Self::Ias15Epsilon => "ias15_epsilon",
            Self::SpanYears => "span_years",
            Self::DisplayCadenceDays => "display_cadence_days",
            Self::AsteroidCount => "asteroid_count",
            Self::Rung => "rung",
            Self::J2Override => "j2_override",
            Self::SpinScale => "spin_scale",
            Self::FitPeriodic => "fit_periodic",
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

/// IAS15 ε of the ladder and the PHY-012 runs (DESIGN §3.2).
pub const DEFAULT_IAS15_EPSILON: f64 = 1e-9;
/// `WHFast` step (days) of the GR-potential rung (WP29).
pub const DEFAULT_WH_STEP_DAYS: f64 = 1.0;
/// Internal sample cadence (days) of IAS15 runs: Mercury's states for the passage search.
pub const IAS15_SAMPLE_DAYS: f64 = 1.0;
/// `WHFast` symplectic corrector order (WP29 default).
pub const WH_CORRECTOR: u32 = 11;

/// The defaults of DESIGN §4.3, in id order.
pub const DEFAULTS: [f64; PARAM_COUNT] = [
    0.0,
    0.0,
    1.0,
    1.0,
    DEFAULT_WH_STEP_DAYS,
    DEFAULT_IAS15_EPSILON,
    1000.0,
    4.0,
    16.0,
    4.0,
    f64::NAN,
    1.0,
    1.0,
];

/// The integrator of a System-mode run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    /// `WHFast`, step `WH_STEP_DAYS`, corrector 11.
    WhFast,
    /// IAS15, f64, ε = `IAS15_EPSILON`.
    Ias15,
}

/// Rung-lab dynamics (DESIGN §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabRung {
    /// Newtonian only.
    Newton,
    /// SR-a (`SrKepler`).
    SrA,
    /// SR-b, isotropic/harmonic gauge.
    SrBHarmonic,
    /// SR-b, areal gauge (pedagogical).
    SrBAreal,
    /// 1PN Schwarzschild with PPN β, γ.
    GrPpn,
}

/// Toggles and parameters of the next run.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerConfig {
    mask: u32,
    params: [f64; PARAM_COUNT],
}

impl Default for ViewerConfig {
    fn default() -> Self {
        Self {
            mask: bits::DEFAULT,
            params: DEFAULTS,
        }
    }
}

fn range_err(name: &str, value: f64, valid: &str) -> ViewerError {
    let code = if value.is_finite() {
        "FA-E0002"
    } else {
        "FA-E0001"
    };
    ViewerError::arg(format!(
        "{code}: parameter {name} = {value} is out of range; valid: {valid}"
    ))
}

/// `value` as an integer in `0..=max`, if it is one exactly.
pub(crate) fn as_index(value: f64, max: u32) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value > f64::from(max) || value.fract() != 0.0 {
        return None;
    }
    (0..=max).find(|&k| (f64::from(k) - value).abs() < 0.5)
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

impl ViewerConfig {
    /// The toggle mask.
    #[must_use]
    pub const fn mask(&self) -> u32 {
        self.mask
    }

    /// Set the toggle mask.
    ///
    /// # Errors
    /// `FAV_E_ARG` for a reserved bit (12–31).
    pub fn set_mask(&mut self, mask: u32) -> Result<(), ViewerError> {
        if mask & !bits::ALL != 0 {
            return Err(ViewerError::arg(format!(
                "FA-E0002: toggle mask {mask:#x} sets reserved bits {:#x} (only bits 0-11 are defined)",
                mask & !bits::ALL
            )));
        }
        self.mask = mask;
        Ok(())
    }

    /// A parameter's value.
    #[must_use]
    pub fn get(&self, p: Param) -> f64 {
        self.params.get(p.index()).copied().unwrap_or(f64::NAN)
    }

    /// Set a parameter after checking its range (DESIGN §4.3).
    ///
    /// # Errors
    /// `FAV_E_ARG` for a value out of range.
    pub fn set(&mut self, p: Param, value: f64) -> Result<(), ViewerError> {
        let name = p.name();
        let within = |lo: f64, hi: f64| (lo..=hi).contains(&value);
        let ok = match p {
            Param::Mode | Param::FitPeriodic => as_index(value, 1).is_some(),
            Param::Engine => as_index(value, 2).is_some(),
            Param::PpnBeta | Param::PpnGamma => value.is_finite() && (value - 1.0).abs() <= 100.0,
            Param::WhStepDays => within(0.125, 4.0),
            Param::Ias15Epsilon => within(1e-12, 1e-6),
            Param::SpanYears => within(1.0, 20000.0),
            Param::DisplayCadenceDays => within(0.5, 50.0),
            Param::AsteroidCount => as_index(value, 343).is_some_and(|k| k >= 1),
            Param::Rung => as_index(value, 4).is_some(),
            Param::J2Override => value.is_nan() || within(0.0, 1e-3),
            Param::SpinScale => within(0.0, 1e6),
        };
        if !ok {
            let valid = match p {
                Param::Mode => "0 (System) or 1 (Rung lab)",
                Param::FitPeriodic => "0 or 1",
                Param::Engine => "0 (AUTO), 1 (WHFAST) or 2 (IAS15)",
                Param::PpnBeta | Param::PpnGamma => "finite, |x - 1| <= 100",
                Param::WhStepDays => "0.125 ... 4 days",
                Param::Ias15Epsilon => "1e-12 ... 1e-6",
                Param::SpanYears => "1 ... 20000 Julian years",
                Param::DisplayCadenceDays => "0.5 ... 50 days",
                Param::AsteroidCount => "an integer 1 ... 343",
                Param::Rung => "an integer 0 ... 4",
                Param::J2Override => "NaN (DE440 J2SUN) or 0 ... 1e-3",
                Param::SpinScale => "0 ... 1e6",
            };
            return Err(range_err(name, value, valid));
        }
        if let Some(slot) = self.params.get_mut(p.index()) {
            *slot = value;
        }
        Ok(())
    }

    /// True in Rung lab.
    #[must_use]
    pub fn is_lab(&self) -> bool {
        as_index(self.get(Param::Mode), 1) == Some(1)
    }

    /// The requested engine: `None` for AUTO.
    #[must_use]
    pub fn requested_engine(&self) -> Option<EngineKind> {
        match as_index(self.get(Param::Engine), 2) {
            Some(1) => Some(EngineKind::WhFast),
            Some(2) => Some(EngineKind::Ias15),
            _ => None,
        }
    }

    /// The Rung-lab dynamics.
    #[must_use]
    pub fn rung(&self) -> LabRung {
        match as_index(self.get(Param::Rung), 4) {
            Some(0) => LabRung::Newton,
            Some(1) => LabRung::SrA,
            Some(2) => LabRung::SrBHarmonic,
            Some(3) => LabRung::SrBAreal,
            _ => LabRung::GrPpn,
        }
    }

    /// PPN β and γ.
    #[must_use]
    pub fn ppn(&self) -> (f64, f64) {
        (self.get(Param::PpnBeta), self.get(Param::PpnGamma))
    }

    /// True when β = γ = 1 exactly (the DE440 values; the GR potential's only case).
    #[must_use]
    pub fn ppn_is_gr(&self) -> bool {
        let (b, g) = self.ppn();
        same(b, 1.0) && same(g, 1.0)
    }

    /// Number of asteroids of bit 10.
    #[must_use]
    pub fn asteroid_count(&self) -> u32 {
        as_index(self.get(Param::AsteroidCount), 343).unwrap_or(16)
    }

    /// Solar J2 override, if any.
    #[must_use]
    pub fn j2_override(&self) -> Option<f64> {
        let j2 = self.get(Param::J2Override);
        (!j2.is_nan()).then_some(j2)
    }

    /// Factor on the DE440 solar spin, if not 1.
    #[must_use]
    pub fn spin_scale(&self) -> Option<f64> {
        let s = self.get(Param::SpinScale);
        (!same(s, 1.0)).then_some(s)
    }

    /// True when the run uses DE440 J2 and spin and ε ≤ 1e-9 (DESIGN §4.3).
    #[must_use]
    pub fn is_nominal(&self) -> bool {
        self.j2_override().is_none()
            && self.spin_scale().is_none()
            && self.get(Param::Ias15Epsilon) <= DEFAULT_IAS15_EPSILON
    }

    /// True when the linear-plus-periodic fit is requested.
    #[must_use]
    pub fn fit_periodic(&self) -> bool {
        as_index(self.get(Param::FitPeriodic), 1) == Some(1)
    }

    /// The parameters as `(name, value)` in id order.
    #[must_use]
    pub fn params(&self) -> Vec<(&'static str, f64)> {
        (0..u32::try_from(PARAM_COUNT).unwrap_or(0))
            .filter_map(Param::from_id)
            .map(|p| (p.name(), self.get(p)))
            .collect()
    }

    /// Check the toggles against the mode (DESIGN §3.1, §3.3).
    ///
    /// # Errors
    /// `FAV_E_ARG` for bits 8 and 11 together, or planet, asteroid or EIH bits in Rung lab.
    pub fn validate(&self) -> Result<(), ViewerError> {
        let m = self.mask;
        if m & bits::GR_1PN != 0 && m & bits::EIH != 0 {
            return Err(ViewerError::arg(
                "FA-E0002: toggle bits 8 (GR 1PN of the Sun) and 11 (EIH among all bodies) are \
                 mutually exclusive: EIH contains the Sun's 1PN field",
            ));
        }
        if self.is_lab() && m & (bits::PLANETS | bits::ASTEROIDS | bits::EIH) != 0 {
            return Err(ViewerError::arg(format!(
                "FA-E0002: Rung lab integrates Mercury as a test particle about a fixed Sun; \
                 clear the planet, asteroid and EIH bits (mask {m:#x}); RUNG selects the dynamics \
                 and bits 7 and 9 add solar J2 and Lense-Thirring"
            )));
        }
        Ok(())
    }

    /// True if the System-mode toggles can be integrated with `WHFast` by this viewer: no
    /// Lense–Thirring, asteroids or EIH, and GR only as the GR potential, i.e. with β = γ = 1.
    /// (Propagate's `refuse_unsupported` remains the authority for what it accepts; this only
    /// excludes switches that the WH mapping cannot express.)
    #[must_use]
    pub fn wh_expressible(&self) -> bool {
        self.mask & bits::GR_1PN == 0 || self.ppn_is_gr()
    }

    /// The bodies of the System-mode scenario, Sun first (the Jacobi chain of `WHFast`).
    #[must_use]
    pub fn bodies(&self) -> Vec<&'static str> {
        let mut b = vec!["Sun", "Mercury"];
        for (bit, name) in [
            (bits::VENUS, "Venus"),
            (bits::EMB, "EMB"),
            (bits::MARS, "Mars"),
            (bits::JUPITER, "Jupiter"),
            (bits::SATURN, "Saturn"),
            (bits::URANUS, "Uranus"),
            (bits::NEPTUNE, "Neptune"),
        ] {
            if self.mask & bit != 0 {
                b.push(name);
            }
        }
        b
    }

    /// The internal sample cadence (days) of an engine: the WH step, or 1 d for IAS15.
    #[must_use]
    pub fn sample_days(&self, engine: EngineKind) -> f64 {
        match engine {
            EngineKind::WhFast => self.get(Param::WhStepDays),
            EngineKind::Ias15 => IAS15_SAMPLE_DAYS,
        }
    }

    /// The System-mode [`ScenarioSpec`] of this configuration on `engine` (DESIGN §3.1): the
    /// toggled bodies, the force switches, the DE440 `header.440` initial conditions, the span,
    /// and Mercury's states sampled at the engine's cadence. Built as scenario TOML and parsed,
    /// so the spec is exactly what `fav_scenario` exports.
    ///
    /// # Errors
    /// `FAV_E_ARG` if the scenario does not validate; `FAV_E_UNSUPPORTED` if `WHFast` cannot
    /// express GR with β, γ ≠ 1.
    pub fn to_scenario(&self, engine: EngineKind) -> Result<ScenarioSpec, ViewerError> {
        ScenarioSpec::parse(&self.scenario_toml(engine)?)
            .map_err(|e| ViewerError::arg(e.to_string()))
    }

    /// The scenario TOML of [`ViewerConfig::to_scenario`].
    ///
    /// # Errors
    /// As [`ViewerConfig::to_scenario`].
    pub fn scenario_toml(&self, engine: EngineKind) -> Result<String, ViewerError> {
        let m = self.mask;
        let on = |b: u32| m & b != 0;
        let wh = engine == EngineKind::WhFast;
        if wh && on(bits::GR_1PN) && !self.ppn_is_gr() {
            return Err(ViewerError::new(
                crate::FAV_E_UNSUPPORTED,
                "FA-E1201: WHFast represents GR only by the GR effective potential, which is the \
                 beta = gamma = 1 case; use IAS15 (gr_1pn_central with ppn) for other PPN values",
            ));
        }
        let (beta, gamma) = self.ppn();
        let span = self.get(Param::SpanYears);
        let quoted: Vec<String> = self.bodies().iter().map(|b| format!("{b:?}")).collect();
        let mut s = String::new();
        let _ = writeln!(s, "schema_version = 1");
        let _ = writeln!(
            s,
            "name = \"viewer-system-m{m:03x}-{}\"",
            if wh { "whfast" } else { "ias15" }
        );
        let _ = writeln!(
            s,
            "description = \"FerroAstro orbit viewer, System mode, toggle mask {m:#x} (docs/viewer/DESIGN.md)\""
        );
        let _ = writeln!(s, "mode = \"n-body\"");
        let _ = writeln!(s, "bodies = [{}]", quoted.join(", "));
        let _ = writeln!(s, "central_body = \"Sun\"");
        s.push_str("\n[initial_conditions]\nsource = \"de440-header\"\n");
        s.push_str("\n[constants]\ngm_set = \"de440\"\n");
        s.push_str("\n[forces]\nnewton = true\n");
        if on(bits::SUN_J2) {
            s.push_str("sun_j2 = true\n");
        }
        if on(bits::GR_1PN) {
            s.push_str(if wh {
                "gr_potential = true\n"
            } else {
                "gr_1pn_central = true\n"
            });
        }
        if on(bits::EIH) {
            s.push_str("gr_eih = true\n");
        }
        if on(bits::LENSE_THIRRING) {
            s.push_str("lense_thirring = true\n");
        }
        if on(bits::ASTEROIDS) {
            let _ = writeln!(s, "asteroids = {}", self.asteroid_count());
        }
        let relativistic = (!wh && on(bits::GR_1PN)) || on(bits::EIH) || on(bits::LENSE_THIRRING);
        if relativistic && !self.ppn_is_gr() {
            let _ = writeln!(s, "ppn = {{ beta = {beta:?}, gamma = {gamma:?} }}");
        }
        s.push_str("\n[integrator]\n");
        if wh {
            let _ = writeln!(
                s,
                "name = \"whfast\"\nprecision = \"f64\"\nstep_days = {:?}\nwh_corrector = {WH_CORRECTOR}",
                self.get(Param::WhStepDays)
            );
        } else {
            let _ = writeln!(
                s,
                "name = \"ias15\"\nprecision = \"f64\"\nepsilon = {:?}",
                self.get(Param::Ias15Epsilon)
            );
        }
        let _ = writeln!(s, "\n[span]\nyears = {span:?}");
        let _ = writeln!(
            s,
            "\n[output]\ncadence_days = {:?}",
            self.sample_days(engine)
        );
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: API-004
    #[test]
    fn defaults_and_ranges() {
        let mut c = ViewerConfig::default();
        assert_eq!(c.mask(), bits::DEFAULT);
        assert!(c.set_mask(1 << 12).is_err());
        assert!(c.set(Param::WhStepDays, 0.0).is_err());
        assert!(c.set(Param::AsteroidCount, 2.5).is_err()); // lint: allow-literal(a non-integer probe of an integer parameter)
        assert!(c.set(Param::J2Override, f64::NAN).is_ok());
        assert!(c.set(Param::Mode, 1.0).is_ok());
        assert!(c.is_lab());
        assert!(c.validate().is_err(), "planets are not allowed in Rung lab");
    }

    /// Verifies: API-004
    #[test]
    fn full_model_scenarios_parse() {
        let c = ViewerConfig::default();
        let ias = c.to_scenario(EngineKind::Ias15).unwrap();
        assert!(ias.forces.gr_1pn_central && ias.forces.lense_thirring && ias.forces.sun_j2);
        assert_eq!(ias.bodies.len(), 9);
        let mut k = ViewerConfig::default();
        k.set_mask(bits::GR_1PN).unwrap();
        let wh = k.to_scenario(EngineKind::WhFast).unwrap();
        assert!(wh.forces.gr_potential);
        k.set(Param::PpnBeta, 0.5).unwrap();
        assert!(k.to_scenario(EngineKind::WhFast).is_err());
        assert!(
            k.to_scenario(EngineKind::Ias15)
                .unwrap()
                .forces
                .ppn
                .is_some()
        );
    }
}
