// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The per-scenario EIH scope (maintainer decision 2026-11-12; NUM-009, NUM-021): `EihScope::All`
//! (DE440, decision D1, the default) and `EihScope::Majors` (EPM2021, Subbotin et al. 2023: only
//! the Major bodies receive and source the 1PN terms).
//!
//! * S1: under `Majors`, the scalar `Eih` with the five EPM2021 asteroids as Minor bodies equals,
//!   bit for bit on the Major bodies, `Eih` on the same system without them, and the Minor bodies
//!   get exactly 0 (they stay Newtonian);
//! * S2: under `Majors` with Minor bodies present, `EihSoa` runs its own kernel and agrees with
//!   the scalar `Eih` (`SIMD-SOA-EIH-REL`);
//! * S3: under `All` with Minor bodies present, `EihSoa` falls back to `Eih` (bitwise equal), and
//!   the Minor bodies get non-zero corrections;
//! * S4: under `Majors` the term does not depend on the Minor reaction setting, so it combines
//!   with `MinorFromMajors(Off)`; under `All` the settings must agree.
//!
//! Needs the cached header.440 (CI-005) and the committed REF-003 states.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

mod common;

use common::{EPM2021_ASTEROIDS, FlatSystem, flat_systems, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::Velocity;
use ferroastro_eih::{Eih, EihScope, EihSoa, RhsAcceleration};
use ferroastro_ephem::NaifId;
use ferroastro_forces::soa::SoaBackend;
use ferroastro_forces::{
    ForceTerm, MinorFromMajors, MinorReaction, MoonVariables, NBody, NBodyModel, NewtonianMutual,
    SecondOrderSystem,
};
use ferroastro_system::{ASTEROID_NAIF_BASE, Tier};

fn eih(de: &De440, rhs: RhsAcceleration, scope: EihScope) -> Eih {
    Eih::new(
        Velocity::from_km_per_s(de.speed_of_light().km_per_s()).unwrap(),
        de.ppn_beta(),
        de.ppn_gamma(),
        rhs,
    )
    .unwrap()
    .with_scope(scope)
}

fn is_epm_asteroid(id: NaifId) -> bool {
    EPM2021_ASTEROIDS
        .iter()
        .any(|&k| id.0 == ASTEROID_NAIF_BASE + k)
}

/// Bodies of `sys`, the EPM2021 asteroids as Minor bodies.
fn tiered(sys: &FlatSystem) -> Vec<NBody> {
    sys.ids
        .iter()
        .zip(&sys.gm)
        .map(|(&id, &gm)| NBody {
            id,
            gm,
            tier: if is_epm_asteroid(id) {
                Tier::Minor
            } else {
                Tier::Major
            },
        })
        .collect()
}

/// The accelerations (component-major) of a model with only `term`.
fn only(bodies: &[NBody], sys: &FlatSystem, term: Box<dyn ForceTerm<Icrf>>) -> Vec<f64> {
    let m = NBodyModel::<Icrf>::new(bodies, MoonVariables::Barycentric, vec![term]).unwrap();
    let mut a = vec![0.0; sys.x.len()];
    m.acceleration(0.0, &sys.x, &sys.v, &mut a).unwrap();
    a
}

fn epm16() -> FlatSystem {
    flat_systems()
        .into_iter()
        .find(|s| s.label.starts_with("epm16"))
        .unwrap()
}

/// S1.
///
/// Verifies: NUM-009
#[test]
fn majors_scope_leaves_minor_bodies_newtonian() {
    let de = De440::load().unwrap();
    let sys = epm16();
    let n = sys.ids.len();
    let bodies = tiered(&sys);
    // The same system without the asteroids (the Major bodies only), packed in the same order.
    let keep: Vec<usize> = (0..n).filter(|&i| !is_epm_asteroid(sys.ids[i])).collect();
    let m = keep.len();
    let sub = |flat: &[f64]| -> Vec<f64> {
        (0..3)
            .flat_map(|c| keep.iter().map(move |&i| flat[c * n + i]))
            .collect()
    };
    let small = FlatSystem {
        label: "epm16-majors",
        ids: keep.iter().map(|&i| sys.ids[i]).collect(),
        gm: keep.iter().map(|&i| sys.gm[i]).collect(),
        x: sub(&sys.x),
        v: sub(&sys.v),
        states: sys.states.clone(),
    };
    let small_bodies: Vec<NBody> = tiered(&small);
    for rhs in [RhsAcceleration::Newtonian, RhsAcceleration::Iterated] {
        let a = only(&bodies, &sys, Box::new(eih(&de, rhs, EihScope::Majors)));
        let b = only(
            &small_bodies,
            &small,
            Box::new(eih(&de, rhs, EihScope::Majors)),
        );
        for (k, &i) in keep.iter().enumerate() {
            for c in 0..3 {
                assert_eq!(a[c * n + i].to_bits(), b[c * m + k].to_bits(), "body {i}");
            }
        }
        for i in (0..n).filter(|&i| is_epm_asteroid(sys.ids[i])) {
            assert!((0..3).all(|c| a[c * n + i].to_bits() == 0), "Minor {i}");
        }
    }
}

/// S2 and S3.
///
/// Verifies: NUM-021, NUM-009
#[test]
fn soa_kernel_under_majors_and_scalar_fallback_under_all() {
    let de = De440::load().unwrap();
    let sys = epm16();
    let n = sys.ids.len();
    let bodies = tiered(&sys);
    let mut worst = 0.0_f64;
    for rhs in [RhsAcceleration::Newtonian, RhsAcceleration::Iterated] {
        // S2: Majors, Minor bodies present: SoA lanes vs scalar.
        let soa = only(
            &bodies,
            &sys,
            Box::new(EihSoa::new(
                eih(&de, rhs, EihScope::Majors),
                SoaBackend::default(),
            )),
        );
        let scalar = only(&bodies, &sys, Box::new(eih(&de, rhs, EihScope::Majors)));
        let mut lanes_differ = false;
        for i in 0..n {
            let (mut d2, mut t2) = (0.0, 0.0);
            for c in 0..3 {
                let (p, q) = (soa[c * n + i], scalar[c * n + i]);
                d2 += (p - q) * (p - q);
                t2 += q * q;
                lanes_differ |= p.to_bits() != q.to_bits();
            }
            if is_epm_asteroid(sys.ids[i]) {
                assert!(t2 == 0.0 && d2 == 0.0, "Minor body {i} must stay Newtonian");
            } else {
                worst = worst.max((d2 / t2).sqrt());
            }
        }
        println!(
            "S2 {rhs:?}: Majors scope, SoA vs scalar Eih: max rel diff {worst:.3e} (bitwise \
             different somewhere: {lanes_differ})"
        );
        // S3: All, Minor bodies present: EihSoa falls back to Eih, bit for bit.
        let soa_all = only(
            &bodies,
            &sys,
            Box::new(EihSoa::new(
                eih(&de, rhs, EihScope::All),
                SoaBackend::default(),
            )),
        );
        let scalar_all = only(&bodies, &sys, Box::new(eih(&de, rhs, EihScope::All)));
        for k in 0..3 * n {
            assert_eq!(soa_all[k].to_bits(), scalar_all[k].to_bits());
        }
        let minor_nonzero = (0..n)
            .filter(|&i| is_epm_asteroid(sys.ids[i]))
            .all(|i| (0..3).any(|c| soa_all[c * n + i] != 0.0));
        assert!(
            minor_nonzero,
            "under All the Minor bodies get 1PN corrections"
        );
    }
    vectors()
        .get("SIMD-SOA-EIH-REL")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// S4.
///
/// Verifies: NUM-009
#[test]
fn majors_scope_does_not_constrain_the_minor_reaction() {
    let de = De440::load().unwrap();
    let sys = epm16();
    let bodies = tiered(&sys);
    let build = |scope: EihScope, minor: MinorReaction| {
        let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
            Box::new(NewtonianMutual),
            Box::new(MinorFromMajors::new(minor)),
            Box::new(eih(&de, RhsAcceleration::Newtonian, scope)),
        ];
        NBodyModel::<Icrf>::new(&bodies, MoonVariables::Barycentric, terms)
    };
    assert!(build(EihScope::Majors, MinorReaction::Off).is_ok());
    assert!(build(EihScope::Majors, MinorReaction::On).is_ok());
    assert!(build(EihScope::All, MinorReaction::On).is_ok());
    assert!(build(EihScope::All, MinorReaction::Off).is_err());
}
