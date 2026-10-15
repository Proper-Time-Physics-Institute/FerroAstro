# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""The committed scenarios validate against the generated schema in Python (WP13 check 1)."""

from __future__ import annotations

import validate_scenarios


# Verifies: CI-002
def test_every_committed_scenario_validates() -> None:
    """Every ``ci/scenarios/*.toml`` is valid under the committed JSON Schema."""
    paths = sorted(validate_scenarios.SCENARIO_DIR.glob("*.toml"))
    assert paths
    assert validate_scenarios.validate_files(paths, validate_scenarios.load_schema()) == []
    assert validate_scenarios.main([]) == 0


# Verifies: CI-002, REF-011
def test_unknown_switch_and_bad_values_are_rejected() -> None:
    """An unknown force switch, a wrong mode and a missing required key are each rejected."""
    schema = validate_scenarios.load_schema()
    base = (validate_scenarios.SCENARIO_DIR / "sun-mercury-newton-header440.toml").read_text(
        encoding="utf-8"
    )
    assert validate_scenarios.validate_text(base, schema) == []
    unknown = validate_scenarios.validate_text(
        base.replace("newton = true", "newton = true\nfifth_force = true"), schema
    )
    assert any("fifth_force" in p for p in unknown), unknown
    assert validate_scenarios.validate_text(base.replace('"n-body"', '"nbody"'), schema)
    assert validate_scenarios.validate_text(base.replace("newton = true", ""), schema)
    assert validate_scenarios.validate_text("schema_version = [", schema)
