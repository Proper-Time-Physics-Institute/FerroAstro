// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Lint-config canary control (tools/lint_canary.py): must PASS clippy under the workspace lints,
//! proving that a failing canary fails for its own lint and not for a harness problem.

/// Adds two counters.
#[must_use]
pub fn add(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}
