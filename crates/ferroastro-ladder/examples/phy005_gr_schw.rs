// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung GR-SCHW numerical (PHY-005): 1PN Schwarzschild test particle (PPN isotropic/harmonic,
//! β = γ = 1) integrated with IAS15; rate from perihelion passages.
//!
//! `cargo run -p ferroastro-ladder --example phy005_gr_schw`

#[path = "common/mod.rs"]
mod common;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let r = c.run("GR-SCHW")?;
    c.check("PHY005-GR-NUM-RATE", r.numerical_arcsec_per_century())
}
