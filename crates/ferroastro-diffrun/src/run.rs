// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Running a plan: the IC-identity check, legs in checkpointed segments (CI-010) with an optional
//! on-disk store, one estimator for every configuration, and the contribution table.

use crate::DiffrunError;
use crate::estimator::{Estimate, Estimator, PassageSource};
use crate::plan::{Combination, Plan, Protocol};
use crate::track::Track;
use ferroastro_propagate::{
    Checkpoint, DataSources, InitialRecords, initial_conditions, resume_until, run, run_until,
};
use ferroastro_scenario::{IcRecord, Mode, ScenarioSpec, ic_hash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Format tag of a store file.
pub const STORE_FORMAT: &str = "ferroastro-diffrun store v1";

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
    format: String,
    scenario_digest: String,
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

fn store_path(dir: &Path, spec: &ScenarioSpec) -> PathBuf {
    dir.join(format!("{}.leg", spec.digest()))
}

fn load(path: &Path, spec: &ScenarioSpec) -> Result<Option<Stored>, DiffrunError> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(store_err(path, e)),
    };
    let s: Stored = postcard::from_bytes(&bytes).map_err(|e| store_err(path, e))?;
    if s.format != STORE_FORMAT || s.scenario_digest != spec.digest() {
        return Err(store_err(
            path,
            format!(
                "holds {} for scenario digest {}, not {STORE_FORMAT} for {}",
                s.format,
                s.scenario_digest,
                spec.digest()
            ),
        ));
    }
    Ok(Some(s))
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
/// `opts.store` if a file for this scenario exists. With `stop_after_segments = Some(k)` the call
/// returns `Ok(None)` after `k` new segments, leaving the store file for a later call (used to
/// test interruption; requires a store).
///
/// # Errors
/// `FA-E1402` for store I/O or a foreign store file; `FA-E1302` from the driver; the run's own
/// errors.
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
    let sign = if spec.span.years < 0.0 { -1.0 } else { 1.0 };
    let path = opts.store.as_deref().map(|d| store_path(d, spec));
    let stored = match &path {
        Some(p) => load(p, spec)?,
        None => None,
    };
    let resumed_from_store = stored.is_some();
    let (mut track, mut segment, mut ck) = match stored {
        Some(s) => (s.track, s.segment, Some(Checkpoint::decode(&s.checkpoint)?)),
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
    let specs: Vec<&ScenarioSpec> = plan.configs.iter().flat_map(|c| &c.legs).collect();
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
        let _ = writeln!(s, "**Constants** {}\n", self.constants);
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
        for p in [
            Protocol::Add,
            Protocol::Remove,
            Protocol::CrossTerm,
            Protocol::CrossTermRemove,
        ] {
            if self.rows_of(p).next().is_none() {
                continue;
            }
            let _ = writeln!(
                s,
                "\n| {} row | value (″/cy) | σ_rss | cross-check | reference | value − reference |",
                p.label()
            );
            let _ = writeln!(s, "|---|---|---|---|---|---|");
            for r in self.rows_of(p) {
                let reference = reference(&r.combination.label, p);
                let _ = writeln!(
                    s,
                    "| {} | {:.6} | {:.6} | {} | {} | {} |",
                    r.combination.label,
                    r.value,
                    r.sigma_rss,
                    r.cross_check
                        .map_or_else(|| "—".into(), |x| format!("{x:.6}")),
                    reference.map_or_else(|| "—".into(), |x| format!("{x}")),
                    reference.map_or_else(|| "—".into(), |x| format!("{:+.6}", r.value - x))
                );
            }
        }
        s
    }
}
