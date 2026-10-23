// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Criterion benchmark (NUM-009, WP15 check 2c): one acceleration evaluation of the header.440
//! system with 16 Majors (the 11 DE440 majors + the 5 largest-GM asteroids), the same with the
//! other 338 numbered asteroids as Minor bodies, and all 354 bodies as Majors. Needs the cached
//! header.440 (CI-005) and the committed REF-003 states.
//!
//! Run: `cargo bench -p ferroastro-forces --bench num009_tiers`.

#![expect(
    clippy::unwrap_used,
    reason = "benchmark set-up: a failure aborts the run"
)]
#![expect(
    missing_docs,
    reason = "criterion_group! generates an undocumented pub fn"
)]

use criterion::{Criterion, criterion_group, criterion_main};
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MinorFromMajors, MinorReaction,
    MoonVariables, NBody, NBodyModel, NewtonianMutual,
};
use ferroastro_integrators::SecondOrderSystem;
use ferroastro_system::{Catalogue, InitialConditions, SmallBodyStates, Tier, place_small_bodies};
use std::hint::black_box;
use std::path::PathBuf;

const PROMOTED: usize = 5;

/// Label, model, positions, velocities.
type Case = (&'static str, NBodyModel<Icrf>, Vec<f64>, Vec<f64>);

fn models() -> Vec<Case> {
    let f = ferroastro_testdata::locate_product("de440-header").unwrap();
    let text = std::fs::read_to_string(&f.path).unwrap();
    let h = Header::parse(&text).unwrap();
    let prov = f.provenance();
    let de = De440::load().unwrap();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file = SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cat = Catalogue::from_header(&h, &prov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let mut ast: Vec<(NaifId, f64)> = cat
        .bodies()
        .iter()
        .filter(|b| b.tier() == Tier::Minor)
        .map(|b| (b.id(), b.gm().km3_per_s2()))
        .collect();
    ast.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.0.cmp(&b.0.0)));
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let ids: Vec<NaifId> = ast.iter().map(|a| a.0).collect();
    let mut states = ic.states().clone();
    for (id, s) in place_small_bodies(&file, &ids, &sun)
        .unwrap()
        .states()
        .unwrap()
    {
        states.push(id, &s).unwrap();
    }
    let majors: Vec<NBody> = cat
        .bodies()
        .iter()
        .filter(|b| b.tier() == Tier::Major)
        .map(|b| NBody {
            id: b.id(),
            gm: b.gm().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    let body = |&(id, gm): &(NaifId, f64), tier| NBody { id, gm, tier };
    let (promoted, minor) = ast.split_at(PROMOTED);
    let mut a = majors.clone();
    a.extend(promoted.iter().map(|x| body(x, Tier::Major)));
    let mut b = a.clone();
    b.extend(minor.iter().map(|x| body(x, Tier::Minor)));
    let mut c = majors;
    c.extend(ast.iter().map(|x| body(x, Tier::Major)));
    let build = |bodies: &[NBody], minor_term: bool| {
        let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
        ];
        if minor_term {
            terms.push(Box::new(MinorFromMajors::new(MinorReaction::On)));
        }
        let m = NBodyModel::<Icrf>::new(
            bodies,
            MoonVariables::Geocentric {
                emrat: de.earth_moon_mass_ratio(),
            },
            terms,
        )
        .unwrap();
        let (x, v) = m.pack(&states, ic.earth_moon()).unwrap();
        (m, x, v)
    };
    [
        ("16-major", &a[..], false),
        ("16-major+338-minor", &b[..], true),
        ("354-major", &c[..], false),
    ]
    .into_iter()
    .map(|(label, bodies, minor_term)| {
        let (m, x, v) = build(bodies, minor_term);
        (label, m, x, v)
    })
    .collect()
}

fn bench(c: &mut Criterion) {
    for (label, m, x, v) in models() {
        let mut a = vec![0.0; x.len()];
        c.bench_function(&format!("num009 acceleration {label}"), |bch| {
            bch.iter(|| {
                m.acceleration(0.0, black_box(&x), black_box(&v), &mut a)
                    .unwrap();
                black_box(&a);
            });
        });
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
