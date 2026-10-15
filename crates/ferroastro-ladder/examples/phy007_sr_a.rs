// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung SR-a (PHY-007): special-relativistic Kepler, `H = γmc² − μm/r`, gives GR/6. The fraction
//! belongs to this model only (NUM-016).
//!
//! `cargo run -p ferroastro-ladder --example phy007_sr_a`

#[path = "common/mod.rs"]
mod common;

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let r = c.run("SR-a")?;
    c.check("PHY007-SRA-NUM-RATE", r.numerical_arcsec_per_century())
}
