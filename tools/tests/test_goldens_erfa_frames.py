# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
# Verifies: REF-009, TST-003
"""Shape checks for the pyerfa frames golden generator (skipped without the `goldens` group)."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType

import pytest

TOOLS = Path(__file__).resolve().parent.parent


def load() -> ModuleType:
    pytest.importorskip("erfa")
    spec = importlib.util.spec_from_file_location(
        "erfa_frames", TOOLS / "goldens" / "erfa_frames.py"
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Verifies: REF-009
def test_at_least_50_epochs_spanning_1550_2650() -> None:
    mod = load()
    years = [int(mod.erfa.jd2cal(tt1, tt2)[0]) for _, tt1, tt2 in mod.epochs()]
    assert len(years) >= 50
    assert min(years) == mod.FIRST_YEAR
    assert max(years) == mod.LAST_YEAR


# Verifies: TST-003
def test_header_records_generator_engine_and_pin(tmp_path: Path) -> None:
    mod = load()
    mod.main(["--out-dir", str(tmp_path)])
    text = (tmp_path / "erfa-frames-pyerfa.csv").read_text(encoding="utf-8")
    assert "generated_by: tools/goldens/erfa_frames.py@" in text
    assert f"engine: pyerfa {mod.PYERFA_PIN} (ERFA 2.0.1, SOFA 20231011)" in text
    rows = [line for line in text.splitlines() if line and not line.startswith("#")]
    assert len(rows) - 1 >= 50
    assert all(len(r.split(",")) == len(mod.COLUMNS) for r in rows)
