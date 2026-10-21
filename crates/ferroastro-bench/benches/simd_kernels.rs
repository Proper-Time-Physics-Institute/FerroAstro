// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Criterion benchmark of WP26 check P8 (NUM-021, OI-45): the N = 16 system of header.440 at
//! JDEPOC (the 11 DE440 Major bodies and the EPM2021 five asteroids from REF-003), one
//! acceleration evaluation of
//!
//! * the Newtonian mutual term: production pairwise kernel, SoA kernel with scalar lanes, SoA
//!   kernel with SIMD lanes (feature `simd`), as model evaluations (gather included);
//! * the bare SoA Newtonian kernel on pre-gathered arrays, scalar and SIMD;
//! * the EIH c⁻² term: production `Eih`, `EihSoa` scalar and SIMD.
//!
//! The SIMD/scalar ratio is reported against the 2.5× NUM-021 target (not a gate). Run on the
//! fixed machine, baseline and native:
//!
//! ```text
//! cargo bench -p ferroastro-bench --features simd --bench simd_kernels
//! RUSTFLAGS="-C target-cpu=native" CARGO_TARGET_DIR=target/wp26-native \
//!   cargo bench -p ferroastro-bench --features simd --bench simd_kernels
//! ```
//!
//! Needs the cached header.440 (CI-005) and the committed REF-003 states.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "benchmark set-up: a failure aborts the run; indices are in range by construction"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, criterion_group, criterion_main};
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::{Length, Velocity};
use ferroastro_eih::{Eih, EihSoa, RhsAcceleration};
use ferroastro_ephem::NaifId;
use ferroastro_forces::soa::{SoaBackend, SoaBodies, newtonian_soa};
use ferroastro_forces::{
    ForceTerm, MoonVariables, NBody, NBodyModel, NewtonianMutual, NewtonianMutualSoa,
    SecondOrderSystem,
};
use ferroastro_system::{
    ASTEROID_NAIF_BASE, Catalogue, DE440_MAJOR, InitialConditions, SmallBodyStates, Tier,
    place_small_bodies,
};
use std::hint::black_box;
use std::path::PathBuf;

/// The EPM2021 Major asteroids (Subbotin et al. 2023 §2), asteroid numbers.
const EPM2021: [i32; 5] = [1, 2, 4, 7, 324];

/// (ids, GMs, flat x, flat v): the N = 16 system, all Major, component-major.
fn system() -> (Vec<NaifId>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let cat = Catalogue::from_header(&h, &prov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file = SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let ast: Vec<NaifId> = EPM2021
        .iter()
        .map(|&k| NaifId(ASTEROID_NAIF_BASE + k))
        .collect();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let placed = place_small_bodies(&file, &ast, &sun).unwrap();
    let mut ids: Vec<NaifId> = DE440_MAJOR.iter().map(|&(id, _)| id).collect();
    ids.extend(&ast);
    let n = ids.len();
    let (mut x, mut v) = (vec![0.0; 3 * n], vec![0.0; 3 * n]);
    for (i, &id) in ids.iter().enumerate() {
        let s = ic.states().state(id).or_else(|_| placed.state(id)).unwrap();
        let p = s.position().components().map(Length::km);
        let w = s.velocity().map(Velocity::km_per_s);
        for c in 0..3 {
            x[c * n + i] = p[c];
            v[c * n + i] = w[c];
        }
    }
    let gm = ids
        .iter()
        .map(|&id| cat.gm(id).unwrap().km3_per_s2())
        .collect();
    (ids, gm, x, v)
}

fn model(ids: &[NaifId], gm: &[f64], term: Box<dyn ForceTerm<Icrf>>) -> NBodyModel<Icrf> {
    let bodies: Vec<NBody> = ids
        .iter()
        .zip(gm)
        .map(|(&id, &gm)| NBody {
            id,
            gm,
            tier: Tier::Major,
        })
        .collect();
    NBodyModel::new(&bodies, MoonVariables::Barycentric, vec![term]).unwrap()
}

/// One model acceleration evaluation per iteration.
fn bench_model(
    g: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    name: &str,
    m: &NBodyModel<Icrf>,
    x: &[f64],
    v: &[f64],
) {
    let mut a = vec![0.0; x.len()];
    g.bench_function(name, |b| {
        b.iter(|| {
            m.acceleration(0.0, black_box(x), black_box(v), &mut a)
                .unwrap();
            black_box(a[0])
        });
    });
}

fn backends() -> Vec<SoaBackend> {
    let mut b = vec![SoaBackend::Scalar];
    if SoaBackend::preferred() != SoaBackend::Scalar {
        b.push(SoaBackend::preferred());
    }
    b
}

fn kernels(c: &mut Criterion) {
    let (ids, gm, x, v) = system();
    let n = ids.len();
    let de = De440::load().unwrap();
    let mut g = c.benchmark_group(format!("wp26-newtonian-n{n}"));
    let pw = model(&ids, &gm, Box::new(NewtonianMutual));
    bench_model(&mut g, "production-pairwise", &pw, &x, &v);
    for be in backends() {
        let m = model(&ids, &gm, Box::new(NewtonianMutualSoa::new(be)));
        bench_model(&mut g, &format!("soa-model-{}", be.label()), &m, &x, &v);
    }
    let pos: Vec<[f64; 3]> = (0..n).map(|i| [x[i], x[n + i], x[2 * n + i]]).collect();
    let vel: Vec<[f64; 3]> = (0..n).map(|i| [v[i], v[n + i], v[2 * n + i]]).collect();
    let groups: Vec<usize> = (0..n).collect();
    let bodies = SoaBodies::from_bodies(&pos, &vel, &gm, &groups);
    for be in backends() {
        let mut out: [Vec<f64>; 3] = Default::default();
        g.bench_function(format!("soa-kernel-{}", be.label()), |b| {
            b.iter(|| {
                newtonian_soa(black_box(&bodies), be, &mut out).unwrap();
                black_box(out[0][0])
            });
        });
    }
    g.finish();
    let light = Velocity::from_km_per_s(de.speed_of_light().km_per_s()).unwrap();
    let e = Eih::new(
        light,
        de.ppn_beta(),
        de.ppn_gamma(),
        RhsAcceleration::Newtonian,
    )
    .unwrap();
    let mut g = c.benchmark_group(format!("wp26-eih-n{n}"));
    let pw = model(&ids, &gm, Box::new(e));
    bench_model(&mut g, "production-pairwise", &pw, &x, &v);
    for be in backends() {
        let m = model(&ids, &gm, Box::new(EihSoa::new(e, be)));
        bench_model(&mut g, &format!("soa-model-{}", be.label()), &m, &x, &v);
    }
    g.finish();
}

criterion_group!(benches, kernels);
criterion_main!(benches);
