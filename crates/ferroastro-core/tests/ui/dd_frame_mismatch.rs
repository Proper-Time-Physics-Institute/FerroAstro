// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::frames::{Barycentric, EclipticJ2000, Icrf, Position};
use ferroastro_core::units::Length;
use ferroastro_real::DoubleDouble;

fn main() {
    let z = Length::<DoubleDouble>::zero();
    let a = Position::<Icrf, Barycentric, DoubleDouble>::new(z, z, z);
    let b = Position::<EclipticJ2000, Barycentric, DoubleDouble>::new(z, z, z);
    let _ = a - b;
}
