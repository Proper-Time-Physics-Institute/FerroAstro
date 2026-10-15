// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung SR-a (PHY-007, NUM-015 R2, NUM-016): special-relativistic Kepler motion.

mod common;

use ferroastro_analytic::advance::{pn_unit, rate_arcsec_per_century};
use ferroastro_forces::Gauge;

/// Verifies: PHY-007, NUM-015, NUM-016
#[test]
fn sr_a_gives_one_sixth_of_gr() {
    let c = common::ctx();
    let r = c.setup.run(&c.rung("SR-a")).unwrap();
    common::show(&r);
    assert_eq!(r.gauge, Gauge::SpecialRelativistic);
    // NUM-016: the rung names its Hamiltonian.
    assert!(
        r.report.dynamics.contains("H = gamma m c^2 - mu m/r"),
        "{}",
        r.report.dynamics
    );
    c.check("PHY007-SRA-NUM-RATE", r.numerical_arcsec_per_century());
    // Analytic pi GM/(c^2 p) with PHY-004 inputs (SW92 a, e; GM_sun advance; GM_sun + GM_Me period).
    let el = c.scn.kepler_elements(c.de.au()).unwrap();
    let p = ferroastro_core::units::Length::from_km(el.a.km() * el.e.mul_add(-el.e, 1.0)).unwrap();
    let unit = pn_unit(c.de.gm_sun(), c.de.speed_of_light(), p).unwrap();
    let mu_total = c.de.gm_sun() + c.de.gm(ferroastro_constants::Body::Mercury);
    let period = ferroastro_analytic::advance::keplerian_period(mu_total, el.a).unwrap();
    let analytic = rate_arcsec_per_century(unit / period);
    c.check("PHY007-SRA-ANALYTIC-RATE", analytic);
    println!(
        "SR-a numerical - pi GM/(c^2 p) analytic = {:+.3e} arcsec/cy",
        r.numerical_arcsec_per_century() - analytic
    );
    // NUM-015 R2: the exact formula 2 pi [(1 - mu^2/c^2 h^2)^(-1/2) - 1] per orbit.
    c.check_num015(&r);
    println!(
        "exact / leading-order advance = {:.10} (expected 1 + O(GM/c^2 p))",
        r.analytic_per_orbit.radians() / c.setup.pn_unit().unwrap().radians()
    );
}
