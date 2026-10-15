// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-009: one coefficient of the general 1PN test-particle Lagrangian at a time. Per unit of
//! A the advance is 8 units of `πGM/(c²p)` per orbit (1 at A = 1/8); B gives 4, C 2, D 2.
//!
//! `cargo run -p ferroastro-ladder --example phy009_coefficients`

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
    let unit = c.setup.pn_unit()?.radians();
    for (vector, k) in [
        ("PHY009-UNITS-PER-A", PnCoefficients { a: 1.0, ..zero }),
        ("PHY009-UNITS-PER-B", PnCoefficients { b: 1.0, ..zero }),
    ] {
        let r = c
            .setup
            .run(&Rung::custom("PN", "PHY-009 example", Model::Lagrangian(k)))?;
        let units = r.fit.advance.radians() / unit;
        println!("{k:?}: {units:.9} units of pi GM/(c^2 p) per orbit");
        c.check(vector, units)?;
    }
    Ok(())
}
