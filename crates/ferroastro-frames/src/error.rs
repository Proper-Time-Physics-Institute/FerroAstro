// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Errors of the frame rotations (`FA-E09xx`, API-002).

use ferroastro_core::ErrorCode;

/// A frame-rotation error with a stable code.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum FrameError {
    /// `FA-E0900`: a frame definition is degenerate.
    #[error("FA-E0900: {what} (value {value}); valid: {valid}; fix: {fix}")]
    Definition {
        /// What is wrong.
        what: &'static str,
        /// The offending number (e.g. a norm or an orthogonality residual).
        value: f64,
        /// The valid range.
        valid: &'static str,
        /// Remediation hint.
        fix: &'static str,
    },
    /// `FA-E0901`: a rotated component is not finite.
    #[error("FA-E0901: rotated {quantity} is not finite ({source}); fix: check the input state")]
    Transform {
        /// Which quantity.
        quantity: &'static str,
        /// The underlying quantity error.
        source: ferroastro_core::Error,
    },
}

impl FrameError {
    /// The stable code.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            FrameError::Definition { .. } => ErrorCode::FrameDefinition,
            FrameError::Transform { .. } => ErrorCode::FrameTransform,
        }
    }
}
