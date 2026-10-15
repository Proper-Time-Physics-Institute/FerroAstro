// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The committed scenario's physical inputs equal their cited test vectors.

mod common;

/// Verifies: PHY-010, PHY-005
#[test]
fn scenario_inputs_match_vectors() {
    let c = common::ctx();
    let el = c.scn.elements;
    for (id, got) in [
        ("SW92-MERCURY-A", el.a_au),
        ("SW92-MERCURY-E", el.e),
        ("SW92-MERCURY-I", el.i_deg),
        ("SW92-MERCURY-L", el.mean_longitude_deg),
        ("SW92-MERCURY-VARPI", el.long_perihelion_deg),
        ("SW92-MERCURY-NODE", el.node_deg),
        ("PARK17-J2SUN", c.scn.park17_j2),
        ("PARK17-SOLAR-EQUATOR-TILT", c.scn.park17_tilt_deg),
    ] {
        c.check(id, got);
    }
    assert!(c.scn.orbits >= 1000, "NUM-015 averages over 1e3 orbits");
}

/// Verifies: API-002
#[test]
fn malformed_scenarios_are_coded_errors() {
    let bad = ferroastro_ladder::MERCURY_SINGLE_BODY.replace("orbits = 1000", "orbits = 1");
    let e = ferroastro_ladder::Scenario::parse(&bad).unwrap_err();
    assert_eq!(e.code(), ferroastro_core::ErrorCode::LadderScenario);
    let bad = ferroastro_ladder::MERCURY_SINGLE_BODY.replace("\"0.20563593\"", "\"x\"");
    assert!(ferroastro_ladder::Scenario::parse(&bad).is_err());
}
