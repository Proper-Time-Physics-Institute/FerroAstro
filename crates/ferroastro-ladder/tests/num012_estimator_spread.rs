// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! NUM-012: cross-estimator spread for the single-body rungs. Four estimators on the same
//! dynamics and initial state: perihelion passages (the rung estimator), the LRL eccentricity-
//! vector longitude (= osculating ϖ in the initial orbit plane), and the Park2017 estimator
//! (NUM-011) on 2-day samples in the initial orbit plane with a linear and with a quadratic fit.
//! The spread is printed as a table (the docs report it) and bounded by NUM012-ESTIMATOR-SPREAD.

mod common;

use ferroastro_core::time::SECONDS_PER_DAY;
use ferroastro_core::units::TimeSpan;

/// Verifies: NUM-012
#[test]
fn single_body_estimators_agree() {
    let c = common::ctx();
    let cadence = TimeSpan::from_seconds(2.0 * SECONDS_PER_DAY).unwrap();
    println!(
        "| rung | passages | LRL | Park2017 linear (jk sigma) | Park2017 quadratic | analytic | spread | IC hash |"
    );
    println!("|---|---|---|---|---|---|---|---|");
    let mut spreads = Vec::new();
    for id in ["SR-a", "SR-b/harmonic", "GR-SCHW", "J2", "LT"] {
        let s = c.setup.estimator_spread(&c.rung(id), cadence).unwrap();
        println!(
            "| {} | {:.9} | {:.9} | {:.9} ({:.1e}) | {:.9} | {:.9} | {:.2e} | {} |",
            s.rung,
            s.passages,
            s.lrl,
            s.park_linear,
            s.park_linear_jackknife,
            s.park_quadratic,
            s.analytic,
            s.spread(),
            s.ic_hash
        );
        spreads.push(s);
    }
    println!(
        "Park2017 input: {} samples every {} d per rung",
        spreads[0].samples, spreads[0].cadence_days
    );
    for s in &spreads {
        println!("{}:", s.rung);
        c.check("NUM012-ESTIMATOR-SPREAD", s.spread());
    }
}
