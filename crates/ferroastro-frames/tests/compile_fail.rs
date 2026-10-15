// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Compile-fail proof that a state rotated into Mercury's mean-orbit frame cannot be passed where
//! an ICRF state is required, and that a rotation cannot be applied to a state in the wrong frame
//! (NUM-014, API-001). The `.stderr` snapshots are tied to the pinned toolchain.

/// Verifies: NUM-014, API-001
#[test]
fn mean_orbit_state_is_not_an_icrf_state() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
