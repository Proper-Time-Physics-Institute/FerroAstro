// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The integrator of an N-body run: IAS15 (default, ADR-0008) or fixed-step ABM (WP24), behind
//! one interface for the driver (steps, dense output, events, counters, checkpoints).
//!
//! IAS15 shortens its last step to land on the end of the span. ABM never shortens a step: its
//! last step may end after the end of the span, and the driver then takes the final state,
//! samples and events at or before the end from dense output.

use ferroastro_abm::{Abm, AbmSnapshot};
use ferroastro_integrators::Ias15Stats as RunStats;
use ferroastro_integrators::{Ias15, Ias15Snapshot, IntegratorError, SecondOrderSystem};
use ferroastro_real::Real;
use serde::{Deserialize, Serialize};

/// The integrator snapshot of a checkpoint (format v3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegratorSnapshot {
    /// IAS15.
    Ias15(Ias15Snapshot),
    /// Adams–Bashforth–Moulton.
    Abm(AbmSnapshot),
}

impl IntegratorSnapshot {
    /// Accepted steps at the snapshot.
    #[must_use]
    pub const fn steps(&self) -> u64 {
        match self {
            IntegratorSnapshot::Ias15(s) => s.stats.steps,
            IntegratorSnapshot::Abm(s) => s.stats.steps,
        }
    }

    /// `"ias15"` or `"abm"`.
    #[must_use]
    pub const fn integrator(&self) -> &'static str {
        match self {
            IntegratorSnapshot::Ias15(_) => "ias15",
            IntegratorSnapshot::Abm(_) => "abm",
        }
    }
}

/// One step of either integrator.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stepped<R: Real> {
    /// Time at the end of the step.
    pub(crate) t: R,
    /// False when no step was taken (the run had already reached its end).
    pub(crate) moved: bool,
    /// True when the step reached or passed `t_end`.
    pub(crate) reached_end: bool,
}

/// The integrator of a run.
#[derive(Debug, Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "one integrator per run; boxing would add an indirection to every step"
)]
pub(crate) enum Stepper<R: Real> {
    Ias15(Ias15<R>),
    Abm(Abm<R>),
}

impl<R: Real> Stepper<R> {
    pub(crate) fn restore(snap: &IntegratorSnapshot) -> Result<Self, IntegratorError> {
        Ok(match snap {
            IntegratorSnapshot::Ias15(s) => Stepper::Ias15(Ias15::restore(s)?),
            IntegratorSnapshot::Abm(s) => Stepper::Abm(Abm::restore(s)?),
        })
    }

    /// Take one step towards `t_end` (IAS15 lands on it; ABM takes a full step).
    pub(crate) fn step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t_end: R,
    ) -> Result<Stepped<R>, IntegratorError> {
        match self {
            Stepper::Ias15(i) => {
                let r = i.step(sys, t_end)?;
                Ok(Stepped {
                    t: r.t,
                    moved: r.dt_done != R::ZERO,
                    reached_end: r.reached_limit,
                })
            }
            Stepper::Abm(a) => {
                let h = a.step_size();
                if (t_end - a.time()) * h <= R::ZERO {
                    return Ok(Stepped {
                        t: a.time(),
                        moved: false,
                        reached_end: true,
                    });
                }
                let r = a.step(sys)?;
                Ok(Stepped {
                    t: r.t,
                    moved: true,
                    reached_end: (t_end - r.t) * h <= R::ZERO,
                })
            }
        }
    }

    pub(crate) fn time(&self) -> R {
        match self {
            Stepper::Ias15(i) => i.time(),
            Stepper::Abm(a) => a.time(),
        }
    }

    pub(crate) fn positions(&self) -> &[R] {
        match self {
            Stepper::Ias15(i) => i.positions(),
            Stepper::Abm(a) => a.positions(),
        }
    }

    pub(crate) fn velocities(&self) -> &[R] {
        match self {
            Stepper::Ias15(i) => i.velocities(),
            Stepper::Abm(a) => a.velocities(),
        }
    }

    pub(crate) fn last_step_span(&self) -> Option<(R, R)> {
        match self {
            Stepper::Ias15(i) => i.last_step_span(),
            Stepper::Abm(a) => a.last_step_span(),
        }
    }

    pub(crate) fn dense_state(
        &self,
        t: R,
        x: &mut [R],
        v: &mut [R],
    ) -> Result<(), IntegratorError> {
        match self {
            Stepper::Ias15(i) => i.dense_state(t, x, v),
            Stepper::Abm(a) => a.dense_state(t, x, v),
        }
    }

    pub(crate) fn locate_event<G>(&self, g: G) -> Result<Option<R>, IntegratorError>
    where
        G: Fn(R, &[R], &[R]) -> R,
    {
        match self {
            Stepper::Ias15(i) => i.locate_event(g),
            Stepper::Abm(a) => a.locate_event(g),
        }
    }

    /// The counters, in the run-output record (ABM fills steps and force evaluations).
    pub(crate) fn stats(&self) -> RunStats {
        match self {
            Stepper::Ias15(i) => i.stats(),
            Stepper::Abm(a) => {
                let s = a.stats();
                RunStats {
                    steps: s.steps,
                    force_evaluations: s.force_evaluations,
                    ..RunStats::default()
                }
            }
        }
    }

    /// Steps taken so far.
    pub(crate) fn steps(&self) -> u64 {
        match self {
            Stepper::Ias15(i) => i.stats().steps,
            Stepper::Abm(a) => a.stats().steps,
        }
    }

    pub(crate) fn snapshot(&self) -> Result<IntegratorSnapshot, IntegratorError> {
        Ok(match self {
            Stepper::Ias15(i) => IntegratorSnapshot::Ias15(i.snapshot()?),
            Stepper::Abm(a) => IntegratorSnapshot::Abm(a.snapshot()?),
        })
    }
}
