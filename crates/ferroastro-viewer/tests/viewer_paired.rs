// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The paired-difference jack-knife of a contribution (DESIGN §6.4): the σ of
//! `ϖ̇(S) − ϖ̇(S∖X)` is a jack-knife on the **difference** of the two legs (same initial
//! conditions, same estimator, the same time block deleted from both), computed by the engine
//! from the legs' passage rows through the `pair` calls the page uses.
//!
//! # Oracles
//! - The independent combination `√(σ_S² + σ_W²)` of each leg's own jack-knife, which the paired σ
//!   must not exceed for legs that share initial conditions (their residuals are correlated).
//! - For the uniform γ0 centring with velocity-independent (Newtonian) forces, both legs follow
//!   the same heliocentric orbit to round-off (PHY-001, `viewer_conformal.rs`), so the paired σ is
//!   below `ABS_UNIFORM = 1e-6 ″/cy`, the PHY-001 bound, while the independent combination is the
//!   fit floor of a 100-yr run (about 1e-3 ″/cy, DESIGN E1).
//!
//! All tests are PR-tier; none is `#[ignore]`.

#![expect(
    clippy::expect_used,
    reason = "test helpers: a missing pair record is a test failure"
)]

mod common;

use common::{WHFAST, configured, run_to_end};
use ferroastro_viewer::config::{Param, bits};
use ferroastro_viewer::{FAV_E_ARG, FAV_OK, PAIR_STRIDE, PASSAGES_STRIDE, Viewer};

const CENTURY: u32 = 100;
/// Exaggerated Sun-centred γ0 (cm⁻¹), as in `viewer_conformal.rs` (about −60 ″/cy).
const GAMMA0: f64 = 1e-27; // lint: allow-literal(exaggerated gamma0, see viewer_conformal.rs)
/// Absolute bound for the uniform centring, ″/cy (PHY-001's bound).
const ABS_UNIFORM: f64 = 1e-6; // lint: allow-literal(PHY-001 integrator bound)

/// A whole run's passage rows and epoch.
fn passages(mask: u32, params: &[(Param, f64)]) -> (Vec<f64>, f64) {
    let mut v = configured(mask, params, CENTURY);
    run_to_end(&mut v, 5.0);
    (v.passages().to_vec(), v.epoch_jd())
}

/// The pair record of leg `a` minus leg `b`, pushed row by row as the page does.
fn pair(a: (&[f64], u32), b: (&[f64], u32), epoch_jd: f64) -> [f64; PAIR_STRIDE] {
    let mut p = Viewer::new();
    assert_eq!(p.pair_clear(), FAV_OK);
    for (leg, rows) in [(0, a.0), (1, b.0)] {
        for r in rows.chunks_exact(PASSAGES_STRIDE) {
            let [t, lon, ..] = r else {
                unreachable!("a passage row has {PASSAGES_STRIDE} values")
            };
            assert_eq!(p.pair_push(leg, *t, *lon), FAV_OK, "{}", p.error());
        }
    }
    // Flags: FIT_PERIODIC = 1 (the default), System mode.
    assert_eq!(p.pair_fit(a.1, b.1, 1, epoch_jd), FAV_OK, "{}", p.error());
    <[f64; PAIR_STRIDE]>::try_from(p.pair_record()).expect("a pair record")
}

/// The pair calls refuse a bad leg, a non-finite row and bad flags.
///
/// Verifies: API-004
#[test]
fn pair_calls_check_their_arguments() {
    let mut p = Viewer::new();
    assert_eq!(p.pair_push(2, 0.0, 0.0), FAV_E_ARG);
    assert_eq!(p.pair_push(0, f64::NAN, 0.0), FAV_E_ARG);
    let j2000 = ferroastro_core::time::J2000_JD;
    assert_eq!(p.pair_fit(0, 0, 4, j2000), FAV_E_ARG);
    assert!(p.pair_fit(0, 0, 1, j2000) < 0, "no passages");
    assert_eq!(p.pair_record().len(), 0);
}

/// Conformal gravity (comparison model) on and off over Venus and Jupiter: uniform γ0 gives a
/// paired σ at round-off, and Sun-centred γ0 a paired σ below the independent combination.
///
/// Verifies: PHY-020, NUM-011
#[test]
fn paired_sigma_resolves_the_conformal_row() {
    if !common::system_mode_or_skip("paired_sigma_resolves_the_conformal_row") {
        return;
    }
    let base = bits::VENUS | bits::JUPITER;
    let (off, jd) = passages(base, &[WHFAST]);
    let on = base | bits::CONFORMAL;
    let (uniform, jd_u) = passages(on, &[WHFAST, (Param::CgGamma0Centre, 0.0)]);
    assert_eq!(jd.to_bits(), jd_u.to_bits(), "one epoch for both legs");
    let r = pair((&uniform, on), (&off, base), jd);
    println!("uniform: {r:?}");
    assert!(r[0].abs() < ABS_UNIFORM, "{r:?}");
    assert!(r[1] < ABS_UNIFORM, "paired sigma {r:?}");
    assert!(r[1] <= r[2], "{r:?}");
    let (sun, _) = passages(
        on,
        &[
            WHFAST,
            (Param::CgGamma0Centre, 1.0),
            (Param::CgGamma0, GAMMA0),
        ],
    );
    let r = pair((&sun, on), (&off, base), jd);
    println!("Sun-centred: {r:?}");
    assert!(r[0] < 0.0 && r[1] <= r[2], "{r:?}");
    assert!(r[0].abs() > 3.0 * r[1], "resolved: {r:?}");
}
