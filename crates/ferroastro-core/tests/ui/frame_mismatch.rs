// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::frames::{Barycentric, EclipticJ2000, Icrf, Position};
use ferroastro_core::units::Length;

fn main() {
    let z = Length::ZERO;
    let a = Position::<Icrf, Barycentric>::new(z, z, z);
    let b = Position::<EclipticJ2000, Barycentric>::new(z, z, z);
    let _ = a - b;
}
