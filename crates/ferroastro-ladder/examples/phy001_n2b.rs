// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung N-2B (PHY-001): Newtonian motion shows no perihelion precession; any fitted rate is
//! integrator error. This example uses the Mercury **test particle** (all 1PN coefficients
//! zero); the two-body Sun + Mercury version with the 1000-yr span is the PHY-001 test.
//!
//! `cargo run -p ferroastro-ladder --example phy001_n2b`

#[path = "common/mod.rs"]
mod common;

use ferroastro_forces::PnCoefficients;
use ferroastro_ladder::single_body::{Model, Rung};

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let zero = PnCoefficients {
        a: 0.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    let r = c.setup.run(&Rung::custom(
        "N-TP",
        "N-2B: Newtonian test particle",
        Model::Lagrangian(zero),
    ))?;
    println!("{}", r.report);
    println!(
        "  fitted {:e} arcsec/cy: NUMERICAL ERROR (the true rate is exactly 0)",
        r.numerical_arcsec_per_century()
    );
    c.check("PHY001-NEWTON-RATE", r.numerical_arcsec_per_century())
}
