// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Cost-only timing of the WP12 Brouwer-ensemble model (Sun + Mercury..Neptune with the EMB as one
//! body, Newtonian + solar J2 with reaction, IAS15 default). Prints steps and wall time only; it
//! reads no energy and makes no physics claim. `cargo run --release -p ferroastro-forces --example
//! nbody_cost -- <years>`.
#![allow(
    clippy::unwrap_used,
    clippy::print_stdout,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    reason = "cost-only example; indices are in range by construction"
)]

use ferroastro_core::frames::Icrf;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    MoonVariables, NBody, NBodyModel, NewtonianMutual, Pole, SolarJ2WithReaction,
};
use ferroastro_integrators::{Ias15, Ias15Config};
use ferroastro_system::Tier;

fn main() {
    let years: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100.0);
    // Structural orbits (circular, coplanar-ish): only the step count matters here.
    let gm = [
        1.327e11, 2.2e4, 3.25e5, 4.03e5, 4.28e4, 1.267e8, 3.79e7, 5.79e6, 6.84e6,
    ];
    let a_km = [
        0.0, 5.79e7, 1.082e8, 1.496e8, 2.279e8, 7.785e8, 1.434e9, 2.871e9, 4.495e9,
    ];
    let ids = [10, 1, 2, 3, 4, 5, 6, 7, 8];
    let bodies: Vec<NBody> = ids
        .iter()
        .zip(gm)
        .map(|(&id, g)| NBody {
            id: NaifId(id),
            gm: g,
            tier: Tier::Major,
        })
        .collect();
    let j2 = SolarJ2WithReaction::new(
        2.2e-7,
        Length::from_km(6.96e5).unwrap(),
        Pole::<Icrf>::from_components([0.12, -0.42, 0.9]).unwrap(),
    )
    .unwrap();
    let m = NBodyModel::<Icrf>::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual), Box::new(j2)],
    )
    .unwrap();
    let n = bodies.len();
    let mut x = vec![0.0; 3 * n];
    let mut v = vec![0.0; 3 * n];
    for i in 1..n {
        let e = if i == 1 { 0.2056 } else { 0.0 };
        let r = a_km[i] * (1.0 - e);
        x[i] = r;
        v[n + i] = (gm[0] / a_km[i] * (1.0 + e) / (1.0 - e)).sqrt();
        let _ = Velocity::from_km_per_s(v[n + i]);
    }
    let mut ias = Ias15::new(3 * n, 0.0, &x, &v, SECONDS_PER_DAY, Ias15Config::default()).unwrap();
    let t = std::time::Instant::now();
    ias.integrate_to(
        &m,
        years * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY,
        |_, _| Ok(()),
    )
    .unwrap();
    let w = t.elapsed().as_secs_f64();
    let s = ias.stats().steps;
    println!(
        "{years} yr: {s} steps in {w:.2} s = {:.0} steps/s",
        s as f64 / w
    );
}
