# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/trace.py on synthetic repositories."""

from __future__ import annotations

import shutil
from pathlib import Path
from types import ModuleType

REQ = """
### {rid}

```yaml
id: {rid}
title: "t"
status: {status}
criticality: high
verification: test
statement: >-
  s
acceptance: >-
  a
parent: null
related: []
```
"""


def make_repo(tmp: Path, blocks: list[tuple[str, str]], test_src: str = "") -> Path:
    req_dir = tmp / "docs" / "requirements"
    req_dir.mkdir(parents=True)
    (req_dir / "PHY.md").write_text(
        "".join(REQ.format(rid=r, status=s) for r, s in blocks), encoding="utf-8"
    )
    vec_dir = tmp / "tests" / "vectors"
    vec_dir.mkdir(parents=True)
    shutil.copy(Path(__file__).resolve().parents[2] / "tests/vectors/schema.json", vec_dir)
    (tmp / "crates" / "x" / "tests").mkdir(parents=True)
    (tmp / "crates" / "x" / "tests" / "t.rs").write_text(test_src, encoding="utf-8")
    return tmp


# Verifies: ORG-003
def test_parses_real_catalogue(fa_trace: ModuleType) -> None:
    reqs, errors = fa_trace.parse_requirements(fa_trace.REQ_DIR)
    assert errors == []
    assert len(reqs) >= 100
    assert all(fa_trace.ID_RE.match(r) for r in reqs)


# Verifies: ORG-003
def test_rejects_duplicate_and_malformed(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_repo(tmp_path, [("PHY-001", "draft"), ("PHY-001", "draft"), ("PHY-1", "draft")])
    _, errors = fa_trace.parse_requirements(repo / "docs" / "requirements")
    assert any("duplicate id PHY-001" in e for e in errors)
    assert any("malformed id 'PHY-1'" in e for e in errors)


# Verifies: ORG-003
def test_rejects_bad_enum(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_repo(tmp_path, [("PHY-001", "done")])
    _, errors = fa_trace.parse_requirements(repo / "docs" / "requirements")
    assert any("status='done'" in e for e in errors)


# Verifies: ORG-004
def test_approved_requirement_needs_a_citing_test(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_repo(tmp_path, [("PHY-001", "approved")])
    report = fa_trace.build_report(repo)
    assert any("PHY-001 is approved" in e for e in report.errors)

    cited = "/// Verifies: PHY-001\n#[test]\nfn rung() {}\n"
    repo2 = make_repo(tmp_path / "b", [("PHY-001", "approved")], cited)
    report2 = fa_trace.build_report(repo2)
    assert report2.errors == []
    assert report2.tests["PHY-001"] == ["crates/x/tests/t.rs::rung"]


# Verifies: ORG-004
def test_unknown_cited_id_fails(fa_trace: ModuleType, tmp_path: Path) -> None:
    cited = "/// Verifies: PHY-001, PHY-999\n#[test]\nfn rung() {}\n"
    repo = make_repo(tmp_path, [("PHY-001", "draft")], cited)
    report = fa_trace.build_report(repo)
    assert any("unknown requirement PHY-999" in e for e in report.errors)


# Verifies: TST-002, TST-003
def test_vector_schema_is_enforced(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_repo(tmp_path, [("PHY-001", "draft")])
    (repo / "tests" / "vectors" / "v.toml").write_text(
        'schema_version = 1\n[[vector]]\nid = "X"\nrequirement = ["PHY-001"]\n'
        'quantity = "q"\nvalue = 1\nunit = "1"\nkind = "computed-reference"\n'
        'source = "UNVERIFIED"\ncitation = "c"\nretrieved = "2026-10-14"\n'
        'tolerance = { abs = 0 }\ntolerance_rationale = "r"\nverified_by = "v"\n',
        encoding="utf-8",
    )
    _, errors = fa_trace.scan_vectors(repo / "tests" / "vectors")
    joined = "\n".join(errors)
    assert "source" in joined
    assert "generated_by" in joined


# Verifies: ORG-004
def test_render_is_deterministic(fa_trace: ModuleType) -> None:
    report = fa_trace.build_report(fa_trace.ROOT)
    assert fa_trace.render(report) == fa_trace.render(fa_trace.build_report(fa_trace.ROOT))


WORKFLOW = """
name: ci
on: [pull_request]
jobs:
  rust:
    runs-on: ubuntu-24.04
    steps:
      - name: cargo deny check
        run: cargo deny check
      - id: fmt
        run: cargo fmt --all --check
"""


def make_gate_repo(tmp: Path, rid: str, crit: str, gate: str, status: str = "draft") -> Path:
    req_dir = tmp / "docs" / "requirements"
    req_dir.mkdir(parents=True)
    block = REQ.format(rid=rid, status=status).replace("criticality: high", f"criticality: {crit}")
    block = block.replace("related: []\n", f'related: []\nverified_by_gate: "{gate}"\n')
    (req_dir / f"{rid[:3]}.md").write_text(block, encoding="utf-8")
    vec_dir = tmp / "tests" / "vectors"
    vec_dir.mkdir(parents=True)
    shutil.copy(Path(__file__).resolve().parents[2] / "tests/vectors/schema.json", vec_dir)
    wf_dir = tmp / ".github" / "workflows"
    wf_dir.mkdir(parents=True)
    (wf_dir / "ci.yml").write_text(WORKFLOW, encoding="utf-8")
    return tmp


# Verifies: ORG-003, ORG-004
def test_gate_outside_scope_is_rejected(fa_trace: ModuleType, tmp_path: Path) -> None:
    for stem, rid in (("PHY", "PHY-001"), ("NUM", "NUM-001"), ("STD", "STD-012")):
        repo = make_gate_repo(tmp_path / stem, rid, "medium", "ci/rust")
        report = fa_trace.build_report(repo)
        assert any(f"{rid} has verified_by_gate" in e for e in report.errors), rid
        assert not report.requirements[rid].gate_counts
    assert "PHY-001" not in fa_trace.GATE_SCOPE
    assert {"STD-001", "STD-011", "TST-001", "DEP-006", "ORG-008"} <= fa_trace.GATE_SCOPE


# Verifies: ORG-003, ORG-004
def test_malformed_gate_is_rejected(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_gate_repo(tmp_path, "STD-001", "low", "ci")
    _, errors = fa_trace.parse_requirements(repo / "docs" / "requirements")
    assert any("is not workflow/job[/step]" in e for e in errors)


# Verifies: ORG-004
def test_missing_workflow_job_or_step_is_rejected(fa_trace: ModuleType, tmp_path: Path) -> None:
    cases = {
        "nope/rust": "workflow .github/workflows/nope.yml does not exist",
        "ci/python": "workflow ci has no job 'python'",
        "ci/rust/cargo doc": "has no step named 'cargo doc'",
    }
    for n, (gate, message) in enumerate(cases.items()):
        repo = make_gate_repo(tmp_path / str(n), "STD-001", "low", gate)
        report = fa_trace.build_report(repo)
        assert any(message in e for e in report.errors), gate
    for n, gate in enumerate(("ci/rust", "ci/rust/cargo deny check", "ci/rust/fmt")):
        repo = make_gate_repo(tmp_path / f"ok{n}", "STD-001", "low", gate)
        assert fa_trace.build_report(repo).errors == [], gate


# Verifies: ORG-004
def test_high_criticality_gate_is_unpinned_and_not_counted(
    fa_trace: ModuleType, tmp_path: Path
) -> None:
    assert not fa_trace.PINNED_GATES
    gate = "ci/rust/cargo deny check"
    repo = make_gate_repo(tmp_path, "DEP-001", "high", gate, status="approved")
    report = fa_trace.build_report(repo)
    req = report.requirements["DEP-001"]
    assert not req.gate_counts
    assert req.gate_state == "gate (unpinned)"
    assert any("DEP-001 is approved" in e and "unpinned" in e for e in report.errors)
    assert f"| `{gate}` | gate (unpinned) |" in fa_trace.render(report)


# Verifies: ORG-004
def test_medium_criticality_gate_counts(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_gate_repo(tmp_path, "STD-002", "medium", "ci/rust", status="approved")
    report = fa_trace.build_report(repo)
    assert report.errors == []
    req = report.requirements["STD-002"]
    assert req.gate_counts
    assert req.gate_state == "gate"
    assert "covered by a counted gate alone: **1**" in fa_trace.render(report)


# Verifies: ORG-004
def test_gate_evidence_is_a_separate_section(fa_trace: ModuleType, tmp_path: Path) -> None:
    repo = make_gate_repo(tmp_path, "STD-002", "medium", "ci/rust")
    text = fa_trace.render(fa_trace.build_report(repo))
    tests_part, gate_part = text.split("## Gate evidence")
    assert "## Test evidence" in tests_part
    assert "ci/rust" not in tests_part
    assert "| [STD-002](STD.md) | draft | medium | test | — | — |" in tests_part
    assert "| [STD-002](STD.md) | draft | medium | `ci/rust` | gate |" in gate_part
