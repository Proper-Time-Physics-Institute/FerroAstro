// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
use ferroastro_core::frames::{Heliocentric, Icrf, MercuryMeanOrbit, State};
use ferroastro_frames::{Pole, icrf_to_mean_orbit};

fn needs_icrf(_s: &State<Icrf, Heliocentric>) {}

fn rotate_and_misuse(s: &State<Icrf, Heliocentric>) {
    let to_mo = icrf_to_mean_orbit::<f64>(&Pole::park2017_mercury_mean_orbit().unwrap()).unwrap();
    let mo: State<MercuryMeanOrbit, Heliocentric> = to_mo.state(s).unwrap();
    needs_icrf(&mo);
}

fn main() {}
