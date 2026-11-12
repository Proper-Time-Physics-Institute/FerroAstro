// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::units::{Length, Velocity};
use ferroastro_real::DoubleDouble;

fn main() {
    let _ = Length::<DoubleDouble>::zero() + Velocity::<DoubleDouble>::zero();
}
