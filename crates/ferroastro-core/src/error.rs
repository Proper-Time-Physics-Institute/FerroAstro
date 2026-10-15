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
//! | `FA-E0400`–`FA-E0499` | numerical integrators and their round-off diagnostics (`ferroastro-integrators`) |
//! | `FA-E0500`–`FA-E0599` | force models (`ferroastro-forces`) |
//! | `FA-E0600`–`FA-E0699` | shared by two crates: `FA-E0600` orbital elements and analytic oracles (`ferroastro-analytic`); `FA-E0601`–`FA-E0603` precession estimators and rate reports (`ferroastro-precession`) |
//! | `FA-E0700`–`FA-E0799` | ladder rungs, scenarios and the CLI (`ferroastro-ladder`, `ferroastro-cli`) |
//! | `FA-E0800`–`FA-E0899` | calendar and time-scale conversions: `FA-E0800`–`FA-E0803` ERFA statuses (`ferroastro-erfa`, which cannot depend on this crate and mirrors the strings); `FA-E0804`–`FA-E0805` UTC and conversion results (`ferroastro-time`) |
//! | `FA-E0900`–`FA-E0999` | reference-frame rotations (`ferroastro-frames`) |
//! | `FA-E1000`–`FA-E1099` | DAF/SPK ephemeris files and `testpo` points (`ferroastro-ephem`) |

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
    /// `FA-E0106`: the committed vectors were required but `FA_VECTORS_DIR` redirects them.
    VectorOverrideRefused,
    /// `FA-E0200`: an external data product could not be downloaded.
    DataFetch,
    /// `FA-E0201`: an external data product does not match its pinned SHA-256.
    DataChecksum,
    /// `FA-E0202`: the data manifest is missing or malformed.
    DataManifest,
    /// `FA-E0203`: a data service (e.g. JPL Horizons) returned an error or an unparsable answer.
    DataService,
    /// `FA-E0204`: a pinned data product is not in the local cache (run `tools/fetch_jpl.py`).
    DataMissing,
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
    /// `FA-E0404`: an integrator round-off diagnostic fit (`integrators::diagnostics`) is
    /// ill-posed. Distinct from `FA-E0601`, the precession-estimator fit.
    DiagnosticFitDegenerate,
    /// `FA-E0500`: a force model was built with invalid parameters.
    ForceModelInvalid,
    /// `FA-E0501`: a force model met a singular configuration (e.g. two coincident bodies).
    ForceModelSingular,
    /// `FA-E0600`: orbital elements or a state are outside the domain of a conversion.
    OrbitDomain,
    /// `FA-E0601`: a precession or slope fit is ill-posed (too few or degenerate samples).
    FitDegenerate,
    /// `FA-E0502`: force-model terms with incompatible coordinate gauges were combined.
    ForceModelGaugeMismatch,
    /// `FA-E0602`: a reported precession rate lacks a mandatory metadata field (PHY-015).
    ReportMetadataMissing,
    /// `FA-E0603`: sampled input to an estimator is unusable (unsorted, too coarse, too short).
    EstimatorInput,
    /// `FA-E0700`: an unknown rung, rung set or command-line option was requested.
    LadderSelection,
    /// `FA-E0701`: a ladder scenario file is missing or malformed.
    LadderScenario,
    /// `FA-E0800`: a calendar year is before −4799 (ERFA `cal2jd` status −1).
    CalendarYear,
    /// `FA-E0801`: a calendar month is outside 1–12 (ERFA `cal2jd` status −2).
    CalendarMonth,
    /// `FA-E0802`: a calendar day is outside its month (ERFA `cal2jd` status −3).
    CalendarDay,
    /// `FA-E0803`: a Julian date is outside the calendar range (ERFA `jd2cal` status −1).
    JulianDateRange,
    /// `FA-E0804`: UTC or leap seconds are unavailable for this date or build (before 1972, or
    /// the `hifitime` feature is off).
    UtcUnavailable,
    /// `FA-E0805`: a time-scale conversion produced a non-finite or unrepresentable date.
    TimeConversion,
    /// `FA-E0900`: a frame definition is degenerate (e.g. a pole along the reference axis, or a
    /// matrix that is not a rotation).
    FrameDefinition,
    /// `FA-E0901`: a frame rotation produced a non-finite component.
    FrameTransform,
    /// `FA-E1000`: a file is not a DAF/SPK file, or its file record is invalid.
    DafFormat,
    /// `FA-E1001`: a DAF record, summary or array address lies outside the file (truncated or
    /// corrupt file).
    DafBounds,
    /// `FA-E1002`: an SPK segment is malformed or of an unsupported data type.
    SpkSegment,
    /// `FA-E1003`: an epoch lies outside the coverage of the ephemeris segments.
    EphemCoverage,
    /// `FA-E1004`: no chain of segments connects the target and the centre (or the chain mixes
    /// reference frames).
    EphemNoPath,
    /// `FA-E1005`: an ephemeris evaluation produced a non-finite component.
    EphemNonFinite,
    /// `FA-E1006`: a JPL `testpo` test-point line is malformed.
    TestpoParse,
}

impl ErrorCode {
    /// Every code, in numeric order (used to prove codes are unique).
    pub const ALL: [ErrorCode; 46] = [
        ErrorCode::NonFinite,
        ErrorCode::OutOfRange,
        ErrorCode::VectorIo,
        ErrorCode::VectorParse,
        ErrorCode::VectorSchema,
        ErrorCode::VectorUnknownId,
        ErrorCode::VectorDuplicateId,
        ErrorCode::ToleranceExceeded,
        ErrorCode::VectorOverrideRefused,
        ErrorCode::DataFetch,
        ErrorCode::DataChecksum,
        ErrorCode::DataManifest,
        ErrorCode::DataService,
        ErrorCode::DataMissing,
        ErrorCode::HeaderParse,
        ErrorCode::ConstantMissing,
        ErrorCode::ConstantsTable,
        ErrorCode::IntegratorNonFinite,
        ErrorCode::IntegratorStepUnderflow,
        ErrorCode::IntegratorConfig,
        ErrorCode::IntegratorDimension,
        ErrorCode::DiagnosticFitDegenerate,
        ErrorCode::ForceModelInvalid,
        ErrorCode::ForceModelSingular,
        ErrorCode::OrbitDomain,
        ErrorCode::FitDegenerate,
        ErrorCode::ForceModelGaugeMismatch,
        ErrorCode::ReportMetadataMissing,
        ErrorCode::EstimatorInput,
        ErrorCode::LadderSelection,
        ErrorCode::LadderScenario,
        ErrorCode::CalendarYear,
        ErrorCode::CalendarMonth,
        ErrorCode::CalendarDay,
        ErrorCode::JulianDateRange,
        ErrorCode::UtcUnavailable,
        ErrorCode::TimeConversion,
        ErrorCode::FrameDefinition,
        ErrorCode::FrameTransform,
        ErrorCode::DafFormat,
        ErrorCode::DafBounds,
        ErrorCode::SpkSegment,
        ErrorCode::EphemCoverage,
        ErrorCode::EphemNoPath,
        ErrorCode::EphemNonFinite,
        ErrorCode::TestpoParse,
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
            ErrorCode::VectorOverrideRefused => "FA-E0106",
            ErrorCode::DataFetch => "FA-E0200",
            ErrorCode::DataChecksum => "FA-E0201",
            ErrorCode::DataManifest => "FA-E0202",
            ErrorCode::DataService => "FA-E0203",
            ErrorCode::DataMissing => "FA-E0204",
            ErrorCode::HeaderParse => "FA-E0300",
            ErrorCode::ConstantMissing => "FA-E0301",
            ErrorCode::ConstantsTable => "FA-E0302",
            ErrorCode::IntegratorNonFinite => "FA-E0400",
            ErrorCode::IntegratorStepUnderflow => "FA-E0401",
            ErrorCode::IntegratorConfig => "FA-E0402",
            ErrorCode::IntegratorDimension => "FA-E0403",
            ErrorCode::DiagnosticFitDegenerate => "FA-E0404",
            ErrorCode::ForceModelInvalid => "FA-E0500",
            ErrorCode::ForceModelSingular => "FA-E0501",
            ErrorCode::OrbitDomain => "FA-E0600",
            ErrorCode::FitDegenerate => "FA-E0601",
            ErrorCode::ForceModelGaugeMismatch => "FA-E0502",
            ErrorCode::ReportMetadataMissing => "FA-E0602",
            ErrorCode::EstimatorInput => "FA-E0603",
            ErrorCode::LadderSelection => "FA-E0700",
            ErrorCode::LadderScenario => "FA-E0701",
            ErrorCode::CalendarYear => "FA-E0800",
            ErrorCode::CalendarMonth => "FA-E0801",
            ErrorCode::CalendarDay => "FA-E0802",
            ErrorCode::JulianDateRange => "FA-E0803",
            ErrorCode::UtcUnavailable => "FA-E0804",
            ErrorCode::TimeConversion => "FA-E0805",
            ErrorCode::FrameDefinition => "FA-E0900",
            ErrorCode::FrameTransform => "FA-E0901",
            ErrorCode::DafFormat => "FA-E1000",
            ErrorCode::DafBounds => "FA-E1001",
            ErrorCode::SpkSegment => "FA-E1002",
            ErrorCode::EphemCoverage => "FA-E1003",
            ErrorCode::EphemNoPath => "FA-E1004",
            ErrorCode::EphemNonFinite => "FA-E1005",
            ErrorCode::TestpoParse => "FA-E1006",
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

/// Reject NaN and infinities with [`Error::NonFinite`] (any backend; the error records the
/// nearest `f64`).
pub(crate) fn require_finite<R: ferroastro_real::Real>(
    quantity: &'static str,
    value: R,
    unit: &'static str,
) -> Result<R, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::NonFinite {
            quantity,
            value: value.to_f64(),
            unit,
            valid: "any finite value",
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
