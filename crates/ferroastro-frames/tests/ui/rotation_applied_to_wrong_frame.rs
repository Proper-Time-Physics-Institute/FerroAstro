// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::frames::{EclipticJ2000, Heliocentric, State};
use ferroastro_frames::{Pole, icrf_to_mean_orbit};

fn misuse(ecl: &State<EclipticJ2000, Heliocentric>) {
    let to_mo = icrf_to_mean_orbit::<f64>(&Pole::park2017_mercury_mean_orbit().unwrap()).unwrap();
    let _ = to_mo.state(ecl);
}

fn main() {}
