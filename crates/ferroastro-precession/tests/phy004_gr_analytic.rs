// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung GR-SCHW analytic (PHY-004): 6πGM/(c²a(1−e²)) with DE440 GM☉, c, au and SW92 a, e.

mod common;

use ferroastro_constants::De440;
use ferroastro_precession::analytic::schwarzschild_advance;
use ferroastro_vectors::VectorSet;

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn check(vs: &VectorSet, id: &str, measured: f64) {
    let v = vs.get(id).unwrap();
    let expected = v.value().unwrap();
    let res = v.check(measured);
    println!(
        "{id}: measured {measured:e} {u}; expected {expected} {u}; |diff| = {:e}; allowed {:e}",
        (measured - expected).abs(),
        v.tolerance.allowed(expected),
        u = v.unit
    );
    res.unwrap();
}

/// Verifies: PHY-004
#[test]
fn gr_schw_analytic_rate_from_de440_and_sw92() {
    let de = De440::load().unwrap();
    let vs = VectorSet::load_workspace().unwrap();
    let el = common::sw92_elements(&vs, &de);
    let (gm_sun, gm_me) = common::gms(&de);
    // Caveat C7, documented: GM_sun in the advance (Schwarzschild test-particle formula, as the
    // PHY-004 statement says), GM_sun + GM_Mercury in the period (as its acceptance says).
    let adv =
        schwarzschild_advance(gm_sun, de.speed_of_light(), el.a, el.e, gm_sun + gm_me).unwrap();
    println!(
        "GR-SCHW analytic: frame-independent secular rate; a = {} km, e = {}, \
         P = {} d, constants DE440 (header.440 sha256 {})",
        el.a.km(),
        el.e,
        adv.period.seconds() / ferroastro_core::time::SECONDS_PER_DAY,
        de.table().source().sha256
    );
    check(&vs, "PHY004-GR-PER-ORBIT", adv.per_orbit.radians());
    check(&vs, "PHY004-ORBITS-PER-CY", adv.orbits_per_century);
    check(&vs, "PHY004-GR-RATE", adv.arcsec_per_century());

    // C7 alternative: GM_sun + GM_Mercury in the advance too. Reported, and must also pass.
    let alt = schwarzschild_advance(
        gm_sun + gm_me,
        de.speed_of_light(),
        el.a,
        el.e,
        gm_sun + gm_me,
    )
    .unwrap();
    println!(
        "C7 alternative (mu = GM_sun + GM_Mercury in the advance): {:.7} arcsec/cy (shift {:e})",
        alt.arcsec_per_century(),
        alt.arcsec_per_century() - adv.arcsec_per_century()
    );
    check(&vs, "PHY004-GR-RATE", alt.arcsec_per_century());

    // Published cross-check (journal/requirement, not the acceptance): Park17 Table 3.
    let park = vs.get("PARK17-GE-RATE").unwrap().value().unwrap();
    println!(
        "Park17 Table 3 GE rate {park} arcsec/cy: analytic - Park17 = {:+.4} arcsec/cy \
         (mean SW92 elements vs Park's numerical mean-orbit-frame estimate)",
        adv.arcsec_per_century() - park
    );
}

/// Domain errors are coded, never panics (API-002).
///
/// Verifies: PHY-004, API-002
#[test]
fn analytic_rejects_unbound_orbits() {
    let de = De440::load().unwrap();
    let (gm_sun, _) = common::gms(&de);
    let err = schwarzschild_advance(gm_sun, de.speed_of_light(), de.au(), 1.0, gm_sun).unwrap_err();
    assert_eq!(err.code(), ferroastro_core::ErrorCode::OrbitDomain);
}
