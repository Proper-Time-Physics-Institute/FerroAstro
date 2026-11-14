// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The comparison dashboard's reference columns (API-004; F4 of the conformal-gravity plan,
//! ADR-0016) in the metadata's `dashboard` object.
//!
//! # Oracles
//! - **Pass-through:** the M5 evidence rows, the Park17 literature rows and the `INPOP15a` bound must
//!   equal their `tests/vectors` entries exactly (bitwise after parsing): the page shows them, so a
//!   copy that drifted from the committed provenance would be a silent golden change. The M5 rows
//!   are `computed-reference` (FerroAstro's own nightly results), never observations.
//! - **Analytic estimates:** the ladder's analytic vectors, each with its own tolerance:
//!   PHY004-GR-RATE (GR 1PN), PHY010-J2-DE440-ANALYTIC-RATE and PHY010-J2-PARK-ANALYTIC-RATE (planar
//!   solar J2 at DE440's and, through the override, Park17's J2), PHY011-LT-ANALYTIC-RATE
//!   (Lense–Thirring), PHY007-SRA-ANALYTIC-RATE and PHY008-SRB-{HARMONIC,AREAL}-RATE (Rung-lab rungs).
//!   The conformal-gravity estimate must be the engine's own Gauss reference
//!   (`reference::conformal_reference`); its fit values are never asserted (OI-79, decision D7).
//!
//! All tests are PR-tier; none is `#[ignore]`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed parse is a test failure"
)]

mod common;

use common::{LAB, check, vectors};
use ferroastro_viewer::Viewer;
use ferroastro_viewer::config::{Param, bits};
use ferroastro_viewer::reference::conformal_reference;
use serde_json::Value;

fn meta(v: &Viewer) -> Value {
    serde_json::from_str(&v.meta_json()).expect("metadata JSON")
}

fn f(v: &Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

/// True for a non-blank label string.
fn labelled(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !s.trim().is_empty())
}

fn viewer(mask: u32, params: &[(Param, f64)]) -> Viewer {
    let mut v = Viewer::new();
    assert_eq!(
        v.set_toggles(mask),
        ferroastro_viewer::FAV_OK,
        "{}",
        v.error()
    );
    for (p, x) in params {
        assert_eq!(
            v.set_param(*p as u32, *x),
            ferroastro_viewer::FAV_OK,
            "{}",
            v.error()
        );
    }
    v
}

/// The M5 evidence rows (computed, not observed), the Park17 literature rows and the `INPOP15a`
/// bound reach the metadata unchanged from `tests/vectors`, with the column labels the page shows.
/// Needs no header.440: none of these depends on the DE440 constants.
///
/// Verifies: API-004, PHY-014, PHY-020
#[test]
fn dashboard_passes_m5_evidence_literature_and_bound_through() {
    let vs = vectors();
    let m = meta(&Viewer::new());
    let d = &m["dashboard"];
    assert!(d.is_object(), "metadata has a dashboard object: {m}");

    let m5 = &d["m5"];
    assert_eq!(m5["kind"], "computed-reference");
    assert!(m5["label"].as_str().unwrap().contains("not observed"));
    for k in ["frame", "window", "protocol", "run"] {
        assert!(labelled(&m5[k]), "m5.{k} labelled");
    }
    for (key, id, sigma) in [
        ("total", "M5-EIH-FULL-RATE", "M5-EIH-FULL-RATE-JK-SIGMA"),
        ("b7", "M5-J2-PREMOVE-RATE", "M5-J2-LT-PREMOVE-SIGMA-RSS"),
        (
            "b8",
            "M5-GR-SUN-MERCURY-RATE",
            "M5-GR-SUN-MERCURY-RATE-JK-SIGMA",
        ),
        ("b9", "M5-LT-PREMOVE-RATE", "M5-J2-LT-PREMOVE-SIGMA-RSS"),
        (
            "b11",
            "M5-GR-SUN-MERCURY-RATE",
            "M5-GR-SUN-MERCURY-RATE-JK-SIGMA",
        ),
    ] {
        let row = &m5["rows"][key];
        assert_eq!(row["id"], id);
        let want = vs.get(id).unwrap().value().unwrap();
        assert_eq!(f(&row["value"]).to_bits(), want.to_bits(), "{key}: {id}");
        let want_s = vs.get(sigma).unwrap().value().unwrap();
        assert_eq!(
            f(&row["sigma"]).to_bits(),
            want_s.to_bits(),
            "{key}: {sigma}"
        );
        assert!(labelled(&row["protocol"]));
    }

    let lit = &d["literature"];
    for k in ["frame", "window", "protocol"] {
        assert!(labelled(&lit[k]), "literature.{k} labelled");
    }
    for (key, id) in [
        ("b0", "PARK17-T3-VENUS"),
        ("b1", "PARK17-T3-EARTH-MOON"),
        ("b2", "PARK17-T3-MARS"),
        ("b3", "PARK17-T3-JUPITER"),
        ("b4", "PARK17-T3-SATURN"),
        ("b5", "PARK17-T3-URANUS"),
        ("b6", "PARK17-T3-NEPTUNE"),
        ("b7", "PARK17-J2-RATE"),
        ("b8", "PARK17-GE-RATE"),
        ("b9", "PARK17-LT-RATE"),
        ("b10", "PARK17-T3-ASTEROIDS"),
        ("b11", "PARK17-GE-RATE"),
    ] {
        let row = &lit["rows"][key];
        assert_eq!(row["id"], id);
        let want = vs.get(id).unwrap().value().unwrap();
        assert_eq!(f(&row["value"]).to_bits(), want.to_bits(), "{key}: {id}");
        assert_eq!(row["applies"], true, "{key}: nominal parameters");
    }
    assert!(
        lit["rows"]["b12"].is_null(),
        "no literature value for the comparison model"
    );

    let b = &d["bound"];
    let centre = vs
        .get("INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE")
        .unwrap()
        .value()
        .unwrap();
    let half = vs
        .get("INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE-INTERVAL")
        .unwrap()
        .value()
        .unwrap();
    assert_eq!(f(&b["value"]).to_bits(), centre.to_bits());
    assert_eq!(f(&b["half_width"]).to_bits(), half.to_bits());
    assert_eq!(b["applies_to"], serde_json::json!(["b12"]));
    assert_eq!(
        d["comparison_bits"],
        serde_json::json!([bits::CONFORMAL.trailing_zeros()])
    );
    for k in ["frame", "window", "protocol", "criterion"] {
        assert!(labelled(&b[k]), "bound.{k} labelled");
    }
}

/// An override of an effect's own parameter marks its Park17 row as not applying (nominal physics
/// only), so the page differences against the engine's analytic estimate instead.
///
/// Verifies: API-004
#[test]
fn literature_rows_stop_applying_when_their_parameter_is_overridden() {
    let vs = vectors();
    let park_j2 = vs.get("PARK17-J2SUN").unwrap().value().unwrap();
    let v = viewer(
        bits::DEFAULT,
        &[
            (Param::J2Override, park_j2),
            (Param::SpinScale, 2.0),
            (Param::PpnBeta, 0.5),
        ],
    );
    let rows = &meta(&v)["dashboard"]["literature"]["rows"];
    for key in ["b7", "b8", "b9", "b11"] {
        assert_eq!(rows[key]["applies"], false, "{key}");
    }
    for key in ["b0", "b3", "b10"] {
        assert_eq!(rows[key]["applies"], true, "{key}");
    }
}

/// The engine analytic estimates agree with the ladder's analytic vectors (System mode and the
/// Rung lab), follow the J2 override, and the conformal-gravity estimate is the engine's own Gauss
/// reference. Needs the DE440 constants (embed-header); without them the estimates are null.
///
/// Verifies: API-004, PHY-004, PHY-007, PHY-008, PHY-010, PHY-011, PHY-020
#[test]
fn analytic_estimates_match_the_ladder_analytic_vectors() {
    let m = meta(&Viewer::new());
    if !common::rung_lab_or_skip("analytic_estimates_match_the_ladder_analytic_vectors") {
        assert!(
            m["dashboard"]["analytic"].is_null(),
            "no constants, no estimates"
        );
        return;
    }
    let vs = vectors();
    let rows = &m["dashboard"]["analytic"]["rows"];
    check(&vs, "PHY004-GR-RATE", f(&rows["b8"]["value"]));
    assert_eq!(
        rows["b11"]["value"], rows["b8"]["value"],
        "EIH: the Sun's term"
    );
    check(
        &vs,
        "PHY010-J2-DE440-ANALYTIC-RATE",
        f(&rows["b7"]["value"]),
    );
    check(&vs, "PHY011-LT-ANALYTIC-RATE", f(&rows["b9"]["value"]));
    assert!(rows["b0"].is_null(), "no closed form for a planet");
    assert!(rows["rung"].is_null(), "System mode has no rung row");
    let cfg = Viewer::new().config().clone();
    let cg = conformal_reference(&cfg.conformal_settings()).unwrap();
    // The JSON text round trip may move the last bit (serde_json's default float parser).
    let got = f(&rows["b12"]["value"]);
    assert!(
        (got - cg.total).abs() <= 4.0 * f64::EPSILON * cg.total.abs(),
        "{got} vs {}",
        cg.total
    );

    let park_j2 = vs.get("PARK17-J2SUN").unwrap().value().unwrap();
    let v = viewer(bits::DEFAULT, &[(Param::J2Override, park_j2)]);
    check(
        &vs,
        "PHY010-J2-PARK-ANALYTIC-RATE",
        f(&meta(&v)["dashboard"]["analytic"]["rows"]["b7"]["value"]),
    );

    for (rung, id) in [
        (1.0, "PHY007-SRA-ANALYTIC-RATE"),
        (2.0, "PHY008-SRB-HARMONIC-RATE"),
        (3.0, "PHY008-SRB-AREAL-RATE"),
        (4.0, "PHY004-GR-RATE"),
    ] {
        let v = viewer(0, &[LAB, (Param::Rung, rung)]);
        check(
            &vs,
            id,
            f(&meta(&v)["dashboard"]["analytic"]["rows"]["rung"]["value"]),
        );
    }
}
