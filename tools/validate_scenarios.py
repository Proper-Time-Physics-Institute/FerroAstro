# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Validate scenario files against the committed scenario JSON Schema (CI-002).

The schema ``ci/scenarios/scenario.schema.json`` is generated from the Rust types of
``ferroastro-scenario`` (a Rust test fails if it drifts), so FerroAstro and the Python oracle
runners check the same files against the same rules. An unknown force switch is rejected
because ``[forces]`` admits no additional properties (REF-011).

Usage: ``uv run python validate_scenarios.py [files...]`` (default: every
``ci/scenarios/*.toml``). Exit code 0 when every file is valid, 1 otherwise.
"""

from __future__ import annotations

import json
import sys
import tomllib
from collections.abc import Iterable
from pathlib import Path
from typing import Any

import jsonschema

ROOT = Path(__file__).resolve().parent.parent
SCENARIO_DIR = ROOT / "ci" / "scenarios"
SCHEMA_PATH = SCENARIO_DIR / "scenario.schema.json"


def load_schema(path: Path = SCHEMA_PATH) -> dict[str, Any]:
    """Return the scenario JSON Schema."""
    schema: dict[str, Any] = json.loads(path.read_text(encoding="utf-8"))
    return schema


def validate_text(text: str, schema: dict[str, Any]) -> list[str]:
    """Return the schema violations of one scenario's TOML text (empty when valid)."""
    try:
        data = tomllib.loads(text)
    except tomllib.TOMLDecodeError as exc:
        return [f"invalid TOML: {exc}"]
    validator = jsonschema.Draft202012Validator(schema)
    return [
        f"{'/'.join(str(p) for p in err.absolute_path) or '<root>'}: {err.message}"
        for err in sorted(validator.iter_errors(data), key=lambda e: list(e.absolute_path))
    ]


def validate_files(paths: Iterable[Path], schema: dict[str, Any]) -> list[str]:
    """Return ``file: problem`` lines for every invalid file."""
    problems: list[str] = []
    for path in paths:
        problems.extend(
            f"{path.name}: {p}" for p in validate_text(path.read_text(encoding="utf-8"), schema)
        )
    return problems


def main(argv: list[str]) -> int:
    """Validate the given files (default: all committed scenarios) and print a summary."""
    paths = [Path(a) for a in argv] if argv else sorted(SCENARIO_DIR.glob("*.toml"))
    problems = validate_files(paths, load_schema())
    for line in problems:
        print(line)
    print(f"{len(paths)} scenario file(s), {len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
