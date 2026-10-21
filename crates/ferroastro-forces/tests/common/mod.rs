// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared set-up of the WP12 N-body tests: header.440 ICs, DE440 GMs and J2, model builders.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers: not every test binary uses every helper; failures are test failures"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::TimeSpan;
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

/// Sun + the 8 planets (system barycentres; Earth and Moon separately), optionally Pluto.
pub(crate) fn ids(with_pluto: bool) -> Vec<NaifId> {
    DE440_MAJOR
        .iter()
        .map(|&(id, _)| id)
        .filter(|&id| with_pluto || id != NaifId::PLUTO_BARYCENTER)
        .collect()
}

pub(crate) fn name(id: NaifId) -> &'static str {
    DE440_MAJOR
        .iter()
        .find(|&&(i, _)| i == id)
        .map_or("?", |&(_, n)| n)
}

/// Which Earth–Moon variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moon {
    Geocentric,
    Barycentric,
}

/// The WP12 model: Newtonian mutual (+ geocentric Moon) (+ solar J2 with reaction: DE440 J2SUN,
/// ASUN, IAU WGCCRE 2015 pole from ferroastro-frames), DE440 GMs, all Major.
pub(crate) fn model<R: Real>(
    de: &De440,
    ids: &[NaifId],
    moon: Moon,
    j2: bool,
) -> NBodyModel<Icrf, R> {
    model_lt(de, ids, moon, j2, None)
}

/// `G·S` of the Sun from DE440: `GMS · MOISUN · ASUN² · OMGSUN` (km⁵/s³).
pub(crate) fn de440_gs(de: &De440) -> f64 {
    let r = de.sun_radius().km();
    de.gm_sun().km3_per_s2()
        * de.sun_moment_of_inertia()
        * r
        * r
        * de.sun_rotation_rate().rad_per_s()
}

/// [`model`] plus, with `lt = Some(reaction)`, the solar Lense–Thirring term (DE440 spin, PPN γ,
/// c, WGCCRE 2015 pole) after J2 (WP20).
pub(crate) fn model_lt<R: Real>(
    de: &De440,
    ids: &[NaifId],
    moon: Moon,
    j2: bool,
    lt: Option<ferroastro_forces::LtReaction>,
) -> NBodyModel<Icrf, R> {
    let cat = Catalogue::de440_major(de);
    let bodies: Vec<NBody<R>> = ids
        .iter()
        .map(|&id| NBody {
            id,
            gm: R::from_f64(cat.gm(id).unwrap().km3_per_s2()),
            tier: Tier::Major,
        })
        .collect();
    let mut terms: Vec<Box<dyn ForceTerm<Icrf, R>>> = vec![Box::new(NewtonianMutual)];
    let vars = match moon {
        Moon::Geocentric => {
            terms.push(Box::new(
                GeocentricMoon::<R>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap(),
            ));
            MoonVariables::Geocentric {
                emrat: R::from_f64(de.earth_moon_mass_ratio()),
            }
        }
        Moon::Barycentric => MoonVariables::Barycentric,
    };
    if j2 {
        let k = ferroastro_frames::Pole::<R>::sun_wgccre2015()
            .unwrap()
            .unit_vector();
        let radius = de.sun_radius();
        terms.push(Box::new(
            SolarJ2WithReaction::new(
                R::from_f64(de.sun_j2()),
                ferroastro_core::units::Length::from_km(R::from_f64(radius.km())).unwrap(),
                Pole::from_components(k).unwrap(),
            )
            .unwrap(),
        ));
    }
    if let Some(reaction) = lt {
        let k = ferroastro_frames::Pole::<R>::sun_wgccre2015()
            .unwrap()
            .unit_vector();
        terms.push(Box::new(
            ferroastro_forces::SolarLenseThirring::new(
                ferroastro_core::units::SpinGm::from_km5_per_s3(R::from_f64(de440_gs(de))).unwrap(),
                ferroastro_core::units::Velocity::from_km_per_s(R::from_f64(
                    de.speed_of_light().km_per_s(),
                ))
                .unwrap(),
                R::from_f64(de.ppn_gamma()),
                Pole::from_components(k).unwrap(),
                reaction,
            )
            .unwrap(),
        ));
    }
    NBodyModel::new(&bodies, vars, terms).unwrap()
}

/// A propagator from the header.440 ICs (the DE Earth–Moon pair passed through when geocentric).
pub(crate) fn propagator<R: Real>(
    model: NBodyModel<Icrf, R>,
    ic: &InitialConditions<R>,
    cfg: Ias15Config,
) -> NBodyPropagator<Icrf, R> {
    let dt0 = TimeSpan::from_seconds(R::from_f64(SECONDS_PER_DAY / 10.0)).unwrap();
    NBodyPropagator::new(model, ic.states(), ic.earth_moon(), ic.epoch(), dt0, cfg).unwrap()
}

/// `n` epochs `step_days` apart after `start` (TDB).
pub(crate) fn epochs<R: Real>(
    start: ferroastro_core::time::Epoch<ferroastro_core::time::Tdb, R>,
    step_days: f64,
    n: u32,
) -> Vec<ferroastro_core::time::Epoch<ferroastro_core::time::Tdb, R>> {
    (1..=n)
        .map(|k| {
            start
                + TimeSpan::from_seconds(R::from_f64(f64::from(k) * step_days * SECONDS_PER_DAY))
                    .unwrap()
        })
        .collect()
}
