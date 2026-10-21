# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Reject bare physical literals in test code (TST-002, AGENTS.md rule 1).

Expected values belong in ``tests/vectors/*.toml`` with provenance. This lint scans test code and
flags numeric literals that look like physical values:

* every float literal except trivially structural ones: integers with ``|v| <= 10`` written as
  floats (``2.0``) and the halves/quarters ``0.5`` / ``0.25``;
* every integer literal with ``|v| > 1000`` (counts and indices stay small; physical magnitudes such
  as ``86400`` or ``696000`` do not).

Scanned code: ``crates/*/tests/**/*.rs``, the part of ``crates/*/src/**/*.rs`` from the first
``#[cfg(test)]`` to the end of the file (by convention the unit-test module is last), and
``tools/tests/**/*.py``. String literals and comments are ignored. A line ending in the marker
``lint: allow-literal(<reason>)`` is exempt; the reason is mandatory.

Usage: ``uv run python lint_literals.py [paths...]`` (default: the whole repository).
"""

from __future__ import annotations

import io
import re
import sys
import tokenize
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ALLOW_RE = re.compile(r"lint:\s*allow-literal\((?P<reason>[^)]*\S[^)]*)\)")
MAX_PLAIN_INT = 1000
SMALL_FLOAT_LIMIT = 10
ALLOWED_FRACTIONS = (0.5, 0.25)

# A Rust numeric literal: decimal float, exponent form, or integer, with optional suffix.
RUST_NUM_RE = re.compile(
    r"(?<![\w.])"
    r"(?P<num>(?:\d[\d_]*\.\d[\d_]*(?:[eE][+-]?\d[\d_]*)?"
    r"|\d[\d_]*[eE][+-]?\d[\d_]*"
    r"|\d[\d_]*\.(?![\w.])"
    r"|0x[\da-fA-F_]+|0o[0-7_]+|0b[01_]+"
    r"|\d[\d_]*))"
    r"(?P<suffix>(?:_?(?:f32|f64|[iu](?:8|16|32|64|128|size))))?"
    r"(?![\w])"
)
# Rust string and char literals and comments (replaced by spaces, newlines preserved).
RUST_STRIP_RE = re.compile(
    r"//[^\n]*"
    r"|/\*.*?\*/"
    r'|b?r(?P<hashes>#*)".*?"(?P=hashes)'
    r'|b?"(?:\\.|[^"\\])*"'
    r"|b?'(?:\\.|[^\\'\n])'",
    re.DOTALL,
)


def _blank(match: re.Match[str]) -> str:
    return "".join(ch if ch == "\n" else " " for ch in match.group(0))


def classify(text: str, *, is_float: bool) -> bool:
    """Return True when a numeric literal counts as a bare physical value."""
    clean = text.replace("_", "")
    if clean.lower().startswith(("0x", "0o", "0b")):
        return False
    try:
        value = float(clean)
    except ValueError:
        return False
    if is_float:
        if value.is_integer() and abs(value) <= SMALL_FLOAT_LIMIT:
            return False
        return abs(value) not in ALLOWED_FRACTIONS
    return abs(value) > MAX_PLAIN_INT


def lint_rust_source(text: str, *, start_line: int = 1) -> list[tuple[int, str]]:
    """Return ``(line, literal)`` for every bare physical literal in Rust code."""
    raw_lines = text.splitlines()
    code = RUST_STRIP_RE.sub(_blank, text)
    found: list[tuple[int, str]] = []
    for offset, line in enumerate(code.splitlines()):
        lineno = start_line + offset
        original = raw_lines[offset] if offset < len(raw_lines) else ""
        if ALLOW_RE.search(original):
            continue
        for m in RUST_NUM_RE.finditer(line):
            num, suffix = m.group("num"), m.group("suffix") or ""
            is_float = (
                "." in num or ("e" in num.lower() and not num.lower().startswith("0x"))
            ) or suffix.endswith(("f32", "f64"))
            if classify(num, is_float=is_float):
                found.append((lineno, num + suffix))
    return found


def lint_python_source(text: str) -> list[tuple[int, str]]:
    """Return ``(line, literal)`` for every bare physical literal in Python code."""
    lines = text.splitlines()
    found: list[tuple[int, str]] = []
    for tok in tokenize.generate_tokens(io.StringIO(text).readline):
        if tok.type != tokenize.NUMBER:
            continue
        line = lines[tok.start[0] - 1] if tok.start[0] <= len(lines) else ""
        if ALLOW_RE.search(line):
            continue
        s = tok.string.lower()
        if s.endswith("j"):
            continue
        is_float = "." in s or ("e" in s and not s.startswith("0x"))
        if classify(tok.string, is_float=is_float):
            found.append((tok.start[0], tok.string))
    return found


def _rust_test_region(path: Path) -> tuple[str, int] | None:
    """Return the test-only part of a Rust file and its first line number, or None."""
    text = path.read_text(encoding="utf-8")
    parts = path.parts
    if "tests" in parts or "benches" in parts:
        return text, 1
    idx = text.find("#[cfg(test)]")
    if idx < 0:
        return None
    return text[idx:], text.count("\n", 0, idx) + 1


def lint_paths(paths: list[Path], root: Path = ROOT) -> list[str]:
    """Lint the given files; return human-readable findings."""
    findings: list[str] = []
    for path in paths:
        rel = path.relative_to(root).as_posix() if path.is_relative_to(root) else str(path)
        if path.suffix == ".rs":
            region = _rust_test_region(path)
            if region is None:
                continue
            hits = lint_rust_source(region[0], start_line=region[1])
        elif path.suffix == ".py":
            hits = lint_python_source(path.read_text(encoding="utf-8"))
        else:
            continue
        findings += [
            f"{rel}:{line}: bare physical literal {lit}; load it from tests/vectors/*.toml "
            "or mark the line `lint: allow-literal(<reason>)`"
            for line, lit in hits
        ]
    return findings


def files_to_lint(root: Path = ROOT) -> list[Path]:
    """Return every file this lint applies to."""
    files = list(root.glob("crates/*/tests/**/*.rs"))
    files += list(root.glob("crates/*/src/**/*.rs"))
    files += list(root.glob("tools/tests/**/*.py"))
    return sorted(files)


def lint_repo(root: Path = ROOT) -> list[str]:
    """Lint every test file in the repository."""
    return lint_paths(files_to_lint(root), root)


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    args = sys.argv[1:] if argv is None else argv
    findings = lint_paths([Path(a).resolve() for a in args]) if args else lint_repo()
    for finding in findings:
        print(finding, file=sys.stderr)
    print(f"lint_literals: {len(findings)} finding(s)")
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
