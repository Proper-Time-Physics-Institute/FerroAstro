// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung PPN (PHY-006): differential runs from identical initial conditions. The asserted quantity
//! is the ratio rate(β, γ)/rate(1, 1) against (2 + 2γ − β)/3 (red-team §1 correction), plus the
//! two example shifts of the requirement.

mod common;

use ferroastro_ladder::single_body::{Model, Rung};
use ferroastro_precession::analytic::ppn_scaling;

#[expect(clippy::unwrap_used, reason = "test helper: failure is a test failure")]
fn rate(c: &common::Ctx, beta: f64, gamma: f64) -> f64 {
    let rung = Rung::custom("PPN", "PPN study", Model::Schwarzschild { beta, gamma });
    let r = c.setup.run(&rung).unwrap();
    c.check_num015(&r);
    r.numerical_arcsec_per_century()
}

/// Verifies: PHY-006, NUM-015
#[test]
fn ppn_rate_scales_as_two_plus_two_gamma_minus_beta_over_three() {
    let c = common::ctx();
    let base = rate(&c, 1.0, 1.0);
    // Test points at the edge of PHY-006's range |beta - 1|, |gamma - 1| <= 1e-3.
    let d = 1.0e-3; // lint: allow-literal(PHY-006 range bound, from the requirement text)
    for (beta, gamma) in [
        (1.0 + d, 1.0),
        (1.0 - d, 1.0),
        (1.0, 1.0 + d),
        (1.0, 1.0 - d),
        (1.0 + d, 1.0 - d),
    ] {
        let ratio = rate(&c, beta, gamma) / base;
        let want = ppn_scaling(beta, gamma);
        println!(
            "beta = {beta}, gamma = {gamma}: ratio {ratio:.12}, (2+2gamma-beta)/3 = {want:.12}"
        );
        c.check("PHY006-RATIO-REL-DEV", ratio / want - 1.0);
    }
    // Example shifts: beta at Park17's sigma, gamma at the PHY-006-notes Cassini sigma.
    let sb = c.value("PARK17-BETA-SIGMA");
    c.check("PHY006-BETA-SHIFT", rate(&c, 1.0 + sb, 1.0) - base);
    let sg = 2.3e-5; // lint: allow-literal(test point from PHY-006 notes: Cassini sigma_gamma, see vector PHY006-GAMMA-SHIFT)
    c.check("PHY006-GAMMA-SHIFT", rate(&c, 1.0, 1.0 + sg) - base);
}
