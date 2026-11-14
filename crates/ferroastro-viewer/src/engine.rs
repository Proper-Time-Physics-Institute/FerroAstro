// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The viewer state machine ([`Viewer`]) and the System-mode run (DESIGN §2.3, §3, §4).

#![forbid(unsafe_code)]

use crate::config::{EngineKind, Param, ViewerConfig, bits};
use crate::fit::{FitResult, ModelChoice};
use crate::lab::LabRun;
use crate::observe::{ElementObserver, PassageLocator, Unwrap, VarpiForm, arcsec};
use crate::{
    BODIES_STRIDE, ENGINE_IAS15, ENGINE_WHFAST, FAV_DONE, FAV_E_ARG, FAV_E_STATE,
    FAV_E_UNSUPPORTED, FAV_OK, FIT_STRIDE, SECONDS_PER_YEAR, ViewerError,
};
use ferroastro_core::frames::{EclipticJ2000, Frame, Icrf};
use ferroastro_core::time::{J2000_JD, SECONDS_PER_DAY};
use ferroastro_diffrun::{Estimator, PassageSource, Track};
use ferroastro_frames::{Rotation, icrf_to_ecliptic_j2000};
use ferroastro_precession::park2017::{
    FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane, SampledPassage,
};
use ferroastro_propagate::{
    Checkpoint, DataSources, PropagateError, RunOutput, SolarOverrides, de440_solar_spin,
    refuse_unsupported, resume_until, run_until,
};
use ferroastro_scenario::{BodyName, Parts, ScenarioSpec};

/// `x` as a `usize` if it is a finite whole number in range.
pub(crate) fn to_usize(x: f64) -> Option<usize> {
    to_u64(x).and_then(|k| usize::try_from(k).ok())
}

/// `x` as a `u64` if it is a finite whole number in `0 ..= 2^53`.
pub(crate) fn to_u64(x: f64) -> Option<u64> {
    // 2^53: every whole f64 below it converts exactly.
    let max = 9_007_199_254_740_992.0;
    if !(x.is_finite() && (0.0..=max).contains(&x) && x.fract() == 0.0) {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "checked above: a whole number in 0 ..= 2^53"
    )]
    Some(x as u64)
}

fn sum(p: Parts) -> f64 {
    p[0] + p[1]
}

/// The drain buffers (DESIGN §4.4 A–C).
#[derive(Debug, Default, Clone)]
pub(crate) struct Buffers {
    pub(crate) bodies: Vec<f64>,
    pub(crate) elements: Vec<f64>,
    pub(crate) passages: Vec<f64>,
}

/// What the metadata says about a run (PHY-015).
#[derive(Debug, Clone, Default)]
pub(crate) struct RunInfo {
    pub(crate) mode: &'static str,
    pub(crate) engine: u32,
    pub(crate) engine_label: String,
    pub(crate) epoch_jd: f64,
    pub(crate) model: String,
    pub(crate) gauge: String,
    pub(crate) frame: String,
    pub(crate) display_frame: String,
    pub(crate) ic_source: String,
    pub(crate) ic_hash: String,
    pub(crate) constants: String,
    pub(crate) scenario_digest: String,
    pub(crate) estimator: String,
    pub(crate) scenario_text: String,
}

/// The perihelion passages of a run with their unwrapped projected longitudes.
#[derive(Debug, Default, Clone)]
pub(crate) struct PassageSeries {
    pub(crate) passages: Vec<SampledPassage>,
    pub(crate) lon: Vec<f64>,
    unwrap: Unwrap,
}

impl PassageSeries {
    /// Record a passage and append its buffer row (`ecl` is its position in ECLIPJ2000, km).
    pub(crate) fn add<F: Frame>(
        &mut self,
        p: SampledPassage,
        plane: &ReferencePlane<F>,
        ecl: [f64; 3],
        (au_km, offset_s): (f64, f64),
        out: &mut Vec<f64>,
    ) {
        let l = self.unwrap.push(plane.projected_angle(p.r));
        out.extend_from_slice(&[
            (p.t_s - offset_s) / SECONDS_PER_YEAR,
            arcsec(l),
            ecl[0] / au_km,
            ecl[1] / au_km,
            ecl[2] / au_km,
            arcsec(p.direction_error_rad),
        ]);
        self.passages.push(p);
        self.lon.push(l);
    }

    pub(crate) fn times(&self) -> Vec<f64> {
        self.passages.iter().map(|p| p.t_s).collect()
    }
}

/// The NAIF ids of the bodies buffer slots after `t`.
const SLOTS: [i32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// The Park2017 estimator of the viewer (DESIGN §6.2): Mercury relative to the Sun, Park17
/// mean-orbit plane, projected longitude, Lagrange order 8, on the sampled states.
///
/// # Errors
/// `FAV_E_RUN` if the plane cannot be built (never for the built-in pole).
pub fn estimator(model: FitModel, blocks: usize) -> Result<Estimator, ViewerError> {
    Ok(Estimator {
        park: Park2017 {
            plane: ReferencePlane::<Icrf>::park2017_mean_orbit()
                .map_err(|e| ViewerError::run(e.to_string()))?,
            definition: LongitudeDefinition::Projected,
            passages: PassageConfig::default(),
            model,
            jackknife_blocks: blocks,
        },
        source: PassageSource::SampledStates,
        body: BodyName::Mercury,
        central: BodyName::Sun,
        window_jd: None,
    })
}

fn propagate_err(e: &PropagateError) -> ViewerError {
    let m = e.to_string();
    if m.starts_with("FA-E1201") {
        ViewerError::new(FAV_E_UNSUPPORTED, m)
    } else {
        ViewerError::run(m)
    }
}

/// A System-mode run: a scenario driven in checkpointed segments.
#[derive(Debug)]
pub(crate) struct SystemRun {
    spec: ScenarioSpec,
    overrides: SolarOverrides,
    ck: Option<Checkpoint>,
    started: bool,
    finished: bool,
    target_years: f64,
    span_years: f64,
    t_years: f64,
    sample_index: u64,
    display_every: u64,
    locator: PassageLocator,
    observer: ElementObserver<Icrf>,
    park: ReferencePlane<Icrf>,
    rot: Rotation<Icrf, EclipticJ2000, f64>,
    au_km: f64,
    offset_s: f64,
    pub(crate) series: PassageSeries,
    pub(crate) info: RunInfo,
    pub(crate) mask: u32,
}

/// Resolve the engine (DESIGN §3.2) and build the scenario.
fn resolve(cfg: &ViewerConfig) -> Result<(EngineKind, ScenarioSpec), ViewerError> {
    let refused = |spec: &ScenarioSpec| refuse_unsupported(spec).map_err(|e| propagate_err(&e));
    if let Some(e) = cfg.requested_engine() {
        let spec = cfg.to_scenario(e)?;
        refused(&spec)?;
        return Ok((e, spec));
    }
    // AUTO: WHFast when propagate accepts it, IAS15 after an FA-E1201 refusal.
    if cfg.wh_expressible() {
        let wh = cfg.to_scenario(EngineKind::WhFast)?;
        match refused(&wh) {
            Ok(()) => return Ok((EngineKind::WhFast, wh)),
            Err(e) if e.status == FAV_E_UNSUPPORTED => {}
            Err(e) => return Err(e),
        }
    }
    let spec = cfg.to_scenario(EngineKind::Ias15)?;
    refused(&spec)?;
    Ok((EngineKind::Ias15, spec))
}

impl SystemRun {
    pub(crate) fn start(cfg: &ViewerConfig, out: &mut Buffers) -> Result<Self, ViewerError> {
        // A build without `embed-header` has no initial conditions: refuse before any work
        // (FAV_E_UNSUPPORTED, "built without embed-header"; DESIGN §5).
        crate::ic::header()?;
        let (engine, spec) = resolve(cfg)?;
        let sample_days = cfg.sample_days(engine);
        let display = cfg.get(Param::DisplayCadenceDays);
        if engine == EngineKind::WhFast && display < sample_days {
            return Err(ViewerError::arg(format!(
                "FA-E0002: DISPLAY_CADENCE_DAYS = {display} is shorter than the WHFast step \
                 {sample_days} d"
            )));
        }
        let de = crate::ic::de440()?;
        let mut overrides = SolarOverrides {
            j2: cfg.j2_override(),
            provenance: "orbit viewer parameters J2_OVERRIDE / SPIN_SCALE (non-nominal)".into(),
            ..SolarOverrides::default()
        };
        if let Some(k) = cfg.spin_scale() {
            let gs = de440_solar_spin(de, &spec.name).map_err(|e| propagate_err(&e))?;
            overrides.spin_gm = Some(gs.km5_per_s3() * k);
        }
        let display_every = to_u64((display / sample_days).round()).unwrap_or(1).max(1);
        let au_km = de.au().km();
        let mu = de.gm_sun().km3_per_s2() + de.gm(ferroastro_constants::Body::Mercury).km3_per_s2();
        let park = ReferencePlane::<Icrf>::park2017_mean_orbit()
            .map_err(|e| ViewerError::run(e.to_string()))?;
        let est = estimator(
            FitModel {
                quadratic: false,
                frequencies: Vec::new(),
            },
            2,
        )?;
        let engine_label = match engine {
            EngineKind::WhFast => format!(
                "whfast h={} d, corrector {}",
                cfg.get(Param::WhStepDays),
                crate::config::WH_CORRECTOR
            ),
            EngineKind::Ias15 => format!("ias15 eps={:e}", cfg.get(Param::Ias15Epsilon)),
        };
        let m = cfg.mask();
        let gauge = if m & bits::EIH != 0 || (m & bits::GR_1PN != 0 && engine == EngineKind::Ias15)
        {
            "PPN isotropic/harmonic"
        } else if m & bits::GR_1PN != 0 {
            "GR effective potential (Nobili-Roxburgh; reproduces the 1PN perihelion advance, not the 1PN mean motion)"
        } else if m & bits::LENSE_THIRRING != 0 {
            "PPN isotropic/harmonic (Lense-Thirring term only)"
        } else {
            "Newtonian"
        };
        let info = RunInfo {
            mode: "system",
            engine: match engine {
                EngineKind::WhFast => ENGINE_WHFAST,
                EngineKind::Ias15 => ENGINE_IAS15,
            },
            engine_label,
            gauge: gauge.into(),
            frame: "Park17 mean Mercury orbit plane, ICRF pole RA 280.9876 deg Dec 61.4481 deg"
                .into(),
            display_frame: "ECLIPJ2000 (R_x(84381.448 arcsec) of ICRF), heliocentric, au".into(),
            scenario_digest: spec.digest(),
            estimator: est.id(PassageSource::SampledStates),
            scenario_text: cfg.scenario_toml(engine)?,
            ..RunInfo::default()
        };
        let mut run = Self {
            spec,
            overrides,
            ck: None,
            started: false,
            finished: false,
            target_years: 0.0,
            span_years: cfg.get(Param::SpanYears),
            t_years: 0.0,
            sample_index: 0,
            display_every,
            locator: PassageLocator::new(0.0, sample_days),
            observer: ElementObserver::new(mu, au_km, park.clone(), VarpiForm::DogLeg),
            park,
            rot: icrf_to_ecliptic_j2000::<f64>(),
            au_km,
            offset_s: 0.0,
            series: PassageSeries::default(),
            info,
            mask: m,
        };
        // The first segment: one step, which assembles the ICs and the force model and records
        // the initial sample; its records are the run's first.
        run.segment(0.0, out)?;
        Ok(run)
    }

    fn data(&self) -> Result<DataSources<'_>, ViewerError> {
        let mut d =
            DataSources::none().with_header(crate::ic::header()?, crate::ic::header_provenance());
        if self.spec.forces.asteroids > 0 {
            d = d.with_small_bodies(crate::ic::small_bodies()?);
        }
        if !self.overrides.is_empty() {
            d = d.with_solar_overrides(&self.overrides);
        }
        Ok(d)
    }

    /// The scenario of this run.
    pub(crate) const fn spec(&self) -> &ScenarioSpec {
        &self.spec
    }

    /// The solar overrides of this run.
    pub(crate) const fn overrides(&self) -> &SolarOverrides {
        &self.overrides
    }

    pub(crate) const fn time_years(&self) -> f64 {
        self.t_years
    }

    pub(crate) const fn offset_s(&self) -> f64 {
        self.offset_s
    }

    pub(crate) const fn finished(&self) -> bool {
        self.finished
    }

    /// Advance by `years` (the stop is cumulative, so the step sequence does not depend on how
    /// a span is cut into calls, CI-010).
    pub(crate) fn advance(&mut self, years: f64, out: &mut Buffers) -> Result<i32, ViewerError> {
        if self.finished {
            return Ok(FAV_DONE);
        }
        self.target_years = (self.target_years + years).min(self.span_years);
        self.segment(self.target_years, out)?;
        Ok(if self.finished { FAV_DONE } else { FAV_OK })
    }

    fn segment(&mut self, stop_years: f64, out: &mut Buffers) -> Result<(), ViewerError> {
        let data = self.data()?;
        let (o, next) = match (&self.ck, self.started) {
            (None, false) => run_until(&self.spec, &data, stop_years),
            (Some(c), _) => resume_until(&self.spec, &data, c, stop_years),
            (None, true) => {
                self.finished = true;
                return Ok(());
            }
        }
        .map_err(|e| propagate_err(&e))?;
        if !self.started {
            self.started = true;
            self.info.epoch_jd = sum(o.epoch_jd_tdb);
            self.offset_s = (self.info.epoch_jd - J2000_JD) * SECONDS_PER_DAY;
            self.locator = PassageLocator::new(
                self.info.epoch_jd,
                self.spec.output.cadence_days.unwrap_or(1.0),
            );
            self.info.model.clone_from(&o.model);
            self.info.ic_source.clone_from(&o.ic_source);
            self.info.ic_hash.clone_from(&o.ic_hash);
            self.info.constants.clone_from(&o.constants);
        }
        self.absorb(&o, out)?;
        self.finished = next.is_none();
        self.ck = next;
        Ok(())
    }

    fn absorb(&mut self, o: &RunOutput, out: &mut Buffers) -> Result<(), ViewerError> {
        let mut track = Track::new(self.info.epoch_jd);
        let missing = track.absorb(o, BodyName::Mercury.naif(), BodyName::Sun.naif());
        if missing > 0 || track.t.len() != o.states.len() {
            return Err(ViewerError::run(format!(
                "{missing} sampled states lack Mercury or the Sun"
            )));
        }
        for (k, s) in o.states.iter().enumerate() {
            let g = self.sample_index;
            self.sample_index += 1;
            if g % self.display_every != 0 {
                continue;
            }
            let t_years = sum(s.t) / SECONDS_PER_YEAR;
            let sun_i = s.bodies.iter().position(|&b| b == BodyName::Sun.naif());
            out.bodies.reserve(BODIES_STRIDE);
            out.bodies.push(t_years);
            for id in SLOTS {
                let rel = s
                    .bodies
                    .iter()
                    .position(|&b| b == id)
                    .zip(sun_i)
                    .and_then(|(i, j)| Some((s.positions.get(i)?, s.positions.get(j)?)))
                    .map(|(a, b)| {
                        [0, 1, 2].map(|c| match (a.get(c), b.get(c)) {
                            (Some(x), Some(y)) => (x[0] - y[0]) + (x[1] - y[1]),
                            _ => f64::NAN,
                        })
                    });
                match rel {
                    Some(r) => out.bodies.extend(self.rot.apply(r).map(|x| x / self.au_km)),
                    None => out.bodies.extend([f64::NAN; 3]),
                }
            }
            if let (Some(r), Some(v)) = (track.r.get(k), track.v.get(k)) {
                let row =
                    self.observer
                        .row(t_years, (*r, *v), (self.rot.apply(*r), self.rot.apply(*v)));
                out.elements.extend_from_slice(&row);
            }
        }
        let samples = track
            .t
            .iter()
            .zip(&track.r)
            .zip(&track.v)
            .map(|((t, r), v)| (*t, *r, *v));
        for p in self.locator.push::<Icrf>(samples)? {
            let ecl = self.rot.apply(p.r);
            self.series.add(
                p,
                &self.park,
                ecl,
                (self.au_km, self.offset_s),
                &mut out.passages,
            );
        }
        if let Some(f) = &o.final_state {
            self.t_years = sum(f.t) / SECONDS_PER_YEAR;
        }
        Ok(())
    }
}

/// A configured run.
#[derive(Debug)]
pub(crate) enum Run {
    System(Box<SystemRun>),
    Lab(Box<LabRun>),
}

impl Run {
    fn info(&self) -> &RunInfo {
        match self {
            Run::System(r) => &r.info,
            Run::Lab(r) => &r.info,
        }
    }
    fn series(&self) -> &PassageSeries {
        match self {
            Run::System(r) => &r.series,
            Run::Lab(r) => &r.series,
        }
    }
    fn offset_s(&self) -> f64 {
        match self {
            Run::System(r) => r.offset_s(),
            // The lab's epoch is J2000, the origin of the passage times.
            Run::Lab(_) => 0.0,
        }
    }
    fn time_years(&self) -> f64 {
        match self {
            Run::System(r) => r.time_years(),
            Run::Lab(r) => r.time_years(),
        }
    }
    fn finished(&self) -> bool {
        match self {
            Run::System(r) => r.finished(),
            Run::Lab(r) => r.finished(),
        }
    }
}

/// The viewer: configuration, the current run, its drain buffers, the last fit and the last
/// error. The `fav_*` exports ([`crate::ffi`]) are thin wrappers over its methods, so native
/// tests and the browser exercise the same code.
#[derive(Debug, Default)]
pub struct Viewer {
    config: ViewerConfig,
    run: Option<Run>,
    failed: bool,
    buf: Buffers,
    fit: Vec<f64>,
    last_fit: Option<FitResult>,
    error: String,
    meta_text: String,
    scenario_text: String,
}

impl Viewer {
    /// A viewer with the default configuration (DESIGN §4.3) and no run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn status(&mut self, r: Result<i32, ViewerError>) -> i32 {
        match r {
            Ok(s) => s,
            Err(e) => {
                self.error = e.message;
                e.status
            }
        }
    }

    /// Drop the run; toggles and parameters return to the defaults. Returns [`FAV_OK`].
    pub fn reset(&mut self) -> i32 {
        *self = Self::default();
        FAV_OK
    }

    /// Set the toggle mask (effective at the next [`Viewer::configure`]).
    pub fn set_toggles(&mut self, mask: u32) -> i32 {
        let r = self.config.set_mask(mask).map(|()| FAV_OK);
        self.status(r)
    }

    /// Set parameter `id` (range-checked now, effective at the next [`Viewer::configure`]).
    pub fn set_param(&mut self, id: u32, value: f64) -> i32 {
        let r = match Param::from_id(id) {
            Some(p) => self.config.set(p, value).map(|()| FAV_OK),
            None => Err(ViewerError::arg(format!(
                "FA-E0002: unknown parameter id {id} (valid 0-12)"
            ))),
        };
        self.status(r)
    }

    /// Parameter `id` (NaN for an unknown id).
    #[must_use]
    pub fn get_param(&self, id: u32) -> f64 {
        Param::from_id(id).map_or(f64::NAN, |p| self.config.get(p))
    }

    /// The configuration of the next run.
    #[must_use]
    pub const fn config(&self) -> &ViewerConfig {
        &self.config
    }

    /// Validate, resolve the engine, build the scenario and initial conditions, and start a run
    /// at t = 0, discarding any previous run and its buffers.
    pub fn configure(&mut self) -> i32 {
        self.run = None;
        self.failed = false;
        self.buf = Buffers::default();
        self.fit.clear();
        self.last_fit = None;
        let r = self.config.validate().and_then(|()| {
            let mut buf = Buffers::default();
            let run = if self.config.is_lab() {
                Run::Lab(Box::new(LabRun::start(&self.config, &mut buf)?))
            } else {
                Run::System(Box::new(SystemRun::start(&self.config, &mut buf)?))
            };
            self.buf = buf;
            self.run = Some(run);
            Ok(FAV_OK)
        });
        self.status(r)
    }

    /// Advance the run by `years` (0 < years ≤ 50); [`FAV_DONE`] at the end of the span.
    pub fn run(&mut self, years: f64) -> i32 {
        let r = self.run_inner(years);
        if r.as_ref()
            .is_err_and(|e| e.status != FAV_E_ARG && e.status != FAV_E_STATE)
        {
            self.failed = true;
        }
        self.status(r)
    }

    fn run_inner(&mut self, years: f64) -> Result<i32, ViewerError> {
        if !(years.is_finite() && years > 0.0 && years <= 50.0) {
            return Err(ViewerError::arg(format!(
                "FA-E0002: fav_run years = {years}; valid 0 < years <= 50"
            )));
        }
        if self.failed {
            return Err(ViewerError::state(
                "the run stopped with an error; call fav_configure to start again",
            ));
        }
        let buf = &mut self.buf;
        match self.run.as_mut() {
            None => Err(ViewerError::state("fav_run before fav_configure")),
            Some(Run::System(r)) => r.advance(years, buf),
            Some(Run::Lab(r)) => r.advance(years, buf),
        }
    }

    /// TDB Julian years since the epoch reached so far (0 without a run).
    #[must_use]
    pub fn time_years(&self) -> f64 {
        self.run.as_ref().map_or(0.0, Run::time_years)
    }

    /// True once the run has reached its span.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.run.as_ref().is_some_and(Run::finished)
    }

    /// The run epoch, TDB Julian date (NaN without a run).
    #[must_use]
    pub fn epoch_jd(&self) -> f64 {
        self.run.as_ref().map_or(f64::NAN, |r| r.info().epoch_jd)
    }

    /// The resolved engine: 1 `WHFast`, 2 IAS15, 3 IAS15 test particle (0 without a run).
    #[must_use]
    pub fn engine(&self) -> u32 {
        self.run.as_ref().map_or(0, |r| r.info().engine)
    }

    /// Empty the drain buffers.
    pub fn drain(&mut self) -> i32 {
        self.buf.bodies.clear();
        self.buf.elements.clear();
        self.buf.passages.clear();
        FAV_OK
    }

    /// Bodies buffer since the last drain.
    #[must_use]
    pub fn bodies(&self) -> &[f64] {
        &self.buf.bodies
    }

    /// Elements buffer since the last drain.
    #[must_use]
    pub fn elements(&self) -> &[f64] {
        &self.buf.elements
    }

    /// Passages buffer since the last drain.
    #[must_use]
    pub fn passages(&self) -> &[f64] {
        &self.buf.passages
    }

    /// Every passage of the run so far (TDB seconds from J2000, heliocentric km, km/s).
    #[must_use]
    pub fn passage_records(&self) -> &[SampledPassage] {
        self.run
            .as_ref()
            .map_or(&[], |r| r.series().passages.as_slice())
    }

    /// The unwrapped projected longitudes (rad) of [`Viewer::passage_records`].
    #[must_use]
    pub fn passage_longitudes(&self) -> &[f64] {
        self.run.as_ref().map_or(&[], |r| r.series().lon.as_slice())
    }

    /// The System-mode scenario of the run.
    #[must_use]
    pub fn scenario(&self) -> Option<&ScenarioSpec> {
        match &self.run {
            Some(Run::System(r)) => Some(r.spec()),
            _ => None,
        }
    }

    /// The solar overrides of the System-mode run.
    #[must_use]
    pub fn solar_overrides(&self) -> Option<&SolarOverrides> {
        match &self.run {
            Some(Run::System(r)) => Some(r.overrides()),
            _ => None,
        }
    }

    /// The IC hash of the run (empty without one).
    #[must_use]
    pub fn ic_hash(&self) -> &str {
        self.run.as_ref().map_or("", |r| r.info().ic_hash.as_str())
    }

    /// Fit ϖ̇ over the passages with `t` in `[t0, t1]` years (NaN, NaN: all so far).
    pub fn fit(&mut self, t0: f64, t1: f64) -> i32 {
        let r = self.fit_inner(t0, t1);
        self.status(r)
    }

    fn fit_inner(&mut self, t0: f64, t1: f64) -> Result<i32, ViewerError> {
        let run = self
            .run
            .as_ref()
            .ok_or_else(|| ViewerError::state("fav_fit before fav_configure"))?;
        let choice = match run {
            Run::System(r) => ModelChoice {
                mask: r.mask,
                periodic: self.config.fit_periodic(),
                single_body: false,
            },
            Run::Lab(_) => ModelChoice {
                mask: 0,
                periodic: false,
                single_body: true,
            },
        };
        let s = run.series();
        let res = crate::fit::fit(&s.times(), &s.lon, run.offset_s(), (t0, t1), choice)?;
        self.fit = res.record.to_vec();
        self.last_fit = Some(res);
        Ok(FAV_OK)
    }

    /// The last fit record (DESIGN §4.4 D), empty before a successful fit.
    #[must_use]
    pub fn fit_record(&self) -> &[f64] {
        debug_assert!(self.fit.is_empty() || self.fit.len() == FIT_STRIDE);
        &self.fit
    }

    /// The last fit's model and jack-knife block count, if any (to rebuild the same estimator,
    /// DESIGN E12).
    #[must_use]
    pub fn last_fit(&self) -> Option<(&FitModel, usize)> {
        self.last_fit
            .as_ref()
            .map(|f| (&f.model, f.fit.jackknife_blocks))
    }

    /// The metadata JSON (DESIGN §4.5).
    #[must_use]
    pub fn meta_json(&self) -> String {
        crate::meta::json(
            &self.config,
            self.run.as_ref().map(Run::info),
            self.last_fit.as_ref(),
        )
    }

    /// The metadata JSON as bytes. The allocation is replaced only when the text changes, so
    /// the `_ptr` and `_len` exports may be called in either order.
    pub fn meta_bytes(&mut self) -> &[u8] {
        let s = self.meta_json();
        if s != self.meta_text {
            self.meta_text = s;
        }
        self.meta_text.as_bytes()
    }

    /// The run's scenario as TOML (System mode), or the Rung-lab set-up, as bytes (replaced
    /// only when it changes, as [`Viewer::meta_bytes`]).
    pub fn scenario_bytes(&mut self) -> &[u8] {
        let s = self
            .run
            .as_ref()
            .map_or_else(String::new, |r| r.info().scenario_text.clone());
        if s != self.scenario_text {
            self.scenario_text = s;
        }
        self.scenario_text.as_bytes()
    }

    /// The last error message.
    #[must_use]
    pub fn error(&self) -> &str {
        &self.error
    }
}
