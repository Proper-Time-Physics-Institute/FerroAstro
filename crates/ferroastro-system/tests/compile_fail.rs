// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compile-fail proofs (API-001, WP11 check 6): a heliocentric state cannot be stored in, or a
//! heliocentric system passed where, a barycentric one is required. The `.stderr` snapshots are
//! tied to the pinned toolchain (rust-toolchain.toml); the MSRV job skips this binary.

/// Verifies: API-001, NUM-009
#[test]
fn compile_fail_heliocentric_where_barycentric_is_required() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
