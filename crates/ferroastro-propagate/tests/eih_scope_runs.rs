// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The scenario key `forces.eih_scope` (maintainer decision 2026-11-12; NUM-009): refused without
//! `gr_eih`; `all` is the default and leaves the scenario digest and model string unchanged;
//! `majors` is recorded in the scenario digest (and so in checkpoints) and in the model string.
//! Needs the cached header.440 (CI-005).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test: a failed unwrap is a test failure"
)]

use ferroastro_constants::header440::Header;
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::{EihScope, ScenarioSpec};
use std::path::PathBuf;

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// Verifies: NUM-009, CI-002
#[test]
fn eih_scope_is_validated_recorded_and_defaults_to_all() {
    let base = spec("stage-c-sun-planets-moon-eih-100yr.toml");
    assert!(base.forces.gr_eih);
    assert_eq!(base.forces.eih_scope, EihScope::All);

    // Refused without gr_eih.
    let mut off = base.clone();
    off.forces.gr_eih = false;
    off.forces.eih_scope = EihScope::Majors;
    let e = off.validate().unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E1203");
    assert!(e.to_string().contains("forces.eih_scope"), "{e}");

    // `all` written explicitly is the default: same digest; `majors` changes it.
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ci/scenarios/stage-c-sun-planets-moon-eih-100yr.toml"),
    )
    .unwrap();
    let explicit =
        ScenarioSpec::parse(&text.replace("gr_eih = true", "gr_eih = true\neih_scope = \"all\""))
            .unwrap();
    assert_eq!(explicit.forces.eih_scope, EihScope::All);
    assert_eq!(explicit.digest(), base.digest());
    let mut majors = base.clone();
    majors.forces.eih_scope = EihScope::Majors;
    assert_ne!(majors.digest(), base.digest());

    // The model string records the scope.
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let data = DataSources::none().with_header(&h, &prov);
    let short = |s: &ScenarioSpec| {
        let mut s = s.clone();
        s.span.years = 0.01; // lint: allow-literal(a few steps: only the model string is used)
        s.output.cadence_days = None;
        s
    };
    let a = run(&short(&base), &data).unwrap();
    let m = run(&short(&majors), &data).unwrap();
    assert!(
        a.model
            .contains("1PN sums over the tiered interaction graph"),
        "{}",
        a.model
    );
    assert!(!a.model.contains("eih_scope"), "{}", a.model);
    assert!(m.model.contains("eih_scope = majors"), "{}", m.model);
    assert_eq!(
        a.ic_hash, m.ic_hash,
        "the scope is a model choice, not an IC change"
    );
}
