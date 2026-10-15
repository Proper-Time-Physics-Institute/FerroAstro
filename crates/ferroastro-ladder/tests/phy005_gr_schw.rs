// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung GR-SCHW numerical (PHY-005) and its NUM-015 per-orbit check, PPN isotropic/harmonic gauge
//! (NUM-017).

mod common;

use ferroastro_analytic::advance::schwarzschild_advance;
use ferroastro_forces::Gauge;

/// Verifies: PHY-005, NUM-015, NUM-017
#[test]
fn gr_schw_numerical_reproduces_42_980() {
    let c = common::ctx();
    let r = c.setup.run(&c.rung("GR-SCHW")).unwrap();
    common::show(&r);
    assert_eq!(
        r.gauge,
        Gauge::PpnIsotropicHarmonic,
        "NUM-017: 1PN EOM in PPN harmonic gauge"
    );
    let num = r.numerical_arcsec_per_century();
    c.check("PHY005-GR-NUM-RATE", num);
    // PHY-005: |numerical - PHY-004 analytic| <= 0.001, with PHY-004's own inputs and period.
    let el = c.scn.kepler_elements(c.de.au()).unwrap();
    let gm_sun = c.de.gm_sun();
    let gm_me = c.de.gm(ferroastro_constants::Body::Mercury);
    let phy004 = schwarzschild_advance(gm_sun, c.de.speed_of_light(), el.a, el.e, gm_sun + gm_me)
        .unwrap()
        .arcsec_per_century();
    println!(
        "PHY-004 analytic {phy004:.7} arcsec/cy; numerical - analytic = {:+.3e}",
        num - phy004
    );
    c.check("PHY005-GR-NUM-MINUS-ANALYTIC", num - phy004);
    c.check_num015(&r);
    // Park17 Table 3 (published, numerical-ephemeris estimate in the mean-orbit frame): reported.
    let park = c.value("PARK17-GE-RATE");
    println!(
        "Park17 Table 3 GE {park}: numerical - Park17 = {:+.5} arcsec/cy (reported, not asserted)",
        num - park
    );
}

/// GR-SCHW integrated in f64 and in double-double from the same SW92 initial state (DEP-007):
/// both meet PHY-005 and they agree within the WP7 bound.
///
/// Verifies: PHY-005, DEP-007, NUM-006
#[test]
fn gr_schw_in_double_double_matches_f64() {
    use ferroastro_real::DoubleDouble;
    let c = common::ctx();
    let rung = c.rung("GR-SCHW");
    let f = c.setup.run(&rung).unwrap();
    let d = c.setup.run_in::<DoubleDouble>(&rung).unwrap();
    let (rf, rd) = (
        f.numerical_arcsec_per_century(),
        d.numerical_arcsec_per_century(),
    );
    println!(
        "GR-SCHW f64 {rf:.12} arcsec/cy ({} steps, {:.2} s); double-double {rd:.12} arcsec/cy \
         ({} steps, {:.2} s); f64 - DD = {:+.3e}; dynamics: {}",
        f.stats.steps,
        f.wall_seconds,
        d.stats.steps,
        d.wall_seconds,
        rf - rd,
        d.report.dynamics
    );
    c.check("PHY005-GR-NUM-RATE", rf);
    c.check("PHY005-GR-NUM-RATE", rd);
    c.check("PHY005-F64-DD-AGREEMENT", rf - rd);
}
