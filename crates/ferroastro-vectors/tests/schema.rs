// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Schema enforcement tests (TST-002, TST-003) and a load of the committed workspace vectors.

use ferroastro_core::ErrorCode;
use ferroastro_vectors::{VectorError, VectorSet};
use std::path::Path;

/// A fully valid vector whose fields can be replaced one at a time. The numbers are synthetic
/// schema fixtures inside a string, not physical expectations.
fn fixture(replace: &str, with: &str) -> String {
    let base = r#"
schema_version = 1
[[vector]]
id = "FIXTURE-A"
requirement = ["TST-002"]
quantity = "synthetic fixture"
value = 3
unit = "1"
kind = "measured"
source = "fixture source"
citation = "fixture citation"
retrieved = "2026-10-14"
tolerance = { abs = 1, rel = 0 }
tolerance_rationale = "fixture"
verified_by = "fixture"
"#;
    base.replacen(replace, with, 1)
}

fn load(text: &str) -> Result<VectorSet, VectorError> {
    let mut set = VectorSet::default();
    set.insert_file(Path::new("fixture.toml"), text)?;
    Ok(set)
}

#[expect(
    clippy::panic,
    reason = "test helper: a wrong error kind fails the calling test"
)]
fn schema_error(text: &str) -> String {
    match load(text) {
        Err(e @ VectorError::Schema { .. }) => e.to_string(),
        other => panic!("expected FA-E0102 schema error, got {other:?}"),
    }
}

/// Verifies: TST-002
#[test]
fn valid_fixture_loads_and_checks() {
    let set = load(&fixture("", "")).unwrap();
    let v = set.get("FIXTURE-A").unwrap();
    assert!(v.check(v.value().unwrap()).is_ok());
    let err = v.check(v.value().unwrap() * 3.0).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ToleranceExceeded);
    assert!(err.to_string().contains("FIXTURE-A"));
}

/// The pass rule is `|x − v| ≤ max(abs, rel·|v|)`: whichever of the absolute floor and the
/// relative band is wider wins (NUM-015 absolute floor, red-team §1). Kills a `max` → `min` mutant.
/// Verifies: TST-002, NUM-015
#[test]
fn tolerance_is_the_wider_of_abs_and_rel() {
    // Relative band wider: value 3, abs 1, rel 1/2 → allowed 3/2.
    let set = load(&fixture("abs = 1, rel = 0", "abs = 1, rel = 0.5")).unwrap();
    let v = set.get("FIXTURE-A").unwrap();
    let x = v.value().unwrap();
    assert!(v.check(x * 3.0 / 2.0).is_ok());
    assert!(v.check(x * 2.0).is_err());
    // Absolute floor wider: value 3, abs 2, rel 1/4 → allowed 2.
    let set = load(&fixture("abs = 1, rel = 0", "abs = 2, rel = 0.25")).unwrap();
    let v = set.get("FIXTURE-A").unwrap();
    let x = v.value().unwrap();
    assert!(v.check(x + 2.0).is_ok());
    assert!(v.check(x + 3.0).is_err());
    // A non-finite measurement never passes.
    assert!(v.check(f64::NAN).is_err());
}

/// Verifies: TST-002
#[test]
fn every_required_field_is_enforced() {
    for field in [
        "id =",
        "requirement =",
        "quantity =",
        "value =",
        "unit =",
        "kind =",
        "source =",
        "citation =",
        "retrieved =",
        "tolerance =",
        "tolerance_rationale =",
        "verified_by =",
    ] {
        let text = fixture(field, "# removed =");
        let err = load(&text).unwrap_err();
        assert!(
            matches!(err.code(), ErrorCode::VectorSchema | ErrorCode::VectorParse),
            "removing {field} gave {err}"
        );
    }
}

/// Verifies: TST-002, PHY-018
#[test]
fn bad_values_are_rejected() {
    assert!(
        schema_error(&fixture(r#""fixture source""#, r#""a value, UNVERIFIED""#))
            .contains("OPEN-ITEMS")
    );
    assert!(
        schema_error(&fixture(r#""fixture citation""#, r#""Clemence [S]""#)).contains("OPEN-ITEMS")
    );
    assert!(schema_error(&fixture(r#"["TST-002"]"#, r#"["PHYS-2"]"#)).contains("requirement"));
    assert!(schema_error(&fixture(r#""FIXTURE-A""#, r#""lower-case""#)).contains("id"));
    assert!(schema_error(&fixture("2026-10-14", "14/10/2026")).contains("retrieved"));
    assert!(schema_error(&fixture("abs = 1, rel = 0", "rel = -1")).contains("tolerance"));
    assert!(schema_error(&fixture("{ abs = 1, rel = 0 }", "{ }")).contains("tolerance"));
    assert!(schema_error(&fixture(r#""fixture""#, r#""   ""#)).contains("empty"));
    assert!(schema_error(&fixture("value = 3", r#"value = "three""#)).contains("value"));
    assert_eq!(
        load(&fixture("kind", "typo = 1\nkind")).unwrap_err().code(),
        ErrorCode::VectorParse
    );
}

/// Verifies: TST-003
#[test]
fn computed_reference_needs_generator_and_engine() {
    let cr = fixture(r#""measured""#, r#""computed-reference""#);
    assert!(schema_error(&cr).contains("generated_by"));
    let with_gen = cr.replacen(
        "verified_by",
        "generated_by = \"tools/gen.py@0123abc\"\nverified_by",
        1,
    );
    assert!(schema_error(&with_gen).contains("engine"));
    let full = with_gen.replacen(
        "verified_by",
        "engine = { name = \"x\", version = \"1\", settings = \"s\" }\nverified_by",
        1,
    );
    assert!(load(&full).is_ok());
}

/// Verifies: TST-002
#[test]
fn duplicate_ids_across_files_are_rejected() {
    let mut set = VectorSet::default();
    set.insert_file(Path::new("a.toml"), &fixture("", ""))
        .unwrap();
    let err = set
        .insert_file(Path::new("b.toml"), &fixture("", ""))
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::VectorDuplicateId);
    assert_eq!(
        VectorSet::default().get("NOPE").unwrap_err().code(),
        ErrorCode::VectorUnknownId
    );
}

/// Verifies: TST-002, TST-003
#[test]
fn workspace_vectors_all_validate() {
    let set = VectorSet::load_workspace().unwrap();
    assert!(!set.is_empty());
    for (path, v) in set.iter() {
        assert!(
            v.value().unwrap().is_finite(),
            "{} in {}",
            v.id,
            path.display()
        );
    }
}

/// `blocked_by` names an open item; the vector is non-gating while OPEN-ITEMS.md leaves that item
/// unticked (or lacks it), and gating once it is ticked.
///
/// Verifies: TST-002
#[test]
fn blocked_by_follows_the_open_item_checkbox() {
    let blocked = fixture("verified_by", "blocked_by = \"OI-38\"\nverified_by");
    let set = load(&blocked).unwrap();
    let v = set.get("FIXTURE-A").unwrap();
    assert_eq!(v.blocked_by.as_deref(), Some("OI-38"));
    let open = "- [ ] **OI-38** Park 2017's protocol.\n- [x] **OI-3** closed.\n";
    let closed = "- [x] **OI-38** Park 2017's protocol (closed by a maintainer).\n";
    let other_closed = "- [x] **OI-380** another item.\n";
    assert_eq!(v.blocking_open_item(open), Some("OI-38"));
    assert_eq!(v.blocking_open_item(""), Some("OI-38"));
    assert_eq!(v.blocking_open_item(other_closed), Some("OI-38"));
    assert_eq!(v.blocking_open_item(closed), None);
    let unblocked = load(&fixture("", "")).unwrap();
    assert_eq!(
        unblocked.get("FIXTURE-A").unwrap().blocking_open_item(open),
        None
    );
    assert!(
        schema_error(&fixture("verified_by", "blocked_by = \"38\"\nverified_by"))
            .contains("blocked_by")
    );
}

/// `provisional` names an open item under which the assertion is PROVISIONAL (ADR-0014). It gates
/// like any vector, it must be a well-formed `OI-NN` that OPEN-ITEMS.md lists, it cannot be
/// dropped or blanked to an empty reference, and it excludes `blocked_by`.
///
/// Verifies: TST-002
#[test]
fn provisional_gates_and_keeps_its_open_item_reference() {
    let with = |value: &str| {
        fixture(
            "verified_by",
            &format!("provisional = {value}\nverified_by"),
        )
    };
    let set = load(&with("\"OI-38\"")).unwrap();
    let v = set.get("FIXTURE-A").unwrap();
    assert_eq!(v.provisional.as_deref(), Some("OI-38"));
    // Gates: a provisional vector is never non-gating, and its failure message says PROVISIONAL.
    assert_eq!(v.blocking_open_item(""), None);
    let x = v.value().unwrap();
    assert!(v.check(x).is_ok());
    let err = v.check(x * 3.0).unwrap_err();
    assert_eq!(err.code(), ErrorCode::ToleranceExceeded);
    let msg = err.to_string();
    assert!(msg.contains("PROVISIONAL") && msg.contains("OI-38") && msg.contains("revert"));

    // The open item must exist in OPEN-ITEMS.md; its checkbox sets the label.
    let open = "- [ ] **OI-38** Park 2017's protocol.\n- [x] **OI-3** closed.\n";
    let closed = "- [x] **OI-38** Park 2017's protocol (closed by a maintainer).\n";
    let p = v.provisional_open_item(open).unwrap().unwrap();
    assert_eq!((p.open_item, p.open), ("OI-38", true));
    let label = p.to_string();
    assert!(label.contains("PROVISIONAL") && label.contains("revert"));
    let p = v.provisional_open_item(closed).unwrap().unwrap();
    assert!(!p.open);
    assert!(p.to_string().contains("stale"));
    for missing in [
        "",
        "- [ ] **OI-380** another item.\n",
        "- [x] **OI-3** closed.\n",
    ] {
        let e = v.provisional_open_item(missing).unwrap_err();
        assert_eq!(e.code(), ErrorCode::VectorSchema);
        assert!(e.to_string().contains("provisional"));
    }
    assert!(
        load(&fixture("", ""))
            .unwrap()
            .get("FIXTURE-A")
            .unwrap()
            .provisional_open_item(open)
            .unwrap()
            .is_none()
    );

    // The OI reference cannot be blanked, truncated or replaced by a non-string.
    for bad in [
        "\"\"",
        "\"   \"",
        "\"OI-\"",
        "\"38\"",
        "\"OI-38 \"",
        "\"oi-38\"",
        "\"OI-7\"", // ADR-0004 revision 2026-10-21: `^OI-\d{2,}$`
    ] {
        assert!(
            schema_error(&with(bad)).contains("provisional"),
            "provisional = {bad} was accepted"
        );
    }
    for bad in ["true", "38", "[\"OI-38\"]"] {
        assert_eq!(
            load(&with(bad)).unwrap_err().code(),
            ErrorCode::VectorParse,
            "provisional = {bad} was accepted"
        );
    }

    // Never both: a provisional vector gates, a blocked one does not.
    let both = fixture(
        "verified_by",
        "provisional = \"OI-38\"\nblocked_by = \"OI-38\"\nverified_by",
    );
    let e = schema_error(&both);
    assert!(e.contains("provisional") && e.contains("blocked_by"), "{e}");
    let both_other = fixture(
        "verified_by",
        "blocked_by = \"OI-24\"\nprovisional = \"OI-38\"\nverified_by",
    );
    assert!(schema_error(&both_other).contains("blocked_by"));
}

/// Every committed provisional vector names an item that the committed OPEN-ITEMS.md lists, and
/// the PHY-012 rows asserted under P-remove (ADR-0014) stay provisional while OI-38 is open: the
/// label can only be dropped together with closing OI-38.
///
/// Verifies: TST-002, PHY-012
#[test]
fn workspace_provisional_vectors_reference_recorded_open_items() {
    let set = VectorSet::load_workspace().unwrap();
    let open_items = std::fs::read_to_string(
        VectorSet::canonical_dir().join("../../docs/requirements/OPEN-ITEMS.md"),
    )
    .unwrap();
    for (path, v) in set.iter() {
        let state = v.provisional_open_item(&open_items);
        assert!(state.is_ok(), "{} in {}: {state:?}", v.id, path.display());
    }
    let oi38_open = open_items
        .lines()
        .any(|l| l.trim_start().starts_with("- [ ] **OI-38**"));
    for row in [
        "VENUS",
        "EARTH-MOON",
        "MARS",
        "JUPITER",
        "SATURN",
        "URANUS",
        "NEPTUNE",
    ] {
        let v = set.get(&format!("PARK17-T3-{row}")).unwrap();
        if oi38_open {
            assert_eq!(v.provisional.as_deref(), Some("OI-38"), "{}", v.id);
            assert!(v.blocked_by.is_none(), "{}: PHY-012 rows gate", v.id);
        }
    }
}
