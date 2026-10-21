// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung LT (PHY-011): solar Lense–Thirring with the DE440 solar spin (GMS·MOISUN·ASUN²·OMGSUN),
//! orbit in the solar equator.
//!
//! `cargo run -p ferroastro-ladder --example phy011_lt`

#[path = "common/mod.rs"]
mod common;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let r = c.run("LT")?;
    c.check("PHY011-LT-NUM-RATE", r.numerical_arcsec_per_century())
}
