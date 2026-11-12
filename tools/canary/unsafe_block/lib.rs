// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Lint-config canary (tools/lint_canary.py, STD-003): must FAIL under `unsafe_code = "forbid"`.

/// Opens an empty unsafe block, which the workspace lints forbid.
pub fn canary() {
    unsafe {}
}
