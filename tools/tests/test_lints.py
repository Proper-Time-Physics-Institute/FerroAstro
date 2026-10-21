# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for the literal and workflow lints."""

from __future__ import annotations

import lint_literals
import lint_workflows

# Synthetic Rust snippets; the numbers live inside strings, which the Python lint ignores.
RUST_BAD = "let gm = 1.32712440041e11;\nlet secs = 86400;\nlet c = 299_792.458_f64;\n"
RUST_OK = (
    'let a = 2.0 * x + 0.5; let n = 1000; let s = "42.98";\n'
    "// 42.98 in a comment\nlet i = v[3];\n"
    "let x = 7.25; // lint: allow-literal(fixture for a pure-math identity)\n"
)


# Verifies: TST-002
def test_rust_literals_flagged() -> None:
    hits = lint_literals.lint_rust_source(RUST_BAD)
    assert [lit for _, lit in hits] == ["1.32712440041e11", "86400", "299_792.458_f64"]
    assert [line for line, _ in hits] == [1, 2, 3]


# Verifies: TST-002
def test_rust_structural_literals_allowed() -> None:
    assert lint_literals.lint_rust_source(RUST_OK) == []


# Verifies: TST-002
def test_allow_marker_needs_reason() -> None:
    assert lint_literals.lint_rust_source("let x = 7.25; // lint: allow-literal()\n") != []


# Verifies: TST-002
def test_python_literals_flagged() -> None:
    src = "x = 42.98\ny = 2.0\nz = 'text 575.31'\nn = 86400\n"
    assert [lit for _, lit in lint_literals.lint_python_source(src)] == ["42.98", "86400"]


# Verifies: TST-002
def test_repository_is_clean() -> None:
    assert lint_literals.lint_repo() == []


# Verifies: STD-012
def test_workflow_lint() -> None:
    bad = (
        "on: pull_request_target\njobs:\n  a:\n    steps:\n"
        "      - uses: actions/checkout@v4\n      - uses: ./local\n"
    )
    findings = lint_workflows.lint_text("w.yml", bad)
    assert any("not pinned" in f for f in findings)
    assert any("pull_request_target" in f for f in findings)
    assert any("permissions" in f for f in findings)
    good = (
        "on: pull_request\npermissions:\n  contents: read\njobs:\n  a:\n    steps:\n"
        "      - uses: actions/checkout@0123456789abcdef0123456789abcdef01234567 # v4\n"
    )
    assert lint_workflows.lint_text("w.yml", good) == []
