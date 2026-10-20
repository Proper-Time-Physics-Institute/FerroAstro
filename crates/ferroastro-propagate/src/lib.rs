// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-propagate
//!
//! The run driver (CRATE-PLAN L8; CI-010, TST-005, NUM-022): from a
//! [`ScenarioSpec`] it assembles the initial conditions
//! (`ferroastro-system`), the force model (`ferroastro-forces`) and IAS15
//! (`ferroastro-integrators`), integrates, and returns states at the output cadence and
//! perihelion passages (NUM-010) as exact `(hi, lo)` records ([`RunOutput`]).
//!
//! * [`run`]: a whole scenario. Scenarios are first checked against what this runner
//!   implements; anything else is refused with `FA-E1201` naming it (REF-011).
//! * [`run_until`] / [`resume`] / [`resume_until`]: stop after the first accepted step at or
//!   beyond a time, write a [`Checkpoint`] (integrator state including the IAS15 predictor
//!   coefficients, postcard encoded), and continue later, in another process, bit for bit
//!   (CI-010); a long run can be cut into any number of such segments.
//! * [`initial_conditions`]: the initial conditions a scenario assembles, per body, exactly as
//!   hashed, so that differential runs can prove they share them (NUM-013).
//! * [`run_with_energy`]: [`run`] plus the N-body model energy at the start and the end
//!   (WP18 speed-table accuracy column).
//! * [`run_batch`]: independent scenarios on `threads` worker threads (rayon, behind the
//!   optional `parallel` feature; sequential without it, and on wasm32). Results do not depend
//!   on the thread count (TST-005).
//!
//! Test-particle scenarios use the passage search of the single-body rungs ([`passages`]), so
//! a scenario reproduces the ladder's direct-API rung bit for bit.
//!
//! ```
//! use ferroastro_propagate::{DataSources, run};
//! use ferroastro_scenario::ScenarioSpec;
//! let spec = ScenarioSpec::parse(r#"
//! schema_version = 1
//! name = "eih-canary"
//! description = "a switch this runner does not implement yet"
//! mode = "n-body"
//! bodies = ["Sun", "Mercury"]
//! central_body = "Sun"
//! initial_conditions = { source = "de440-header" }
//! constants = { gm_set = "de440" }
//! forces = { newton = true, gr_eih = true }
//! integrator = { name = "ias15", precision = "f64" }
//! span = { years = 1.0 }
//! "#)?;
//! let err = run(&spec, &DataSources::none()).unwrap_err();
//! assert!(err.to_string().starts_with("FA-E1201"));
//! assert!(err.to_string().contains("forces.gr_eih"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod field;
mod nbody;
mod output;
mod particle;
pub mod passages;
mod setup;

pub use error::PropagateError;
pub use nbody::DEFAULT_DT0_DAYS;
pub use output::{PassageRecord, RunOutput, StateRecord};
pub use setup::{
    DataSources, RUNNER, SuppliedState, constants_version, de440_solar_spin, refuse_unsupported,
    supports,
};

use ferroastro_integrators::{Ias15Config, Ias15Snapshot};
use ferroastro_real::DoubleDouble;
use ferroastro_scenario::{IcRecord, Mode, Parts, Precision, ScenarioSpec};
use nbody::{DriverState, NBodyRun};
use serde::{Deserialize, Serialize};

/// Version tag of the checkpoint encoding.
pub const CHECKPOINT_FORMAT: &str = "ferroastro-checkpoint v1";

/// IAS15 settings of a scenario: the defaults (Rein & Spiegel 2015) with the scenario's
/// overrides; dense output on for N-body runs (samples and events).
#[must_use]
pub fn ias15_config(spec: &ScenarioSpec, dense_output: bool) -> Ias15Config {
    let d = Ias15Config::default();
    Ias15Config {
        epsilon: spec.integrator.epsilon.unwrap_or(d.epsilon),
        pc_tolerance: spec.integrator.pc_tolerance.unwrap_or(d.pc_tolerance),
        dense_output,
        ..d
    }
}

/// A resumable state of an N-body run (CI-010): the integrator snapshot, the driver's sample
/// and passage counters, and what identifies the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// [`CHECKPOINT_FORMAT`].
    pub format: String,
    /// Scenario name.
    pub scenario: String,
    /// [`ScenarioSpec::digest`] of the scenario that produced it.
    pub scenario_digest: String,
    /// IC hash of the run.
    pub ic_hash: String,
    /// `f64` or `dd`.
    pub precision: String,
    /// The integrator state.
    pub integrator: Ias15Snapshot,
    /// Index of the next output sample.
    pub next_sample: u64,
    /// Passages found so far.
    pub passages: u64,
}

impl Checkpoint {
    /// postcard encoding.
    ///
    /// # Errors
    /// `FA-E1302` if encoding fails.
    pub fn encode(&self) -> Result<Vec<u8>, PropagateError> {
        postcard::to_allocvec(self).map_err(|e| PropagateError::Checkpoint {
            problem: format!("cannot encode: {e}"),
        })
    }

    /// Decode [`Checkpoint::encode`]'s bytes.
    ///
    /// # Errors
    /// `FA-E1302` for malformed bytes or another format version.
    pub fn decode(bytes: &[u8]) -> Result<Self, PropagateError> {
        let c: Self = postcard::from_bytes(bytes).map_err(|e| PropagateError::Checkpoint {
            problem: format!("cannot decode: {e}"),
        })?;
        if c.format != CHECKPOINT_FORMAT {
            return Err(PropagateError::Checkpoint {
                problem: format!("format {:?}, expected {CHECKPOINT_FORMAT:?}", c.format),
            });
        }
        Ok(c)
    }
}

/// The initial conditions of a scenario exactly as [`ferroastro_scenario::ic_hash`] digests
/// them: one record per body in storage order (`(hi, lo)` parts), the constants version, the mode
/// and the epoch.
#[derive(Debug, Clone, PartialEq)]
pub struct InitialRecords {
    /// Constants version (DE440 table SHA-256).
    pub constants: String,
    /// `n-body` or `test-particle`.
    pub mode: String,
    /// Initial epoch, TDB Julian date, as hashed.
    pub epoch_jd_tdb: Parts,
    /// Per-body records, in storage order.
    pub records: Vec<IcRecord>,
    /// The IC hash of the whole set (equals [`RunOutput::ic_hash`] of a run of the scenario).
    pub ic_hash: String,
}

/// Assemble a scenario's initial conditions without integrating.
///
/// # Errors
/// As [`run`] up to the start of the integration.
pub fn initial_conditions(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<InitialRecords, PropagateError> {
    prepare(spec)?;
    match (spec.mode, spec.integrator.precision) {
        (Mode::TestParticle, _) => particle::initial(spec, data),
        (Mode::EphemerisField, _) => Err(setup::setup(
            &spec.name,
            "ephemeris-field runs have no differential-run IC records",
            "use run() for oracle stage B",
        )),
        (Mode::NBody, Precision::F64) => Ok(NBodyRun::<f64>::new(spec, data, None)?.initial()),
        (Mode::NBody, Precision::Dd) => {
            Ok(NBodyRun::<DoubleDouble>::new(spec, data, None)?.initial())
        }
    }
}

fn prepare(spec: &ScenarioSpec) -> Result<(), PropagateError> {
    spec.validate()?;
    refuse_unsupported(spec)
}

fn years_to_seconds(years: f64) -> f64 {
    years * ferroastro_scenario::DAYS_PER_JULIAN_YEAR * ferroastro_core::time::SECONDS_PER_DAY
}

/// Run a scenario to the end of its span (or its passage count).
///
/// # Errors
/// `FA-E1201`/`FA-E1203` for a refused or invalid scenario, `FA-E1300` for missing data, and
/// the errors of the integrator, force model and IC assembly with their own codes.
pub fn run(spec: &ScenarioSpec, data: &DataSources<'_>) -> Result<RunOutput, PropagateError> {
    prepare(spec)?;
    match (spec.mode, spec.integrator.precision) {
        (Mode::TestParticle, Precision::F64) => particle::run::<f64>(spec, data),
        (Mode::TestParticle, Precision::Dd) => particle::run::<DoubleDouble>(spec, data),
        // `supports` refuses double-double for this mode before we get here.
        (Mode::EphemerisField, _) => field::run(spec, data),
        (Mode::NBody, Precision::F64) => NBodyRun::<f64>::new(spec, data, None)?.advance(None),
        (Mode::NBody, Precision::Dd) => {
            NBodyRun::<DoubleDouble>::new(spec, data, None)?.advance(None)
        }
    }
}

/// The model energy of an N-body run at its initial conditions and at its end (energy divided
/// by G, km⁵/s⁴; Neumaier-summed over every term's potential).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyCheck {
    /// Energy / G at the initial conditions.
    pub initial: f64,
    /// Energy / G at the end of the run.
    pub last: f64,
}

impl EnergyCheck {
    /// `|E(T) − E(0)| / |E(0)|`.
    #[must_use]
    pub fn relative_change(&self) -> f64 {
        ((self.last - self.initial) / self.initial).abs()
    }
}

/// [`run`], plus the model energy at the start and the end of an N-body run (`None` for the
/// test-particle and ephemeris-field modes, whose Newtonian energy is not a conserved quantity of
/// the model, and for a model with a non-conservative term). The output is [`run`]'s, bit for
/// bit: the energy is evaluated outside the integration (WP18 speed-table accuracy column).
///
/// # Errors
/// As [`run`].
pub fn run_with_energy(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
) -> Result<(RunOutput, Option<EnergyCheck>), PropagateError> {
    fn nbody<R: ferroastro_real::Real>(
        spec: &ScenarioSpec,
        data: &DataSources<'_>,
    ) -> Result<(RunOutput, Option<EnergyCheck>), PropagateError> {
        let mut r = NBodyRun::<R>::new(spec, data, None)?;
        let initial = r.energy_over_g();
        let out = r.advance(None)?;
        let energy = initial
            .zip(r.energy_over_g())
            .map(|(initial, last)| EnergyCheck { initial, last });
        Ok((out, energy))
    }
    prepare(spec)?;
    match (spec.mode, spec.integrator.precision) {
        (Mode::NBody, Precision::F64) => nbody::<f64>(spec, data),
        (Mode::NBody, Precision::Dd) => nbody::<DoubleDouble>(spec, data),
        _ => Ok((run(spec, data)?, None)),
    }
}

fn checkpoint_of(
    spec: &ScenarioSpec,
    (integrator, driver, ic_hash): (Ias15Snapshot, DriverState, String),
) -> Checkpoint {
    Checkpoint {
        format: CHECKPOINT_FORMAT.into(),
        scenario: spec.name.clone(),
        scenario_digest: spec.digest(),
        ic_hash,
        precision: spec.integrator.precision.label().into(),
        integrator,
        next_sample: driver.next_sample,
        passages: driver.passages,
    }
}

fn not_for_test_particles(spec: &ScenarioSpec) -> Result<(), PropagateError> {
    if spec.mode != Mode::NBody {
        return Err(PropagateError::Checkpoint {
            problem: format!(
                "scenario {} is a {} run; checkpointing is implemented for n-body runs",
                spec.name,
                spec.mode.label()
            ),
        });
    }
    Ok(())
}

/// Run an N-body scenario until the first accepted step at or beyond `stop_years` (Julian years
/// from the initial epoch) and return the output so far with a [`Checkpoint`], or the complete
/// output and `None` if the run ends first.
///
/// # Errors
/// As [`run`]; `FA-E1302` for a test-particle scenario.
pub fn run_until(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    stop_years: f64,
) -> Result<(RunOutput, Option<Checkpoint>), PropagateError> {
    prepare(spec)?;
    not_for_test_particles(spec)?;
    let stop = Some(years_to_seconds(stop_years));
    let (out, parts) = match spec.integrator.precision {
        Precision::F64 => {
            let mut r = NBodyRun::<f64>::new(spec, data, None)?;
            let out = r.advance(stop)?;
            (out, r.checkpoint_parts()?)
        }
        Precision::Dd => {
            let mut r = NBodyRun::<DoubleDouble>::new(spec, data, None)?;
            let out = r.advance(stop)?;
            (out, r.checkpoint_parts()?)
        }
    };
    let ck = (!out.finished).then(|| checkpoint_of(spec, parts));
    Ok((out, ck))
}

fn matching<'c>(
    spec: &ScenarioSpec,
    ck: &'c Checkpoint,
) -> Result<(DriverState, &'c Ias15Snapshot), PropagateError> {
    prepare(spec)?;
    not_for_test_particles(spec)?;
    if ck.scenario_digest != spec.digest() || ck.precision != spec.integrator.precision.label() {
        return Err(PropagateError::Checkpoint {
            problem: format!(
                "checkpoint of scenario {} ({}, digest {}) does not match scenario {} ({}, digest {})",
                ck.scenario,
                ck.precision,
                ck.scenario_digest,
                spec.name,
                spec.integrator.precision.label(),
                spec.digest()
            ),
        });
    }
    Ok((
        DriverState {
            next_sample: ck.next_sample,
            passages: ck.passages,
        },
        &ck.integrator,
    ))
}

/// Continue a run from a checkpoint to the end of its span; the output holds the records after
/// the checkpoint (join with [`RunOutput::extend_with`]).
///
/// # Errors
/// `FA-E1302` if the checkpoint belongs to another scenario, precision or IC hash; otherwise as
/// [`run`].
pub fn resume(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    ck: &Checkpoint,
) -> Result<RunOutput, PropagateError> {
    let (driver, snap) = matching(spec, ck)?;
    let restore = Some((snap, &driver, ck.ic_hash.as_str()));
    match spec.integrator.precision {
        Precision::F64 => NBodyRun::<f64>::new(spec, data, restore)?.advance(None),
        Precision::Dd => NBodyRun::<DoubleDouble>::new(spec, data, restore)?.advance(None),
    }
}

/// Continue a run from a checkpoint until the first accepted step at or beyond `stop_years`
/// (Julian years from the initial epoch, signed as the span), and return that segment with the
/// next [`Checkpoint`], or the rest of the run and `None` if it ends first. Chaining
/// [`run_until`] and `resume_until` over any stop times reproduces the uninterrupted run bit for
/// bit, because no step is ever shortened for a stop (CI-010).
///
/// # Errors
/// As [`resume`].
pub fn resume_until(
    spec: &ScenarioSpec,
    data: &DataSources<'_>,
    ck: &Checkpoint,
    stop_years: f64,
) -> Result<(RunOutput, Option<Checkpoint>), PropagateError> {
    let (driver, snap) = matching(spec, ck)?;
    let restore = Some((snap, &driver, ck.ic_hash.as_str()));
    let stop = Some(years_to_seconds(stop_years));
    let (out, parts) = match spec.integrator.precision {
        Precision::F64 => {
            let mut r = NBodyRun::<f64>::new(spec, data, restore)?;
            let out = r.advance(stop)?;
            (out, r.checkpoint_parts()?)
        }
        Precision::Dd => {
            let mut r = NBodyRun::<DoubleDouble>::new(spec, data, restore)?;
            let out = r.advance(stop)?;
            (out, r.checkpoint_parts()?)
        }
    };
    let next = (!out.finished).then(|| checkpoint_of(spec, parts));
    Ok((out, next))
}

/// Run independent scenarios, `threads` at a time with the `parallel` feature (rayon), in input
/// order otherwise. Each run is the deterministic [`run`], so the results do not depend on
/// `threads` (TST-005, NUM-022).
///
/// # Errors
/// `FA-E1303` if the thread pool cannot be built; each run's own error is returned in its slot.
pub fn run_batch(
    specs: &[ScenarioSpec],
    data: &DataSources<'_>,
    threads: usize,
) -> Result<Vec<Result<RunOutput, PropagateError>>, PropagateError> {
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .build()
            .map_err(|e| PropagateError::Output {
                problem: format!("cannot build a {threads}-thread pool: {e}"),
            })?;
        Ok(pool.install(|| specs.par_iter().map(|s| run(s, data)).collect()))
    }
    #[cfg(not(feature = "parallel"))]
    {
        let _ = threads;
        Ok(specs.iter().map(|s| run(s, data)).collect())
    }
}
