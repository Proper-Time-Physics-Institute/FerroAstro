// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Physics of the viewer (DESIGN §8): the Kepler-only baseline, GR, PPN scaling, solar J2 and
//! Lense–Thirring contributions, single-planet rates, first-order additivity, the SR rungs and
//! engine agreement, each against `tests/vectors/viewer.toml`. PR tier (100–200-yr spans).

mod common;

use common::{IAS15, LAB, WHFAST, check, rate, vectors};
use ferroastro_viewer::config::{Param, bits};

const CENTURY: u32 = 100;

/// Verifies: PHY-001, API-004
#[test]
fn e1_kepler_only_baseline_does_not_precess() {
    if !common::system_mode_or_skip("e1_kepler_only_baseline_does_not_precess") {
        return;
    }
    let vs = vectors();
    for engine in [WHFAST, IAS15] {
        check(&vs, "VIEWER-E1-KEPLER-RATE", rate(0, &[engine], CENTURY));
    }
}

/// Verifies: PHY-004, NUM-004, API-004
#[test]
fn e2_gr_potential_whfast_gives_43_arcsec_per_century() {
    if !common::system_mode_or_skip("e2_gr_potential_whfast_gives_43_arcsec_per_century") {
        return;
    }
    check(
        &vectors(),
        "VIEWER-E2-WH-GR-RATE",
        rate(bits::GR_1PN, &[WHFAST], CENTURY),
    );
}

/// Verifies: PHY-005, API-004
#[test]
fn e2_rung_lab_gr_gives_43_arcsec_per_century() {
    check(
        &vectors(),
        "VIEWER-E2-LAB-GR-RATE",
        rate(0, &[LAB], CENTURY),
    );
}

/// Verifies: PHY-006, API-004
#[test]
fn e3_ppn_scaling_in_the_rung_lab() {
    let vs = vectors();
    let gr = rate(0, &[LAB], CENTURY);
    for (beta, gamma) in [(0.0, 2.0), (2.0, 1.0), (0.5, 0.0)] {
        let r = rate(
            0,
            &[LAB, (Param::PpnBeta, beta), (Param::PpnGamma, gamma)],
            CENTURY,
        );
        let scale = (2.0f64.mul_add(gamma, 2.0) - beta) / 3.0;
        check(
            &vs,
            "VIEWER-E3-PPN-SCALING-DEV",
            (r / (gr * scale) - 1.0).abs(),
        );
    }
}

/// Verifies: PHY-006, API-004
#[test]
fn e3_ppn_scaling_in_system_mode_ias15() {
    if !common::system_mode_or_skip("e3_ppn_scaling_in_system_mode_ias15") {
        return;
    }
    let vs = vectors();
    let gr = rate(bits::GR_1PN, &[IAS15], CENTURY);
    let r = rate(
        bits::GR_1PN,
        &[IAS15, (Param::PpnBeta, 0.0), (Param::PpnGamma, 2.0)],
        CENTURY,
    );
    check(
        &vs,
        "VIEWER-E3-PPN-SCALING-DEV",
        (r / (gr * 2.0) - 1.0).abs(),
    );
}

/// Verifies: PHY-010, API-004
#[test]
fn e4_solar_j2_contribution() {
    if !common::system_mode_or_skip("e4_solar_j2_contribution") {
        return;
    }
    let with = rate(bits::GR_1PN | bits::SUN_J2, &[WHFAST], CENTURY);
    let without = rate(bits::GR_1PN, &[WHFAST], CENTURY);
    check(&vectors(), "VIEWER-E4-J2-PREMOVE", with - without);
}

/// Verifies: PHY-011, API-004
#[test]
fn e5_lense_thirring_contribution() {
    if !common::system_mode_or_skip("e5_lense_thirring_contribution") {
        return;
    }
    let years = 2 * CENTURY;
    let with = rate(bits::GR_1PN | bits::LENSE_THIRRING, &[IAS15], years);
    let without = rate(bits::GR_1PN, &[IAS15], years);
    check(&vectors(), "VIEWER-E5-LT-PREMOVE", with - without);
}

/// Verifies: PHY-012, API-004
#[test]
fn e6_single_planet_rates() {
    if !common::system_mode_or_skip("e6_single_planet_rates") {
        return;
    }
    let vs = vectors();
    let kepler = rate(0, &[WHFAST], CENTURY);
    check(
        &vs,
        "VIEWER-E6-VENUS-100YR",
        rate(bits::VENUS, &[WHFAST], CENTURY) - kepler,
    );
    check(
        &vs,
        "VIEWER-E6-JUPITER-100YR",
        rate(bits::JUPITER, &[WHFAST], CENTURY) - kepler,
    );
}

/// Verifies: NUM-013, API-004
#[test]
fn toggles_add_to_first_order() {
    if !common::system_mode_or_skip("toggles_add_to_first_order") {
        return;
    }
    let k = rate(0, &[WHFAST], CENTURY);
    let v = rate(bits::VENUS, &[WHFAST], CENTURY);
    let g = rate(bits::GR_1PN, &[WHFAST], CENTURY);
    let vg = rate(bits::VENUS | bits::GR_1PN, &[WHFAST], CENTURY);
    check(&vectors(), "VIEWER-ADDITIVITY-VENUS-GR", vg - v - g + k);
}

/// Verifies: PHY-007, PHY-008, API-004
#[test]
fn e9_sr_rungs_in_the_rung_lab() {
    let vs = vectors();
    for (rung, id) in [
        (1.0, "VIEWER-E9-SRA-RATE"),
        (2.0, "VIEWER-E9-SRB-HARMONIC-RATE"),
        (3.0, "VIEWER-E9-SRB-AREAL-RATE"),
    ] {
        check(&vs, id, rate(0, &[LAB, (Param::Rung, rung)], CENTURY));
    }
}

/// Verifies: NUM-004, API-004
#[test]
fn e11_whfast_and_ias15_agree() {
    if !common::system_mode_or_skip("e11_whfast_and_ias15_agree") {
        return;
    }
    let mask = bits::VENUS | bits::GR_1PN;
    let wh = rate(mask, &[WHFAST], CENTURY);
    let ias = rate(mask, &[IAS15], CENTURY);
    check(&vectors(), "VIEWER-E11-ENGINE-AGREEMENT", (wh - ias).abs());
}

/// Verifies: PHY-014, API-004
#[test]
#[ignore = "nightly tier: the full model with IAS15 over 100 yr"]
fn nightly_e7_full_model_over_a_century() {
    if !common::system_mode_or_skip("nightly_e7_full_model_over_a_century") {
        return;
    }
    let f = common::fit_of(bits::DEFAULT, &[], CENTURY);
    println!(
        "full model: {} +- {} arcsec/cy (jack-knife), {} passages",
        f[0], f[2], f[5]
    );
    check(&vectors(), "VIEWER-E7-FULL-100YR", *f.first().unwrap());
}
