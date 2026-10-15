// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-013(a): isolated single-body terms. Each term (GR-SCHW, J2 at the Park17 J2 in an
//! equatorial orbit, LT in an equatorial orbit) is toggled on against a Newtonian test-particle
//! run from **identical initial conditions** (asserted by IC hash), and the differential rate is
//! compared with the analytic rate within 1e-4 ″/cy.
//!
//! Comparison basis (NUM-013 notes: "SHALL be documented"): heliocentric test particle about a
//! fixed Sun (the Sun + Mercury isolation), μ = GM☉ for every term, SW92 J2000 initial state,
//! perihelion-passage estimator over 1000 anomalistic orbits; analytic rate = analytic per-orbit
//! advance / Keplerian period of the initial osculating orbit.

mod common;

use ferroastro_forces::PnCoefficients;
use ferroastro_ladder::single_body::{Model, Rung};

/// Verifies: NUM-013
#[test]
fn single_body_terms_isolate_to_their_analytic_rates() {
    let c = common::ctx();
    let newton = c
        .setup
        .run(&Rung::custom(
            "N-TP",
            "Newtonian test particle",
            Model::Lagrangian(PnCoefficients {
                a: 0.0,
                b: 0.0,
                c: 0.0,
                d: 0.0,
            }),
        ))
        .unwrap();
    println!(
        "comparison basis: Newtonian test particle, IC hash {}, rate {:.3e} arcsec/cy (numerical error; true 0)",
        newton.ic_hash,
        newton.numerical_arcsec_per_century()
    );
    for id in ["GR-SCHW", "J2", "LT"] {
        let r = c.setup.run(&c.rung(id)).unwrap();
        assert_eq!(
            r.ic_hash, newton.ic_hash,
            "{id}: differential runs must start from identical initial conditions"
        );
        let delta = r.numerical_arcsec_per_century() - newton.numerical_arcsec_per_century();
        println!(
            "NUM-013(a) {id} [{}]: differential {delta:.9} arcsec/cy, analytic {:.9}, IC hash {}",
            r.gauge.label(),
            r.analytic_arcsec_per_century(),
            r.ic_hash
        );
        c.check(
            "NUM013-ISOLATION-DIFF",
            delta - r.analytic_arcsec_per_century(),
        );
    }
}
