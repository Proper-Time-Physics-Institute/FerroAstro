// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Running a plan: the IC-identity check, legs in checkpointed segments (CI-010) with an optional
//! on-disk store, one estimator for every configuration, and the contribution table.

use crate::DiffrunError;
use crate::error::plan_error;
use crate::estimator::{Estimate, Estimator, PassageSource};
use crate::plan::{Combination, Plan, Protocol};
use crate::track::Track;
use ferroastro_propagate::{
    Checkpoint, DataSources, InitialRecords, IntegratorSnapshot, abm_config, ias15_config,
    initial_conditions, irk16_config, resume_until, run, run_until, wh_config,
};
use ferroastro_scenario::{IcRecord, IntegratorName, Mode, ScenarioSpec, ic_hash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Format tag of a store file.
///
/// v2 (issue #31) records the integrator settings of the leg and keys the file name by them (see
/// [`store_file`]). A file of any other format, including v1 (keyed by the scenario digest only,
/// so it could resume a leg under other integrator settings), is refused with `FA-E1402` and
/// never resumed: delete it to run that leg from the start.
pub const STORE_FORMAT: &str = "ferroastro-diffrun store v2";

/// Format tag of the store files written before issue #31 (`<scenario digest>.leg`).
const STORE_FORMAT_V1: &str = "ferroastro-diffrun store v1";

/// How legs are run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOptions {
    /// Segment length (Julian years) of N-body legs: each segment ends with a checkpoint.
    pub segment_years: f64,
    /// Directory for per-leg store files (checkpoint and track after every segment); a
    /// completed leg's file is deleted. `None` keeps everything in memory.
    pub store: Option<PathBuf>,
    /// Worker threads for the legs (scoped threads; sequential on wasm32).
    pub threads: usize,
}

/// Most checkpoint segments one N-body leg may be split into (`|span| / segment_years`, rounded
/// up). A bound on work and store writes, far above any real plan (2000 yr in 50-yr segments is
/// 40); it also keeps the segment counter far from `u32` overflow (PR #13 review R13-CODE-5).
pub const MAX_SEGMENTS: u32 = 1_000_000;

impl RunOptions {
    /// Check the options on their own: `segment_years` finite and > 0.
    ///
    /// # Errors
    /// `FA-E1400` otherwise (a zero, negative or NaN segment length never advances a leg).
    pub fn validate(&self) -> Result<(), DiffrunError> {
        if self.segment_years.is_finite() && self.segment_years > 0.0 {
            Ok(())
        } else {
            Err(plan_error(
                "run options",
                format!(
                    "segment_years {} is not a finite length > 0 (Julian years)",
                    self.segment_years
                ),
                "use a finite segment length > 0, e.g. the default 50",
            ))
        }
    }

    /// Number of segments of an N-body leg of `spec`, checked against [`MAX_SEGMENTS`].
    ///
    /// # Errors
    /// `FA-E1400` for invalid options or more than [`MAX_SEGMENTS`] segments.
    pub fn segments(&self, spec: &ScenarioSpec) -> Result<u32, DiffrunError> {
        self.validate()?;
        let n = (spec.span.years.abs() / self.segment_years).ceil();
        if n.is_finite() && n <= f64::from(MAX_SEGMENTS) {
            // A whole number in [0, MAX_SEGMENTS]: exact.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "n is a whole number in [0, MAX_SEGMENTS], checked above"
            )]
            Ok(n as u32)
        } else {
            Err(plan_error(
                &spec.name,
                format!(
                    "{} Julian years in segments of {:e} yr needs more than {MAX_SEGMENTS} segments",
                    spec.span.years, self.segment_years
                ),
                "use a longer segment_years",
            ))
        }
    }
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            segment_years: 50.0,
            store: None,
            threads: 1,
        }
    }
}

/// One leg's result.
#[derive(Debug, Clone, PartialEq)]
pub struct LegResult {
    /// The track of this leg.
    pub track: Track,
    /// Its initial conditions as hashed.
    pub initial: InitialRecords,
    /// The run's IC hash (equal to `initial.ic_hash`, checked).
    pub ic_hash: String,
    /// Force model and integrator settings.
    pub model: String,
    /// IC source and provenance.
    pub ic_source: String,
    /// IAS15 steps.
    pub steps: u64,
    /// Segments run in this call (0 for a test-particle leg, which is never segmented).
    pub segments_run: u32,
    /// True if the leg resumed from a store file.
    pub resumed_from_store: bool,
}

/// What a store file holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    /// [`STORE_FORMAT`]; first, so another version is recognised before the rest is decoded.
    format: String,
    scenario_digest: String,
    /// [`integrator_settings`] of the leg (v2, issue #31).
    integrator: String,
    segment: u32,
    checkpoint: Vec<u8>,
    track: Track,
    steps: u64,
}

fn store_err(path: &Path, problem: impl std::fmt::Display) -> DiffrunError {
    DiffrunError::Store {
        path: path.display().to_string(),
        problem: problem.to_string(),
    }
}

fn hex_sha256(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// The integrator settings that decide a leg's result beyond its scenario: the JSON of the whole
/// configuration of the leg's integrator (for IAS15 `ferroastro_propagate::ias15_config`, e.g.
/// the predictor `pc_b_from_g`, which the scenario digest does not cover; issue #31). The whole
/// struct is serialised, so a field added to `Ias15Config` (or to `AbmConfig`, `WhConfig` or
/// `Irk16Config`) is keyed without a change here.
fn integrator_settings(cfg: &impl Serialize) -> Result<String, DiffrunError> {
    serde_json::to_string(cfg).map_err(|e| store_err(Path::new("-"), e))
}

/// The settings a fresh N-body leg of `spec` runs with: the configuration of the scenario's
/// integrator as the driver builds it (IAS15 with dense output on).
fn leg_settings(spec: &ScenarioSpec) -> Result<String, DiffrunError> {
    match spec.integrator.name {
        IntegratorName::Ias15 => integrator_settings(&ias15_config(spec, true)),
        IntegratorName::Abm => integrator_settings(&abm_config(spec)),
        IntegratorName::Whfast => integrator_settings(&wh_config(spec)),
        IntegratorName::Irk16 => integrator_settings(&irk16_config(spec)),
    }
}

/// The settings a checkpointed integrator state continues with (its own configuration).
fn snapshot_settings(snap: &IntegratorSnapshot) -> Result<String, DiffrunError> {
    match snap {
        IntegratorSnapshot::Ias15(s) => integrator_settings(&s.config),
        IntegratorSnapshot::Abm(s) => integrator_settings(&s.config),
        IntegratorSnapshot::Whfast(s) => integrator_settings(&s.config),
        IntegratorSnapshot::Irk16(s) => integrator_settings(&s.config),
    }
}

fn store_path(dir: &Path, spec: &ScenarioSpec, settings: &str) -> PathBuf {
    dir.join(format!("{}-{}.leg", spec.digest(), hex_sha256(settings)))
}

/// Where the store of format v1 kept a leg of `spec` (scenario digest only).
fn legacy_store_path(dir: &Path, spec: &ScenarioSpec) -> PathBuf {
    dir.join(format!("{}.leg", spec.digest()))
}

/// The store file of an N-body leg of `spec` in `dir`:
/// `<scenario digest>-<SHA-256 of the integrator settings>.leg`. A leg is resumed only from the
/// file written under the same scenario **and** the same IAS15 settings (issue #31); under other
/// settings it runs from the start, and the other file is left untouched.
///
/// # Errors
/// `FA-E1402` if the integrator settings cannot be serialised.
pub fn store_file(dir: &Path, spec: &ScenarioSpec) -> Result<PathBuf, DiffrunError> {
    Ok(store_path(dir, spec, &leg_settings(spec)?))
}

fn refuse_format(path: &Path, format: &str) -> DiffrunError {
    let why = if format == STORE_FORMAT_V1 {
        " (v1 did not record the integrator settings, so the leg could resume under other ones; \
         issue #31)"
    } else {
        ""
    };
    store_err(
        path,
        format!(
            "holds {format:?}, not {STORE_FORMAT:?}{why}; a store file of another format is \
             never resumed: delete it to run the leg from the start"
        ),
    )
}

/// The store file of this leg and its decoded checkpoint, if there is one; refuses a file of
/// another format, scenario or integrator settings (header and checkpoint both checked).
fn load(
    dir: &Path,
    spec: &ScenarioSpec,
    settings: &str,
) -> Result<Option<(Stored, Checkpoint)>, DiffrunError> {
    let path = store_path(dir, spec, settings);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let legacy = legacy_store_path(dir, spec);
            return if legacy.exists() {
                Err(refuse_format(&legacy, STORE_FORMAT_V1))
            } else {
                Ok(None)
            };
        }
        Err(e) => return Err(store_err(&path, e)),
    };
    let (format, _) =
        postcard::take_from_bytes::<String>(&bytes).map_err(|e| store_err(&path, e))?;
    if format != STORE_FORMAT {
        return Err(refuse_format(&path, &format));
    }
    let s: Stored = postcard::from_bytes(&bytes).map_err(|e| store_err(&path, e))?;
    if s.scenario_digest != spec.digest() {
        return Err(store_err(
            &path,
            format!(
                "holds scenario digest {}, not {}",
                s.scenario_digest,
                spec.digest()
            ),
        ));
    }
    check_settings(&path, "its header records", &s.integrator, settings)?;
    let c = Checkpoint::decode(&s.checkpoint)?;
    // The checkpoint's own settings are the ones the restored integrator continues with.
    check_settings(
        &path,
        "its checkpoint holds",
        &snapshot_settings(&c.integrator)?,
        settings,
    )?;
    Ok(Some((s, c)))
}

fn check_settings(
    path: &Path,
    whose: &str,
    stored: &str,
    settings: &str,
) -> Result<(), DiffrunError> {
    if stored == settings {
        Ok(())
    } else {
        Err(store_err(
            path,
            format!(
                "{whose} integrator settings {stored}, this run uses {settings}; a leg is never \
                 resumed under other integrator settings (issue #31): delete the file to run the \
                 leg from the start"
            ),
        ))
    }
}

fn save(path: &Path, s: &Stored) -> Result<(), DiffrunError> {
    let bytes = postcard::to_allocvec(s).map_err(|e| store_err(path, e))?;
    let tmp = path.with_extension("leg.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| store_err(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| store_err(path, e))
}

fn test_particle_leg(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    initial: InitialRecords,
    epoch: f64,
    (body, central): (i32, i32),
) -> Result<LegResult, DiffrunError> {
    let out = run(spec, data)?;
    let mut track = Track::new(epoch);
    track.absorb(&out, body, central);
    Ok(LegResult {
        track,
        ic_hash: out.ic_hash,
        initial,
        model: out.model,
        ic_source: out.ic_source,
        steps: out.stats.steps,
        segments_run: 0,
        resumed_from_store: false,
    })
}

/// Run one leg, in segments of `opts.segment_years` (N-body) through checkpoints, resuming from
/// `opts.store` if a file for this scenario and these integrator settings exists ([`store_file`]). With `stop_after_segments = Some(k)` the call
/// returns `Ok(None)` after `k` new segments, leaving the store file for a later call (used to
/// test interruption; requires a store).
///
/// # Errors
/// `FA-E1400` for invalid [`RunOptions`] (see [`RunOptions::segments`]); `FA-E1402` for store
/// I/O, a foreign store file, a store file of another format (including v1) or one whose
/// recorded or checkpointed integrator settings differ from this run's; `FA-E1302` from the
/// driver; the run's own errors.
pub fn run_leg(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    est: &Estimator,
    opts: &RunOptions,
    stop_after_segments: Option<u32>,
) -> Result<Option<LegResult>, DiffrunError> {
    let initial = initial_conditions(spec, data)?;
    let (body, central) = (est.body.naif(), est.central.naif());
    let epoch = initial.epoch_jd_tdb[0] + initial.epoch_jd_tdb[1];
    if spec.mode == Mode::TestParticle {
        return test_particle_leg(spec, data, initial, epoch, (body, central)).map(Some);
    }
    opts.segments(spec)?;
    let sign = if spec.span.years < 0.0 { -1.0 } else { 1.0 };
    let settings = leg_settings(spec)?;
    let path = opts
        .store
        .as_deref()
        .map(|d| store_path(d, spec, &settings));
    let stored = match opts.store.as_deref() {
        Some(d) => load(d, spec, &settings)?,
        None => None,
    };
    let resumed_from_store = stored.is_some();
    let (mut track, mut segment, mut ck) = match stored {
        Some((s, c)) => (s.track, s.segment, Some(c)),
        None => (Track::new(epoch), 0, None),
    };
    let mut segments_run = 0_u32;
    loop {
        segment += 1;
        let stop = sign * opts.segment_years * f64::from(segment);
        let (out, next) = match &ck {
            None if segment == 1 => run_until(spec, data, stop)?,
            None => break,
            Some(c) => resume_until(spec, data, c, stop)?,
        };
        if out.ic_hash != initial.ic_hash {
            return Err(DiffrunError::IcMismatch {
                a: spec.name.clone(),
                b: format!("{} (as assembled)", spec.name),
                problem: format!(
                    "run IC hash {} vs assembled {}",
                    out.ic_hash, initial.ic_hash
                ),
            });
        }
        track.absorb(&out, body, central);
        segments_run += 1;
        let steps = out.stats.steps;
        match next {
            None => {
                if let Some(p) = path.as_ref().filter(|p| p.exists()) {
                    std::fs::remove_file(p).map_err(|e| store_err(p, e))?;
                }
                return Ok(Some(LegResult {
                    track,
                    ic_hash: out.ic_hash,
                    initial,
                    model: out.model,
                    ic_source: out.ic_source,
                    steps,
                    segments_run,
                    resumed_from_store,
                }));
            }
            Some(c) => {
                if let Some(p) = &path {
                    save(
                        p,
                        &Stored {
                            format: STORE_FORMAT.into(),
                            scenario_digest: spec.digest(),
                            integrator: settings.clone(),
                            segment,
                            checkpoint: c.encode()?,
                            track: track.clone(),
                            steps,
                        },
                    )?;
                }
                ck = Some(c);
                if stop_after_segments.is_some_and(|k| segments_run >= k) {
                    if path.is_none() {
                        return Err(store_err(
                            Path::new("-"),
                            "an interrupted leg needs a store directory to resume from",
                        ));
                    }
                    return Ok(None);
                }
            }
        }
    }
    Err(DiffrunError::Store {
        path: path.map_or_else(|| "-".into(), |p| p.display().to_string()),
        problem: "the leg ended without output".into(),
    })
}

fn record_bits(r: &IcRecord) -> Vec<u64> {
    let mut w = vec![r.gm[0].to_bits(), r.gm[1].to_bits()];
    w.extend(
        r.position
            .iter()
            .chain(&r.velocity)
            .flatten()
            .map(|x| x.to_bits()),
    );
    w
}

/// The IC-identity check of a plan (NUM-013, CI-009): the same constants version, mode and epoch
/// everywhere; every body's record bitwise identical in every leg that contains it. Returns the
/// shared IC hash over the base bodies.
///
/// # Errors
/// `FA-E1401` naming the first two legs that differ.
pub fn check_identical_ics(
    plan: &Plan,
    initial: &[(String, InitialRecords)],
) -> Result<String, DiffrunError> {
    let mut per_body: BTreeMap<i32, (String, Vec<u64>)> = BTreeMap::new();
    let mut shared: Option<(String, String)> = None;
    let first = initial.first();
    for (leg, ic) in initial {
        // Nested `if let` (not a let-chain): the workspace MSRV is 1.85.
        if let Some((l0, ic0)) = first {
            if ic.constants != ic0.constants
                || ic.mode != ic0.mode
                || ic.epoch_jd_tdb.map(f64::to_bits) != ic0.epoch_jd_tdb.map(f64::to_bits)
            {
                return Err(DiffrunError::IcMismatch {
                    a: l0.clone(),
                    b: leg.clone(),
                    problem: "constants version, mode or epoch differ".into(),
                });
            }
        }
        for r in &ic.records {
            let bits = record_bits(r);
            match per_body.get(&r.naif) {
                Some((l0, b0)) if *b0 != bits => {
                    return Err(DiffrunError::IcMismatch {
                        a: l0.clone(),
                        b: leg.clone(),
                        problem: format!("initial condition of NAIF {} differs", r.naif),
                    });
                }
                Some(_) => {}
                None => {
                    per_body.insert(r.naif, (leg.clone(), bits));
                }
            }
        }
        let base: Vec<IcRecord> = plan
            .base
            .iter()
            .filter_map(|id| ic.records.iter().find(|r| r.naif == *id).copied())
            .collect();
        if base.len() != plan.base.len() {
            return Err(DiffrunError::IcMismatch {
                a: plan.name.clone(),
                b: leg.clone(),
                problem: "a base body has no initial condition".into(),
            });
        }
        let h = ic_hash(&ic.constants, &ic.mode, ic.epoch_jd_tdb, &base);
        match &shared {
            Some((l0, h0)) if *h0 != h => {
                return Err(DiffrunError::IcMismatch {
                    a: l0.clone(),
                    b: leg.clone(),
                    problem: format!("shared IC hash {h0} vs {h}"),
                });
            }
            Some(_) => {}
            None => shared = Some((leg.clone(), h)),
        }
    }
    shared.map(|(_, h)| h).ok_or_else(|| DiffrunError::Plan {
        plan: plan.name.clone(),
        problem: "no legs".into(),
        fix: "add configurations",
    })
}

/// One configuration's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigResult {
    /// Label.
    pub label: String,
    /// Scenario names of the legs.
    pub legs: Vec<String>,
    /// NAIF ids of the bodies.
    pub bodies: Vec<i32>,
    /// IC hash of each leg (the whole body set).
    pub ic_hashes: Vec<String>,
    /// Primary estimate.
    pub estimate: Estimate,
    /// The cross-check estimate from the other passage source, if available.
    pub cross_check: Option<Estimate>,
    /// IAS15 steps over all legs.
    pub steps: u64,
    /// Force model and integrator settings (of the first leg).
    pub model: String,
    /// IC source and provenance (of the first leg).
    pub ic_source: String,
}

/// One row of the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The combination.
    pub combination: Combination,
    /// Its value from the primary estimates (″/cy).
    pub value: f64,
    /// Root-sum-square of the terms' jack-knife σ (″/cy; an upper bound, the runs share most
    /// of their noise).
    pub sigma_rss: f64,
    /// Its value from the cross-check estimates, if every term has one.
    pub cross_check: Option<f64>,
}

/// A plan's result: provenance, per-configuration rates and the table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanResult {
    /// Plan name.
    pub plan: String,
    /// Estimator id of the primary estimates (one for every row, CI-009).
    pub estimator: String,
    /// Estimator id of the cross-check estimates.
    pub cross_check_estimator: String,
    /// IC hash over the base bodies, identical for every leg (asserted).
    pub shared_ic_hash: String,
    /// Constants version.
    pub constants: String,
    /// Initial epoch shared by every leg, TDB Julian date (rule 4, PO-5). `None` for a plan with
    /// no legs and for results written before the field existed (never a sentinel such as NaN,
    /// which JSON cannot hold, or JD 0).
    #[serde(default)]
    pub ic_epoch_jd_tdb: Option<f64>,
    /// Per configuration.
    pub configs: Vec<ConfigResult>,
    /// The table.
    pub rows: Vec<Row>,
}

fn other(source: PassageSource) -> PassageSource {
    match source {
        PassageSource::SampledStates => PassageSource::DenseOutputEvents,
        PassageSource::DenseOutputEvents => PassageSource::SampledStates,
    }
}

type LegOutcome = Result<Option<LegResult>, DiffrunError>;

fn run_legs(
    specs: &[&ScenarioSpec],
    data: &DataSources<'_>,
    est: &Estimator,
    opts: &RunOptions,
) -> Vec<LegOutcome> {
    let one = |s: &ScenarioSpec| run_leg(s, data, est, opts, None);
    #[cfg(not(target_arch = "wasm32"))]
    if opts.threads > 1 && specs.len() > 1 {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Independent legs on scoped worker threads; each leg is deterministic, so the results do
        // not depend on the thread count or the scheduling (TST-005).
        let next = AtomicUsize::new(0);
        let slots: Vec<Mutex<Option<LegOutcome>>> =
            specs.iter().map(|_| Mutex::new(None)).collect();
        std::thread::scope(|scope| {
            for _ in 0..opts.threads.min(specs.len()) {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let (Some(s), Some(slot)) = (specs.get(i), slots.get(i)) else {
                            break;
                        };
                        let r = one(s);
                        if let Ok(mut g) = slot.lock() {
                            *g = Some(r);
                        }
                    }
                });
            }
        });
        return slots
            .into_iter()
            .map(|m| {
                m.into_inner().ok().flatten().unwrap_or_else(|| {
                    Err(store_err(
                        Path::new("-"),
                        "a worker thread did not finish its leg",
                    ))
                })
            })
            .collect();
    }
    specs.iter().map(|s| one(s)).collect()
}

/// Evaluate the combinations of `plan` on per-configuration estimates.
///
/// # Errors
/// `FA-E1400` if a combination names a configuration without a result.
pub fn evaluate(plan: &Plan, configs: &[ConfigResult]) -> Result<Vec<Row>, DiffrunError> {
    let by_label: BTreeMap<&str, &ConfigResult> =
        configs.iter().map(|c| (c.label.as_str(), c)).collect();
    plan.combinations
        .iter()
        .map(|k| {
            let mut value = 0.0;
            let mut var = 0.0;
            let mut cross = Some(0.0);
            for (coef, label) in &k.terms {
                let c = by_label
                    .get(label.as_str())
                    .ok_or_else(|| DiffrunError::Plan {
                        plan: plan.name.clone(),
                        problem: format!("no result for configuration {label}"),
                        fix: "run every configuration of the plan",
                    })?;
                let w = f64::from(*coef);
                value = w.mul_add(c.estimate.rate, value);
                var = c
                    .estimate
                    .jackknife_sigma
                    .mul_add(c.estimate.jackknife_sigma, var);
                cross = match (cross, &c.cross_check) {
                    (Some(s), Some(e)) => Some(w.mul_add(e.rate, s)),
                    _ => None,
                };
            }
            Ok(Row {
                combination: k.clone(),
                value,
                sigma_rss: var.sqrt(),
                cross_check: cross,
            })
        })
        .collect()
}

/// Run a plan: check that every leg shares the base ICs (`FA-E1401` before any integration),
/// run every leg, merge each configuration's legs, estimate every configuration with `est`
/// (and the cross-check source), and evaluate the combinations.
///
/// # Errors
/// `FA-E1400`/`FA-E1401`/`FA-E1402` and the errors of the runs and the estimator.
pub fn run_plan(
    plan: &Plan,
    data: &DataSources<'_>,
    est: &Estimator,
    opts: &RunOptions,
) -> Result<PlanResult, DiffrunError> {
    plan.validate()?;
    opts.validate()?;
    let specs: Vec<&ScenarioSpec> = plan.configs.iter().flat_map(|c| &c.legs).collect();
    for s in specs.iter().filter(|s| s.mode != Mode::TestParticle) {
        opts.segments(s)?;
    }
    let initial = specs
        .iter()
        .map(|s| Ok((s.name.clone(), initial_conditions(s, data)?)))
        .collect::<Result<Vec<_>, DiffrunError>>()?;
    let shared = check_identical_ics(plan, &initial)?;
    let mut legs = run_legs(&specs, data, est, opts).into_iter();
    let mut configs = Vec::with_capacity(plan.configs.len());
    for c in &plan.configs {
        let mut tracks = Vec::new();
        let mut first: Option<LegResult> = None;
        let mut steps = 0;
        let mut hashes = Vec::new();
        for s in &c.legs {
            let r = legs
                .next()
                .ok_or_else(|| store_err(Path::new("-"), "missing leg result"))??
                .ok_or_else(|| store_err(Path::new("-"), format!("leg {} interrupted", s.name)))?;
            steps += r.steps;
            hashes.push(r.ic_hash.clone());
            tracks.push(r.track.clone());
            if first.is_none() {
                first = Some(r);
            }
        }
        let track = Track::merge(tracks);
        let estimate = est.estimate(&track, est.source)?;
        let cross_check = est.estimate(&track, other(est.source)).ok();
        let (model, ic_source) = first.map(|r| (r.model, r.ic_source)).unwrap_or_default();
        configs.push(ConfigResult {
            label: c.label.clone(),
            legs: c.legs.iter().map(|s| s.name.clone()).collect(),
            bodies: c
                .legs
                .first()
                .map(|s| s.bodies.iter().map(|b| b.naif()).collect())
                .unwrap_or_default(),
            ic_hashes: hashes,
            estimate,
            cross_check,
            steps,
            model,
            ic_source,
        });
    }
    let rows = evaluate(plan, &configs)?;
    Ok(PlanResult {
        plan: plan.name.clone(),
        estimator: est.id(est.source),
        cross_check_estimator: est.id(other(est.source)),
        shared_ic_hash: shared,
        constants: initial
            .first()
            .map(|(_, i)| i.constants.clone())
            .unwrap_or_default(),
        ic_epoch_jd_tdb: initial
            .first()
            .map(|(_, i)| i.epoch_jd_tdb[0] + i.epoch_jd_tdb[1]),
        configs,
        rows,
    })
}

impl PlanResult {
    /// The rows of one protocol.
    pub fn rows_of(&self, protocol: Protocol) -> impl Iterator<Item = &Row> {
        self.rows
            .iter()
            .filter(move |r| r.combination.protocol == protocol)
    }

    /// JSON (pretty), for the nightly evidence artifact.
    ///
    /// # Errors
    /// `FA-E1402` if serialisation fails.
    pub fn to_json(&self) -> Result<String, DiffrunError> {
        serde_json::to_string_pretty(self).map_err(|e| store_err(Path::new("-"), e))
    }

    /// A markdown table: the per-configuration rates, then the rows by protocol, with
    /// `reference(label, protocol)` (e.g. Park17 Table 3) printed beside each row when given.
    /// Row values are printed in Park17's sign convention ([`Protocol::to_park17`], ADR-0014),
    /// labelled in each table header; [`Row::value`] and the JSON keep the raw signed sums.
    #[must_use]
    pub fn markdown(&self, reference: &dyn Fn(&str, Protocol) -> Option<f64>) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        let _ = writeln!(s, "**Plan** `{}`  ", self.plan);
        let _ = writeln!(s, "**Estimator** {}  ", self.estimator);
        let _ = writeln!(s, "**Cross-check** {}  ", self.cross_check_estimator);
        let _ = writeln!(
            s,
            "**Shared IC hash (base bodies)** `{}`  ",
            self.shared_ic_hash
        );
        let _ = writeln!(s, "**Constants** {}  ", self.constants);
        let _ = writeln!(
            s,
            "**IC epoch** {} (shared by every leg)\n",
            self.ic_epoch_jd_tdb
                .map_or_else(|| "unknown".into(), |jd| format!("JD {jd:.6} TDB"))
        );
        let _ = writeln!(
            s,
            "| configuration | bodies (NAIF) | ϖ̇ (″/cy) | jack-knife σ | formal σ | Q (″/cy²) | rms (″) | passages | cross-check ϖ̇ | steps |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|");
        for c in &self.configs {
            let e = &c.estimate;
            let _ = writeln!(
                s,
                "| {} | {:?} | {:.6} | {:.6} | {:.6} | {:.6} | {:.4} | {} | {} | {} |",
                c.label,
                c.bodies,
                e.rate,
                e.jackknife_sigma,
                e.formal_sigma,
                e.q,
                e.rms_residual,
                e.passages,
                c.cross_check
                    .as_ref()
                    .map_or_else(|| "—".into(), |x| format!("{:.6}", x.rate)),
                c.steps
            );
        }
        // Rule 4 (PO-5): every configuration states its force model and gauge.
        let _ = writeln!(
            s,
            "\n| configuration | force model (gauge) and integrator |"
        );
        let _ = writeln!(s, "|---|---|");
        for c in &self.configs {
            let _ = writeln!(s, "| {} | {} |", c.label, c.model.replace('\n', " "));
        }
        for p in [
            Protocol::Add,
            Protocol::Remove,
            Protocol::CrossTerm,
            Protocol::CrossTermRemove,
        ] {
            if self.rows_of(p).next().is_none() {
                continue;
            }
            // Every value column is in Park17's convention (ADR-0014, `Protocol::to_park17`), so
            // `value − reference` compares like with like; the JSON keeps the raw signed sums.
            // Cross terms also print the raw inclusion–exclusion second difference ([`Row::value`])
            // in its own labelled column (progress review after WP18, R1).
            let cross = p.is_cross_term();
            let _ = writeln!(
                s,
                "\n| {} row | value (″/cy; {}) | σ_rss | cross-check | reference | value − reference |{}",
                p.label(),
                p.park17_convention(),
                if cross {
                    " second difference (″/cy) |"
                } else {
                    ""
                }
            );
            let _ = writeln!(
                s,
                "|---|---|---|---|---|---|{}",
                if cross { "---|" } else { "" }
            );
            for r in self.rows_of(p) {
                let reference = reference(&r.combination.label, p);
                let value = p.to_park17(r.value);
                let _ = writeln!(
                    s,
                    "| {} | {:.6} | {:.6} | {} | {} | {} |{}",
                    r.combination.label,
                    value,
                    r.sigma_rss,
                    r.cross_check
                        .map(|x| p.to_park17(x))
                        .map_or_else(|| "—".into(), |x| format!("{x:.6}")),
                    reference.map_or_else(|| "—".into(), |x| format!("{x}")),
                    reference.map_or_else(|| "—".into(), |x| format!("{:+.6}", value - x)),
                    if cross {
                        format!(" {:.6} |", r.value)
                    } else {
                        String::new()
                    }
                );
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    //! The store key covers the integrator settings (issue #31), on the real DE440 initial
    //! conditions (PR tier: header.440 from the pinned JPL cache, CI-005). Unit tests, because
    //! emulating a store file written under other settings needs the private store layout.
    use super::*;
    use ferroastro_constants::header440::Header;
    use ferroastro_core::ErrorCode;
    use ferroastro_core::frames::Icrf;
    use ferroastro_integrators::Ias15Config;
    use ferroastro_precession::park2017::{
        FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane,
    };
    use ferroastro_scenario::BodyName;

    // Run controls (not physical values): PR-tier lengths and output cadence.
    const LEG_YEARS: f64 = 3.0; // lint: allow-literal(run length, PR-tier cost)
    const SEGMENT_YEARS: f64 = 1.0; // lint: allow-literal(checkpoint segment length)
    const SEGMENTS: u32 = 3; // LEG_YEARS / SEGMENT_YEARS
    const CADENCE_DAYS: f64 = 1.0; // lint: allow-literal(output cadence of the estimator samples)
    const JACKKNIFE_BLOCKS: usize = 4;

    fn header() -> (Header, String) {
        let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
        let prov = f.provenance();
        (
            Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
            prov,
        )
    }

    fn spec() -> ScenarioSpec {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ci/scenarios/sun-mercury-newton-header440.toml");
        let mut s = ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
        s.output.cadence_days = Some(CADENCE_DAYS);
        s.output.perihelion = vec![BodyName::Mercury];
        s.bodies = vec![BodyName::Sun, BodyName::Mercury, BodyName::Venus];
        s.span.years = LEG_YEARS;
        s.name = "issue31-store-key".into();
        s
    }

    fn estimator() -> Estimator {
        Estimator {
            park: Park2017 {
                plane: ReferencePlane::<Icrf>::park2017_mean_orbit().unwrap(),
                definition: LongitudeDefinition::Projected,
                passages: PassageConfig::default(),
                model: FitModel {
                    quadratic: true,
                    frequencies: vec![],
                },
                jackknife_blocks: JACKKNIFE_BLOCKS,
            },
            source: PassageSource::SampledStates,
            body: BodyName::Mercury,
            central: BodyName::Sun,
            window_jd: None,
        }
    }

    fn read(path: &Path) -> Stored {
        postcard::from_bytes(&std::fs::read(path).unwrap()).unwrap()
    }

    /// The format v1 layout, to write a store file as the code before issue #31 did.
    #[derive(Serialize)]
    struct StoredV1 {
        format: String,
        scenario_digest: String,
        segment: u32,
        checkpoint: Vec<u8>,
        track: Track,
        steps: u64,
    }

    fn v1_of(s: &Stored) -> Vec<u8> {
        postcard::to_allocvec(&StoredV1 {
            format: STORE_FORMAT_V1.into(),
            scenario_digest: s.scenario_digest.clone(),
            segment: s.segment,
            checkpoint: s.checkpoint.clone(),
            track: s.track.clone(),
            steps: s.steps,
        })
        .unwrap()
    }

    /// Every IAS15 setting changes the store key, and equal settings give the same file. The
    /// exhaustive destructure stops compiling when `Ias15Config` gains a field, so the new field
    /// must be added here (and is then checked to reach the key).
    ///
    /// Verifies: CI-010
    #[test]
    fn every_ias15_setting_changes_the_store_key() {
        let s = spec();
        let dir = Path::new("store");
        let base = ias15_config(&s, true);
        let key = |c: &Ias15Config| store_path(dir, &s, &integrator_settings(c).unwrap());
        assert_eq!(key(&base), store_file(dir, &s).unwrap());
        assert_eq!(key(&base), key(&ias15_config(&s, true)));
        let Ias15Config {
            epsilon,
            safety,
            max_iterations,
            pc_tolerance,
            min_dt,
            max_rejections,
            dense_output,
            pc_b_from_g,
        } = base;
        let changed = [
            Ias15Config {
                epsilon: epsilon * 2.0,
                ..base
            },
            Ias15Config {
                safety: safety * 0.5,
                ..base
            },
            Ias15Config {
                max_iterations: max_iterations + 1,
                ..base
            },
            Ias15Config {
                pc_tolerance: pc_tolerance + 1.0,
                ..base
            },
            Ias15Config {
                min_dt: min_dt + 1.0,
                ..base
            },
            Ias15Config {
                max_rejections: max_rejections + 1,
                ..base
            },
            Ias15Config {
                dense_output: !dense_output,
                ..base
            },
            Ias15Config {
                pc_b_from_g: !pc_b_from_g,
                ..base
            },
        ];
        for c in &changed {
            assert_ne!(c, &base);
            assert_ne!(key(c), key(&base), "{c:?} must change the store key");
        }
    }

    /// Issue #31: a leg interrupted under one IAS15 configuration is never resumed under another.
    /// Same settings: the leg resumes from its store file and equals the one-piece leg bit for
    /// bit. Other settings (the store file of a build with the other `pc_b_from_g` predictor,
    /// emulated by flipping it in the stored checkpoint and header): the leg runs from the start,
    /// equals the one-piece leg, and leaves that file alone. A file under this run's name whose
    /// header or checkpoint records other settings, and a format-v1 file, are refused
    /// (`FA-E1402`).
    ///
    /// Verifies: CI-010
    #[test]
    fn interrupted_leg_resumes_only_under_the_same_integrator_settings() {
        let (h, prov) = header();
        let data = DataSources::none().with_header(&h, &prov);
        let spec = spec();
        let est = estimator();
        let dir = std::env::temp_dir().join(format!("fa-diffrun-key31-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let opts = RunOptions {
            segment_years: SEGMENT_YEARS,
            store: Some(dir.clone()),
            threads: 1,
        };
        let one_piece = run_leg(
            &spec,
            &data,
            &est,
            &RunOptions {
                segment_years: LEG_YEARS,
                ..RunOptions::default()
            },
            None,
        )
        .unwrap()
        .unwrap();
        let file = store_file(&dir, &spec).unwrap();
        let interrupt = || {
            assert!(
                run_leg(&spec, &data, &est, &opts, Some(1))
                    .unwrap()
                    .is_none()
            );
            assert!(file.exists());
            read(&file)
        };

        // Same settings: resumed.
        let stored = interrupt();
        assert_eq!(stored.integrator, leg_settings(&spec).unwrap());
        let resumed = run_leg(&spec, &data, &est, &opts, None).unwrap().unwrap();
        assert!(resumed.resumed_from_store);
        assert_eq!(resumed.segments_run, SEGMENTS - 1);
        assert_eq!(one_piece.track.differing_words(&resumed.track), Some(0));
        assert!(!file.exists());

        // Other settings: the file of the other predictor is not reused.
        let stored = interrupt();
        let mut ck = Checkpoint::decode(&stored.checkpoint).unwrap();
        let IntegratorSnapshot::Ias15(ias) = &mut ck.integrator else {
            panic!("an IAS15 leg checkpoints an IAS15 state");
        };
        ias.config.pc_b_from_g = !ias.config.pc_b_from_g;
        let other = Stored {
            integrator: snapshot_settings(&ck.integrator).unwrap(),
            checkpoint: ck.encode().unwrap(),
            ..stored.clone()
        };
        let other_file = store_path(&dir, &spec, &other.integrator);
        assert_ne!(other_file, file);
        save(&other_file, &other).unwrap();
        std::fs::remove_file(&file).unwrap();
        let fresh = run_leg(&spec, &data, &est, &opts, None).unwrap().unwrap();
        assert!(!fresh.resumed_from_store, "resumed under other settings");
        assert_eq!(fresh.segments_run, SEGMENTS);
        assert_eq!(one_piece.track.differing_words(&fresh.track), Some(0));
        assert_eq!(one_piece.steps, fresh.steps);
        assert!(
            other_file.exists(),
            "the other settings' file is left alone"
        );
        assert!(!file.exists());

        // A file under this run's name recording other settings: refused, header or checkpoint.
        let refused = |what: &str| {
            let e = run_leg(&spec, &data, &est, &opts, None).unwrap_err();
            println!("{what}: {e}");
            assert_eq!(e.code(), ErrorCode::DiffrunStore, "{what}");
            assert!(e.to_string().contains("issue #31"), "{what}: {e}");
            assert!(file.exists(), "{what}: a refused file is kept");
        };
        std::fs::rename(&other_file, &file).unwrap();
        refused("header with other settings");
        save(
            &file,
            &Stored {
                integrator: stored.integrator.clone(),
                ..other
            },
        )
        .unwrap();
        refused("checkpoint with other settings");

        // Format v1, at its own name or this run's: refused, never resumed.
        std::fs::remove_file(&file).unwrap();
        let legacy = legacy_store_path(&dir, &spec);
        std::fs::write(&legacy, v1_of(&stored)).unwrap();
        let e = run_leg(&spec, &data, &est, &opts, None).unwrap_err();
        println!("v1 file: {e}");
        assert_eq!(e.code(), ErrorCode::DiffrunStore);
        assert!(e.to_string().contains(STORE_FORMAT_V1), "{e}");
        assert!(!file.exists(), "nothing ran past the refusal");
        std::fs::rename(&legacy, &file).unwrap();
        refused("v1 file under the v2 name");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
