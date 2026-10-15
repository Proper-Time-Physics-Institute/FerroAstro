# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for the crate layering check (CRATE-PLAN §1).

No requirement covers crate layering yet (NEEDS HUMAN REVIEW, IMPLEMENTATION-NOTES WP6), so these
tests cite none.
"""

from __future__ import annotations

import shutil
from typing import Any

import pytest

import check_layers


def _meta(deps: dict[str, list[tuple[str, str | None]]]) -> dict[str, Any]:
    """Synthetic ``cargo metadata`` output: crate -> [(dependency, kind)]."""
    packages = [
        {
            "id": f"{name} 0.0.0",
            "name": name,
            "dependencies": [{"name": d, "kind": k, "rename": None} for d, k in ds],
        }
        for name, ds in deps.items()
    ]
    return {"workspace_members": [p["id"] for p in packages], "packages": packages}


def _findings(deps: dict[str, list[tuple[str, str | None]]]) -> list[str]:
    names, edges = check_layers.workspace_edges(_meta(deps))
    return check_layers.check(names, edges, exceptions={})


GOOD: dict[str, list[tuple[str, str | None]]] = {
    "ferroastro-core": [("thiserror", None)],
    "ferroastro-vectors": [("ferroastro-core", None)],
    "ferroastro-analytic": [("ferroastro-core", None), ("ferroastro-vectors", "dev")],
    "ferroastro-precession": [("ferroastro-analytic", None)],
    "ferroastro-ladder": [("ferroastro-precession", None), ("ferroastro-vectors", "dev")],
}


def test_clean_graph_passes() -> None:
    assert _findings(GOOD) == []


def test_upward_dependency_fails() -> None:
    deps = dict(GOOD)
    deps["ferroastro-precession"] = [("ferroastro-analytic", None), ("ferroastro-ladder", None)]
    found = _findings(deps)
    assert any("ferroastro-precession (L5) depends on ferroastro-ladder (L10)" in f for f in found)
    assert any(f.startswith("dependency cycle") for f in found)


def test_same_layer_and_upward_dev_dependency_fail() -> None:
    deps = dict(GOOD)
    deps["ferroastro-integrators"] = [("ferroastro-analytic", None)]
    deps["ferroastro-analytic"] = [("ferroastro-core", None), ("ferroastro-ladder", "dev")]
    found = _findings(deps)
    assert any(
        "ferroastro-integrators (L4) depends on ferroastro-analytic (L4)" in f for f in found
    )
    assert any("dev-depends on ferroastro-ladder" in f for f in found)


def test_dev_only_cycle_and_test_support_runtime_use_fail() -> None:
    deps = dict(GOOD)
    deps["ferroastro-vectors"] = [("ferroastro-core", None), ("ferroastro-ladder", "dev")]
    deps["ferroastro-precession"] = [("ferroastro-analytic", None), ("ferroastro-vectors", None)]
    found = _findings(deps)
    assert any("test-support crate ferroastro-vectors" in f for f in found)
    assert any(f.startswith("dependency cycle") for f in found)


def test_self_dev_dependency_is_not_an_edge_but_self_runtime_cycle_is() -> None:
    # ferroastro-time dev-depends on itself to enable its `hifitime` feature in its own tests.
    assert _findings({"ferroastro-core": [], "ferroastro-time": [("ferroastro-time", "dev")]}) == []
    found = _findings({"ferroastro-core": [], "ferroastro-time": [("ferroastro-time", None)]})
    assert any("cycle" in f for f in found)


def test_unplaced_crate_fails() -> None:
    deps = dict(GOOD)
    deps["ferroastro-newthing"] = [("ferroastro-core", None)]
    assert any("ferroastro-newthing: no layer" in f for f in _findings(deps))


@pytest.mark.skipif(shutil.which("cargo") is None, reason="cargo not installed")
def test_workspace_obeys_layers() -> None:
    assert check_layers.check_repo() == []
