// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Lint-config canary (tools/lint_canary.py, TST-001): must FAIL under `clippy::float_cmp = "deny"`.

/// Compares two `f64` values with `==`, which the workspace lints deny.
#[must_use]
pub fn canary(a: f64, b: f64) -> bool {
    a == b
}
