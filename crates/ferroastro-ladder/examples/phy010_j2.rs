// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung J2 (PHY-010): solar oblateness, Mercury's orbit in the solar equator (the geometry of the
//! planar oracle `3πJ₂R²/p²`), with the Park17 J2 = 2.25e-7 and with DE440's J2SUN.
//!
//! `cargo run -p ferroastro-ladder --example phy010_j2`

#[path = "common/mod.rs"]
mod common;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    for (id, vector) in [
        ("J2", "PHY010-J2-PARK-ANALYTIC-RATE"),
        ("J2-DE440", "PHY010-J2-DE440-ANALYTIC-RATE"),
    ] {
        let r = c.run(id)?;
        c.check(vector, r.analytic_arcsec_per_century())?;
        c.check(
            "PHY010-J2-NUM-MINUS-ANALYTIC",
            r.numerical_arcsec_per_century() - r.analytic_arcsec_per_century(),
        )?;
    }
    Ok(())
}
