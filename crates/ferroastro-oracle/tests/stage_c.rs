// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Stage C (REF-005, CI-005 PR-fast; WP21 checks S1 and S1c): FerroAstro IAS15 with `gr_eih`
//! (DE440 Eq. 27) against the cached REBOUND 5.2.2 + `REBOUNDx` 5.1.0 `gr_full` golden on the same
//! scenario file, and the Newtonian control pair with the same bodies and settings. No container
//! is started here (CI-005): the goldens were produced by `tools/oracles/run_oracles.sh rebound`
//! (`REBOUNDx` used as a black box through its documented Python API, LIC-001, DEP-008).

#![expect(
    clippy::unwrap_used,
    reason = "test: a failed unwrap is a test failure"
)]

mod common;

use ferroastro_constants::header440::Header;
use ferroastro_oracle::{GoldenStates, Manifest, StateComparison, compare_states, sha256_hex};
use ferroastro_propagate::{DataSources, run};
use ferroastro_scenario::ScenarioSpec;
use ferroastro_vectors::VectorSet;
use std::path::PathBuf;

const EIH: &str = "stage-c-sun-planets-moon-eih-100yr";
const NEWTON: &str = "stage-c-sun-planets-moon-newton-100yr";
const NAIF_MERCURY: i32 = 1;
const YEARLY_TIMES: usize = 101; // lint: allow-literal(101 yearly output times, 0..=100 yr)

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// FerroAstro on the committed scenario against the committed REBOUND golden of the same file:
/// manifest, inputs and the `REBOUNDx` effect checked; every body printed; R2 brute-force maxima.
fn against_golden(scenario: &str, effect: &str) -> StateComparison {
    let dir = root().join("ci/golden/rebound").join(scenario);
    let m = Manifest::verify_dir(&dir).unwrap();
    let csv = format!("{scenario}.rebound.states.csv");
    let entry = m.entry(&csv).unwrap();
    let text =
        std::fs::read_to_string(root().join("ci/scenarios").join(format!("{scenario}.toml")))
            .unwrap();
    entry
        .check_input(&format!("{scenario}.toml"), &sha256_hex(text.as_bytes()))
        .unwrap();
    let header = ferroastro_testdata::locate_product("de440-header").unwrap();
    let pin = ferroastro_testdata::product("de440-header").unwrap().sha256;
    println!("de440-header: {}", header.provenance());
    assert_eq!(header.sha256, pin);
    entry.check_input("header.440", &pin).unwrap();
    assert_eq!(
        entry.extra.get("reboundx_effect").map(String::as_str),
        Some(effect),
        "the golden's REBOUNDx effect"
    );
    println!(
        "golden: {} ({}), image {}, runner {}, {:?}",
        entry.file, entry.version, entry.image_digest, entry.runner_sha, entry.extra
    );
    let h = Header::parse(&std::fs::read_to_string(&header.path).unwrap()).unwrap();
    let prov = header.provenance();
    let spec = ScenarioSpec::parse(&text).unwrap();
    let out = run(&spec, &DataSources::none().with_header(&h, &prov)).unwrap();
    println!("FerroAstro model: {}", out.model);
    let golden =
        GoldenStates::parse(&csv, &std::fs::read_to_string(dir.join(&csv)).unwrap()).unwrap();
    assert_eq!(golden.center_naif().unwrap(), 0, "SSB-centred golden");
    let cmp = compare_states(&golden, &out).unwrap();
    // Review R2: the reported maxima are the brute-force maxima over every golden row.
    common::check_against_brute_force(&golden, &out, &cmp);
    assert_eq!(cmp.bodies.len(), spec.bodies.len());
    assert_eq!(cmp.times, YEARLY_TIMES);
    println!(
        "{scenario}: FerroAstro {} IAS15 steps, IC hash {}; {} rows at {} times",
        out.stats.steps, out.ic_hash, cmp.rows, cmp.times
    );
    println!("naif  max|dr| (km)       at t (d)   max|dv| (km/s)");
    for b in &cmp.bodies {
        println!(
            "{:>4}  {:<18.6e} {:>9.2}  {:.6e}",
            b.naif, b.max_dr_km, b.t_days_at_max_dr, b.max_dv_km_s
        );
    }
    cmp
}

fn mercury(cmp: &StateComparison) -> f64 {
    cmp.bodies
        .iter()
        .find(|b| b.naif == NAIF_MERCURY)
        .unwrap()
        .max_dr_km
}

/// S1: FerroAstro `gr_eih` against the `REBOUNDx` `gr_full` golden over 100 yr. Mercury's largest
/// position difference is asserted against `ORACLE-STAGE-C-REBOUNDX-MERCURY-MAX-DR` (PROVISIONAL
/// 1 m, pre-registered); every body is printed.
///
/// Verifies: REF-005, CI-005, CI-009, NUM-017
#[test]
fn stage_c_ferroastro_eih_matches_the_reboundx_gr_full_golden() {
    let cmp = against_golden(EIH, "gr_full");
    let dr = mercury(&cmp);
    println!("stage C: Mercury max |dr| = {dr:.6e} km");
    VectorSet::load_canonical()
        .unwrap()
        .get("ORACLE-STAGE-C-REBOUNDX-MERCURY-MAX-DR")
        .unwrap()
        .check(dr)
        .unwrap();
}

/// S1c: the Newtonian control (same bodies, Moon included, same IAS15 settings) against its
/// REBOUND golden: the round-off baseline of S1. Reported, not asserted against a bound (WP21
/// plan); the comparison itself (manifest, inputs, R2 maxima) is checked.
///
/// Verifies: REF-005, CI-005
#[test]
fn stage_c_newtonian_control_against_the_rebound_golden() {
    let cmp = against_golden(NEWTON, "none");
    println!(
        "stage C control: Mercury max |dr| = {:.6e} km; all bodies {:.6e} km",
        mercury(&cmp),
        cmp.max_dr_km()
    );
    assert!(cmp.max_dr_km().is_finite());
}
