// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Stable error codes (`FA-Exxxx`) and the foundation error type (API-002).
//!
//! Codes are **never renumbered or reused**. Ranges:
//!
//! | Range | Area |
//! |---|---|
//! | `FA-E0001`–`FA-E0099` | quantities, frames, time scales (`ferroastro-core`) |
//! | `FA-E0100`–`FA-E0199` | test-vector loading and checking (`ferroastro-vectors`) |
//! | `FA-E0200`–`FA-E0299` | external data products (`ferroastro-testdata`) |
//! | `FA-E0300`–`FA-E0399` | ephemeris constants and `header.440` parsing (`ferroastro-constants`) |
//! | `FA-E0400`–`FA-E0499` | numerical integrators (`ferroastro-integrators`) |
//! | `FA-E0500`–`FA-E0599` | force models (`ferroastro-forces`) |
//! | `FA-E0600`–`FA-E0699` | orbital elements and precession estimators (`ferroastro-precession`) |

use std::fmt;

/// A stable FerroAstro error code, rendered as `FA-Exxxx`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// `FA-E0001`: a quantity was NaN or infinite.
    NonFinite,
    /// `FA-E0002`: a quantity lies outside its valid range.
    OutOfRange,
    /// `FA-E0100`: a test-vector file could not be read.
    VectorIo,
    /// `FA-E0101`: a test-vector file is not valid TOML.
    VectorParse,
    /// `FA-E0102`: a test vector violates the ADR-0004 schema.
    VectorSchema,
    /// `FA-E0103`: a requested test-vector id does not exist.
    VectorUnknownId,
    /// `FA-E0104`: two vector files define the same id.
    VectorDuplicateId,
    /// `FA-E0105`: a measured value lies outside the vector's tolerance.
    ToleranceExceeded,
    /// `FA-E0200`: an external data product could not be downloaded.
    DataFetch,
    /// `FA-E0201`: an external data product does not match its pinned SHA-256.
    DataChecksum,
    /// `FA-E0202`: the data manifest is missing or malformed.
    DataManifest,
    /// `FA-E0300`: a JPL `header.440` file is malformed.
    HeaderParse,
    /// `FA-E0301`: a requested ephemeris constant is not in the table or header.
    ConstantMissing,
    /// `FA-E0302`: the committed constants table is malformed or inconsistent.
    ConstantsTable,
    /// `FA-E0400`: an integrator met a NaN or infinite state, acceleration or step.
    IntegratorNonFinite,
    /// `FA-E0401`: the adaptive step size collapsed below the configured minimum.
    IntegratorStepUnderflow,
    /// `FA-E0402`: an integrator configuration value is invalid.
    IntegratorConfig,
    /// `FA-E0403`: a state or acceleration slice has the wrong length.
    IntegratorDimension,
    /// `FA-E0500`: a force model was built with invalid parameters.
    ForceModelInvalid,
    /// `FA-E0501`: a force model met a singular configuration (e.g. two coincident bodies).
    ForceModelSingular,
    /// `FA-E0600`: orbital elements or a state are outside the domain of a conversion.
    OrbitDomain,
    /// `FA-E0601`: a precession or slope fit is ill-posed (too few or degenerate samples).
    FitDegenerate,
}

impl ErrorCode {
    /// Every code, in numeric order (used to prove codes are unique).
    pub const ALL: [ErrorCode; 22] = [
        ErrorCode::NonFinite,
        ErrorCode::OutOfRange,
        ErrorCode::VectorIo,
        ErrorCode::VectorParse,
        ErrorCode::VectorSchema,
        ErrorCode::VectorUnknownId,
        ErrorCode::VectorDuplicateId,
        ErrorCode::ToleranceExceeded,
        ErrorCode::DataFetch,
        ErrorCode::DataChecksum,
        ErrorCode::DataManifest,
        ErrorCode::HeaderParse,
        ErrorCode::ConstantMissing,
        ErrorCode::ConstantsTable,
        ErrorCode::IntegratorNonFinite,
        ErrorCode::IntegratorStepUnderflow,
        ErrorCode::IntegratorConfig,
        ErrorCode::IntegratorDimension,
        ErrorCode::ForceModelInvalid,
        ErrorCode::ForceModelSingular,
        ErrorCode::OrbitDomain,
        ErrorCode::FitDegenerate,
    ];

    /// The stable textual code, e.g. `"FA-E0001"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ErrorCode::NonFinite => "FA-E0001",
            ErrorCode::OutOfRange => "FA-E0002",
            ErrorCode::VectorIo => "FA-E0100",
            ErrorCode::VectorParse => "FA-E0101",
            ErrorCode::VectorSchema => "FA-E0102",
            ErrorCode::VectorUnknownId => "FA-E0103",
            ErrorCode::VectorDuplicateId => "FA-E0104",
            ErrorCode::ToleranceExceeded => "FA-E0105",
            ErrorCode::DataFetch => "FA-E0200",
            ErrorCode::DataChecksum => "FA-E0201",
            ErrorCode::DataManifest => "FA-E0202",
            ErrorCode::HeaderParse => "FA-E0300",
            ErrorCode::ConstantMissing => "FA-E0301",
            ErrorCode::ConstantsTable => "FA-E0302",
            ErrorCode::IntegratorNonFinite => "FA-E0400",
            ErrorCode::IntegratorStepUnderflow => "FA-E0401",
            ErrorCode::IntegratorConfig => "FA-E0402",
            ErrorCode::IntegratorDimension => "FA-E0403",
            ErrorCode::ForceModelInvalid => "FA-E0500",
            ErrorCode::ForceModelSingular => "FA-E0501",
            ErrorCode::OrbitDomain => "FA-E0600",
            ErrorCode::FitDegenerate => "FA-E0601",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors raised by the foundation types. Every message names the code, the quantity, its unit,
/// the valid range and a remediation hint (API-002).
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A quantity was NaN or infinite.
    #[error(
        "FA-E0001: {quantity} = {value} {unit} is not finite; valid range: {valid}; fix: {fix}"
    )]
    NonFinite {
        /// Name of the offending quantity.
        quantity: &'static str,
        /// The rejected value.
        value: f64,
        /// Unit the value was given in.
        unit: &'static str,
        /// Human-readable valid range.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
    /// A quantity lies outside its valid range.
    #[error("FA-E0002: {quantity} = {value} {unit} is outside the valid range {valid}; fix: {fix}")]
    OutOfRange {
        /// Name of the offending quantity.
        quantity: &'static str,
        /// The rejected value.
        value: f64,
        /// Unit the value was given in.
        unit: &'static str,
        /// Human-readable valid range.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
}

impl Error {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Error::NonFinite { .. } => ErrorCode::NonFinite,
            Error::OutOfRange { .. } => ErrorCode::OutOfRange,
        }
    }
}

/// Reject NaN and infinities with [`Error::NonFinite`].
pub(crate) fn require_finite(
    quantity: &'static str,
    value: f64,
    unit: &'static str,
) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::NonFinite {
            quantity,
            value,
            unit,
            valid: "any finite f64",
            fix: "check the upstream computation or input file for NaN/inf",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Verifies: API-002
    #[test]
    fn codes_are_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for code in ErrorCode::ALL {
            let s = code.as_str();
            assert!(seen.insert(s), "duplicate code {s}");
            assert_eq!(s.len(), 8, "{s}");
            assert!(s.starts_with("FA-E"), "{s}");
            assert!(s[4..].chars().all(|c| c.is_ascii_digit()), "{s}");
        }
    }

    /// Verifies: API-002
    #[test]
    fn messages_name_code_quantity_unit_and_range() {
        let err = require_finite("semi-major axis", f64::NAN, "km").unwrap_err();
        let msg = err.to_string();
        assert_eq!(err.code(), ErrorCode::NonFinite);
        for needle in ["FA-E0001", "semi-major axis", "km", "valid range", "fix"] {
            assert!(msg.contains(needle), "{msg:?} lacks {needle:?}");
        }
    }
}
