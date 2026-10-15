// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! API-003: every PHY rung the ladder implements has a runnable program in `examples/`, and every
//! example runs (here, in the PR tier) and passes its cited test vector. The examples are
//! compiled into this test with `#[path]`, so `cargo nextest run` executes them. The examples
//! run 100 orbits and only demonstrate the rungs; the PHY-xxx evidence is the rung tests, so these
//! tests cite API-003 alone.

#![expect(
    clippy::duplicate_mod,
    reason = "each example includes the shared examples/common module, as it does when built alone"
)]

#[path = "../examples/phy001_n2b.rs"]
mod phy001_n2b;
#[path = "../examples/phy004_gr_analytic.rs"]
mod phy004_gr_analytic;
#[path = "../examples/phy005_gr_schw.rs"]
mod phy005_gr_schw;
#[path = "../examples/phy006_ppn.rs"]
mod phy006_ppn;
#[path = "../examples/phy007_sr_a.rs"]
mod phy007_sr_a;
#[path = "../examples/phy008_sr_b.rs"]
mod phy008_sr_b;
#[path = "../examples/phy009_coefficients.rs"]
mod phy009_coefficients;
#[path = "../examples/phy010_j2.rs"]
mod phy010_j2;
#[path = "../examples/phy011_lt.rs"]
mod phy011_lt;

use ferroastro_constants::De440;
use ferroastro_ladder::Scenario;
use ferroastro_ladder::single_body::single_body_rungs;
use std::collections::BTreeSet;

/// PHY rungs implemented outside the `single-body` catalogue: N-2B (two-body test), GR-SCHW
/// analytic, the PPN study and the PHY-009 coefficient study.
const NON_CATALOGUE_RUNGS: [&str; 4] = ["PHY-001", "PHY-004", "PHY-006", "PHY-009"];

/// Every implemented PHY rung has an `examples/phyNNN_*.rs` program.
///
/// Verifies: API-003
#[test]
fn every_implemented_phy_rung_has_an_example() {
    let de = De440::load().unwrap();
    let scn = Scenario::mercury_single_body().unwrap();
    let mut wanted: BTreeSet<String> = NON_CATALOGUE_RUNGS.iter().map(|s| (*s).into()).collect();
    for rung in single_body_rungs(&scn, &de).unwrap() {
        for req in rung.requirements.split(", ") {
            if req.starts_with("PHY-") {
                wanted.insert(req.into());
            }
        }
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "rs").then(|| path.file_name()?.to_str().map(String::from))?
        })
        .collect();
    for req in &wanted {
        let prefix = format!("phy{}_", req.trim_start_matches("PHY-"));
        assert!(
            files.iter().any(|f| f.starts_with(&prefix)),
            "{req}: no examples/{prefix}*.rs"
        );
    }
    println!("PHY rungs with examples: {wanted:?}");
}

/// Verifies: API-003
#[test]
fn example_phy001_runs() {
    phy001_n2b::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy004_runs() {
    phy004_gr_analytic::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy005_runs() {
    phy005_gr_schw::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy006_runs() {
    phy006_ppn::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy007_runs() {
    phy007_sr_a::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy008_runs() {
    phy008_sr_b::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy009_runs() {
    phy009_coefficients::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy010_runs() {
    phy010_j2::main().unwrap();
}

/// Verifies: API-003
#[test]
fn example_phy011_runs() {
    phy011_lt::main().unwrap();
}
