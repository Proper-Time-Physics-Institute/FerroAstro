// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Componentwise helpers for 3-arrays (no indexing).

/// `[f(a0, b0), f(a1, b1), f(a2, b2)]`.
pub(crate) fn zip3<A: Copy, B: Copy, T>(a: [A; 3], b: [B; 3], f: impl Fn(A, B) -> T) -> [T; 3] {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [f(a0, b0), f(a1, b1), f(a2, b2)]
}
