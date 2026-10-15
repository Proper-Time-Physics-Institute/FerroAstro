# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
# Verifies: REF-009, TST-003
"""Shape checks for the pyerfa golden generator (skipped unless the `goldens` group is synced)."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType

import pytest

TOOLS = Path(__file__).resolve().parent.parent


def load() -> ModuleType:
    pytest.importorskip("erfa")
    spec = importlib.util.spec_from_file_location("erfa_time", TOOLS / "goldens" / "erfa_time.py")
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Verifies: REF-009
def test_epochs_span_1550_2650_with_at_least_200_dates() -> None:
    mod = load()
    years = [e[1] for e in mod.epochs()]
    assert len(years) >= 200
    assert min(years) == mod.FIRST_YEAR
    assert max(years) == mod.LAST_YEAR


# Verifies: TST-003
def test_header_records_generator_engine_and_pin(tmp_path: Path) -> None:
    mod = load()
    mod.main(["--out-dir", str(tmp_path)])
    text = (tmp_path / "erfa-time-pyerfa.csv").read_text(encoding="utf-8")
    assert "generated_by: tools/goldens/erfa_time.py@" in text
    assert f"engine: pyerfa {mod.PYERFA_PIN} (ERFA 2.0.1, SOFA 20231011)" in text
    rows = [line for line in text.splitlines() if line and not line.startswith("#")]
    assert len(rows) - 1 >= 200
    assert len(rows[1].split(",")) == len(mod.TIME_COLUMNS)
