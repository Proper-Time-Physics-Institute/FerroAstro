// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of the DAF/SPK reader and the testpo parser (`FA-E10xx`, API-002).
//!
//! Every message names the code, the offending quantity, its unit, the valid range and a fix.

use ferroastro_core::ErrorCode;

/// An ephemeris-file error with a stable code.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum EphemError {
    /// `FA-E1000`: not a DAF/SPK file, or an invalid file record.
    #[error(
        "FA-E1000: DAF file record: {quantity} = {value} is invalid; valid: {valid}; fix: {fix}"
    )]
    Format {
        /// The offending file-record field.
        quantity: &'static str,
        /// What was found (rendered).
        value: String,
        /// The valid range or form.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E1001`: an address or record lies outside the file.
    #[error(
        "FA-E1001: {quantity} = {value} (unit: {unit}) lies outside the file, valid range \
         {min}..={max}; fix: the file is truncated or corrupt, fetch it again and verify its SHA-256"
    )]
    Bounds {
        /// The offending quantity (e.g. "summary record number").
        quantity: &'static str,
        /// Its value.
        value: u64,
        /// Its unit ("record", "word", "byte").
        unit: &'static str,
        /// Smallest valid value.
        min: u64,
        /// Largest valid value.
        max: u64,
    },
    /// `FA-E1002`: a malformed or unsupported SPK segment.
    #[error(
        "FA-E1002: SPK segment {segment} (target {target}, centre {center}): {quantity} = {value} \
         {unit} is invalid; valid: {valid}; fix: {fix}"
    )]
    Segment {
        /// 0-based segment index in file order.
        segment: usize,
        /// NAIF target id.
        target: i32,
        /// NAIF centre id.
        center: i32,
        /// The offending quantity.
        quantity: &'static str,
        /// Its value (rendered).
        value: String,
        /// Its unit.
        unit: &'static str,
        /// The valid range or set.
        valid: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E1003`: the epoch is outside the coverage of every segment for the body.
    #[error(
        "FA-E1003: epoch {et_s} s TDB past J2000 (JD {jd_tdb} TDB) is outside the coverage of \
         NAIF body {body} relative to {center}; valid range: {valid}; fix: use an epoch inside the \
         file's coverage or load a longer ephemeris"
    )]
    Coverage {
        /// NAIF id of the body whose segment was needed.
        body: i32,
        /// NAIF id of that segment's centre (or of the requested centre if none was found).
        center: i32,
        /// Requested epoch, seconds TDB past J2000 (nearest f64).
        et_s: f64,
        /// Requested epoch, Julian date TDB (nearest f64).
        jd_tdb: f64,
        /// The covered intervals, rendered.
        valid: String,
    },
    /// `FA-E1004`: no chain of segments connects target and centre, or the chain mixes frames.
    #[error(
        "FA-E1004: no ephemeris path from NAIF {target} to NAIF {center}: {reason}; fix: {fix}"
    )]
    NoPath {
        /// Requested target.
        target: i32,
        /// Requested centre.
        center: i32,
        /// What is missing.
        reason: String,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E1005`: the evaluated state is not finite.
    #[error(
        "FA-E1005: SPK evaluation of NAIF {target} relative to {center} gave a non-finite \
         {quantity} (unit: {unit}); valid range: any finite value; fix: the segment coefficients \
         are corrupt, verify the file's SHA-256"
    )]
    NonFinite {
        /// NAIF target id of the segment.
        target: i32,
        /// NAIF centre id of the segment.
        center: i32,
        /// "position" or "velocity".
        quantity: &'static str,
        /// "km" or "km/s".
        unit: &'static str,
    },
    /// `FA-E1006`: a malformed `testpo` line.
    #[error(
        "FA-E1006: testpo line {line}: {quantity} {value:?} is invalid; valid: {valid}; fix: {fix}"
    )]
    Testpo {
        /// 1-based line number.
        line: usize,
        /// The offending field.
        quantity: &'static str,
        /// The text found.
        value: String,
        /// The valid form.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
}

impl EphemError {
    /// The stable code of this error.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            EphemError::Format { .. } => ErrorCode::DafFormat,
            EphemError::Bounds { .. } => ErrorCode::DafBounds,
            EphemError::Segment { .. } => ErrorCode::SpkSegment,
            EphemError::Coverage { .. } => ErrorCode::EphemCoverage,
            EphemError::NoPath { .. } => ErrorCode::EphemNoPath,
            EphemError::NonFinite { .. } => ErrorCode::EphemNonFinite,
            EphemError::Testpo { .. } => ErrorCode::TestpoParse,
        }
    }
}
