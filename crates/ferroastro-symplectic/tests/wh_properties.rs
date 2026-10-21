// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Structural properties of the Wisdom–Holman integrator (NUM-004, WP29) on a synthetic
//! three-body system (dimensionless: GM☉ = 1, planets of 1e-3 and 3e-4, a = 1 and 1.6; no
//! physical values): time symmetry, bitwise snapshots in f64 and double-double, dense output at
//! the step end, and the refusals of [`check_model`].

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction"
)]

use ferroastro_core::frames::Icrf;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{MoonVariables, NBody, NBodyModel, NewtonianMutual, RingMasses};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_symplectic::{CorrectorOrder, WhConfig, Whfast, check_model};
use ferroastro_system::Tier;

/// Synthetic GMs and circular-ish orbits (structural fixture, dimensionless).
const GM: [f64; 3] = [1.0, 1e-3, 3e-4]; // lint: allow-literal(structural fixture)
const A: [f64; 2] = [1.0, 1.6]; // lint: allow-literal(structural fixture)
/// Steps and step of the fixture runs (about 1/40 of the inner period).
const STEPS: u32 = 400;
const H: f64 = 0.15; // lint: allow-literal(structural fixture step)

fn model<R: Real>(tiers: [Tier; 3]) -> NBodyModel<Icrf, R> {
    let ids = [
        NaifId::SUN,
        NaifId::JUPITER_BARYCENTER,
        NaifId::SATURN_BARYCENTER,
    ];
    let bodies: Vec<NBody<R>> = (0..3)
        .map(|i| NBody {
            id: ids[i],
            gm: R::from_f64(GM[i]),
            tier: tiers[i],
        })
        .collect();
    NBodyModel::new(
        &bodies,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutual)],
    )
    .unwrap()
}

/// Component-major initial state: planets on circular orbits about the Sun at rest.
fn state<R: Real>() -> (Vec<R>, Vec<R>) {
    let mut x = vec![R::ZERO; 9];
    let mut v = vec![R::ZERO; 9];
    for (k, &a) in A.iter().enumerate() {
        let i = k + 1;
        x[i] = R::from_f64(a); // x component
        v[3 + i] = R::from_f64((GM[0] / a).sqrt()); // y component
    }
    (x, v)
}

fn bits<R: Real>(z: &[R]) -> Vec<(u64, u64)> {
    z.iter()
        .map(|w| {
            let (h, l) = w.to_parts();
            (h.to_bits(), l.to_bits())
        })
        .collect()
}

/// Verifies: NUM-004
#[test]
fn wh_is_time_symmetric_to_round_off() {
    let m = model::<f64>([Tier::Major; 3]);
    let (x0, v0) = state::<f64>();
    // Without a corrector: the corrector's real→kernel and kernel→real maps are inverse only to
    // O(ε²), so a run restarted from a corrected output is not the time-reversed kernel run.
    let cfg = WhConfig {
        corrector: CorrectorOrder::None,
        output_every_step: false,
    };
    let mut wh = Whfast::new(&m, &GM, 0.0, &x0, &v0, H, cfg).unwrap();
    for _ in 0..STEPS {
        wh.step(&m).unwrap();
    }
    let (mut x, mut v) = (vec![0.0; 9], vec![0.0; 9]);
    wh.physical_state(&m, &mut x, &mut v).unwrap();
    // A new integrator from the end state with −h retraces the steps.
    let mut back = Whfast::new(&m, &GM, 0.0, &x, &v, -H, cfg).unwrap();
    for _ in 0..STEPS {
        back.step(&m).unwrap();
    }
    back.physical_state(&m, &mut x, &mut v).unwrap();
    let dx = x
        .iter()
        .zip(&x0)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!("WH forward {STEPS} steps then back: max |dx| = {dx:.3e}");
    assert!(dx < 1e-12); // lint: allow-literal(round-off of 800 steps at |x| ~ 1)
}

/// Verifies: NUM-004
#[test]
fn wh_snapshots_resume_bit_for_bit() {
    fn check<R: Real>() {
        let m = model::<R>([Tier::Major; 3]);
        let (x0, v0) = state::<R>();
        let gm = GM.map(R::from_f64);
        let mut a = Whfast::<R>::new(
            &m,
            &gm,
            R::ZERO,
            &x0,
            &v0,
            R::from_f64(H),
            WhConfig::default(),
        )
        .unwrap();
        for _ in 0..STEPS / 2 {
            a.step(&m).unwrap();
        }
        let snap =
            postcard::from_bytes(&postcard::to_allocvec(&a.snapshot().unwrap()).unwrap()).unwrap();
        let mut b = Whfast::<R>::restore(&snap).unwrap();
        for _ in 0..STEPS / 2 {
            a.step(&m).unwrap();
            b.step(&m).unwrap();
        }
        assert_eq!(bits(a.positions()), bits(b.positions()), "{}", R::NAME);
        assert_eq!(bits(a.velocities()), bits(b.velocities()), "{}", R::NAME);
        assert_eq!(a.stats(), b.stats());
        // Dense output at the step end is the step's own output.
        let t = a.time();
        let (mut x, mut v) = (vec![R::ZERO; 9], vec![R::ZERO; 9]);
        a.dense_state(&m, t, &mut x, &mut v).unwrap();
        assert_eq!(bits(&x), bits(a.positions()));
    }
    check::<f64>();
    check::<DoubleDouble>();
}

/// Verifies: NUM-004
#[test]
fn check_model_refuses_what_the_kick_cannot_integrate() {
    assert!(check_model(&model::<f64>([Tier::Major; 3])).is_ok());
    let ring = {
        let ids = [NaifId::SUN, NaifId::JUPITER_BARYCENTER, NaifId(2_000_001)]; // lint: allow-literal(a structural NAIF id for the ring mass)
        let bodies: Vec<NBody> = (0..3)
            .map(|i| NBody {
                id: ids[i],
                gm: GM[i],
                tier: if i == 2 { Tier::Ring } else { Tier::Major },
            })
            .collect();
        NBodyModel::<Icrf>::new(
            &bodies,
            MoonVariables::Barycentric,
            vec![Box::new(NewtonianMutual), Box::new(RingMasses)],
        )
        .unwrap()
    };
    let e = check_model(&ring).unwrap_err();
    println!("{e}");
    assert!(e.to_string().contains("ring-fixed-masses"));
}
