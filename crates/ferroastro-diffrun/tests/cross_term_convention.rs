// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Progress review after WP18, R1 (and PO-4 / ADR-0014): cross-term rows hold the
//! inclusion–exclusion second difference; [`Protocol::to_park17`] maps them into Park et al. 2017
//! Table 3's sign ("pair removal minus single removals"), which the markdown prints, with the
//! second difference kept as its own labelled column. Synthetic rates
//! (no integration): the numbers below are arbitrary test inputs, not physical values.

mod common;

use ferroastro_diffrun::{
    BodyToggle, ConfigResult, Estimate, PassageSource, Plan, PlanResult, Protocol, evaluate,
};
use ferroastro_scenario::BodyName;

/// Synthetic configuration rates (″/cy), chosen so that every combination is a distinct small
/// integer: P-add singles V = 10, J = 20, pair contribution 33; P-remove singles V = 11, J = 18,
/// pair contribution 30. Not physical values.
const RATES: [(&str, f64); 8] = [
    ("base", 0.0),      // lint: allow-literal(synthetic rate)
    ("base+V", 10.0),   // lint: allow-literal(synthetic rate)
    ("base+J", 20.0),   // lint: allow-literal(synthetic rate)
    ("base+V+J", 33.0), // lint: allow-literal(synthetic rate)
    ("full", 100.0),    // lint: allow-literal(synthetic rate)
    ("full-V", 89.0),   // lint: allow-literal(synthetic rate)
    ("full-J", 82.0),   // lint: allow-literal(synthetic rate)
    ("full-V-J", 70.0), // lint: allow-literal(synthetic rate)
];

fn synthetic(label: &str, rate: f64) -> ConfigResult {
    ConfigResult {
        label: label.to_owned(),
        legs: Vec::new(),
        bodies: Vec::new(),
        ic_hashes: Vec::new(),
        estimate: Estimate {
            rate,
            formal_sigma: 0.0,
            jackknife_sigma: 0.0,
            q: 0.0,
            rms_residual: 0.0,
            passages: 0,
            span_jd: (0.0, 0.0),
            max_direction_error_arcsec: 0.0,
        },
        cross_check: None,
        steps: 0,
        model: String::new(),
        ic_source: String::new(),
    }
}

/// Verifies: NUM-013, PHY-012
#[test]
fn cross_terms_follow_park17_convention_with_the_second_difference_alongside() {
    use BodyName::{Jupiter, Mercury, Sun, Venus};
    let mut t = common::scenario("sun-mercury-newton-header440.toml");
    t.output.perihelion = vec![Mercury];
    let plan = Plan::bodies(
        "r1-sign",
        &t,
        &[Sun, Mercury],
        &[Sun, Mercury, Venus, Jupiter],
        &[
            BodyToggle::new("V", &[Venus]),
            BodyToggle::new("J", &[Jupiter]),
        ],
        &[(0, 1)],
        &[1.0],
    )
    .unwrap();
    let configs: Vec<ConfigResult> = RATES.iter().map(|&(l, r)| synthetic(l, r)).collect();
    let rows = evaluate(&plan, &configs).unwrap();
    let row = |p: Protocol, l: &str| {
        rows.iter()
            .find(|r| r.combination.protocol == p && r.combination.label == l)
            .unwrap()
    };
    let rate = |l: &str| RATES.iter().find(|r| r.0 == l).unwrap().1;
    // Park17's pair rows, computed here from their definition ("pair removal minus single
    // removals").
    let rem_pair = (rate("full") - rate("full-V-J"))
        - (rate("full") - rate("full-V"))
        - (rate("full") - rate("full-J"));
    let add_second = rate("base+V+J") - rate("base+V") - rate("base+J") + rate("base");
    let rem_second = rate("full") - rate("full-V") - rate("full-J") + rate("full-V-J");
    let a = row(Protocol::CrossTerm, "V+J");
    let r = row(Protocol::CrossTermRemove, "V+J");
    let a_park = Protocol::CrossTerm.to_park17(a.value);
    let r_park = Protocol::CrossTermRemove.to_park17(r.value);
    println!(
        "P-add second difference {} (Park17 sign {a_park}); P-remove second difference {} (Park17 sign {r_park})",
        a.value, r.value
    );
    // `Row::value` (and the JSON) is the raw inclusion–exclusion second difference of either
    // protocol; `Protocol::to_park17` negates it into Park17's sign (ADR-0014).
    assert_eq!(a.value.to_bits(), add_second.to_bits());
    assert_eq!(r.value.to_bits(), rem_second.to_bits());
    assert_eq!(r_park.to_bits(), rem_pair.to_bits());
    assert_eq!(a_park.to_bits(), (-add_second).to_bits());
    // The P-remove sign is what R1 / PO-4 is about: the two conventions differ in sign there.
    assert!(r_park > 0.0 && r.value < 0.0);
    assert!(
        Protocol::CrossTermRemove
            .park17_convention()
            .starts_with("Park17")
    );
    assert!(
        Protocol::CrossTerm
            .park17_convention()
            .starts_with("Park17")
    );
    let v = row(Protocol::Add, "V").value;
    assert_eq!(Protocol::Add.to_park17(v).to_bits(), v.to_bits());
    // The convention is labelled in the markdown; the JSON keeps the raw second differences.
    let result = PlanResult {
        plan: plan.name.clone(),
        estimator: "synthetic".into(),
        cross_check_estimator: format!("{:?}", PassageSource::SampledStates),
        shared_ic_hash: String::new(),
        constants: String::new(),
        ic_epoch_jd_tdb: None,
        configs,
        rows,
    };
    let md = result.markdown(&|_, _| None);
    assert!(md.contains(Protocol::CrossTermRemove.park17_convention()));
    assert!(md.contains("second difference (″/cy)"));
    assert!(md.contains(&format!("| V+J | {r_park:.6} |")));
    assert!(md.contains(&format!(" {rem_second:.6} |")));
    let back: PlanResult = serde_json::from_str(&result.to_json().unwrap()).unwrap();
    let r_json = back
        .rows
        .iter()
        .find(|w| w.combination.protocol == Protocol::CrossTermRemove)
        .unwrap();
    assert_eq!(r_json.value.to_bits(), rem_second.to_bits());
}

/// WP20: the switch plan with both protocols runs each toggle subset once and its rows follow
/// their definitions (synthetic rates: J2 alone 3, LT alone 5, both 7; not physical values).
///
/// Verifies: NUM-013, NUM-019
#[test]
#[expect(
    clippy::float_cmp,
    reason = "small integers combined exactly; equality is the assertion"
)]
fn switch_plan_with_both_protocols_has_each_subset_once_and_park17_signs() {
    use ferroastro_diffrun::SwitchToggle;
    let t = common::scenario("sun-mercury-newton-header440.toml");
    let toggles = [
        SwitchToggle {
            label: "J2".into(),
            apply: |f| f.sun_j2 = true,
        },
        SwitchToggle {
            label: "LT".into(),
            apply: |f| f.lense_thirring = true,
        },
    ];
    let plan = Plan::switches_both("wp20-sign", &t, &toggles, &[(0, 1)], &[-1.0, 1.0]).unwrap();
    let labels: Vec<&str> = plan.configs.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(labels, ["base", "base+J2", "base+LT", "base+J2+LT"]);
    assert!(plan.configs.iter().all(|c| c.legs.len() == 2));
    let full = &plan.configs[3].legs[1].forces;
    assert!(full.sun_j2 && full.lense_thirring);
    let rates = [
        ("base", 0.0),       // lint: allow-literal(synthetic rate)
        ("base+J2", 3.0),    // lint: allow-literal(synthetic rate)
        ("base+LT", 5.0),    // lint: allow-literal(synthetic rate)
        ("base+J2+LT", 7.0), // lint: allow-literal(synthetic rate)
    ];
    let configs: Vec<ConfigResult> = rates.iter().map(|&(l, r)| synthetic(l, r)).collect();
    let rows = evaluate(&plan, &configs).unwrap();
    let row = |p: Protocol, l: &str| {
        rows.iter()
            .find(|r| r.combination.protocol == p && r.combination.label == l)
            .unwrap()
            .value
    };
    let rate = |l: &str| rates.iter().find(|r| r.0 == l).unwrap().1;
    assert_eq!(row(Protocol::Add, "J2"), rate("base+J2") - rate("base"));
    assert_eq!(row(Protocol::Add, "LT"), rate("base+LT") - rate("base"));
    assert_eq!(
        row(Protocol::Remove, "J2"),
        rate("base+J2+LT") - rate("base+LT")
    );
    assert_eq!(
        row(Protocol::Remove, "LT"),
        rate("base+J2+LT") - rate("base+J2")
    );
    let cross = rate("base+J2+LT") - rate("base+J2") - rate("base+LT") + rate("base");
    assert_eq!(row(Protocol::CrossTerm, "J2+LT"), cross);
    // With two toggles `all − X − Y` is the base, so the P-remove second difference is the same
    // four configurations with the same signs: the raw rows agree, and Park17's sign
    // (`Protocol::to_park17`, pair removal minus single removals) is its negative.
    let rem = row(Protocol::CrossTermRemove, "J2+LT");
    assert_eq!(rem, cross);
    assert_eq!(Protocol::CrossTermRemove.to_park17(rem), -cross);
    let pair_removal = (rate("base+J2+LT") - rate("base"))
        - (rate("base+J2+LT") - rate("base+LT"))
        - (rate("base+J2+LT") - rate("base+J2"));
    assert_eq!(Protocol::CrossTermRemove.to_park17(rem), pair_removal);
}
