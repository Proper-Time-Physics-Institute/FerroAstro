// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-009: each coefficient of L = ½v² + U + c⁻²[A v⁴ + B U v² + C U² + D U (n·v)²] switched on
//! alone advances the perihelion by 8A, 4B, 2C, 2D units of πGM/(c²p) per orbit.

mod common;

use ferroastro_forces::PnCoefficients;
use ferroastro_ladder::single_body::{Model, Rung};

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn units(c: &common::Ctx, k: PnCoefficients) -> f64 {
    let r = c
        .setup
        .run(&Rung::custom(
            "PHY-009",
            "coefficient study",
            Model::Lagrangian(k),
        ))
        .unwrap();
    r.fit.advance.radians() / c.setup.pn_unit().unwrap().radians()
}

/// Verifies: PHY-009
#[test]
fn each_coefficient_alone_gives_its_units() {
    let c = common::ctx();
    let zero = PnCoefficients {
        a: 0.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
    };
    for (vector, k) in [
        ("PHY009-UNITS-PER-A", PnCoefficients { a: 1.0, ..zero }),
        ("PHY009-UNITS-PER-B", PnCoefficients { b: 1.0, ..zero }),
        ("PHY009-UNITS-PER-C", PnCoefficients { c: 1.0, ..zero }),
        ("PHY009-UNITS-PER-D", PnCoefficients { d: 1.0, ..zero }),
        (
            "PHY009-UNITS-A-EIGHTH",
            PnCoefficients {
                a: 1.0 / 8.0,
                ..zero
            },
        ),
    ] {
        let u = units(&c, k);
        println!("{k:?}: {u:.9} units of pi GM/(c^2 p) per orbit");
        c.check(vector, u);
    }
}
