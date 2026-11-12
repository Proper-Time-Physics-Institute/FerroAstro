# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Lint-config canary (STD-003, TST-001; ADR-0010 ruling on #26).

``cargo clippy`` alone stays green if someone deletes the lint keys from the root Cargo.toml, so
this gate checks the configuration and proves the lints still bite:

1. **Config check.** The root ``Cargo.toml`` has ``[workspace.lints.rust] unsafe_code = "forbid"``
   and ``[workspace.lints.clippy] float_cmp = "deny"``, and every workspace member inherits them
   with ``[lints] workspace = true`` (STD-002/003, TST-001).
2. **Compile-fail canaries.** Each fixture under ``tools/canary/`` is built as a throwaway crate in
   a temporary directory, outside the workspace. A crate outside the workspace cannot inherit, so
   the root ``[workspace.lints]`` tables are copied into the fixture's ``[lints]`` at run time:
   deleting a key from the root therefore makes its canary pass clippy, and this gate fail.
   ``unsafe_block`` and ``float_cmp`` must FAIL clippy with their lint named in the output;
   ``clean`` is a control that must PASS, so a canary cannot fail for a harness reason.

Usage (from ``tools/``)::

    uv run python lint_canary.py               # config check + canaries (runs cargo clippy)
    uv run python lint_canary.py --config-only # config check only
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
CANARY_DIR = Path("tools") / "canary"
TARGET_DIR = Path("target") / "lint-canary"

# (lint table, lint name, required level) checked in the root [workspace.lints].
REQUIRED_LINTS: tuple[tuple[str, str, str], ...] = (
    ("rust", "unsafe_code", "forbid"),  # STD-003
    ("clippy", "float_cmp", "deny"),  # TST-001
)


@dataclass(frozen=True)
class Canary:
    """One fixture crate: its directory name, the lint it exercises, and whether clippy fails."""

    name: str
    lint: str | None
    must_fail: bool


CANARIES: tuple[Canary, ...] = (
    Canary("clean", None, must_fail=False),
    Canary("unsafe_block", "unsafe_code", must_fail=True),
    Canary("float_cmp", "clippy::float_cmp", must_fail=True),
)


def _level(value: object) -> object:
    """Lint level from ``"deny"`` or ``{ level = "deny", priority = -1 }``."""
    return value.get("level") if isinstance(value, dict) else value


def _member_manifests(root: Path, workspace: dict[str, Any]) -> list[Path]:
    """Expand ``[workspace] members`` globs (minus ``exclude``) to member ``Cargo.toml`` paths."""
    excluded = {(root / e).resolve() for e in workspace.get("exclude", [])}
    manifests: set[Path] = set()
    for pattern in workspace.get("members", []):
        for path in root.glob(pattern):
            manifest = path / "Cargo.toml"
            if path.resolve() not in excluded and manifest.is_file():
                manifests.add(manifest)
    return sorted(manifests)


def check_config(root: Path = ROOT) -> list[str]:
    """Return findings for the root lint config and every member's lint inheritance."""
    manifest = root / "Cargo.toml"
    data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    workspace = data.get("workspace", {})
    lints = workspace.get("lints", {})
    findings = [
        f"Cargo.toml: [workspace.lints.{table}] {name} must be {level!r}, "
        f"found {_level(lints.get(table, {}).get(name))!r}"
        for table, name, level in REQUIRED_LINTS
        if _level(lints.get(table, {}).get(name)) != level
    ]
    members = _member_manifests(root, workspace)
    if not members:
        findings.append("Cargo.toml: [workspace] members matched no crate")
    for member in members:
        member_data = tomllib.loads(member.read_text(encoding="utf-8"))
        if member_data.get("lints", {}).get("workspace") is not True:
            rel = member.relative_to(root).as_posix()
            findings.append(f"{rel}: missing `[lints] workspace = true`")
    return findings


def _toml_value(value: object) -> str:
    """Serialise the scalar or inline-table values that appear in a lint table."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str | int):
        return json.dumps(value)  # a JSON string is a valid TOML basic string
    if isinstance(value, dict):
        return "{ " + ", ".join(f"{k} = {_toml_value(v)}" for k, v in value.items()) + " }"
    msg = f"unsupported lint value {value!r}"
    raise TypeError(msg)


def fixture_manifest(name: str, root_data: dict[str, Any]) -> str:
    """Cargo.toml for a fixture crate, with the root ``[workspace.lints]`` copied into ``[lints]``.

    The empty ``[workspace]`` table makes the fixture its own workspace root.
    """
    edition = root_data.get("workspace", {}).get("package", {}).get("edition", "2024")
    lines = [
        "[package]",
        f'name = "lint-canary-{name.replace("_", "-")}"',
        'version = "0.0.0"',
        f"edition = {_toml_value(edition)}",
        "publish = false",
        "",
        "[lib]",
        'path = "lib.rs"',
        "",
        "[workspace]",
    ]
    for table, entries in root_data.get("workspace", {}).get("lints", {}).items():
        lines += ["", f"[lints.{table}]"]
        lines += [f"{key} = {_toml_value(value)}" for key, value in entries.items()]
    return "\n".join(lines) + "\n"


def _lint_named(output: str, lint: str) -> bool:
    """Whether rustc/clippy output names ``lint`` (in either ``_`` or ``-`` spelling)."""
    return lint.replace("-", "_") in output.replace("-", "_")


def run_canaries(root: Path = ROOT) -> list[str]:
    """Build each fixture with ``cargo clippy`` and return findings for unexpected results."""
    root_data = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    findings: list[str] = []
    with tempfile.TemporaryDirectory(prefix="fa-lint-canary-") as tmp:
        for canary in CANARIES:
            crate = Path(tmp) / canary.name
            crate.mkdir()
            source = root / CANARY_DIR / canary.name / "lib.rs"
            (crate / "lib.rs").write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
            (crate / "Cargo.toml").write_text(
                fixture_manifest(canary.name, root_data), encoding="utf-8"
            )
            # cwd=root so rustup picks the pinned toolchain from rust-toolchain.toml.
            proc = subprocess.run(  # noqa: S603 - fixed argv, no shell
                [  # noqa: S607 - cargo is resolved from PATH like every other gate
                    "cargo",
                    "clippy",
                    "--quiet",
                    "--manifest-path",
                    str(crate / "Cargo.toml"),
                    "--target-dir",
                    str(root / TARGET_DIR),
                ],
                cwd=root,
                capture_output=True,
                text=True,
                check=False,
            )
            output = proc.stdout + proc.stderr
            failed = proc.returncode != 0
            if canary.must_fail and not failed:
                findings.append(
                    f"canary {canary.name}: clippy PASSED, so `{canary.lint}` no longer bites "
                    "(was the key removed from [workspace.lints]?)"
                )
            elif canary.must_fail and canary.lint and not _lint_named(output, canary.lint):
                findings.append(
                    f"canary {canary.name}: clippy failed without naming `{canary.lint}`:\n{output}"
                )
            elif not canary.must_fail and failed:
                findings.append(f"control {canary.name}: clippy FAILED:\n{output}")
            else:
                verdict = "fails as required" if canary.must_fail else "passes (control)"
                print(f"lint_canary: {canary.name}: {verdict}")
    return findings


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--config-only", action="store_true", help="skip the clippy canaries")
    args = parser.parse_args(argv)
    findings = check_config()
    if not args.config_only:
        findings += run_canaries()
    for finding in findings:
        print(finding, file=sys.stderr)
    print(f"lint_canary: {len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
