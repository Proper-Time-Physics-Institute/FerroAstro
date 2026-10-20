// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP26 checks of the SoA Newtonian kernel (NUM-021; plan in IMPLEMENTATION-NOTES "WP26"):
//!
//! * P1: the exact inverse cube is within 1 ulp (`SIMD-INV-R3-MAX-ULP`), against astro-float;
//! * P2 (feature `simd`): the SIMD and scalar Newtonian kernels are bitwise identical on the
//!   header.440 systems (N = 10, 11, 16; barycentric and geocentric Moon) and on 500 seeded
//!   random configurations with N = 1 … 37;
//! * P3a: the SoA error against the double-double pairwise kernel is within the forward bound
//!   (`SIMD-SOA-NEWTON-ERROR-BOUND-RATIO`);
//! * P3c: with the geocentric Earth–Moon pair, SoA and pairwise model accelerations agree
//!   (`SIMD-SOA-GEOCENTRIC-REL`);
//! * a singular configuration is a coded error (API-002).
//!
//! Needs the cached header.440 (CI-005) and the committed REF-003 states.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              counts below 2^53 are exact in f64; the FMA-free r^2 is the SoA kernel's policy"
)]

mod common;

use astro_float::{BigFloat, RoundingMode, Sign};
use common::{FlatSystem, flat_model, flat_systems, vectors};
use ferroastro_constants::De440;
use ferroastro_core::ErrorCode;
use ferroastro_core::frames::Icrf;
use ferroastro_ephem::NaifId;
use ferroastro_forces::soa::{SoaBackend, SoaBodies, inv_r3_exact_scalar, newtonian_soa};
use ferroastro_forces::{
    ForceTerm, GeocentricMoon, MOON_TAYLOR_TERMS_DEFAULT, MoonVariables, NewtonianMutual,
    NewtonianMutualSoa, SecondOrderSystem,
};
use ferroastro_real::{DoubleDouble, Real};

/// astro-float working precision (bits) of the 1/r³ oracle.
const P: usize = 256;
const RM: RoundingMode = RoundingMode::ToEven;
/// u = 2⁻⁵³.
const U: f64 = f64::EPSILON / 2.0;

/// `SplitMix64`: a seeded, platform-independent generator (no external RNG in the oracle path).
struct SplitMix(u64);

impl SplitMix {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15); // lint: allow-literal(SplitMix64 increment)
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9); // lint: allow-literal(SplitMix64 multiplier)
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb); // lint: allow-literal(SplitMix64 multiplier)
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1) with 53 random bits.
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * U
    }
}

fn big(x: f64) -> BigFloat {
    BigFloat::from_f64(x, P)
}

/// Nearest-ish f64 of a `BigFloat` (leading 64-bit word; for error reporting only).
fn to_f64(b: &BigFloat) -> f64 {
    if b.is_zero() {
        return 0.0;
    }
    let (m, _, s, e, _) = b.as_raw_parts().unwrap();
    let top = *m.last().unwrap();
    let hi32 = f64::from(u32::try_from(top >> 32).unwrap());
    let lo32 = f64::from(u32::try_from(top & 0xffff_ffff).unwrap());
    let frac = (hi32 * 2.0_f64.powi(32) + lo32) * 2.0_f64.powi(-64); // lint: allow-literal(2^32 and 2^64, word-size constants)
    let v = frac * 2.0_f64.powi(e);
    if s == Sign::Neg { -v } else { v }
}

/// |computed − exact| in ulps of `exact` (binary64 ulp of [2^(e−1), 2^e) is 2^(e−53)).
fn ulp_err(computed: f64, exact: &BigFloat) -> f64 {
    let d = big(computed).sub(exact, P, RM).abs();
    let e = exact.exponent().unwrap();
    to_f64(&d) / 2.0_f64.powi(e - 53) // lint: allow-literal(binary64 significand width 53)
}

fn exact_inv_r3(r2: f64) -> BigFloat {
    let b = big(r2);
    let r3 = b.mul(&b.sqrt(P, RM), P, RM);
    big(1.0).div(&r3, P, RM)
}

fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}

/// Per-body positions, velocities and GMs of a flat system.
fn bodies(sys: &FlatSystem) -> (Vec<[f64; 3]>, Vec<[f64; 3]>, Vec<f64>) {
    let n = sys.ids.len();
    let at = |v: &[f64], i: usize| [v[i], v[n + i], v[2 * n + i]];
    (
        (0..n).map(|i| at(&sys.x, i)).collect(),
        (0..n).map(|i| at(&sys.v, i)).collect(),
        sys.gm.clone(),
    )
}

/// Group labels: each body its own, or the Earth and the Moon sharing one (the geocentric pair).
fn groups(sys: &FlatSystem, pair: bool) -> Vec<usize> {
    let e = sys.ids.iter().position(|&i| i == NaifId::EARTH).unwrap();
    (0..sys.ids.len())
        .map(|k| {
            if pair && sys.ids[k] == NaifId::MOON {
                e
            } else {
                k
            }
        })
        .collect()
}

/// P1: the SoA inverse cube against a 256-bit reference, 100 000 log-uniform r² in
/// [1e-6, 1e30] km² plus every pair of the N = 16 header.440 system.
///
/// Verifies: NUM-021
#[test]
fn exact_inverse_cube_is_within_one_ulp() {
    let (lo, hi) = (-6.0, 30.0); // lint: allow-literal(log10 range of r^2 in km^2, test design)
    let cases = 100_000; // lint: allow-literal(case count, test design)
    let mut rng = SplitMix(26);
    let mut inputs: Vec<f64> = (0..cases)
        .map(|_| 10.0_f64.powf(lo + (hi - lo) * rng.unit()))
        .collect();
    let sys = &flat_systems()[2];
    let (pos, _, _) = bodies(sys);
    for (i, p) in pos.iter().enumerate() {
        for q in pos.iter().skip(i + 1) {
            let d = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
            inputs.push((d[0] * d[0] + d[1] * d[1]) + d[2] * d[2]);
        }
    }
    let (mut worst, mut worst_at, mut prod_worst, mut above_half) = (0.0_f64, 0.0, 0.0_f64, 0_u32);
    for &r2 in &inputs {
        let exact = exact_inv_r3(r2);
        let e = ulp_err(inv_r3_exact_scalar(r2), &exact);
        if e > worst {
            (worst, worst_at) = (e, r2);
        }
        if e > 0.5 {
            above_half += 1;
        }
        prod_worst = prod_worst.max(ulp_err(1.0 / (r2 * r2.sqrt()), &exact));
    }
    println!(
        "P1: {} inputs ({} header.440 pairs of {}): exact 1/r^3 max error {worst:.6} ulp (at r^2 = \
         {worst_at:.6e} km^2; {above_half} inputs above 0.5 ulp); production 1/(r2 sqrt r2) max \
         {prod_worst:.6} ulp",
        inputs.len(),
        inputs.len() - cases,
        sys.label
    );
    vectors()
        .get("SIMD-INV-R3-MAX-ULP")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// P2 (feature `simd`): the SIMD and scalar SoA Newtonian kernels give the same bits, on the
/// header.440 systems and on 500 seeded random configurations (N = 1 … 37, shared group labels,
/// zero GMs and widely spread scales).
///
/// Verifies: NUM-021, TST-005
#[cfg(feature = "simd")]
#[test]
fn simd_and_scalar_newtonian_kernels_are_bitwise_identical() {
    let compare = |b: &SoaBodies<f64>| -> (usize, usize) {
        let mut s: [Vec<f64>; 3] = Default::default();
        let mut w: [Vec<f64>; 3] = Default::default();
        newtonian_soa(b, SoaBackend::Scalar, &mut s).unwrap();
        newtonian_soa(b, SoaBackend::Simd, &mut w).unwrap();
        let n = b.len();
        let differing = (0..3)
            .map(|c| {
                (0..n)
                    .filter(|&k| s[c][k].to_bits() != w[c][k].to_bits())
                    .count()
            })
            .sum();
        (differing, 3 * n)
    };
    let mut total = (0, 0);
    for sys in &flat_systems() {
        let (p, v, g) = bodies(sys);
        for pair in [false, true] {
            let (d, c) = compare(&SoaBodies::from_bodies(&p, &v, &g, &groups(sys, pair)));
            println!(
                "P2 {} (Earth-Moon pair excluded: {pair}): {d} of {c} components differ",
                sys.label
            );
            total = (total.0 + d, total.1 + c);
        }
    }
    let mut rng = SplitMix(2026);
    let configs = 500; // lint: allow-literal(case count, test design)
    for case in 0..configs {
        let n = 1 + case % 37; // lint: allow-literal(N range 1..37, test design)
        let scale = 10.0_f64.powf(10.0 * rng.unit()); // lint: allow-literal(position scale up to 1e10 km, test design)
        let p: Vec<[f64; 3]> = (0..n)
            .map(|_| core::array::from_fn(|_| scale * (2.0 * rng.unit() - 1.0)))
            .collect();
        let v: Vec<[f64; 3]> = (0..n).map(|_| [0.0; 3]).collect();
        let g: Vec<f64> = (0..n)
            .map(|_| {
                if rng.unit() < 0.1 {
                    0.0
                } else {
                    10.0_f64.powf(11.0 * rng.unit()) // lint: allow-literal(GM up to 1e11 km^3/s^2, test design)
                }
            })
            .collect();
        let grp: Vec<usize> = (0..n)
            .map(|k| if k > 0 && rng.unit() < 0.1 { k - 1 } else { k })
            .collect();
        let (d, c) = compare(&SoaBodies::from_bodies(&p, &v, &g, &grp));
        total = (total.0 + d, total.1 + c);
    }
    println!(
        "P2 total (3 header systems x 2 pair rules + {configs} random configurations): {} of {} \
         acceleration components differ between {} and {}",
        total.0,
        total.1,
        SoaBackend::Scalar.label(),
        SoaBackend::Simd.label()
    );
    assert_eq!(total.0, 0);
}

/// P2 at model level (feature `simd`): `NewtonianMutualSoa` + `GeocentricMoon` models with the
/// scalar and the SIMD backend give the same bits from the packed header.440 states.
///
/// Verifies: NUM-021, NUM-008, TST-005
#[cfg(feature = "simd")]
#[test]
fn simd_and_scalar_models_are_bitwise_identical_with_the_geocentric_pair() {
    let de = De440::load().unwrap();
    let emrat = de.earth_moon_mass_ratio();
    for sys in &flat_systems() {
        let model = |backend: SoaBackend| {
            let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
                Box::new(NewtonianMutualSoa::new(backend)),
                Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
            ];
            flat_model(sys, MoonVariables::Geocentric { emrat }, terms)
        };
        let (ms, mw) = (model(SoaBackend::Scalar), model(SoaBackend::Simd));
        let (x, v) = ms.pack(&sys.states, None).unwrap();
        let (mut a, mut b) = (vec![0.0; x.len()], vec![0.0; x.len()]);
        ms.acceleration(0.0, &x, &v, &mut a).unwrap();
        mw.acceleration(0.0, &x, &v, &mut b).unwrap();
        let differing = a
            .iter()
            .zip(&b)
            .filter(|(p, q)| p.to_bits() != q.to_bits())
            .count();
        println!(
            "P2 model {} geocentric: {differing} of {} components differ",
            sys.label,
            a.len()
        );
        assert_eq!(differing, 0);
    }
}

/// P3a: the SoA Newtonian accelerations (f64, preferred backend) against the pairwise kernel in
/// double-double on the same f64 inputs, within the forward bound `(m + 9) u Σⱼ |t_ij|` per
/// component. The production pairwise f64 kernel's ratio is reported.
///
/// Verifies: NUM-021, DEP-007
#[test]
fn soa_newtonian_error_is_within_the_forward_bound() {
    let mut worst = 0.0_f64;
    for sys in &flat_systems() {
        let n = sys.ids.len();
        let soa = flat_model::<f64>(
            sys,
            MoonVariables::Barycentric,
            vec![Box::new(NewtonianMutualSoa::default())],
        );
        let pw = flat_model::<f64>(
            sys,
            MoonVariables::Barycentric,
            vec![Box::new(NewtonianMutual)],
        );
        let dd = flat_model::<DoubleDouble>(
            sys,
            MoonVariables::Barycentric,
            vec![Box::new(NewtonianMutual)],
        );
        let (mut a_soa, mut a_pw) = (vec![0.0; 3 * n], vec![0.0; 3 * n]);
        soa.acceleration(0.0, &sys.x, &sys.v, &mut a_soa).unwrap();
        pw.acceleration(0.0, &sys.x, &sys.v, &mut a_pw).unwrap();
        let wide = |v: &[f64]| -> Vec<DoubleDouble> {
            v.iter().map(|&z| DoubleDouble::from_f64(z)).collect()
        };
        let (xd, vd) = (wide(&sys.x), wide(&sys.v));
        let mut a_dd = vec![DoubleDouble::ZERO; 3 * n];
        dd.acceleration(DoubleDouble::ZERO, &xd, &vd, &mut a_dd)
            .unwrap();
        let (mut r_soa, mut r_pw) = (0.0_f64, 0.0_f64);
        for i in 0..n {
            for c in 0..3 {
                let mut sum_abs = 0.0;
                for j in (0..n).filter(|&j| j != i) {
                    let d: [DoubleDouble; 3] =
                        core::array::from_fn(|k| xd[k * n + j] - xd[k * n + i]);
                    let r2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                    let t = DoubleDouble::from_f64(sys.gm[j]) * d[c] / (r2 * r2.sqrt());
                    sum_abs += t.abs().to_f64();
                }
                let bound = (n as f64 + 9.0) * U * sum_abs; // lint: allow-literal(the 9 of the pre-registered (m + 9) u bound)
                let truth = a_dd[c * n + i];
                let e_soa = (DoubleDouble::from_f64(a_soa[c * n + i]) - truth)
                    .abs()
                    .to_f64();
                let e_pw = (DoubleDouble::from_f64(a_pw[c * n + i]) - truth)
                    .abs()
                    .to_f64();
                r_soa = r_soa.max(e_soa / bound);
                r_pw = r_pw.max(e_pw / bound);
            }
        }
        println!(
            "P3a {}: max |a - a_DD| / ((m + 9) u sum|t|): SoA {r_soa:.4}, production pairwise \
             {r_pw:.4}",
            sys.label
        );
        worst = worst.max(r_soa);
    }
    vectors()
        .get("SIMD-SOA-NEWTON-ERROR-BOUND-RATIO")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// P3c: Newtonian mutual (SoA vs pairwise) + `GeocentricMoon` with the geocentric Earth–Moon
/// pair: the model accelerations of every storage slot agree to `SIMD-SOA-GEOCENTRIC-REL`.
///
/// Verifies: NUM-021, NUM-008
#[test]
fn soa_and_pairwise_agree_with_the_geocentric_pair() {
    let de = De440::load().unwrap();
    let emrat = de.earth_moon_mass_ratio();
    let mut worst = 0.0_f64;
    for sys in &flat_systems() {
        let model = |soa: bool| {
            let newton: Box<dyn ForceTerm<Icrf>> = if soa {
                Box::new(NewtonianMutualSoa::default())
            } else {
                Box::new(NewtonianMutual)
            };
            let terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![
                newton,
                Box::new(GeocentricMoon::<f64>::new(MOON_TAYLOR_TERMS_DEFAULT).unwrap()),
            ];
            flat_model(sys, MoonVariables::Geocentric { emrat }, terms)
        };
        let (ms, mp) = (model(true), model(false));
        let (x, v) = ms.pack(&sys.states, None).unwrap();
        let n = sys.ids.len();
        let (mut a, mut b) = (vec![0.0; 3 * n], vec![0.0; 3 * n]);
        ms.acceleration(0.0, &x, &v, &mut a).unwrap();
        mp.acceleration(0.0, &x, &v, &mut b).unwrap();
        let mut rel = 0.0_f64;
        for i in 0..n {
            let s = [a[i], a[n + i], a[2 * n + i]];
            let p = [b[i], b[n + i], b[2 * n + i]];
            rel = rel.max(norm([s[0] - p[0], s[1] - p[1], s[2] - p[2]]) / norm(p));
        }
        println!(
            "P3c {} geocentric: max |a_soa - a_pairwise| / |a| = {rel:.3e}",
            sys.label
        );
        worst = worst.max(rel);
    }
    vectors()
        .get("SIMD-SOA-GEOCENTRIC-REL")
        .unwrap()
        .check(worst)
        .unwrap();
}

/// Two bodies of different groups at one position: a coded singular-force error naming both
/// (API-002), with either backend.
///
/// Verifies: API-002
#[test]
fn coinciding_bodies_are_a_coded_error() {
    let sys = &flat_systems()[0];
    let mut x = sys.x.clone();
    let n = sys.ids.len();
    for c in 0..3 {
        x[c * n + 3] = x[c * n + 1];
    }
    let m = flat_model::<f64>(
        sys,
        MoonVariables::Barycentric,
        vec![Box::new(NewtonianMutualSoa::default())],
    );
    let mut a = vec![0.0; 3 * n];
    let e = m.acceleration(0.0, &x, &sys.v, &mut a).unwrap_err();
    assert_eq!(e.code(), ErrorCode::ForceModelSingular, "{e}");
    let msg = e.to_string();
    assert!(msg.contains('1') && msg.contains('3'), "{msg}");
}
