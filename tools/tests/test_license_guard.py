# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for the tools/ licence guard (``license_guard.py``)."""

from __future__ import annotations

from importlib import metadata
from pathlib import Path

import pytest

import license_guard


def make_dist(root: Path, name: str, *headers: str) -> metadata.Distribution:
    """Write a fake ``.dist-info`` under ``root`` and return its Distribution."""
    info = root / f"{name}-1.0.dist-info"
    info.mkdir()
    lines = ["Metadata-Version: 2.4", f"Name: {name}", "Version: 1.0", *headers]
    (info / "METADATA").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return next(d for d in metadata.distributions(path=[str(root)]) if d.metadata["Name"] == name)


# Verifies: LIC-001, STD-009
@pytest.mark.parametrize(
    "header",
    [
        "License-Expression: GPL-3.0-or-later",
        "License-Expression: AGPL-3.0-only",
        "License: GNU Lesser General Public License v2.1",
        "Classifier: License :: OSI Approved :: GNU General Public License v3 (GPLv3)",
        "Classifier: License :: OSI Approved :: GNU Lesser General Public License v3 (LGPLv3)",
    ],
)
def test_fake_gpl_metadata_trips_the_guard(tmp_path: Path, header: str) -> None:
    """Negative control: a fake GPL/LGPL/AGPL distribution is reported."""
    dist = make_dist(tmp_path, "fakecopyleft", header)
    findings = license_guard.find_violations([dist])
    assert len(findings) == 1
    assert findings[0].startswith("fakecopyleft:")


# Verifies: LIC-001
@pytest.mark.parametrize(
    "header",
    [
        "License-Expression: Apache-2.0",
        "License-Expression: MIT",
        "License: BSD-3-Clause",
        "Classifier: License :: OSI Approved :: MIT License",
    ],
)
def test_permissive_metadata_passes(tmp_path: Path, header: str) -> None:
    """Permissive licences are not flagged."""
    assert license_guard.find_violations([make_dist(tmp_path, "fakepermissive", header)]) == []


# Verifies: LIC-005
def test_exemption_covers_only_the_named_tool(tmp_path: Path) -> None:
    """`reuse` is exempt (CLI only); the same licence on another name is not."""
    gpl = "License-Expression: GPL-3.0-or-later"
    assert license_guard.find_violations([make_dist(tmp_path, "reuse", gpl)]) == []
    other = tmp_path / "other"
    other.mkdir()
    assert license_guard.find_violations([make_dist(other, "notreuse", gpl)])


# Verifies: LIC-001, LIC-005
def test_oracle_packages_are_never_exempt(tmp_path: Path) -> None:
    """A GPL oracle package in tools/ fails even with a permissive label or an exemption."""
    dist = make_dist(tmp_path, "rebound", "License-Expression: MIT")
    assert license_guard.find_violations([dist], exempt={"rebound": "x"})
    assert not set(license_guard.EXEMPT) & license_guard.ORACLE_NAMES


# Verifies: STD-009, LIC-001
def test_installed_tools_environment_is_clean() -> None:
    """The real locked environment resolves no GPL/LGPL/AGPL distribution."""
    assert license_guard.find_violations(metadata.distributions()) == []
