// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung GR-SCHW numerical (PHY-005) and its NUM-015 per-orbit check, PPN isotropic/harmonic gauge
//! (NUM-017).

mod common;

use ferroastro_forces::Gauge;
use ferroastro_precession::analytic::schwarzschild_advance;

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
