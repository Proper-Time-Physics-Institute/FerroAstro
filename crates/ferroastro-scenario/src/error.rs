// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of this crate (`FA-E12xx`, API-002).

use ferroastro_core::ErrorCode;

/// Errors from reading or checking a scenario.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ScenarioError {
    /// Not valid TOML, or the scenario schema is violated (unknown key, wrong type).
    #[error(
        "FA-E1200: scenario {name}: {problem}; fix: compare the file with \
         ci/scenarios/scenario.schema.json"
    )]
    Malformed {
        /// Scenario name (or `?` before it is known).
        name: String,
        /// What is wrong.
        problem: String,
    },
    /// A switch, mode or option that CI-002 defines but this runner does not implement.
    #[error(
        "FA-E1201: scenario {name} requests {switch}, which {runner} does not implement \
         ({reason}); refused rather than silently omitted (REF-011)"
    )]
    Unsupported {
        /// Scenario name.
        name: String,
        /// The switch (or option) as written in the scenario, e.g. `forces.gr_eih`.
        switch: String,
        /// The refusing runner.
        runner: String,
        /// Why, or where it will be implemented.
        reason: String,
    },
    /// A force switch that CI-002 does not define.
    #[error(
        "FA-E1202: scenario {name} names the unknown force switch {switch:?}; valid switches: \
         {valid}"
    )]
    UnknownSwitch {
        /// Scenario name.
        name: String,
        /// The unknown key under `[forces]`.
        switch: String,
        /// The CI-002 switches.
        valid: String,
    },
    /// A value is out of range or inconsistent with another one.
    #[error("FA-E1203: scenario {name}: {field} = {value}: {problem}")]
    Value {
        /// Scenario name.
        name: String,
        /// The field.
        field: &'static str,
        /// The rejected value.
        value: String,
        /// What is wrong and what is valid.
        problem: String,
    },
}

impl ScenarioError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            ScenarioError::Malformed { .. } => ErrorCode::ScenarioMalformed,
            ScenarioError::Unsupported { .. } => ErrorCode::ScenarioUnsupported,
            ScenarioError::UnknownSwitch { .. } => ErrorCode::ScenarioUnknownSwitch,
            ScenarioError::Value { .. } => ErrorCode::ScenarioValue,
        }
    }
}
