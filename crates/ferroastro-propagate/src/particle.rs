// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Test-particle mode: a massless body about a fixed central body (μ = GM of the central body),
//! with the central terms of `ferroastro-forces` and the perihelion search of [`crate::passages`]
//! (the code path of the single-body rungs, so a scenario reproduces them bit for bit).

use crate::output::{PassageRecord, RunOutput, StateRecord, parts, parts3};
use crate::passages::{grid_samples, perihelia_until, sample_uniform};
use crate::setup::{
    DataSources, constants_version, de440_solar_spin, epoch, relative_state, setup,
};
use crate::{PropagateError, ias15_config};
use ferroastro_constants::De440;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::GravParam;
use ferroastro_forces::{
    CentralTerm, ForceModel, LenseThirring, Pole, SchwarzschildPpn, SolarJ2, TestParticle,
};
use ferroastro_real::Real;
use ferroastro_scenario::{BodyName, IcRecord, ScenarioError, ScenarioSpec, ic_hash};
use ferroastro_system::Catalogue;

/// The test particle's force model in backend `R`: Newtonian `−μr/r³` plus the requested
/// central terms, in the order 1PN, J2, Lense–Thirring. The constants are the binary64 DE440
/// values converted exactly (as `SingleBody::model_in`).
fn model<R: Real>(
    spec: &ScenarioSpec,
    de: &De440,
    mu: GravParam,
) -> Result<TestParticle<Icrf, R>, PropagateError> {
    let f = &spec.forces;
    let (mu_r, c) = (mu.cast::<R>(), de.speed_of_light().cast::<R>());
    let (beta, gamma) = f
        .ppn
        .map_or((de.ppn_beta(), de.ppn_gamma()), |p| (p.beta, p.gamma));
    let sun_only = |switch: &'static str| -> Result<(), PropagateError> {
        if spec.central_body == BodyName::Sun {
            Ok(())
        } else {
            Err(ScenarioError::Value {
                name: spec.name.clone(),
                field: switch,
                value: "true".into(),
                problem: "solar terms need central_body = \"Sun\"".into(),
            }
            .into())
        }
    };
    let pole = || -> Result<Pole<Icrf, R>, PropagateError> {
        let k = ferroastro_frames::Pole::<R>::sun_wgccre2015()?.unit_vector();
        Ok(Pole::from_components(k)?)
    };
    let mut terms: Vec<Box<dyn CentralTerm<Icrf, R>>> = Vec::new();
    if f.gr_1pn_central {
        terms.push(Box::new(SchwarzschildPpn::new(
            mu_r,
            c,
            R::from_f64(beta),
            R::from_f64(gamma),
        )?));
    }
    if f.sun_j2 {
        sun_only("forces.sun_j2")?;
        terms.push(Box::new(SolarJ2::new(
            mu_r,
            R::from_f64(de.sun_j2()),
            de.sun_radius().cast(),
            pole()?,
        )?));
    }
    if f.lense_thirring {
        sun_only("forces.lense_thirring")?;
        terms.push(Box::new(LenseThirring::new(
            de440_solar_spin(de, &spec.name)?.cast(),
            c,
            R::from_f64(gamma),
            pole()?,
        )?));
    }
    Ok(TestParticle::new(mu_r, terms)?)
}

/// Run a test-particle scenario in backend `R`.
/// The assembled initial conditions of a test-particle scenario.
pub(crate) struct ParticleIc {
    de: De440,
    jd: f64,
    r0: [f64; 3],
    v0: [f64; 3],
    source: String,
    central: i32,
    particle: i32,
    mu: GravParam,
    records: [IcRecord; 2],
    constants: String,
}

/// Initial conditions, as `run` assembles and hashes them (f64, as WP7's D5).
pub(crate) fn assemble(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<ParticleIc, PropagateError> {
    let n = spec.name.as_str();
    let de = De440::load()?;
    let (_, jd) = epoch::<f64>(spec, data)?;
    let (r0, v0, source) = relative_state(spec, data, jd)?;
    let central = spec.central_body.naif();
    let particle = spec
        .bodies
        .iter()
        .map(|b| b.naif())
        .find(|&id| id != central)
        .ok_or_else(|| setup(n, "no test particle", "list two bodies"))?;
    let mu = if spec.central_body == BodyName::Sun {
        de.gm_sun()
    } else {
        Catalogue::de440_major(&de).gm(ferroastro_ephem::NaifId(central))?
    };
    let p64 = |z: [f64; 3]| z.map(|c| [c, 0.0]);
    let zero = [[0.0, 0.0]; 3];
    let records = [
        IcRecord {
            naif: central,
            gm: parts(mu.km3_per_s2()),
            position: zero,
            velocity: zero,
        },
        IcRecord {
            naif: particle,
            gm: [0.0, 0.0],
            position: p64(r0),
            velocity: p64(v0),
        },
    ];
    let constants = constants_version(&de);
    Ok(ParticleIc {
        de,
        jd,
        r0,
        v0,
        source,
        central,
        particle,
        mu,
        records,
        constants,
    })
}

/// The hashed initial conditions of a test-particle scenario.
pub(crate) fn initial(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<crate::InitialRecords, PropagateError> {
    let ic = assemble(spec, data)?;
    let epoch = parts(ic.jd);
    Ok(crate::InitialRecords {
        ic_hash: ic_hash(&ic.constants, spec.mode.label(), epoch, &ic.records),
        constants: ic.constants,
        mode: spec.mode.label().to_owned(),
        epoch_jd_tdb: epoch,
        records: ic.records.to_vec(),
    })
}

/// Refuse output requests the test-particle runner does not implement; `Ok(true)` when the
/// scenario asks for states at a cadence (f64 only, without perihelion output).
fn check_outputs<R: Real>(spec: &ScenarioSpec) -> Result<bool, PropagateError> {
    let n = spec.name.as_str();
    let sampled = spec.output.cadence_days.is_some();
    let refusal = if spec.span.years < 0.0 {
        Some("span.years < 0 (test-particle mode)")
    } else if sampled && R::NAME != f64::NAME {
        Some("output.cadence_days (test-particle mode, double-double)")
    } else if sampled
        && (spec.span.stop_after_passages.is_some() || !spec.output.perihelion.is_empty())
    {
        Some("output.cadence_days with perihelion output (test-particle mode)")
    } else {
        None
    };
    if let Some(switch) = refusal {
        return Err(ScenarioError::Unsupported {
            name: n.to_owned(),
            switch: switch.into(),
            runner: crate::RUNNER.into(),
            reason: "test-particle runs integrate forwards and report either perihelion passages \
                     or, in f64, states at the output cadence"
                .into(),
        }
        .into());
    }
    Ok(sampled)
}

pub(crate) fn run<R: Real>(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<RunOutput, PropagateError> {
    let n = spec.name.as_str();
    let sampled = check_outputs::<R>(spec)?;
    if sampled {
        return run_sampled(spec, data);
    }
    let ParticleIc {
        de,
        jd,
        r0,
        v0,
        source,
        central,
        particle,
        mu,
        records,
        constants,
    } = assemble(spec, data)?;
    let model = model::<R>(spec, &de, mu)?;
    let cfg = ias15_config(spec, false);
    let wanted = spec
        .span
        .stop_after_passages
        .map(|p| usize::try_from(p).unwrap_or(usize::MAX));
    let run = perihelia_until(
        &model,
        r0.map(R::from_f64),
        v0.map(R::from_f64),
        R::from_f64(mu.km3_per_s2()),
        wanted,
        R::from_f64(spec.span_seconds()),
        cfg,
    )?;
    // Nested `if let` (not a let-chain): the workspace MSRV is 1.85.
    if let Some(w) = wanted {
        if run.passages.len() < w {
            return Err(PropagateError::Run {
                scenario: n.to_owned(),
                problem: format!(
                    "only {} of {w} perihelion passages within span.years = {}",
                    run.passages.len(),
                    spec.span.years
                ),
            });
        }
    }
    let p64 = |z: [f64; 3]| z.map(|c| [c, 0.0]);
    Ok(RunOutput {
        scenario: spec.name.clone(),
        scenario_digest: spec.digest(),
        ic_hash: ic_hash(&constants, spec.mode.label(), parts(jd), &records),
        precision: spec.integrator.precision.label().into(),
        constants,
        ic_source: source,
        epoch_jd_tdb: parts(jd),
        model: format!(
            "{} [{}]; IAS15 epsilon = {:e}, safety = {}, PC tolerance = {:e}, {}",
            model.describe(),
            model.gauge().label(),
            cfg.epsilon,
            cfg.safety,
            cfg.pc_tolerance,
            R::NAME
        ),
        states: vec![StateRecord {
            t: [0.0, 0.0],
            bodies: vec![particle],
            positions: vec![p64(r0)],
            velocities: vec![p64(v0)],
        }],
        passages: run
            .passages
            .iter()
            .map(|p| PassageRecord {
                body: particle,
                central,
                t: [p.t, 0.0],
                r: parts3(p.r),
                v: parts3(p.v),
            })
            .collect(),
        final_state: None,
        stats: run.stats,
        finished: true,
    })
}

/// A test-particle scenario with `output.cadence_days` (f64): the particle's state every cadence
/// interval from the epoch to the end of the span, each grid time hit exactly
/// ([`sample_uniform`]); the central body is listed at the origin so that every record holds
/// both bodies. No passages are searched (the input of sampled-state estimators, CI-009).
fn run_sampled(spec: &ScenarioSpec, data: &DataSources<'_>) -> Result<RunOutput, PropagateError> {
    let ParticleIc {
        de,
        jd,
        r0,
        v0,
        source,
        central,
        particle,
        mu,
        records,
        constants,
    } = assemble(spec, data)?;
    let model = model::<f64>(spec, &de, mu)?;
    let cfg = ias15_config(spec, false);
    let cadence_s = spec
        .output
        .cadence_days
        .map_or(0.0, |c| c * ferroastro_core::time::SECONDS_PER_DAY);
    let span_s = spec.span_seconds();
    let samples = grid_samples(span_s, cadence_s);
    let (t, r, v) = sample_uniform(&model, r0, v0, cadence_s, samples, cfg)?;
    let p64 = |z: [f64; 3]| z.map(|c| [c, 0.0]);
    let zero = [[0.0, 0.0]; 3];
    let states = t
        .iter()
        .zip(r.iter().zip(&v))
        .map(|(tk, (rk, vk))| StateRecord {
            t: [*tk, 0.0],
            bodies: vec![central, particle],
            positions: vec![zero, p64(*rk)],
            velocities: vec![zero, p64(*vk)],
        })
        .collect();
    Ok(RunOutput {
        scenario: spec.name.clone(),
        scenario_digest: spec.digest(),
        ic_hash: ic_hash(&constants, spec.mode.label(), parts(jd), &records),
        precision: spec.integrator.precision.label().into(),
        constants,
        ic_source: source,
        epoch_jd_tdb: parts(jd),
        model: format!(
            "{} [{}]; IAS15 epsilon = {:e}, safety = {}, PC tolerance = {:e}, f64, samples every \
             {} d on an exact grid",
            model.describe(),
            model.gauge().label(),
            cfg.epsilon,
            cfg.safety,
            cfg.pc_tolerance,
            spec.output.cadence_days.unwrap_or(0.0)
        ),
        states,
        passages: Vec::new(),
        final_state: None,
        stats: ferroastro_integrators::Ias15Stats::default(),
        finished: true,
    })
}
