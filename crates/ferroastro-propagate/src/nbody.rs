// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! N-body mode: the `ferroastro-forces` N-body model advanced by IAS15, with states sampled at
//! the output cadence from dense output, perihelion passages located on dense output (NUM-010),
//! and checkpoint/resume (CI-010).
//!
//! The driver never shortens a step except the last one, which lands on the end of the span:
//! samples and events come from the step polynomials, and a run stopped for a checkpoint stops
//! after the first accepted step that reaches the requested time. The interrupted and the
//! uninterrupted runs therefore take the same steps, and a resumed run continues bit for bit.

use crate::output::{PassageRecord, RunOutput, StateRecord, parts, parts3};
use crate::setup::{DataSources, constants_version, epoch, nbody_ics, setup};
use crate::{PropagateError, ias15_config};
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::Length;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MoonVariables, NBody, NBodyModel,
    NewtonianMutual, Pole, SolarJ2WithReaction,
};
use ferroastro_integrators::{Ias15, Ias15Snapshot};
use ferroastro_real::Real;
use ferroastro_scenario::{BodyName, IcRecord, ScenarioError, ScenarioSpec, ic_hash};
use ferroastro_system::{Catalogue, Tier};
use serde::{Deserialize, Serialize};

/// Driver state carried across a checkpoint besides the integrator's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct DriverState {
    /// Index `k` of the next output sample (time `k · cadence`).
    pub(crate) next_sample: u64,
    /// Perihelion passages found so far.
    pub(crate) passages: u64,
}

/// The model of an N-body scenario (term order: Newtonian mutual, geocentric Moon, solar J2 with
/// reaction, as WP12) and the body order.
fn model<R: Real>(
    spec: &ScenarioSpec,
    de: &De440,
) -> Result<(NBodyModel<Icrf, R>, Vec<IcRecordGm>), PropagateError> {
    let cat = Catalogue::de440_major(de);
    let mut gms = Vec::with_capacity(spec.bodies.len());
    let mut bodies = Vec::with_capacity(spec.bodies.len());
    for b in &spec.bodies {
        let id = NaifId(b.naif());
        let gm = if *b == BodyName::Emb {
            de.gm(Body::EarthMoon).km3_per_s2()
        } else {
            cat.gm(id)?.km3_per_s2()
        };
        gms.push((b.naif(), gm));
        bodies.push(NBody {
            id,
            gm: R::from_f64(gm),
            tier: Tier::Major,
        });
    }
    let has = |b: BodyName| spec.bodies.contains(&b);
    let mut terms: Vec<Box<dyn ForceTerm<Icrf, R>>> = vec![Box::new(NewtonianMutual)];
    let vars = if has(BodyName::Earth) && has(BodyName::Moon) {
        terms.push(Box::new(GeocentricMoon::<R>::new(
            MOON_TAYLOR_TERMS_DEFAULT,
        )?));
        MoonVariables::Geocentric {
            emrat: R::from_f64(de.earth_moon_mass_ratio()),
        }
    } else {
        MoonVariables::Barycentric
    };
    if spec.forces.sun_j2 {
        if !has(BodyName::Sun) {
            return Err(ScenarioError::Value {
                name: spec.name.clone(),
                field: "forces.sun_j2",
                value: "true".into(),
                problem: "solar J2 needs the Sun among the bodies".into(),
            }
            .into());
        }
        let k = ferroastro_frames::Pole::<R>::sun_wgccre2015()?.unit_vector();
        terms.push(Box::new(SolarJ2WithReaction::new(
            R::from_f64(de.sun_j2()),
            Length::from_km(R::from_f64(de.sun_radius().km()))
                .map_err(|e| setup(&spec.name, e.to_string(), "check the DE440 table"))?,
            Pole::from_components(k)?,
        )?));
    }
    Ok((NBodyModel::new(&bodies, vars, terms)?, gms))
}

type IcRecordGm = (i32, f64);

/// Position and velocity of storage slot `i` in component-major flat vectors.
fn slot<R: Real>(x: &[R], n: usize, i: usize) -> [R; 3] {
    [0, 1, 2].map(|k| x.get(k * n + i).copied().unwrap_or(R::ZERO))
}

/// `(r_b − r_c)·(v_b − v_c)` of slots `b`, `c`.
fn rdotv<R: Real>(x: &[R], v: &[R], n: usize, b: usize, c: usize) -> R {
    let (rb, rc, vb, vc) = (slot(x, n, b), slot(x, n, c), slot(v, n, b), slot(v, n, c));
    (0..3).fold(R::ZERO, |s, k| {
        let dr = rb.get(k).copied().unwrap_or(R::ZERO) - rc.get(k).copied().unwrap_or(R::ZERO);
        let dv = vb.get(k).copied().unwrap_or(R::ZERO) - vc.get(k).copied().unwrap_or(R::ZERO);
        dr.mul_add(dv, s)
    })
}

/// One perihelion event: storage slots and NAIF ids of the body and the central body.
#[derive(Debug, Clone, Copy)]
struct Event {
    body: usize,
    central: usize,
    body_id: i32,
    central_id: i32,
}

/// A prepared N-body run in backend `R`.
pub(crate) struct NBodyRun<R: Real> {
    spec: ScenarioSpec,
    model: NBodyModel<Icrf, R>,
    ias: Ias15<R>,
    n: usize,
    t_end: R,
    cadence: Option<R>,
    events: Vec<Event>,
    g_last: Vec<R>,
    driver: DriverState,
    meta: RunOutput,
    records: Vec<IcRecord>,
    ic_epoch: ferroastro_scenario::Parts,
}

impl<R: Real> std::fmt::Debug for NBodyRun<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NBodyRun")
            .field("scenario", &self.spec.name)
            .field("t", &self.ias.time().to_f64())
            .finish_non_exhaustive()
    }
}

impl<R: Real> NBodyRun<R> {
    /// Build the run from the scenario; with `restore`, continue from a checkpoint instead of
    /// the initial conditions (the ICs are still assembled, to check the IC hash).
    #[expect(
        clippy::too_many_lines,
        reason = "one linear set-up: ICs, model, hash, integrator or restore, events, metadata"
    )]
    pub(crate) fn new(
        spec: &ScenarioSpec,
        data: &DataSources<'_>,
        restore: Option<(&Ias15Snapshot, &DriverState, &str)>,
    ) -> Result<Self, PropagateError> {
        let name = spec.name.as_str();
        let de = De440::load()?;
        let (ep, jd) = epoch::<R>(spec, data)?;
        let (states, pair, source) = nbody_ics::<R>(spec, data, ep)?;
        let (model, gms) = model::<R>(spec, &de)?;
        let (x0, v0) = model.pack(&states, pair.as_ref())?;
        let n = model.layout().len();
        let records: Vec<IcRecord> = gms
            .iter()
            .enumerate()
            .map(|(i, &(naif, gm))| IcRecord {
                naif,
                gm: [gm, 0.0],
                position: parts3(slot(&x0, n, i)),
                velocity: parts3(slot(&v0, n, i)),
            })
            .collect();
        let constants = constants_version(&de);
        let ic_epoch = parts(ep.jd());
        let hash = ic_hash(&constants, spec.mode.label(), ic_epoch, &records);
        let cfg = ias15_config(spec, true);
        let dt0_days = spec.integrator.dt0_days.unwrap_or(DEFAULT_DT0_DAYS);
        let dir = if spec.span.years < 0.0 { -1.0 } else { 1.0 };
        let ias = match restore {
            Some((snap, _, ic)) => {
                if ic != hash {
                    return Err(PropagateError::Checkpoint {
                        problem: format!(
                            "IC hash {ic} of the checkpoint differs from this run's {hash}"
                        ),
                    });
                }
                Ias15::restore(snap)?
            }
            None => Ias15::new(
                3 * n,
                R::ZERO,
                &x0,
                &v0,
                R::from_f64(dir * dt0_days * SECONDS_PER_DAY),
                cfg,
            )?,
        };
        let mut events = Vec::new();
        let central = spec.central_body.naif();
        let index = |id: i32| model.layout().index_of(NaifId(id));
        let geocentric = model.layout().moon_pair().is_some();
        for b in &spec.output.perihelion {
            let id = b.naif();
            let moon_slot = |x: i32| geocentric && (x == 399 || x == 301);
            if moon_slot(id) || moon_slot(central) {
                return Err(ScenarioError::Unsupported {
                    name: name.to_owned(),
                    switch: format!(
                        "output.perihelion = {} with geocentric Moon variables",
                        b.label()
                    ),
                    runner: crate::RUNNER.into(),
                    reason: "events are located on plain barycentric slots only".into(),
                }
                .into());
            }
            let (Some(bi), Some(ci)) = (index(id), index(central)) else {
                return Err(setup(
                    name,
                    "perihelion body not in the model",
                    "list it in bodies",
                ));
            };
            events.push(Event {
                body: bi,
                central: ci,
                body_id: id,
                central_id: central,
            });
        }
        let g_last = events
            .iter()
            .map(|e| rdotv(ias.positions(), ias.velocities(), n, e.body, e.central))
            .collect();
        let driver = restore.map_or(
            DriverState {
                next_sample: 0,
                passages: 0,
            },
            |(_, d, _)| d.clone(),
        );
        let meta = RunOutput {
            scenario: spec.name.clone(),
            scenario_digest: spec.digest(),
            ic_hash: hash,
            precision: spec.integrator.precision.label().into(),
            constants,
            ic_source: source,
            epoch_jd_tdb: parts(R::from_f64(jd)),
            model: format!(
                "{}; IAS15 epsilon = {:e}, safety = {}, PC tolerance = {:e}, dense output, {}",
                model.describe(),
                cfg.epsilon,
                cfg.safety,
                cfg.pc_tolerance,
                R::NAME
            ),
            states: Vec::new(),
            passages: Vec::new(),
            final_state: None,
            stats: ias.stats(),
            finished: false,
        };
        Ok(Self {
            spec: spec.clone(),
            model,
            ias,
            n,
            t_end: R::from_f64(spec.span_seconds()),
            cadence: spec
                .output
                .cadence_days
                .map(|d| R::from_f64(d * SECONDS_PER_DAY)),
            events,
            g_last,
            driver,
            meta,
            records,
            ic_epoch,
        })
    }

    /// The initial conditions as hashed (constants version, epoch, per-body records, hash).
    pub(crate) fn initial(&self) -> crate::InitialRecords {
        crate::InitialRecords {
            constants: self.meta.constants.clone(),
            mode: self.spec.mode.label().to_owned(),
            epoch_jd_tdb: self.ic_epoch,
            records: self.records.clone(),
            ic_hash: self.meta.ic_hash.clone(),
        }
    }

    fn record(&self, t: R, x: &[R], v: &[R]) -> Result<StateRecord, PropagateError> {
        let s = self.model.unpack(x, v)?;
        let mut rec = StateRecord {
            t: parts(t),
            bodies: Vec::with_capacity(self.n),
            positions: Vec::with_capacity(self.n),
            velocities: Vec::with_capacity(self.n),
        };
        for b in s.states()? {
            rec.bodies.push(b.0.0);
            let (p, w) = crate::setup::raw(&b.1);
            rec.positions.push(parts3(p));
            rec.velocities.push(parts3(w));
        }
        Ok(rec)
    }

    fn sample_time(&self, k: u64) -> Option<R> {
        // k < 2^53 in any feasible run, so the conversion is exact.
        #[expect(
            clippy::cast_precision_loss,
            reason = "sample indices stay far below 2^53"
        )]
        let kf = k as f64;
        self.cadence.map(|c| R::from_f64(kf) * c)
    }

    /// Advance until the end of the span, the passage count, or (if given) the first accepted
    /// step that reaches `stop_at` (seconds from the initial epoch). Returns the output of this
    /// segment.
    pub(crate) fn advance(&mut self, stop_at: Option<f64>) -> Result<RunOutput, PropagateError> {
        let mut out = self.meta.clone();
        let fwd = self.t_end >= R::ZERO;
        let after = |a: R, b: R| if fwd { a > b } else { a < b };
        let not_after = |a: R, b: R| !after(a, b);
        let stop_at = stop_at.map(R::from_f64);
        let (mut xs, mut vs) = (vec![R::ZERO; 3 * self.n], vec![R::ZERO; 3 * self.n]);
        // The initial sample (k = 0) of a fresh run.
        if self.driver.next_sample == 0 && self.ias.stats().steps == 0 && self.cadence.is_some() {
            out.states
                .push(self.record(R::ZERO, self.ias.positions(), self.ias.velocities())?);
            self.driver.next_sample = 1;
        }
        let wanted = self.spec.span.stop_after_passages.map(u64::from);
        loop {
            let rep = self.ias.step(&self.model, self.t_end)?;
            if rep.dt_done == R::ZERO {
                out.finished = true;
                break;
            }
            let (t0, t1) = self
                .ias
                .last_step_span()
                .ok_or_else(|| setup(&self.spec.name, "dense output is off", "internal error"))?;
            // Samples inside (t0, t1].
            while let Some(tk) = self.sample_time(self.driver.next_sample) {
                if !(after(tk, t0) && not_after(tk, t1)) {
                    break;
                }
                self.ias.dense_state(tk, &mut xs, &mut vs)?;
                out.states.push(self.record(tk, &xs, &vs)?);
                self.driver.next_sample += 1;
            }
            // Perihelion events (r·v rising through zero in time).
            for (j, e) in self.events.clone().iter().enumerate() {
                let g_now = rdotv(
                    self.ias.positions(),
                    self.ias.velocities(),
                    self.n,
                    e.body,
                    e.central,
                );
                let g_prev = self.g_last.get(j).copied().unwrap_or(g_now);
                let (early, late) = if fwd {
                    (g_prev, g_now)
                } else {
                    (g_now, g_prev)
                };
                if early < R::ZERO && late >= R::ZERO {
                    let n = self.n;
                    if let Some(te) = self
                        .ias
                        .locate_event(|_, x, v| rdotv(x, v, n, e.body, e.central))?
                    {
                        self.ias.dense_state(te, &mut xs, &mut vs)?;
                        let sub = |a: [R; 3], b: [R; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
                        out.passages.push(PassageRecord {
                            body: e.body_id,
                            central: e.central_id,
                            t: parts(te),
                            r: parts3(sub(slot(&xs, n, e.body), slot(&xs, n, e.central))),
                            v: parts3(sub(slot(&vs, n, e.body), slot(&vs, n, e.central))),
                        });
                        self.driver.passages += 1;
                    }
                }
                if let Some(g) = self.g_last.get_mut(j) {
                    *g = g_now;
                }
            }
            if wanted.is_some_and(|w| self.driver.passages >= w) || rep.reached_limit {
                out.finished = true;
                break;
            }
            if stop_at.is_some_and(|s| not_after(s, rep.t)) {
                break;
            }
        }
        if let Some(w) = wanted
            && out.finished
            && self.driver.passages < w
        {
            return Err(PropagateError::Run {
                scenario: self.spec.name.clone(),
                problem: format!(
                    "only {} of {w} perihelion passages within span.years = {}",
                    self.driver.passages, self.spec.span.years
                ),
            });
        }
        out.final_state =
            Some(self.record(self.ias.time(), self.ias.positions(), self.ias.velocities())?);
        out.stats = self.ias.stats();
        Ok(out)
    }

    /// The integrator snapshot and the driver state, for a checkpoint.
    pub(crate) fn checkpoint_parts(
        &self,
    ) -> Result<(Ias15Snapshot, DriverState, String), PropagateError> {
        Ok((
            self.ias.snapshot()?,
            self.driver.clone(),
            self.meta.ic_hash.clone(),
        ))
    }
}

/// First trial step of N-body runs when the scenario sets none (days; as WP12).
pub const DEFAULT_DT0_DAYS: f64 = 0.1;
