// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung SR-b (PHY-008): the velocity-dependent 1PN terms alone, in two gauges. 7/6 of GR in
//! isotropic/harmonic coordinates and 5/6 in areal coordinates: the split is gauge dependent and
//! not an observable; every output carries its gauge label.
//!
//! `cargo run -p ferroastro-ladder --example phy008_sr_b`

#[path = "common/mod.rs"]
mod common;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    for (id, vector) in [
        ("SR-b/harmonic", "PHY008-SRB-HARMONIC-RATE"),
        ("SR-b/areal", "PHY008-SRB-AREAL-RATE"),
    ] {
        let r = c.run(id)?;
        println!("  gauge: {}", r.gauge.label());
        c.check(vector, r.numerical_arcsec_per_century())?;
    }
    Ok(())
}
