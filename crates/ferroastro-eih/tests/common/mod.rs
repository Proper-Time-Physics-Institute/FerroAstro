// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared set-up of the WP19 EIH checks: header.440 ICs, DE440 GMs, J2 and c, model builders
//! (the WP12 model of `ferroastro-forces/tests/common` plus the EIH term).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test helpers: not every test binary uses every helper; failures are test failures"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::{Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::TimeSpan;
use ferroastro_eih::{Eih, HeliocentricSchwarzschild, RhsAcceleration};
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MoonVariables, NBody, NBodyModel,
    NBodyPropagator, NewtonianMutual, Pole, SolarJ2WithReaction,
};
use ferroastro_integrators::Ias15Config;
use ferroastro_real::Real;
use ferroastro_system::{Catalogue, DE440_MAJOR, InitialConditions, Tier};
use ferroastro_vectors::VectorSet;

pub(crate) fn vectors() -> VectorSet {
    VectorSet::load_canonical().unwrap()
}

/// A pinned JPL product from the cache (CI-005), with its provenance printed.
pub(crate) fn cached(name: &str) -> (Vec<u8>, String) {
    let f = ferroastro_testdata::locate_product(name).unwrap_or_else(|e| panic!("{e}"));
    let prov = f.provenance();
    println!("{name}: {prov}");
    (std::fs::read(&f.path).unwrap(), prov)
}

pub(crate) fn header() -> (Header, String) {
    let (bytes, prov) = cached("de440-header");
    (
        Header::parse(&String::from_utf8(bytes).unwrap()).unwrap(),
        prov,
    )
}

/// Sun + the 8 planets (system barycentres; Earth and Moon separately), without Pluto.
pub(crate) fn ids() -> Vec<NaifId> {
    DE440_MAJOR
        .iter()
        .map(|&(id, _)| id)
        .filter(|&id| id != NaifId::PLUTO_BARYCENTER)
        .collect()
}

pub(crate) fn name(id: NaifId) -> &'static str {
    DE440_MAJOR
        .iter()
        .find(|&&(i, _)| i == id)
        .map_or("?", |&(_, n)| n)
}

/// The relativistic term of a model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Gr {
    /// Newtonian only.
    None,
    /// EIH with β, γ and the RHS mode.
    Eih(f64, f64, RhsAcceleration),
    /// The heliocentric Sun-only 1PN rung.
    Central(f64, f64),
}

/// Newtonian mutual (+ geocentric Moon) (+ EIH or the central term) (+ solar J2 with reaction),
/// DE440 GMs and c, all Major.
pub(crate) fn model(
    de: &De440,
    ids: &[NaifId],
    geocentric: bool,
    gr: Gr,
    j2: bool,
) -> NBodyModel<Icrf> {
    let cat = Catalogue::de440_major(de);
    let bodies: Vec<NBody> = ids
        .iter()
        .map(|&id| NBody {
            id,
            gm: cat.gm(id).unwrap().km3_per_s2(),
            tier: Tier::Major,
        })
        .collect();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    let vars = if geocentric {
        terms.push(Box::new(
            GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap(),
        ));
        MoonVariables::Geocentric {
            emrat: de.earth_moon_mass_ratio(),
        }
    } else {
        MoonVariables::Barycentric
    };
    let c = de.speed_of_light();
    match gr {
        Gr::None => {}
        Gr::Eih(beta, gamma, rhs) => {
            terms.push(Box::new(Eih::new(c, beta, gamma, rhs).unwrap()));
        }
        Gr::Central(beta, gamma) => {
            terms.push(Box::new(
                HeliocentricSchwarzschild::new(c, beta, gamma).unwrap(),
            ));
        }
    }
    if j2 {
        let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
            .unwrap()
            .unit_vector();
        terms.push(Box::new(
            SolarJ2WithReaction::new(
                de.sun_j2(),
                ferroastro_core::units::Length::from_km(de.sun_radius().km()).unwrap(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ));
    }
    NBodyModel::new(&bodies, vars, terms).unwrap()
}

/// A propagator from the header.440 ICs (the DE Earth–Moon pair passed through when geocentric).
pub(crate) fn propagator(
    model: NBodyModel<Icrf>,
    ic: &InitialConditions<f64>,
    cfg: Ias15Config,
) -> NBodyPropagator<Icrf> {
    let dt0 = TimeSpan::from_seconds(SECONDS_PER_DAY / 10.0).unwrap();
    NBodyPropagator::new(model, ic.states(), ic.earth_moon(), ic.epoch(), dt0, cfg).unwrap()
}

/// `n` epochs `step_days` apart after `start` (TDB).
pub(crate) fn epochs(start: Epoch<Tdb>, step_days: f64, n: u32) -> Vec<Epoch<Tdb>> {
    (1..=n)
        .map(|k| {
            start + TimeSpan::from_seconds(f64::from(k) * step_days * SECONDS_PER_DAY).unwrap()
        })
        .collect()
}

pub(crate) fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

pub(crate) fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// A header.440 system for the WP26 SoA kernel checks, as flat component-major arrays
/// (`x[c·n + i]`, km and km/s) with DE440 GMs (km³/s²).
#[derive(Debug, Clone)]
pub(crate) struct FlatSystem {
    pub(crate) label: &'static str,
    pub(crate) ids: Vec<NaifId>,
    pub(crate) gm: Vec<f64>,
    pub(crate) x: Vec<f64>,
    pub(crate) v: Vec<f64>,
    pub(crate) states: ferroastro_system::SystemState<Icrf, ferroastro_core::frames::Barycentric>,
}

/// The EPM2021 Major asteroids of Subbotin et al. 2023 §2 (asteroid numbers).
pub(crate) const EPM2021_ASTEROIDS: [i32; 5] = [1, 2, 4, 7, 324];

/// The three WP26 systems at JDEPOC: `stage-c` (Sun, 8 planets, Earth and Moon; N = 10),
/// `de440-11` (+ Pluto; N = 11) and `epm16` (+ the EPM2021 five from REF-003; N = 16).
pub(crate) fn flat_systems() -> Vec<FlatSystem> {
    use ferroastro_core::frames::{Barycentric, State};
    use ferroastro_core::units::{Length, Velocity};
    use ferroastro_system::{ASTEROID_NAIF_BASE, SmallBodyStates, place_small_bodies};
    let (h, prov) = header();
    let cat = Catalogue::from_header(&h, &prov).unwrap();
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/goldens/sb441-n373s-jdepoc-states.csv");
    let file = SmallBodyStates::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let ast: Vec<NaifId> = EPM2021_ASTEROIDS
        .iter()
        .map(|&k| NaifId(ASTEROID_NAIF_BASE + k))
        .collect();
    let sun = ic.states().state(NaifId::SUN).unwrap();
    let placed = place_small_bodies(&file, &ast, &sun).unwrap();
    let state = |id: NaifId| -> State<Icrf, Barycentric> {
        ic.states().state(id).or_else(|_| placed.state(id)).unwrap()
    };
    let flat = |label: &'static str, ids: Vec<NaifId>| -> FlatSystem {
        let n = ids.len();
        let (mut x, mut v) = (vec![0.0; 3 * n], vec![0.0; 3 * n]);
        let mut states = ferroastro_system::SystemState::new();
        for (i, &id) in ids.iter().enumerate() {
            let s = state(id);
            states.push(id, &s).unwrap();
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
        FlatSystem {
            label,
            ids,
            gm,
            x,
            v,
            states,
        }
    };
    let ten = ids();
    let mut eleven = ten.clone();
    eleven.push(NaifId::PLUTO_BARYCENTER);
    let mut sixteen = eleven.clone();
    sixteen.extend(ast.iter().copied());
    vec![
        flat("stage-c (N = 10)", ten),
        flat("de440-11 (N = 11)", eleven),
        flat("epm16 (N = 16)", sixteen),
    ]
}

/// A model of `sys`'s bodies (all Major) with the given terms and Earth–Moon variables.
pub(crate) fn flat_model<R: Real>(
    sys: &FlatSystem,
    moon: MoonVariables<R>,
    terms: Vec<Box<dyn ForceTerm<Icrf, R>>>,
) -> NBodyModel<Icrf, R> {
    let bodies: Vec<NBody<R>> = sys
        .ids
        .iter()
        .zip(&sys.gm)
        .map(|(&id, &gm)| NBody {
            id,
            gm: R::from_f64(gm),
            tier: Tier::Major,
        })
        .collect();
    NBodyModel::new(&bodies, moon, terms).unwrap()
}
