// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Criterion benchmark: IAS15 propagation of the Newtonian Sun + Mercury two-body problem
//! (rung N-2B set-up: DE440 GMs, SW92 J2000 elements), and one force evaluation.
//!
//! Run: `cargo bench -p ferroastro-precession --bench two_body`.

#![expect(
    clippy::unwrap_used,
    reason = "benchmark set-up: a failure aborts the run"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, criterion_group, criterion_main};
use ferroastro_analytic::kepler::Elements;
use ferroastro_constants::{Body, De440};
use ferroastro_core::frames::{Barycentric, EclipticJ2000, Heliocentric, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, SECONDS_PER_DAY};
use ferroastro_core::units::{Angle, Length, Velocity};
use ferroastro_forces::NewtonianPointMasses;
use ferroastro_integrators::{Ias15, Ias15Config, SecondOrderSystem};
use ferroastro_vectors::VectorSet;
use std::hint::black_box;

type Model = NewtonianPointMasses<EclipticJ2000, Barycentric>;

fn setup() -> (Model, Vec<f64>, Vec<f64>) {
    let de = De440::load().unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let val = |id: &str| vs.get(id).unwrap().value().unwrap();
    let deg = |id: &str| Angle::from_radians(val(id).to_radians()).unwrap();
    let (gs, gm) = (de.gm_sun(), de.gm(Body::Mercury));
    let el = Elements::from_longitudes(
        Length::from_km(val("SW92-MERCURY-A") * de.au().km()).unwrap(),
        val("SW92-MERCURY-E"),
        deg("SW92-MERCURY-I"),
        deg("SW92-MERCURY-NODE"),
        deg("SW92-MERCURY-VARPI"),
        deg("SW92-MERCURY-L"),
    );
    let rel: State<EclipticJ2000, Heliocentric> = el.to_state(gs + gm).unwrap();
    let total = gs.km3_per_s2() + gm.km3_per_s2();
    let (fs, fm) = (gm.km3_per_s2() / total, gs.km3_per_s2() / total);
    let r = rel.position().components().map(Length::km);
    let v = rel.velocity().map(Velocity::km_per_s);
    let x: Vec<f64> = r
        .iter()
        .map(|z| -fs * z)
        .chain(r.iter().map(|z| fm * z))
        .collect();
    let w: Vec<f64> = v
        .iter()
        .map(|z| -fs * z)
        .chain(v.iter().map(|z| fm * z))
        .collect();
    (Model::new(&[gs, gm]).unwrap(), x, w)
}

fn propagate(model: &Model, x: &[f64], v: &[f64], years: f64) -> f64 {
    let mut ias = Ias15::new(6, 0.0, x, v, SECONDS_PER_DAY, Ias15Config::default()).unwrap();
    let t_end = years * DAYS_PER_JULIAN_YEAR * SECONDS_PER_DAY;
    ias.integrate_to(model, t_end, |_, _| Ok(())).unwrap();
    ias.positions().iter().sum()
}

fn bench(c: &mut Criterion) {
    let (model, x, v) = setup();
    let mut g = c.benchmark_group("ias15_sun_mercury");
    g.sample_size(20);
    g.bench_function("propagate_1_julian_year", |b| {
        b.iter(|| propagate(&model, black_box(&x), black_box(&v), 1.0));
    });
    g.bench_function("propagate_100_julian_years", |b| {
        b.iter(|| propagate(&model, black_box(&x), black_box(&v), 100.0)); // lint: allow-literal(benchmark span in years)
    });
    g.finish();
    let mut a = vec![0.0; 6];
    c.bench_function("newtonian_two_body_acceleration", |b| {
        b.iter(|| {
            model
                .acceleration(0.0, black_box(&x), black_box(&v), &mut a)
                .unwrap();
        });
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
