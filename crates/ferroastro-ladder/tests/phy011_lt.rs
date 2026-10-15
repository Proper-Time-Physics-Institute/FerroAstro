// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung LT (PHY-011, NUM-015 R3b): solar Lense–Thirring with the DE440 spin, equatorial orbit.

mod common;

use ferroastro_analytic::advance::{lense_thirring_rate, rate_arcsec_per_century};
use ferroastro_core::units::Angle;
use ferroastro_ladder::single_body::de440_solar_spin;

/// Verifies: PHY-011, NUM-015
#[test]
fn lense_thirring_gives_minus_0_0020() {
    let c = common::ctx();
    let gs = de440_solar_spin(&c.de).unwrap();
    println!(
        "G S (DE440: GMS * MOISUN * ASUN^2 * OMGSUN) = {:.6e} km^5/s^3",
        gs.km5_per_s3()
    );
    let el = c.scn.kepler_elements(c.de.au()).unwrap();
    let analytic = lense_thirring_rate(
        gs,
        c.de.speed_of_light(),
        c.de.ppn_gamma(),
        el.a,
        el.e,
        Angle::ZERO,
    )
    .unwrap();
    c.check("PHY011-LT-ANALYTIC-RATE", rate_arcsec_per_century(analytic));
    let r = c.setup.run(&c.rung("LT")).unwrap();
    common::show(&r);
    c.check("PHY011-LT-NUM-RATE", r.numerical_arcsec_per_century());
    c.check_num015(&r);
}
