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
