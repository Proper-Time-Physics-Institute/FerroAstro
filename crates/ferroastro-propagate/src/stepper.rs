// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The integrator of an N-body run: IAS15 (default, ADR-0008), fixed-step ABM (WP24), or the
//! WP29 integrators `WHFast` (Wisdom–Holman, `ferroastro-symplectic`) and IRK16 (Gauss–Legendre),
//! behind one interface for the driver (steps, dense output, events, counters, checkpoints).
//!
//! IAS15 shortens its last step to land on the end of the span. The fixed-step integrators never
//! shorten a step: their last step may end after the end of the span, and the driver then takes
//! the final state, samples and events at or before the end from dense output. `WHFast`'s dense
//! output and events need the force model (a Wisdom–Holman substep), so those calls take it.

use ferroastro_abm::{Abm, AbmSnapshot};
use ferroastro_integrators::Ias15Stats as RunStats;
use ferroastro_integrators::{
    Ias15, Ias15Snapshot, IntegratorError, Irk16, Irk16Snapshot, SecondOrderSystem,
};
use ferroastro_real::Real;
use ferroastro_symplectic::{WhSnapshot, Whfast};
use serde::{Deserialize, Serialize};

/// The integrator snapshot of a checkpoint (format v3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntegratorSnapshot {
    /// IAS15.
    Ias15(Ias15Snapshot),
    /// Adams–Bashforth–Moulton.
    Abm(AbmSnapshot),
    /// Wisdom–Holman (WP29).
    Whfast(WhSnapshot),
    /// Gauss–Legendre IRK16 (WP29).
    Irk16(Irk16Snapshot),
}

impl IntegratorSnapshot {
    /// Accepted steps at the snapshot.
    #[must_use]
    pub const fn steps(&self) -> u64 {
        match self {
            IntegratorSnapshot::Ias15(s) => s.stats.steps,
            IntegratorSnapshot::Abm(s) => s.stats.steps,
            IntegratorSnapshot::Whfast(s) => s.stats.steps,
            IntegratorSnapshot::Irk16(s) => s.stats.steps,
        }
    }

    /// `"ias15"`, `"abm"`, `"whfast"` or `"irk16"`.
    #[must_use]
    pub const fn integrator(&self) -> &'static str {
        match self {
            IntegratorSnapshot::Ias15(_) => "ias15",
            IntegratorSnapshot::Abm(_) => "abm",
            IntegratorSnapshot::Whfast(_) => "whfast",
            IntegratorSnapshot::Irk16(_) => "irk16",
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

impl<R: Real> Stepped<R> {
    /// No step was taken (the run had already reached `t`).
    const fn stay(t: R) -> Self {
        Self {
            t,
            moved: false,
            reached_end: true,
        }
    }

    /// A fixed step of `h` ended at `t`.
    fn fixed(t: R, h: R, t_end: R) -> Self {
        Self {
            t,
            moved: true,
            reached_end: (t_end - t) * h <= R::ZERO,
        }
    }
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
    Whfast(Whfast<R>),
    Irk16(Irk16<R>),
}

impl<R: Real> Stepper<R> {
    pub(crate) fn restore(snap: &IntegratorSnapshot) -> Result<Self, IntegratorError> {
        Ok(match snap {
            IntegratorSnapshot::Ias15(s) => Stepper::Ias15(Ias15::restore(s)?),
            IntegratorSnapshot::Abm(s) => Stepper::Abm(Abm::restore(s)?),
            IntegratorSnapshot::Whfast(s) => Stepper::Whfast(Whfast::restore(s)?),
            IntegratorSnapshot::Irk16(s) => Stepper::Irk16(Irk16::restore(s)?),
        })
    }

    /// Take one step towards `t_end` (IAS15 lands on it; the fixed-step integrators take a full
    /// step).
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
                let (t, h) = (a.time(), a.step_size());
                if (t_end - t) * h <= R::ZERO {
                    return Ok(Stepped::stay(t));
                }
                Ok(Stepped::fixed(a.step(sys)?.t, h, t_end))
            }
            Stepper::Whfast(w) => {
                let (t, h) = (w.time(), w.step_size());
                if (t_end - t) * h <= R::ZERO {
                    return Ok(Stepped::stay(t));
                }
                Ok(Stepped::fixed(w.step(sys)?.t, h, t_end))
            }
            Stepper::Irk16(k) => {
                let (t, h) = (k.time(), k.step_size());
                if (t_end - t) * h <= R::ZERO {
                    return Ok(Stepped::stay(t));
                }
                Ok(Stepped::fixed(k.step(sys)?.t, h, t_end))
            }
        }
    }

    pub(crate) fn time(&self) -> R {
        match self {
            Stepper::Ias15(i) => i.time(),
            Stepper::Abm(a) => a.time(),
            Stepper::Whfast(w) => w.time(),
            Stepper::Irk16(k) => k.time(),
        }
    }

    pub(crate) fn positions(&self) -> &[R] {
        match self {
            Stepper::Ias15(i) => i.positions(),
            Stepper::Abm(a) => a.positions(),
            Stepper::Whfast(w) => w.positions(),
            Stepper::Irk16(k) => k.positions(),
        }
    }

    pub(crate) fn velocities(&self) -> &[R] {
        match self {
            Stepper::Ias15(i) => i.velocities(),
            Stepper::Abm(a) => a.velocities(),
            Stepper::Whfast(w) => w.velocities(),
            Stepper::Irk16(k) => k.velocities(),
        }
    }

    pub(crate) fn last_step_span(&self) -> Option<(R, R)> {
        match self {
            Stepper::Ias15(i) => i.last_step_span(),
            Stepper::Abm(a) => a.last_step_span(),
            Stepper::Whfast(w) => w.last_step_span(),
            Stepper::Irk16(k) => k.last_step_span(),
        }
    }

    /// Dense output; `sys` is used by `WHFast` only (its interpolant is a Wisdom–Holman substep).
    pub(crate) fn dense_state<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t: R,
        x: &mut [R],
        v: &mut [R],
    ) -> Result<(), IntegratorError> {
        match self {
            Stepper::Ias15(i) => i.dense_state(t, x, v),
            Stepper::Abm(a) => a.dense_state(t, x, v),
            Stepper::Whfast(w) => w.dense_state(sys, t, x, v),
            Stepper::Irk16(k) => k.dense_state(t, x, v),
        }
    }

    /// Event location; `sys` is used by `WHFast` only.
    pub(crate) fn locate_event<S, G>(&mut self, sys: &S, g: G) -> Result<Option<R>, IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        G: Fn(R, &[R], &[R]) -> R,
    {
        match self {
            Stepper::Ias15(i) => i.locate_event(g),
            Stepper::Abm(a) => a.locate_event(g),
            Stepper::Whfast(w) => w.locate_event(sys, g),
            Stepper::Irk16(k) => k.locate_event(g),
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
            Stepper::Whfast(w) => {
                let s = w.stats();
                RunStats {
                    steps: s.steps,
                    force_evaluations: s.force_evaluations,
                    ..RunStats::default()
                }
            }
            Stepper::Irk16(k) => {
                let s = k.stats();
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
            Stepper::Whfast(w) => w.stats().steps,
            Stepper::Irk16(k) => k.stats().steps,
        }
    }

    pub(crate) fn snapshot(&self) -> Result<IntegratorSnapshot, IntegratorError> {
        Ok(match self {
            Stepper::Ias15(i) => IntegratorSnapshot::Ias15(i.snapshot()?),
            Stepper::Abm(a) => IntegratorSnapshot::Abm(a.snapshot()?),
            Stepper::Whfast(w) => IntegratorSnapshot::Whfast(w.snapshot()?),
            Stepper::Irk16(k) => IntegratorSnapshot::Irk16(k.snapshot()?),
        })
    }
}
