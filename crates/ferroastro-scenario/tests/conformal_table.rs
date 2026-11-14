// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The optional `[forces.conformal]` table of the conformal-gravity comparison model (ADR-0016,
//! PHY-020, CI-002): parsing, defaults, units, validation and what the scenario digest covers.

#![expect(
    clippy::unwrap_used,
    clippy::float_cmp_const,
    reason = "test: a failed unwrap is a test failure; the unit conversions are exact (products \
              of small integers and 1e5 = 2^5 5^5), so they are compared bitwise"
)]

use ferroastro_scenario::{CM_PER_KM, Gamma0Centre, SWITCHES, ScenarioSpec};
use std::path::PathBuf;

fn base() -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../ci/scenarios/sun-mercury-newton-header440.toml"),
    )
    .unwrap()
}

fn with_table(table: &str) -> String {
    base().replace("newton = true", &format!("newton = true\n{table}"))
}

/// The table parses as an inline table or a `[forces.conformal]` section; omitted
/// coefficients are 0, the centring defaults to `"uniform"` and the direction to `[1, 0, 0]`; the
/// accessors convert cm⁻¹ / cm⁻² to km⁻¹ / km⁻²; `conformal` is a listed switch and is reported
/// as requested.
///
/// Verifies: PHY-020, CI-002
#[test]
fn conformal_table_parses_with_defaults_and_units() {
    assert!(SWITCHES.contains(&"conformal"));
    let s = ScenarioSpec::parse(&with_table("conformal = { gamma0 = 2.0 }")).unwrap();
    let c = s.forces.conformal.unwrap();
    assert_eq!(c.gamma0_centre, Gamma0Centre::Uniform);
    assert_eq!(c.gamma0_direction, [1.0, 0.0, 0.0]);
    assert_eq!((c.gamma_star, c.kappa), (0.0, 0.0));
    assert_eq!(c.gamma0_per_km(), 2.0 * CM_PER_KM);
    assert!(s.forces.requested().contains(&"conformal"));

    let section = base().replace(
        "[integrator]",
        "[forces.conformal]\ngamma0 = 1.0\ngamma_star = -2.0\nkappa = 3.0\n\
         gamma0_centre = \"sun\"\ngamma0_direction = [0.0, 2.0, 0.0]\n\n[integrator]",
    );
    let c = ScenarioSpec::parse(&section)
        .unwrap()
        .forces
        .conformal
        .unwrap();
    assert_eq!(c.gamma0_centre, Gamma0Centre::Sun);
    assert_eq!(c.gamma_star_per_km(), -2.0 * CM_PER_KM);
    assert_eq!(c.kappa_per_km2(), 3.0 * CM_PER_KM * CM_PER_KM);
    assert_eq!(c.gamma0_direction, [0.0, 2.0, 0.0]);
    assert_eq!(Gamma0Centre::Sun.label(), "sun");
}

/// Invalid tables are refused: an unknown key or centring (FA-E1200), a non-finite coefficient
/// or a zero direction (FA-E1203, naming the field).
///
/// Verifies: PHY-020, CI-002
#[test]
fn conformal_table_refuses_invalid_values() {
    for (table, code, needle) in [
        ("conformal = { gamma = 1.0 }", "FA-E1200", "gamma"),
        (
            "conformal = { gamma0_centre = \"galaxy\" }",
            "FA-E1200",
            "galaxy",
        ),
        (
            "conformal = { kappa = nan }",
            "FA-E1203",
            "forces.conformal.kappa",
        ),
        (
            "conformal = { gamma0 = inf }",
            "FA-E1203",
            "forces.conformal.gamma0",
        ),
        (
            "conformal = { gamma0_direction = [0.0, 0.0, 0.0] }",
            "FA-E1203",
            "forces.conformal.gamma0_direction",
        ),
    ] {
        let e = ScenarioSpec::parse(&with_table(table)).unwrap_err();
        println!("{table}: {e}");
        assert_eq!(e.code().to_string(), code, "{table}");
        assert!(e.to_string().contains(needle), "{table}: {e}");
    }
}

/// The digest covers the effective table: present vs absent differs; an omitted default and
/// the same value written out give the same digest (every field is serialised once the table is
/// present); changing the centring, a coefficient or the direction changes it.
///
/// Verifies: PHY-020, CI-002
#[test]
fn digest_covers_the_effective_conformal_table() {
    let digest = |t: &str| ScenarioSpec::parse(&with_table(t)).unwrap().digest();
    let absent = ScenarioSpec::parse(&base()).unwrap().digest();
    let short = digest("conformal = { gamma0 = 1.0 }");
    let explicit = digest(
        "conformal = { gamma0 = 1.0, gamma_star = 0.0, kappa = 0.0, gamma0_centre = \"uniform\", \
         gamma0_direction = [1.0, 0.0, 0.0] }",
    );
    assert_ne!(absent, short);
    assert_eq!(short, explicit);
    for other in [
        "conformal = { gamma0 = 1.0, gamma0_centre = \"sun\" }",
        "conformal = { gamma0 = 2.0 }",
        "conformal = { gamma0 = 1.0, kappa = 1.0 }",
        "conformal = { gamma0 = 1.0, gamma0_direction = [0.0, 1.0, 0.0] }",
    ] {
        assert_ne!(digest(other), short, "{other}");
    }
}
