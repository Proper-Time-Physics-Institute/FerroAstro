// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung J2 (PHY-010, NUM-015 R1): equatorial (planar oracle) and inclined (Park17 Eq. 3) orbits.

mod common;

use ferroastro_analytic::advance::{j2_planar_advance, keplerian_period, rate_arcsec_per_century};
use ferroastro_core::units::Length;

/// Verifies: PHY-010
#[test]
fn j2_analytic_values_with_phy004_inputs() {
    let c = common::ctx();
    let el = c.scn.kepler_elements(c.de.au()).unwrap();
    let p = Length::from_km(el.a.km() * el.e.mul_add(-el.e, 1.0)).unwrap();
    let mu_total = c.de.gm_sun() + c.de.gm(ferroastro_constants::Body::Mercury);
    let period = keplerian_period(mu_total, el.a).unwrap();
    for (vector, j2) in [
        ("PHY010-J2-PARK-ANALYTIC-RATE", c.value("PARK17-J2SUN")),
        ("PHY010-J2-DE440-ANALYTIC-RATE", c.de.sun_j2()),
    ] {
        let adv = j2_planar_advance(j2, c.de.sun_radius(), p).unwrap();
        c.check(vector, rate_arcsec_per_century(adv / period));
    }
}

/// Verifies: PHY-010, NUM-015
#[test]
fn j2_numerical_matches_planar_oracle_in_equatorial_geometry() {
    let c = common::ctx();
    for id in ["J2", "J2-DE440"] {
        let r = c.setup.run(&c.rung(id)).unwrap();
        common::show(&r);
        let max_incl = 1e-12; // lint: allow-literal(round-off bound on the pole/orbit-normal angle, rad)
        assert!(
            r.inclination.radians().abs() < max_incl,
            "planar oracle needs an equatorial orbit"
        );
        assert!(
            r.report.dynamics.contains("pole ["),
            "PHY-010: pole orientation recorded in the output"
        );
        c.check(
            "PHY010-J2-NUM-MINUS-ANALYTIC",
            r.numerical_arcsec_per_century() - r.analytic_arcsec_per_century(),
        );
        c.check_num015(&r);
    }
}

/// Inclined check: solar equator tilted by Park17's 3.4°; oracle Park17 Eq. 3 (rate along the
/// orbit plane), reported against the planar value it reduces.
///
/// Verifies: PHY-010, NUM-015
#[test]
fn j2_numerical_matches_park17_eq3_for_inclined_orbit() {
    let c = common::ctx();
    let r = c.setup.run(&c.rung("J2-INCL")).unwrap();
    common::show(&r);
    let tilt = c.value("PARK17-SOLAR-EQUATOR-TILT");
    let incl = r.inclination.radians().to_degrees();
    println!("pole-to-orbit-normal angle {incl:.12} deg (scenario tilt {tilt} deg)");
    assert!((incl - tilt).abs() < 1e-9); // lint: allow-literal(round-off bound, degrees)
    c.check_num015(&r);
    let planar = c.setup.run(&c.rung("J2")).unwrap();
    println!(
        "inclined / planar = {:.6} (Park17 Eq. 3 factor 1 - 1.5 sin^2 i = {:.6})",
        r.numerical_arcsec_per_century() / planar.numerical_arcsec_per_century(),
        (-1.5 * tilt.to_radians().sin()).mul_add(tilt.to_radians().sin(), 1.0) // lint: allow-literal(Park17 Eq. 3 geometric factor 3/2)
    );
}
