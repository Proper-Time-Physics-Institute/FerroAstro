// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! WP25: the NUM-007 mixed-mode force adapters on the header.440 system (pinned JPL cache,
//! CI-005): Sun + 8 planets + Pluto with the geocentric Moon, solar J2 and Lense–Thirring.
//!
//! * `MixedForces` in f64 is bitwise the compensated f64 model (one rounding of `Σ + c`).
//! * `MixedForces` and `F64Forces` in double-double against the double-double model: reported.

mod common;

use common::{Moon, header, ids, model_lt};
use ferroastro_constants::De440;
use ferroastro_forces::{F64Forces, LtReaction, MixedForces, SecondOrderSystem};
use ferroastro_real::{DoubleDouble, Real};
use ferroastro_system::InitialConditions;

type Dd = DoubleDouble;

/// `MixedForces` (f64) = the compensated f64 model, bitwise; in double-double it carries the
/// compensation as a low word.
///
/// Verifies: NUM-007, DEP-007
#[test]
fn mixed_forces_is_the_compensated_model_widened() {
    let (h, prov) = header();
    let de = De440::load().unwrap();
    let ids = ids(true);
    let lt = Some(LtReaction::On);
    let m64 = model_lt::<f64>(&de, &ids, Moon::Geocentric, true, lt);
    let comp = model_lt::<f64>(&de, &ids, Moon::Geocentric, true, lt).with_compensation(true);
    let mdd = model_lt::<Dd>(&de, &ids, Moon::Geocentric, true, lt);
    let ic = InitialConditions::<f64>::from_header(&h, &prov).unwrap();
    let icd = InitialConditions::<Dd>::from_header(&h, &prov).unwrap();
    let (x, v) = m64.pack(ic.states(), ic.earth_moon()).unwrap();
    let (xd, vd) = mdd.pack(icd.states(), icd.earth_moon()).unwrap();
    let n = x.len();

    let mut a_mixed = vec![0.0; n];
    MixedForces::new(&m64)
        .acceleration(0.0, &x, &v, &mut a_mixed)
        .unwrap();
    let mut a_comp = vec![0.0; n];
    comp.acceleration(0.0, &x, &v, &mut a_comp).unwrap();
    assert_eq!(
        a_mixed.iter().map(|z| z.to_bits()).collect::<Vec<_>>(),
        a_comp.iter().map(|z| z.to_bits()).collect::<Vec<_>>()
    );

    let mut a_dd = vec![Dd::ZERO; n];
    mdd.acceleration(Dd::ZERO, &xd, &vd, &mut a_dd).unwrap();
    let mut a_mx = vec![Dd::ZERO; n];
    MixedForces::new(&m64)
        .acceleration(Dd::ZERO, &xd, &vd, &mut a_mx)
        .unwrap();
    let mut a_f = vec![Dd::ZERO; n];
    F64Forces::new(&m64)
        .acceleration(Dd::ZERO, &xd, &vd, &mut a_f)
        .unwrap();
    let low_words = a_mx.iter().filter(|z| z.to_parts().1 != 0.0).count();
    let bodies = n / 3;
    let rel = |a: &[Dd], i: usize| {
        let d = (0..3)
            .map(|k| (a[k * bodies + i] - a_dd[k * bodies + i]).to_f64().powi(2))
            .sum::<f64>()
            .sqrt();
        let s = (0..3)
            .map(|k| a_dd[k * bodies + i].to_f64().powi(2))
            .sum::<f64>()
            .sqrt();
        d / s
    };
    for (i, id) in ids.iter().enumerate() {
        println!(
            "  {:>4}: |a - a_DD|/|a|: MixedForces {:.2e}, F64Forces {:.2e}",
            id.0,
            rel(&a_mx, i),
            rel(&a_f, i)
        );
    }
    println!("MixedForces components with a non-zero low word: {low_words} of {n}");
    assert!(
        low_words > 0,
        "the compensation never reaches the double-double result"
    );
}
