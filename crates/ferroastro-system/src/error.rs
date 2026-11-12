// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of the system model (`FA-E11xx`, API-002). Every message names the code, the
//! offending quantity, the valid range or form, and a fix.

use ferroastro_constants::ConstantsError;
use ferroastro_core::ErrorCode;
use ferroastro_ephem::EphemError;

/// A system-model error with a stable code.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SystemError {
    /// `FA-E1100`: a body is unknown to, or duplicated in, a catalogue or system state.
    #[error("FA-E1100: NAIF body {id}: {problem}; valid: {valid}; fix: {fix}")]
    Body {
        /// NAIF id.
        id: i32,
        /// What is wrong.
        problem: &'static str,
        /// The valid set.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E1101`: initial conditions cannot be assembled.
    #[error(
        "FA-E1101: initial conditions: {quantity} = {value} is invalid; valid: {valid}; fix: {fix}"
    )]
    InitialConditions {
        /// The offending quantity (e.g. a header symbol).
        quantity: String,
        /// What was found.
        value: String,
        /// The valid range or form.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E1102`: barycentring is ill-posed.
    #[error("FA-E1102: barycentring: {problem}; fix: {fix}")]
    Barycentre {
        /// What is wrong.
        problem: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// A constants error (`FA-E03xx`) met while building a catalogue or initial conditions.
    #[error(transparent)]
    Constants(#[from] ConstantsError),
    /// An ephemeris error (`FA-E10xx`) met while sampling initial conditions.
    #[error(transparent)]
    Ephem(#[from] EphemError),
}

impl SystemError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            SystemError::Body { .. } => ErrorCode::SystemBody,
            SystemError::InitialConditions { .. } => ErrorCode::SystemInitialConditions,
            SystemError::Barycentre { .. } => ErrorCode::SystemBarycentre,
            SystemError::Constants(e) => e.code(),
            SystemError::Ephem(e) => e.code(),
        }
    }

    pub(crate) fn unknown(id: i32) -> Self {
        SystemError::Body {
            id,
            problem: "not present",
            valid: "a body listed in the catalogue or state",
            fix: "add the body first, or check the NAIF id",
        }
    }

    pub(crate) fn duplicate(id: i32) -> Self {
        SystemError::Body {
            id,
            problem: "already present",
            valid: "each NAIF id at most once",
            fix: "use `set` to replace a body's state",
        }
    }

    pub(crate) fn non_finite(quantity: impl Into<String>, value: f64) -> Self {
        SystemError::InitialConditions {
            quantity: quantity.into(),
            value: format!("{value}"),
            valid: "a finite number",
            fix: "check the input product (header.440, SPK or vectors) for NaN/inf",
        }
    }
}
