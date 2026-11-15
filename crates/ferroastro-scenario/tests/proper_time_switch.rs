// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The optional `forces.proper_time_kernel` switch of the Proper Time (Gill) comparison model
//! (ADR-0016 decision D6, PHY-021, CI-002): parsing, default off, refusal beside the GR 1PN
//! switches and what the scenario digest covers.

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_scenario::{SWITCHES, ScenarioSpec};
use std::path::PathBuf;

fn base() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ci/scenarios/sun-mercury-newton-header440.toml"),
    )
    .unwrap()
}

fn with_forces(extra: &str) -> String {
    base().replace("newton = true", &format!("newton = true\n{extra}"))
}

/// The switch is listed, off by default (and then not serialised, so the digest is unchanged),
/// reported as requested when on, and changes the digest when on.
///
/// Verifies: PHY-021, CI-002
#[test]
fn proper_time_switch_is_off_by_default_and_covered_by_the_digest() {
    assert!(SWITCHES.contains(&"proper_time_kernel"));
    let off = ScenarioSpec::parse(&base()).unwrap();
    assert!(!off.forces.proper_time_kernel);
    assert!(
        !serde_json::to_string(&off)
            .unwrap()
            .contains("proper_time_kernel")
    );
    let explicit_off = ScenarioSpec::parse(&with_forces("proper_time_kernel = false")).unwrap();
    assert_eq!(off.digest(), explicit_off.digest());
    let on = ScenarioSpec::parse(&with_forces("proper_time_kernel = true")).unwrap();
    assert!(on.forces.proper_time_kernel);
    assert!(on.forces.requested().contains(&"proper_time_kernel"));
    assert_ne!(off.digest(), on.digest());
    // It combines with the Newtonian-order switches (solar J2) and the other comparison model.
    assert!(
        ScenarioSpec::parse(&with_forces(
            "proper_time_kernel = true\nsun_j2 = true\nconformal = { gamma_star = 1.0 }"
        ))
        .is_ok()
    );
}

/// It replaces the GR 1PN sector, so it is refused (FA-E1203, naming the field and the other
/// switch) beside `gr_1pn_central`, `gr_eih`, `gr_potential` and `lense_thirring`.
///
/// Verifies: PHY-021, CI-002
#[test]
fn proper_time_switch_is_refused_beside_the_gr_switches() {
    for other in ["gr_1pn_central", "gr_eih", "gr_potential", "lense_thirring"] {
        let e = ScenarioSpec::parse(&with_forces(&format!(
            "proper_time_kernel = true\n{other} = true"
        )))
        .unwrap_err();
        println!("{other}: {e}");
        assert_eq!(e.code().to_string(), "FA-E1203", "{other}");
        let msg = e.to_string();
        assert!(msg.contains("forces.proper_time_kernel"), "{msg}");
        assert!(msg.contains(other), "{msg}");
    }
}
