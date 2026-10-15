// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compile-fail proofs that unit, frame, centre and time-scale mismatches are rejected (API-001).
//! The `.stderr` snapshots are tied to the pinned toolchain (rust-toolchain.toml).

/// Verifies: API-001
#[test]
fn compile_fail_type_mismatches() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
