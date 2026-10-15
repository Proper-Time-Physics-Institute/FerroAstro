// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung PPN (PHY-006): the GR-SCHW rate scales as `(2 + 2γ − β)/3`, from differential runs that
//! share their initial conditions (same IC hash).
//!
//! `cargo run -p ferroastro-ladder --example phy006_ppn`

#[path = "common/mod.rs"]
mod common;

use ferroastro_analytic::advance::ppn_scaling;
use ferroastro_ladder::single_body::{Model, Rung};

pub(crate) fn main() -> common::Res<()> {
    let c = common::ctx()?;
    let rate = |beta: f64, gamma: f64| -> common::Res<(f64, String)> {
        let r = c.setup.run(&Rung::custom(
            "PPN",
            "PPN example",
            Model::Schwarzschild { beta, gamma },
        ))?;
        Ok((r.numerical_arcsec_per_century(), r.ic_hash))
    };
    let (base, h0) = rate(1.0, 1.0)?;
    // A test point inside PHY-006's range |gamma - 1| <= 1e-3 (the range bound itself).
    let gamma = 1.0 + 1.0e-3; // lint: allow-literal(PHY-006 range bound, from the requirement text)
    let (shifted, h1) = rate(1.0, gamma)?;
    if h0 != h1 {
        return Err(format!("IC hashes differ: {h0} vs {h1}").into());
    }
    let ratio = shifted / base;
    println!(
        "gamma = {gamma}: rate {shifted:.9} vs {base:.9} arcsec/cy; ratio {ratio:.12}, \
         (2+2gamma-beta)/3 = {:.12}; IC hash {h0}",
        ppn_scaling(1.0, gamma)
    );
    c.check(
        "PHY006-RATIO-REL-DEV",
        ratio / ppn_scaling(1.0, gamma) - 1.0,
    )
}
