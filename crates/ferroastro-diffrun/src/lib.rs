// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-diffrun
//!
//! The differential-run harness (CRATE-PLAN L9; NUM-013, PHY-012, CI-009): a scenario template,
//! a set of toggles (bodies or force switches) and signed combinations of the resulting rates.
//!
//! * [`Plan::bodies`] builds the PHY-012 plan: `base`, `base + X`, `base + X + Y`, `full` and
//!   `full − X` configurations, with the combinations P-add (primary), P-remove (informational)
//!   and cross terms ([`Protocol`]). [`Plan::switches`] builds NUM-013(a)'s force toggles.
//! * [`run_plan`] first proves that every leg shares its initial conditions
//!   ([`check_identical_ics`]: each body's record is bitwise identical wherever it appears, and
//!   the IC hash over the base bodies is one value; `FA-E1401` otherwise), then runs the legs
//!   in checkpointed segments ([`run_leg`], CI-010; optionally through an on-disk store so an
//!   interrupted run resumes), applies **one** [`Estimator`] to every configuration (CI-009) and
//!   evaluates the combinations into a table ([`PlanResult`]).
//!
//! The harness takes no position on which protocol is right: Park et al. 2017's
//! switch-on/switch-off protocol is UNVERIFIED (OI-38), so a plan reports every protocol it was
//! given, each row labelled.

mod error;
mod estimator;
mod plan;
mod run;
mod track;

pub use error::DiffrunError;
pub use estimator::{Estimate, Estimator, PassageSource};
pub use plan::{BodyToggle, Combination, Config, LegYears, Plan, Protocol, SwitchToggle};
pub use run::{
    ConfigResult, LegResult, PlanResult, Row, RunOptions, STORE_FORMAT, check_identical_ics,
    evaluate, run_leg, run_plan,
};
pub use track::{Track, TrackPassage};
