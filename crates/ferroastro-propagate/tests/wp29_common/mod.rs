// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared helpers of the WP29 tests: a scenario's Newtonian model and initial state (as the
//! scenario assembles them, same IC hash), for the integrators' direct APIs.

#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers: not every test binary uses every helper; failures are test failures"
)]

use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::Velocity;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GrPotential, MoonVariables, NBody, NBodyModel, NewtonianMutual,
};
use ferroastro_propagate::{DataSources, initial_conditions};
use ferroastro_scenario::{Parts, ScenarioSpec};
use ferroastro_system::Tier;
use std::path::PathBuf;

pub(crate) fn header() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    println!("de440-header: {prov}");
    (
        Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap(),
        prov,
    )
}

pub(crate) fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

pub(crate) fn p(x: Parts) -> f64 {
    x[0] + x[1]
}

/// A Newtonian (optionally + GR potential) model and the initial state (component-major).
pub(crate) struct System {
    pub(crate) model: NBodyModel<Icrf>,
    pub(crate) gm: Vec<f64>,
    pub(crate) x0: Vec<f64>,
    pub(crate) v0: Vec<f64>,
    pub(crate) ids: Vec<i32>,
    pub(crate) ic_hash: String,
}

pub(crate) fn system(s: &ScenarioSpec, data: &DataSources<'_>, gr_potential: bool) -> System {
    let ic = initial_conditions(s, data).unwrap();
    let bodies: Vec<NBody> = ic
        .records
        .iter()
        .map(|r| NBody {
            id: NaifId(r.naif),
            gm: p(r.gm),
            tier: Tier::Major,
        })
        .collect();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    if gr_potential {
        let de = ferroastro_constants::De440::load().unwrap();
        terms.push(Box::new(
            GrPotential::new(Velocity::from_km_per_s(de.speed_of_light().km_per_s()).unwrap())
                .unwrap(),
        ));
    }
    let model = NBodyModel::new(&bodies, MoonVariables::Barycentric, terms).unwrap();
    let flat = |f: &dyn Fn(&ferroastro_scenario::IcRecord) -> [Parts; 3]| -> Vec<f64> {
        (0..3)
            .flat_map(|k| ic.records.iter().map(move |r| p(f(r)[k])))
            .collect()
    };
    System {
        model,
        gm: ic.records.iter().map(|r| p(r.gm)).collect(),
        x0: flat(&|r| r.position),
        v0: flat(&|r| r.velocity),
        ids: ic.records.iter().map(|r| r.naif).collect(),
        ic_hash: ic.ic_hash,
    }
}

/// `x_i − x_j` (component-major, n bodies).
pub(crate) fn rel(x: &[f64], n: usize, i: usize, j: usize) -> [f64; 3] {
    [0, 1, 2].map(|k| x[k * n + i] - x[k * n + j])
}

pub(crate) fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
