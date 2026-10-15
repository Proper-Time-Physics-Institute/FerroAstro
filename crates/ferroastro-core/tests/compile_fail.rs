// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compile-fail proofs that unit, frame, centre and time-scale mismatches are rejected, with the
//! default `f64` backend and with the double-double backend, and that mixing backends is rejected
//! too (API-001). A compile-pass case shows the newtypes work with double-double. The `.stderr`
//! snapshots are tied to the pinned toolchain (rust-toolchain.toml).

/// Verifies: API-001, DEP-007, NUM-006
#[test]
fn compile_fail_type_mismatches() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui-pass/*.rs");
}
