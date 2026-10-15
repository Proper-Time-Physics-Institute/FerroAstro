// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::units::Length;
use ferroastro_real::DoubleDouble;

fn main() {
    // Mixing precisions must be explicit (Length::cast), never implicit.
    let _ = Length::ZERO + Length::<DoubleDouble>::zero();
}
