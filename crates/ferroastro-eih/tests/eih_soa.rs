// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP26 checks of the SoA EIH kernel (NUM-021; plan in IMPLEMENTATION-NOTES "WP26"):
//!
//! * P2 (feature `simd`): `EihSoa` with the SIMD and the scalar backend gives the same bits, on
//!   the header.440 systems (N = 10, 11, 16) for PPN (β, γ) = (1, 1) and (1, 0.9) and both
//!   right-hand-side modes, and on 200 seeded random configurations (N = 1 … 37);
//! * P3b: the SoA correction against `Eih` in double-double on the same f64 inputs
//!   (`SIMD-SOA-EIH-REL`); the production `Eih<f64>` error is reported.
//!
//! Needs the cached header.440 (CI-005) and the committed REF-003 states.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              counts below 2^53 are exact in f64"
)]

mod common;

use common::{FlatSystem, flat_model, flat_systems, vectors};
use ferroastro_constants::De440;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::Velocity;
use ferroastro_eih::{Eih, EihSoa, RhsAcceleration};
use ferroastro_forces::soa::SoaBackend;
use ferroastro_forces::{ForceTerm, MoonVariables, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};

/// PPN γ of the non-GR variant (test design: a value away from 1 exercises every γ coefficient).
const GAMMA_ALT: f64 = 0.9; // lint: allow-literal(PPN gamma of the non-GR test variant, test design)

fn eih<R: Real>(de: &De440, beta: f64, gamma: f64, rhs: RhsAcceleration) -> Eih<R> {
    let c = Velocity::from_km_per_s(R::from_f64(de.speed_of_light().km_per_s())).unwrap();
    Eih::new(c, R::from_f64(beta), R::from_f64(gamma), rhs).unwrap()
}

/// The EIH correction of every body (component-major), from a model holding only `term`.
fn correction<R: Real>(sys: &FlatSystem, term: Box<dyn ForceTerm<Icrf, R>>) -> Vec<R> {
    let m = flat_model::<R>(sys, MoonVariables::Barycentric, vec![term]);
    let x: Vec<R> = sys.x.iter().map(|&z| R::from_f64(z)).collect();
    let v: Vec<R> = sys.v.iter().map(|&z| R::from_f64(z)).collect();
    let mut a = vec![R::ZERO; x.len()];
    m.acceleration(R::ZERO, &x, &v, &mut a).unwrap();
    a
}

fn ppn_cases(de: &De440) -> [(f64, f64); 2] {
    [(de.ppn_beta(), de.ppn_gamma()), (de.ppn_beta(), GAMMA_ALT)]
}

/// `SplitMix64` (seeded, platform independent).
struct SplitMix(u64);

impl SplitMix {
    fn unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15); // lint: allow-literal(SplitMix64 increment)
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9); // lint: allow-literal(SplitMix64 multiplier)
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb); // lint: allow-literal(SplitMix64 multiplier)
        ((z ^ (z >> 31)) >> 11) as f64 * (f64::EPSILON / 2.0)
    }
}

/// P2 (feature `simd`): the SIMD and scalar `EihSoa` corrections are bitwise identical.
///
/// Verifies: NUM-021, TST-005
#[cfg(feature = "simd")]
#[test]
fn simd_and_scalar_eih_kernels_are_bitwise_identical() {
    let de = De440::load().unwrap();
    let diff = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .filter(|(p, q)| p.to_bits() != q.to_bits())
            .count()
    };
    let (mut differing, mut compared) = (0, 0);
    for sys in &flat_systems() {
        for (beta, gamma) in ppn_cases(&de) {
            for rhs in [RhsAcceleration::Newtonian, RhsAcceleration::Iterated] {
                let e = eih::<f64>(&de, beta, gamma, rhs);
                let s = correction::<f64>(sys, Box::new(EihSoa::new(e, SoaBackend::Scalar)));
                let w = correction::<f64>(sys, Box::new(EihSoa::new(e, SoaBackend::Simd)));
                let d = diff(&s, &w);
                println!(
                    "P2 EIH {} beta {beta} gamma {gamma} {rhs:?}: {d} of {} components differ",
                    sys.label,
                    s.len()
                );
                differing += d;
                compared += s.len();
            }
        }
    }
    let mut rng = SplitMix(1915);
    let configs: usize = 200; // lint: allow-literal(case count, test design)
    let e = eih::<f64>(&de, de.ppn_beta(), GAMMA_ALT, RhsAcceleration::Iterated);
    for case in 0..configs {
        let n = 1 + case % 37; // lint: allow-literal(N range 1..37, test design)
        let scale = 10.0_f64.powf(6.0 + 4.0 * rng.unit()); // lint: allow-literal(position scale 1e6..1e10 km, test design)
        let speed = 60.0; // lint: allow-literal(velocity scale km/s, test design)
        let sys = FlatSystem {
            label: "random",
            ids: (0..n)
                .map(|k| ferroastro_ephem::NaifId(i32::try_from(k).unwrap() + 1))
                .collect(),
            gm: (0..n)
                .map(|_| 10.0_f64.powf(11.0 * rng.unit())) // lint: allow-literal(GM up to 1e11 km^3/s^2, test design)
                .collect(),
            x: (0..3 * n)
                .map(|_| scale * (2.0 * rng.unit() - 1.0))
                .collect(),
            v: (0..3 * n)
                .map(|_| speed * (2.0 * rng.unit() - 1.0))
                .collect(),
            states: ferroastro_system::SystemState::new(),
        };
        let s = correction::<f64>(&sys, Box::new(EihSoa::new(e, SoaBackend::Scalar)));
        let w = correction::<f64>(&sys, Box::new(EihSoa::new(e, SoaBackend::Simd)));
        differing += diff(&s, &w);
        compared += s.len();
    }
    println!(
        "P2 EIH total (3 header systems x 2 PPN x 2 RHS + {configs} random configurations): \
         {differing} of {compared} components differ between {} and {}",
        SoaBackend::Scalar.label(),
        SoaBackend::Simd.label()
    );
    assert_eq!(differing, 0);
}

/// P3b: `EihSoa` (f64, preferred backend) against `Eih<DoubleDouble>` on the same inputs, per
/// body `|c_soa − c_DD| / |c_DD|`, for both right-hand-side modes; the production `Eih<f64>` is
/// reported. (The iterated mode was first only reported; it is asserted since the WP26 mutation
/// round, where a mutant that ignored the iterated right-hand side survived.)
///
/// Verifies: NUM-021, DEP-007
#[test]
fn soa_eih_correction_agrees_with_double_double() {
    let de = De440::load().unwrap();
    let mut worst = 0.0_f64;
    for sys in &flat_systems() {
        let n = sys.ids.len();
        for (beta, gamma) in ppn_cases(&de) {
            for rhs in [RhsAcceleration::Newtonian, RhsAcceleration::Iterated] {
                let soa = correction::<f64>(
                    sys,
                    Box::new(EihSoa::new(
                        eih(&de, beta, gamma, rhs),
                        SoaBackend::default(),
                    )),
                );
                let pw = correction::<f64>(sys, Box::new(eih::<f64>(&de, beta, gamma, rhs)));
                let dd = correction::<DoubleDouble>(
                    sys,
                    Box::new(eih::<DoubleDouble>(&de, beta, gamma, rhs)),
                );
                let rel = |a: &[f64]| -> f64 {
                    (0..n)
                        .map(|i| {
                            let mut d2 = 0.0;
                            let mut t2 = 0.0;
                            for c in 0..3 {
                                let t = dd[c * n + i];
                                let e = (DoubleDouble::from_f64(a[c * n + i]) - t).to_f64();
                                d2 += e * e;
                                t2 += t.to_f64() * t.to_f64();
                            }
                            (d2 / t2).sqrt()
                        })
                        .fold(0.0, f64::max)
                };
                let (r_soa, r_pw) = (rel(&soa), rel(&pw));
                println!(
                    "P3b {} beta {beta} gamma {gamma} {rhs:?}: max |c - c_DD| / |c_DD| SoA \
                     {r_soa:.3e}, production Eih {r_pw:.3e}",
                    sys.label
                );
                worst = worst.max(r_soa);
            }
        }
    }
    vectors()
        .get("SIMD-SOA-EIH-REL")
        .unwrap()
        .check(worst)
        .unwrap();
}
