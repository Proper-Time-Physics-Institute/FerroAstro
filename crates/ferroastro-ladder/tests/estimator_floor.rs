// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Noise floor of the perihelion-passage estimator: a Newtonian test particle (all 1PN
//! coefficients zero) has an exactly zero advance, so whatever the 1000-orbit fit returns is
//! integrator + estimator error. It must sit below the NUM-015 absolute floor, otherwise that
//! floor would hide real disagreements.

mod common;

use ferroastro_forces::PnCoefficients;
use ferroastro_ladder::single_body::{Model, Rung};

/// Verifies: PHY-001, NUM-015
#[test]
fn newtonian_test_particle_has_no_advance() {
    let c = common::ctx();
    let zero = PnCoefficients {
        a: 0.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    let r = c
        .setup
        .run(&Rung::custom(
            "N-TP",
            "Newtonian test particle",
            Model::Lagrangian(zero),
        ))
        .unwrap();
    common::show(&r);
    let adv = r.fit.advance.radians();
    println!(
        "Newtonian per-orbit advance {adv:.3e} rad (formal 1-sigma {:.3e}); NOTE: the true value is exactly 0, this is numerical error",
        r.fit.advance_stderr.radians()
    );
    c.check("NUM015-ABS-FLOOR", adv);
    c.check("PHY001-NEWTON-RATE", r.numerical_arcsec_per_century());
}
