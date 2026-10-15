// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::time::{Epoch, Tdb, Tt};
use ferroastro_real::{DoubleDouble, Real};

fn main() {
    let a = Epoch::<Tdb, DoubleDouble>::from_jd_parts(DoubleDouble::ZERO, DoubleDouble::ZERO).unwrap();
    let b = Epoch::<Tt, DoubleDouble>::from_jd_parts(DoubleDouble::ZERO, DoubleDouble::ZERO).unwrap();
    let _ = a - b;
}
