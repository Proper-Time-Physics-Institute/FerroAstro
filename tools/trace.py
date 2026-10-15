# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Requirement traceability: requirements -> citing tests -> test vectors (ADR-0010, ORG-003/004).

Usage (from ``tools/``)::

    uv run python trace.py          # regenerate docs/requirements/TRACE.md
    uv run python trace.py --check  # CI gate; never writes

``--check`` fails (exit 1) on any of:

* a malformed requirement block (missing key, bad enum, malformed or duplicate ID, dangling
  ``parent``/``related``) (ORG-003);
* a test or vector citing an unknown requirement ID (ORG-004);
* an ``approved`` (or later) requirement with ``verification: test`` and no citing test (ORG-004);
* a committed TRACE.md that differs from the generated one (ORG-004);
* a vector file that violates ``tests/vectors/schema.json`` (TST-002/003);
* a bare physical literal in test code (TST-002, ``lint_literals.py``);
* a third-party GitHub Action not pinned to a commit SHA (STD-012, ``lint_workflows.py``);
* an upward, same-layer or cyclic crate dependency (CRATE-PLAN §1, ``check_layers.py``).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import jsonschema
import yaml

import check_layers
import lint_literals
import lint_workflows

ROOT = Path(__file__).resolve().parent.parent
REQ_DIR = ROOT / "docs" / "requirements"
TRACE_MD = REQ_DIR / "TRACE.md"
VECTOR_DIR = ROOT / "tests" / "vectors"
NON_CATALOGUE = {"README.md", "OPEN-ITEMS.md", "TRACE.md"}

PREFIXES = ("ORG", "STD", "PHY", "NUM", "TST", "API", "AGT", "DEP", "REF", "CI", "LIC")
ID_RE = re.compile(r"^(" + "|".join(PREFIXES) + r")-\d{3}$")
REQUIRED_KEYS = (
    "id",
    "title",
    "status",
    "criticality",
    "verification",
    "statement",
    "acceptance",
)
ENUMS: dict[str, tuple[str, ...]] = {
    "status": ("draft", "approved", "implemented", "verified", "retired"),
    "criticality": ("high", "medium", "low"),
    "verification": ("test", "analysis", "inspection", "demonstration"),
}
# Statuses from which a test-verified requirement must be cited by at least one test.
COVERAGE_STATUSES = ("approved", "implemented", "verified")

YAML_BLOCK_RE = re.compile(r"^```yaml\n(.*?)^```", re.DOTALL | re.MULTILINE)
CITE_RE = re.compile(r"^\s*(?://[/!]?|#)\s*Verifies:\s*(.+?)\s*$")
CITED_ID_RE = re.compile(r"[A-Z]+-\d+")
RUST_FN_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+(\w+)")
PY_FN_RE = re.compile(r"^\s*(?:async\s+)?def\s+(\w+)")
SKIP_DIRS = {"target", ".venv", ".git", "node_modules", "__pycache__", ".mypy_cache"}


@dataclass
class Requirement:
    """One parsed requirement block."""

    id: str
    title: str
    status: str
    criticality: str
    verification: str
    file: str
    parent: str | None = None
    related: list[str] = field(default_factory=list)


@dataclass
class Report:
    """Everything trace.py learns about the repository."""

    requirements: dict[str, Requirement]
    tests: dict[str, list[str]]
    vectors: dict[str, list[str]]
    errors: list[str]


def _parse_block(text: str, where: str, stem: str) -> tuple[Requirement | None, list[str]]:
    """Parse and validate one YAML requirement block."""
    try:
        data = yaml.safe_load(text)
    except yaml.YAMLError as exc:
        return None, [f"{where}: invalid YAML: {exc}"]
    if not isinstance(data, dict):
        return None, [f"{where}: block is not a mapping"]
    missing = [k for k in REQUIRED_KEYS if not data.get(k)]
    if missing:
        return None, [f"{where}: missing keys {missing}"]
    rid = str(data["id"])
    if not ID_RE.match(rid):
        return None, [f"{where}: malformed id {rid!r}"]
    errors: list[str] = []
    prefix = rid.split("-", maxsplit=1)[0]
    if prefix != stem:
        errors.append(f"{where}: {rid} is filed in {stem}.md, not {prefix}.md")
    errors += [
        f"{where}: {rid} {key}={data[key]!r} not in {allowed}"
        for key, allowed in ENUMS.items()
        if data[key] not in allowed
    ]
    req = Requirement(
        id=rid,
        title=str(data["title"]),
        status=str(data["status"]),
        criticality=str(data["criticality"]),
        verification=str(data["verification"]),
        file=f"{stem}.md",
        parent=data.get("parent"),
        related=[str(r) for r in data.get("related") or []],
    )
    return req, errors


def parse_requirements(req_dir: Path) -> tuple[dict[str, Requirement], list[str]]:
    """Parse every fenced YAML block in the catalogue files and validate it (ORG-003)."""
    reqs: dict[str, Requirement] = {}
    errors: list[str] = []
    for path in sorted(req_dir.glob("*.md")):
        if path.name in NON_CATALOGUE:
            continue
        for n, match in enumerate(YAML_BLOCK_RE.finditer(path.read_text(encoding="utf-8")), 1):
            where = f"{path.name} block {n}"
            req, block_errors = _parse_block(match.group(1), where, path.stem)
            errors += block_errors
            if req is None:
                continue
            if req.id in reqs:
                errors.append(f"{where}: duplicate id {req.id} (first in {reqs[req.id].file})")
                continue
            reqs[req.id] = req
    for req in reqs.values():
        for ref in [req.parent, *req.related]:
            if ref is not None and ref not in reqs:
                errors.append(f"{req.id}: parent/related {ref!r} does not exist")
    return reqs, errors


def _source_files(root: Path) -> list[Path]:
    """Rust and Python files that may carry ``Verifies:`` citations."""
    files: list[Path] = []
    for pattern in ("*.rs", "*.py"):
        for path in root.rglob(pattern):
            rel = path.relative_to(root)
            if SKIP_DIRS.isdisjoint(rel.parts):
                files.append(path)
    return sorted(files)


def scan_citations(root: Path) -> tuple[dict[str, list[str]], list[str]]:
    """Map requirement ID -> ``file::test`` for every ``Verifies:`` comment."""
    cites: dict[str, list[str]] = defaultdict(list)
    errors: list[str] = []
    for path in _source_files(root):
        rel = path.relative_to(root).as_posix()
        fn_re = RUST_FN_RE if path.suffix == ".rs" else PY_FN_RE
        lines = path.read_text(encoding="utf-8").splitlines()
        for i, line in enumerate(lines):
            match = CITE_RE.match(line)
            if not match:
                continue
            target = next((m.group(1) for ln in lines[i + 1 :] if (m := fn_re.match(ln))), None)
            if target is None:
                errors.append(f"{rel}:{i + 1}: 'Verifies:' is not followed by a function")
                continue
            for rid in CITED_ID_RE.findall(match.group(1)):
                cites[rid].append(f"{rel}::{target}")
    return dict(cites), errors


def scan_vectors(vector_dir: Path) -> tuple[dict[str, list[str]], list[str]]:
    """Validate vector files against schema.json; map requirement ID -> vector ids (TST-002/003)."""
    out: dict[str, list[str]] = defaultdict(list)
    errors: list[str] = []
    schema_path = vector_dir / "schema.json"
    if not schema_path.exists():
        return {}, [f"{schema_path.relative_to(ROOT)} is missing"]
    schema: dict[str, Any] = json.loads(schema_path.read_text(encoding="utf-8"))
    validator = jsonschema.Draft202012Validator(schema)
    seen: dict[str, str] = {}
    for path in sorted(vector_dir.glob("*.toml")):
        try:
            data = tomllib.loads(path.read_text(encoding="utf-8"))
        except tomllib.TOMLDecodeError as exc:
            errors.append(f"{path.name}: invalid TOML: {exc}")
            continue
        for err in validator.iter_errors(data):
            loc = "/".join(str(p) for p in err.absolute_path)
            errors.append(f"{path.name}: schema: {loc}: {err.message}")
        for vec in data.get("vector", []):
            vid = str(vec.get("id", "?"))
            if vid in seen:
                errors.append(f"{path.name}: duplicate vector id {vid} (first in {seen[vid]})")
            seen[vid] = path.name
            for rid in vec.get("requirement", []):
                out[str(rid)].append(vid)
    return dict(out), errors


def build_report(root: Path = ROOT) -> Report:
    """Collect requirements, citations and vectors, and every consistency error."""
    reqs, errors = parse_requirements(root / "docs" / "requirements")
    tests, cite_errors = scan_citations(root)
    vectors, vec_errors = scan_vectors(root / "tests" / "vectors")
    errors += cite_errors + vec_errors
    for kind, mapping in (("test", tests), ("vector", vectors)):
        for rid, users in sorted(mapping.items()):
            if rid not in reqs:
                errors.append(f"{kind} cites unknown requirement {rid}: {', '.join(users)}")
    for req in reqs.values():
        if req.status in COVERAGE_STATUSES and req.verification == "test" and not tests.get(req.id):
            errors.append(f"{req.id} is {req.status} with verification: test but no citing test")
    return Report(reqs, tests, vectors, errors)


def render(report: Report) -> str:
    """Render TRACE.md deterministically (no timestamps, sorted everywhere)."""
    reqs = report.requirements
    by_status: dict[str, int] = defaultdict(int)
    for r in reqs.values():
        by_status[r.status] += 1
    test_reqs = [r for r in reqs.values() if r.verification == "test"]
    covered = sum(1 for r in test_reqs if report.tests.get(r.id))
    status_line = ", ".join(f"{s} {by_status[s]}" for s in ENUMS["status"] if by_status[s])
    lines = [
        # REUSE-IgnoreStart
        "<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->",
        "<!-- SPDX-License-Identifier: Apache-2.0 -->",
        # REUSE-IgnoreEnd
        "",
        "# Requirement traceability matrix",
        "",
        "> **Generated by `tools/trace.py`. Do not edit by hand (ORG-004).** Regenerate with",
        "> `(cd tools && uv run python trace.py)`. CI evidence for a commit is its `ci`",
        "> workflow run; run URLs are not written here so that the file stays deterministic.",
        "",
        f"- Requirements: **{len(reqs)}** ({status_line})",
        f"- `verification: test` requirements cited by at least one test: **{covered} / "
        f"{len(test_reqs)}**",
        f"- Citing tests: **{sum(len(v) for v in report.tests.values())}**; "
        f"vectors referencing a requirement: **{sum(len(v) for v in report.vectors.values())}**",
        "",
        "| ID | Status | Crit. | Verification | Tests | Vectors |",
        "|---|---|---|---|---|---|",
    ]
    for rid in sorted(reqs, key=lambda r: (PREFIXES.index(r.split("-")[0]), r)):
        r = reqs[rid]
        tests = "<br>".join(f"`{t}`" for t in sorted(set(report.tests.get(rid, [])))) or "—"
        vecs = "<br>".join(f"`{v}`" for v in sorted(set(report.vectors.get(rid, [])))) or "—"
        lines.append(
            f"| [{rid}]({r.file}) | {r.status} | {r.criticality} | {r.verification} | "
            f"{tests} | {vecs} |"
        )
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="verify only; never write")
    args = parser.parse_args(argv)

    report = build_report(ROOT)
    text = render(report)
    errors = list(report.errors)
    if args.check:
        current = TRACE_MD.read_text(encoding="utf-8") if TRACE_MD.exists() else ""
        if current != text:
            errors.append("docs/requirements/TRACE.md is stale; run `uv run python trace.py`")
        errors += [f"literal: {f}" for f in lint_literals.lint_repo(ROOT)]
        errors += [f"workflow: {f}" for f in lint_workflows.lint_repo(ROOT)]
        errors += [f"layers: {f}" for f in check_layers.check_repo(ROOT)]
    else:
        TRACE_MD.write_text(text, encoding="utf-8")
        print(f"wrote {TRACE_MD.relative_to(ROOT)}")
    for err in errors:
        print(f"error: {err}", file=sys.stderr)
    n_tests = sum(len(v) for v in report.tests.values())
    print(
        f"trace: {len(report.requirements)} requirements, {n_tests} citing tests, "
        f"{len(errors)} error(s)"
    )
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
