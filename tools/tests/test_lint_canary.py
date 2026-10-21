# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for the lint-config canary (``lint_canary.py``)."""

from __future__ import annotations

import subprocess
import tomllib
from collections.abc import Callable
from pathlib import Path

import pytest

import lint_canary

GOOD_ROOT = """\
[workspace]
members = ["crates/*"]
exclude = ["crates/outside"]

[workspace.package]
edition = "2024"

[workspace.lints.rust]
unsafe_code = "forbid"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
float_cmp = "deny"
"""
INHERITS = '[package]\nname = "a"\n\n[lints]\nworkspace = true\n'
NO_LINTS = '[package]\nname = "b"\n'


def make_repo(tmp_path: Path, root: str = GOOD_ROOT, **members: str) -> Path:
    (tmp_path / "Cargo.toml").write_text(root, encoding="utf-8")
    for name, manifest in (members or {"a": INHERITS}).items():
        (tmp_path / "crates" / name).mkdir(parents=True)
        (tmp_path / "crates" / name / "Cargo.toml").write_text(manifest, encoding="utf-8")
    return tmp_path


# Verifies: STD-003, TST-001
def test_repo_config_passes() -> None:
    assert lint_canary.check_config() == []


# Verifies: STD-003, TST-001
def test_good_config_passes(tmp_path: Path) -> None:
    assert lint_canary.check_config(make_repo(tmp_path)) == []


# Verifies: STD-003
def test_missing_unsafe_forbid_flagged(tmp_path: Path) -> None:
    root = make_repo(tmp_path, GOOD_ROOT.replace('unsafe_code = "forbid"\n', ""))
    assert [f for f in lint_canary.check_config(root) if "unsafe_code" in f]


# Verifies: STD-003
def test_unsafe_downgraded_to_deny_flagged(tmp_path: Path) -> None:
    root = make_repo(tmp_path, GOOD_ROOT.replace('"forbid"', '"deny"'))
    assert [f for f in lint_canary.check_config(root) if "unsafe_code" in f]


# Verifies: TST-001
def test_missing_float_cmp_flagged(tmp_path: Path) -> None:
    root = make_repo(tmp_path, GOOD_ROOT.replace('float_cmp = "deny"\n', ""))
    assert [f for f in lint_canary.check_config(root) if "float_cmp" in f]


# Verifies: TST-001
def test_float_cmp_as_table_accepted(tmp_path: Path) -> None:
    root = make_repo(
        tmp_path, GOOD_ROOT.replace('float_cmp = "deny"', 'float_cmp = { level = "deny" }')
    )
    assert lint_canary.check_config(root) == []


# Verifies: STD-003, TST-001
def test_member_without_inheritance_flagged(tmp_path: Path) -> None:
    root = make_repo(tmp_path, a=INHERITS, b=NO_LINTS, outside=NO_LINTS)
    assert lint_canary.check_config(root) == [
        "crates/b/Cargo.toml: missing `[lints] workspace = true`"
    ]


# Verifies: STD-003, TST-001
def test_fixture_manifest_copies_root_lints() -> None:
    root_data = tomllib.loads(GOOD_ROOT)
    fixture = tomllib.loads(lint_canary.fixture_manifest("float_cmp", root_data))
    assert fixture["lints"] == root_data["workspace"]["lints"]
    assert fixture["workspace"] == {}
    assert fixture["package"]["edition"] == "2024"


# Verifies: TST-001
def test_fixture_manifest_drops_deleted_key() -> None:
    root_data = tomllib.loads(GOOD_ROOT.replace('float_cmp = "deny"\n', ""))
    fixture = tomllib.loads(lint_canary.fixture_manifest("float_cmp", root_data))
    assert "float_cmp" not in fixture["lints"]["clippy"]


Run = Callable[..., subprocess.CompletedProcess[str]]


def _fake_clippy(results: dict[str, tuple[int, str]]) -> Run:
    def run(argv: list[str], **_: object) -> subprocess.CompletedProcess[str]:
        crate = Path(argv[argv.index("--manifest-path") + 1]).parent.name
        code, stderr = results[crate]
        return subprocess.CompletedProcess(argv, code, "", stderr)

    return run


OK_RESULTS = {
    "clean": (0, ""),
    "unsafe_block": (101, "error: usage of an `unsafe` block\n-F unsafe-code"),
    "float_cmp": (101, "error: strict comparison\n-D clippy::float-cmp"),
}


# Verifies: STD-003, TST-001
@pytest.mark.parametrize(
    ("crate", "result", "expect"),
    [
        (None, None, None),
        ("float_cmp", (0, ""), "no longer bites"),
        ("unsafe_block", (0, ""), "no longer bites"),
        ("float_cmp", (101, "error: missing docs"), "without naming"),
        ("clean", (101, "error: missing docs"), "control clean"),
    ],
)
def test_canary_verdicts(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    crate: str | None,
    result: tuple[int, str] | None,
    expect: str | None,
) -> None:
    root = make_repo(tmp_path)
    for canary in lint_canary.CANARIES:
        src = root / lint_canary.CANARY_DIR / canary.name
        src.mkdir(parents=True)
        (src / "lib.rs").write_text("//! fixture\n", encoding="utf-8")
    results = dict(OK_RESULTS)
    if crate is not None and result is not None:
        results[crate] = result
    monkeypatch.setattr(subprocess, "run", _fake_clippy(results))
    findings = lint_canary.run_canaries(root)
    if expect is None:
        assert findings == []
    else:
        assert len(findings) == 1
        assert expect in findings[0]
