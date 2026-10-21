// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::time::{Epoch, Tdb, Tt};

fn main() {
    let a = Epoch::<Tdb>::from_jd_parts(0.0, 0.0).unwrap();
    let b = Epoch::<Tt>::from_jd_parts(0.0, 0.0).unwrap();
    let _ = a - b;
}
