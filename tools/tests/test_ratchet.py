# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/ratchet.py, the evidence ratchet (TST-006, TST-004, ORG-011)."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path
from typing import Any

import pytest
import yaml

import ratchet

ROOT = Path(__file__).resolve().parents[2]
CFG = ratchet.Config.load(ROOT / "ci" / "ratchet.toml")


def vec(**kw: Any) -> dict[str, Any]:
    base: dict[str, Any] = {
        "id": "X",
        "kind": "measured",
        "value": "1.0",
        "tolerance": {"abs": "0.1"},
        "source": "s",
        "verified_by": "v",
    }
    base.update(kw)
    return base


def diff(old: dict[str, Any] | None, new: dict[str, Any] | None, commits: list[str] | None = None):  # type: ignore[no-untyped-def]
    b = {} if old is None else {"X": ("f.toml", old)}
    h = {} if new is None else {"X": ("f.toml", new)}
    return ratchet.vector_findings(b, h, CFG, commits or [])


def classes(findings: list[ratchet.Finding]) -> list[str]:
    return sorted(f.cls for f in findings)


# Verifies: TST-006
def test_unchanged_vector_has_no_finding() -> None:
    assert diff(vec(), vec()) == []


# Verifies: TST-006
def test_tolerance_widening_needs_approval_and_golden() -> None:
    found = diff(vec(), vec(tolerance={"abs": "0.2"}, verified_by="new"))
    assert classes(found) == ["approval", "golden"]
    assert "tolerance widened" in found[0].what


# Verifies: TST-006
def test_adding_rel_to_abs_only_tolerance_is_widening() -> None:
    found = diff(vec(), vec(tolerance={"abs": "0.1", "rel": "0.5"}, verified_by="new"))
    assert any("tolerance widened" in f.what for f in found)


# Verifies: TST-006
def test_tightening_within_band_is_golden_only() -> None:
    found = diff(vec(), vec(value="1.05", tolerance={"abs": "0.05"}, verified_by="new"))
    assert classes(found) == ["golden"]
    assert "NO new evidence" not in found[0].what


# Verifies: TST-006
def test_value_jump_beyond_k_old_tolerance_needs_approval() -> None:
    found = diff(vec(), vec(value="1.2", verified_by="new"))
    assert any("exceeds" in f.what for f in found)
    assert diff(vec(), vec(value="1.1", verified_by="new"))[0].cls == "golden"


# Verifies: TST-006
def test_kind_downgrade_delete_and_blocked_by() -> None:
    assert "downgraded" in diff(vec(kind="analytic"), vec())[0].what
    assert diff(vec(kind="computed-reference"), vec()) == []
    assert diff(vec(), None)[0].what == "vector deleted"
    assert "blocked_by" in diff(vec(), vec(blocked_by="OI-38"))[0].what


# Verifies: TST-004
def test_golden_change_without_new_evidence_is_flagged() -> None:
    (found,) = diff(vec(), vec(value="1.05"))
    assert found.cls == "golden"
    assert "NO new evidence" in found.what
    assert ratchet.verdict([found], approved=True, golden_change=True) == 1


# Verifies: TST-004
def test_computed_reference_generated_in_this_pr_is_rejected() -> None:
    new = vec(kind="computed-reference", generated_by="tools/g.py@abc1234")
    found = diff(None, new, commits=["abc1234deadbeef"])
    assert classes(found) == ["reject"]
    assert ratchet.verdict(found, approved=True, golden_change=True) == 1
    assert diff(None, new, commits=["0000000"]) == []


# Verifies: TST-006
def test_verdict_overrides() -> None:
    approval = ratchet.Finding("approval", "w", "x")
    golden = ratchet.Finding("golden", "w", "value or tolerance changed")
    assert ratchet.verdict([], approved=False, golden_change=False) == 0
    assert ratchet.verdict([approval], approved=False, golden_change=True) == 1
    assert ratchet.verdict([approval], approved=True, golden_change=False) == 0
    assert ratchet.verdict([golden], approved=True, golden_change=False) == 1
    assert ratchet.verdict([golden], approved=False, golden_change=True) == 0


def write(root: Path, rel: str, text: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


RS = """
#[test]
fn keeps() {}

#[test]
#[ignore = "data: network"]
fn data_fetch() {}
"""


# Verifies: TST-006
def test_deleted_test_and_new_gates(tmp_path: Path) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "crates/c/tests/t.rs", RS + "\n#[test]\nfn goes_away() {}\n")
    write(
        head,
        "crates/c/tests/t.rs",
        RS
        + '\n#[test]\n#[ignore = "nightly tier"]\nfn slow() {}\n'
        + '\n#[cfg(test)]\nmod m {}\n#[test]\n#[ignore = "data: more"]\nfn data_more() {}\n'
        + "/// #[ignore] in a doc comment is not a gate\n",
    )
    found = ratchet.source_findings(base, head, CFG)
    whats = sorted(f.what for f in found)
    assert whats == [
        'new gate not on the allowlist: ignore = "nightly tier"',
        "test deleted or renamed",
    ]


# Verifies: TST-006
def test_new_pytest_skip_needs_approval(tmp_path: Path) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", "def test_a():\n    pass\n")
    write(head, "tools/tests/test_a.py", "@pytest.mark.skip(reason='x')\ndef test_a():\n    pass\n")
    (found,) = ratchet.source_findings(base, head, CFG)
    assert "pytest.mark.skip" in found.what


PY_BASE = "import pytest\n\n\ndef test_a():\n    pass\n"


# Verifies: TST-006
@pytest.mark.parametrize(
    ("added", "expect"),
    [
        ('pytestmark = pytest.mark.skip(reason="x")\n', "pytestmark"),
        ("@pytest.mark.skipif(True, reason='x')\ndef test_b(): ...\n", "pytest.mark.skipif"),
        ("@pytest.mark.xfail(reason='x')\ndef test_b(): ...\n", "pytest.mark.xfail"),
        ("pytestmark = [pytest.mark.slow]\n", "pytestmark"),
        ("def test_b():\n    pytest.skip('x')\n", "pytest.skip('x')"),
        ("def test_b():\n    pytest.xfail('x')\n", "pytest.xfail('x')"),
        ("erfa = pytest.importorskip('erfa')\n", "pytest.importorskip('erfa')"),
        ("from pytest import mark\n\n\n@mark.skip\ndef test_b():\n    pass\n", "mark.skip"),
        ("import pytest as p\n\n\n@p.mark.skipif(True, reason='x')\ndef test_b(): ...\n", "skipif"),
        ("from pytest import skip as s\n\n\ndef test_b():\n    s()\n", "s()"),
        ("import unittest\n\n\n@unittest.skip('x')\ndef test_b(): ...\n", "unittest.skip"),
        ("import unittest\n\n\ndef test_b():\n    raise unittest.SkipTest\n", "SkipTest"),
        ("def pytest_collection_modifyitems(items):\n    items.clear()\n", "def pytest_"),
        ("def test_b(:\n", "does not parse"),
    ],
)
def test_every_detected_python_gate_needs_approval(tmp_path: Path, added: str, expect: str) -> None:
    """The forms TST-006 and `python_gates` list are each caught, in any test file."""
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", PY_BASE)
    write(head, "tools/tests/test_a.py", PY_BASE + "\n\n" + added)
    found = ratchet.source_findings(base, head, CFG)
    assert found
    assert all(f.cls == "approval" for f in found)
    assert any(expect in f.what for f in found), found


# Verifies: TST-006
def test_python_gate_text_in_strings_and_comments_is_not_a_gate() -> None:
    text = 'X = "@pytest.mark.skip"  # pytest.skip()\nparser.add_argument("--x", help="skip it")\n'
    assert ratchet.python_gates(text) == []


CARGO = '[package]\nname = "c"\n\n[features]\nstd = []\nserde = []\n'


# Verifies: TST-006
@pytest.mark.parametrize(
    "attr",
    [
        "#[ignore]",
        "# [ignore]",
        "#[ ignore ]",
        '#[cfg_attr(feature = "std", ignore)]',
        '#[ignore="nightly"]',
        '#[ignore =\n    "nightly"]',
        '#[cfg(target_arch = "riscv64")]',
        '#![cfg(target_arch = "riscv64")]',
        '#[cfg(target_arch = "wasm32")]',
        '#[cfg(feature = "nope")]',
        '#[cfg(not(feature = "nope"))]',
        '#[cfg(all(feature = "std", fa_nightly))]',
    ],
)
def test_rust_gate_spellings_and_hiding_cfgs_need_approval(tmp_path: Path, attr: str) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    for root in (base, head):
        write(root, "crates/c/Cargo.toml", CARGO)
    write(base, "crates/c/tests/t.rs", RS)
    write(head, "crates/c/tests/t.rs", RS + f"\n{attr}\n#[test]\nfn hidden() {{}}\n")
    (found,) = ratchet.source_findings(base, head, CFG)
    assert found.cls == "approval"


# Verifies: TST-006
@pytest.mark.parametrize(
    "attr",
    [
        '#[cfg(feature = "std")]',
        '#[cfg(not(feature = "std"))]',
        '#[cfg(not(target_arch = "wasm32"))]',
        '#[cfg_attr(feature = "serde", serde(default))]',
        '#[cfg_attr(feature = "serde", serde(rename = "x"))]',
        '# [ignore = "data: network fetch"]',
    ],
)
def test_allowlisted_gates_pass(tmp_path: Path, attr: str) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    for root in (base, head):
        write(root, "crates/c/Cargo.toml", CARGO)
    write(base, "crates/c/src/lib.rs", "pub struct S;\n")
    write(head, "crates/c/src/lib.rs", f"pub struct S;\n{attr}\npub struct T;\n")
    assert ratchet.source_findings(base, head, CFG) == []


# Verifies: TST-006
def test_multiline_attribute_between_test_and_fn_is_not_a_deletion(tmp_path: Path) -> None:
    """PR #51's shape: an `#[expect(...)]` spanning lines added under `#[test]`."""
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "crates/c/tests/t.rs", "/// Verifies: X\n#[test]\nfn kept() {}\n")
    write(
        head,
        "crates/c/tests/t.rs",
        "/// Verifies: X\n#[test]\n#[expect(\n    clippy::suboptimal_flops,\n"
        '    reason = "a [bracket] in a string"\n)]\n// note\n/* block */\npub fn kept() {}\n',
    )
    assert ratchet.rust_test_fns((head / "crates/c/tests/t.rs").read_text()) == ["kept"]
    assert ratchet.source_findings(base, head, CFG) == []


# Verifies: TST-006
def test_moving_a_gate_onto_another_test_is_a_new_gate(tmp_path: Path) -> None:
    """Swapping an existing ignore onto a failing test is caught although the counts are equal."""
    base, head = tmp_path / "b", tmp_path / "h"
    write(
        base,
        "crates/c/tests/t.rs",
        '#[test]\n#[ignore = "nightly: slow"]\nfn slow() {}\n\n#[test]\nfn failing() {}\n',
    )
    write(
        head,
        "crates/c/tests/t.rs",
        '#[test]\nfn slow() {}\n\n#[test]\n#[ignore = "nightly: slow"]\nfn failing() {}\n',
    )
    (found,) = ratchet.source_findings(base, head, CFG)
    assert found.where == "crates/c/tests/t.rs (fn failing)"
    assert found.cls == "approval"
    # The same function name in another inline module is another item.
    two = (
        "mod a {{\n    #[test]\n{0}    fn t() {{}}\n}}\n"
        "mod b {{\n    #[test]\n{1}    fn t() {{}}\n}}\n"
    )
    ign = '    #[ignore = "nightly: slow"]\n'
    write(base, "crates/c/tests/t.rs", two.format(ign, ""))
    write(head, "crates/c/tests/t.rs", two.format("", ign))
    (found,) = ratchet.source_findings(base, head, CFG)
    assert found.where == "crates/c/tests/t.rs (b::fn t)"


# Verifies: TST-006
@pytest.mark.parametrize(
    "hidden",
    [
        '#[rustfmt::skip]\n#[test] #[ignore = "x"] fn t() {}\n',
        "#[rustfmt::skip] #[test] # [ ignore ] fn t() {}\n",
        "/*\n#[test]\nfn t() {}\n*/\n",
        "/* outer /* nested */\n#[test]\nfn t() {}\n*/\n",
        'const S: &str = "\n#[test]\nfn t() {}\n";\n',
        'const S: &str = r#"\n#[test]\nfn t() {}\n"#;\n',
    ],
)
def test_rust_tests_hidden_on_one_line_or_in_comments_are_found(
    tmp_path: Path, hidden: str
) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "crates/c/tests/t.rs", "#[test]\nfn t() {}\n")
    write(head, "crates/c/tests/t.rs", hidden)
    found = ratchet.source_findings(base, head, CFG)
    assert found
    assert all(f.cls == "approval" for f in found)


# Verifies: TST-006
def test_rust_literals_and_comments_do_not_confuse_the_scanner() -> None:
    text = (
        "fn f<'a>(x: &'a str) -> char { let _ = \"#[ignore]\"; '\"' }\n"
        "/// #[ignore]\n#[test]\nfn kept() { let _ = '['; }\n"
    )
    tests, gates = ratchet.rust_scan(text)
    assert tests == ["kept"]
    assert gates == []


PY_TWO = (
    "import shutil\n\nimport pytest\n\n\n"
    "{0}def test_a():\n    pass\n\n\n{1}def test_b():\n    pass\n"
)
SKIPIF = '@pytest.mark.skipif(shutil.which("cargo") is None, reason="no cargo")\n'


# Verifies: TST-006
def test_moving_a_python_gate_onto_another_test_is_a_new_gate(tmp_path: Path) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", PY_TWO.format(SKIPIF, ""))
    write(head, "tools/tests/test_a.py", PY_TWO.format("", SKIPIF))
    (found,) = ratchet.source_findings(base, head, CFG)
    assert found.where == "tools/tests/test_a.py (test_b)"


# Verifies: TST-006
@pytest.mark.parametrize(
    ("rel", "head_text", "expect"),
    [
        ("tools/tests/test_a.py", PY_BASE + "\n__test__ = False\n", "__test__"),
        ("tools/tests/test_a.py", PY_BASE + "\ntest_a.__test__ = False\n", "__test__"),
        (
            "tools/tests/test_a.py",
            'import pytest\n\n"""\ndef test_a():\n    pass\n"""\n',
            "deleted",
        ),
        (
            "tools/tests/test_a.py",
            "import pytest\n\nif False:\n    def test_a():\n        pass\n",
            "deleted",
        ),
        ("tools/tests/test_a.py", PY_BASE + "\n\ndel test_a\n", "deleted"),
        ("tools/tests/test_a.py", PY_BASE + "\n\ntest_a = None\n", "deleted"),
        ("tools/tests/test_a.py", PY_BASE + "\n\npytest.exit('stop')\n", "pytest.exit"),
        ("tools/tests/test_a.py", PY_BASE + "\nfrom pytest import exit as e\n\ne('x')\n", "e('x')"),
        ("tools/tests/sub/conftest.py", 'collect_ignore = ["x.py"]\n', "collect_ignore"),
        ("tools/tests/sub/conftest.py", 'collect_ignore_glob = ["*"]\n', "collect_ignore_glob"),
        ("tools/tests/sub/conftest.py", 'collect_ignore.append("x.py")\n', "collect_ignore"),
        ("tools/pytest.ini", "[pytest]\naddopts = -k nothing\n", "pytest configuration"),
        ("tools/tests/tox.ini", "[pytest]\naddopts = -k nothing\n", "pytest configuration"),
        ("tools/setup.cfg", "[tool:pytest]\naddopts = -k nothing\n", "pytest configuration"),
        ("tools/pyproject.toml", '[tool.pytest.ini_options]\naddopts = "-k nothing"\n', "pytest"),
    ],
)
def test_python_collection_controls_are_found(
    tmp_path: Path, rel: str, head_text: str, expect: str
) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    for root in (base, head):
        write(root, "tools/tests/test_a.py", PY_BASE)
        write(root, "tools/pyproject.toml", '[tool.pytest.ini_options]\ntestpaths = ["tests"]\n')
    write(head, rel, head_text)
    found = ratchet.source_findings(base, head, CFG)
    assert found
    assert all(f.cls == "approval" for f in found)
    assert any(expect in f.what or expect in f.where for f in found), found


# Verifies: TST-006
def test_python_tests_come_from_the_ast() -> None:
    text = (
        "class TestX:\n    def test_m(self):\n        pass\n\n    class TestY:\n"
        "        def test_n(self):\n            pass\n\n\nclass Helper:\n"
        "    def test_not_collected(self):\n        pass\n\n\nasync def test_c():\n    pass\n"
    )
    assert sorted(ratchet.python_test_names(text)) == [
        "TestX::TestY::test_n",
        "TestX::test_m",
        "test_c",
    ]
    assert ratchet.python_test_names("def test_a(:\n") == []


# Verifies: TST-006
def test_non_pytest_pyproject_change_is_not_a_finding(tmp_path: Path) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    pytest_cfg = '[tool.pytest.ini_options]\ntestpaths = ["tests"]\n'
    write(base, "tools/pyproject.toml", '[project]\nname = "t"\n' + pytest_cfg)
    write(head, "tools/pyproject.toml", '[project]\nname = "t"\nversion = "1"\n' + pytest_cfg)
    assert ratchet.source_findings(base, head, CFG) == []


# Verifies: TST-006
def test_respelled_tolerance_is_not_a_change() -> None:
    as_float = float(vec()["tolerance"]["abs"])  # the TOML float spelling of the same tolerance
    assert diff(vec(), vec(tolerance={"abs": as_float})) == []
    assert diff(vec(), vec(tolerance={"abs": "1e-1"})) == []


REQ = """
```yaml
id: {rid}
status: {status}
statement: >-
  {statement}
```
"""


# Verifies: ORG-011
def test_requirement_meta_fields(tmp_path: Path) -> None:
    base, head = tmp_path / "b", tmp_path / "h"
    write(
        base, "docs/requirements/PHY.md", REQ.format(rid="PHY-001", status="draft", statement="1")
    )
    write(
        base, "docs/requirements/STD.md", REQ.format(rid="STD-001", status="draft", statement="1")
    )
    write(
        head, "docs/requirements/PHY.md", REQ.format(rid="PHY-001", status="draft", statement="2")
    )
    write(
        head,
        "docs/requirements/STD.md",
        REQ.format(rid="STD-001", status="approved", statement="2")
        + REQ.format(rid="STD-002", status="draft", statement="new"),
    )
    found = ratchet.requirement_findings(
        ratchet.load_requirements(base), ratchet.load_requirements(head)
    )
    assert sorted(str(f) for f in found) == [
        "[approval] PHY.md:PHY-001: statement changed (value/tolerance)",
        "[approval] STD.md:STD-001: status 'draft' -> 'approved'",
    ]


# Verifies: TST-006
def test_executed_count_from_nextest_json() -> None:
    listing = {
        "rust-suites": {
            "a": {
                "testcases": {
                    "run": {"ignored": False, "filter-match": {"status": "matches"}},
                    "ign": {"ignored": True, "filter-match": {"status": "matches"}},
                    "out": {"ignored": False, "filter-match": {"status": "mismatch"}},
                }
            }
        }
    }
    assert ratchet.executed_count(listing) == 1


# Verifies: TST-006
def test_real_repository_passes_against_itself() -> None:
    found = ratchet.vector_findings(ratchet.load_vectors(ROOT), ratchet.load_vectors(ROOT), CFG, [])
    assert found == []
    assert ratchet.source_findings(ROOT, ROOT, CFG) == []
    reqs = ratchet.load_requirements(ROOT)
    assert len(reqs) >= 100
    assert ratchet.requirement_findings(reqs, reqs) == []


def git(repo: Path, *args: str) -> str:
    env = {
        "GIT_AUTHOR_NAME": "t",
        "GIT_AUTHOR_EMAIL": "t@example.invalid",
        "GIT_COMMITTER_NAME": "t",
        "GIT_COMMITTER_EMAIL": "t@example.invalid",
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_NOSYSTEM": "1",
        "PATH": "/usr/bin:/bin",
    }
    res = subprocess.run(  # noqa: S603
        ["git", "-c", "commit.gpgsign=false", *args],  # noqa: S607
        cwd=repo,
        env=env,
        check=True,
        capture_output=True,
        text=True,
    )
    return res.stdout.strip()


# Verifies: TST-006
def test_stale_branch_is_compared_with_its_merge_base(tmp_path: Path) -> None:
    """Tests and vectors that main gained after the branch point are not "removed by the PR"."""
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q", "-b", "main")
    (repo / "ci").mkdir()
    (repo / "ci" / "ratchet.toml").write_text(
        (ROOT / "ci" / "ratchet.toml").read_text(encoding="utf-8"), encoding="utf-8"
    )
    write(repo, "crates/c/tests/t.rs", RS)
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "root")
    git(repo, "checkout", "-q", "-b", "pr")
    write(repo, "crates/c/tests/pr.rs", "#[test]\nfn from_the_pr() {}\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "pr")
    head_sha = git(repo, "rev-parse", "HEAD")
    git(repo, "checkout", "-q", "main")
    write(repo, "crates/c/tests/main.rs", "#[test]\nfn landed_on_main_later() {}\n")
    write(repo, "tools/tests/test_new.py", "def test_new():\n    pass\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-q", "-m", "main moves on")
    base_sha = git(repo, "rev-parse", "HEAD")
    base = tmp_path / "base"
    ratchet.export_tree(repo, base_sha, base)
    git(repo, "checkout", "-q", "pr")

    # The naive base-tip comparison reports main's new tests as deleted by the PR ...
    naive = ratchet.source_findings(base, repo, CFG)
    assert sorted(f.where for f in naive) == [
        "c::landed_on_main_later",
        "tools/test_new.py::test_new",
    ]
    # ... the merge-base comparison does not, and the CLI uses it by default.
    assert ratchet.merge_base(repo, base_sha, head_sha) != base_sha
    args = ["--base", str(base), "--head", str(repo), "--base-sha", base_sha]
    assert ratchet.main(["static", *args, "--head-sha", head_sha]) == 0

    # A test the PR itself deletes is still found.
    (repo / "crates/c/tests/t.rs").write_text("", encoding="utf-8")
    git(repo, "commit", "-q", "-am", "drop a test")
    head_sha = git(repo, "rev-parse", "HEAD")
    assert ratchet.main(["static", *args, "--head-sha", head_sha]) == 1
    approved = ["static", *args, "--head-sha", head_sha, "--maintainer-approved"]
    assert ratchet.main(approved) == 0


# Verifies: TST-006
@pytest.mark.parametrize(
    "cmd", [["static", "--help"], ["test-count", "--help"], ["merge-base", "--help"]]
)
def test_cli_help(cmd: list[str]) -> None:
    res = subprocess.run(  # noqa: S603
        ["python", str(ROOT / "tools" / "ratchet.py"), *cmd],  # noqa: S607
        capture_output=True,
        text=True,
        check=False,
    )
    assert res.returncode == 0


# --- Known gaps --------------------------------------------------------------------------------
# Each test below pins a form of test weakening that the ratchet does NOT detect today. They assert
# the current behaviour (no finding), so a fix that closes a gap fails its test: delete that test
# and the gap's entry in KNOWN_GAPS, TST-006 notes and the ratchet.py docstring together. The
# follow-up issue #55 ("ratchet: close the detection gaps listed in TST-006") tracks closing them.

KNOWN_GAPS = (
    "known_gap_py_runtime_gate",
    "known_gap_py_existing_helper",
    "known_gap_hollowed_test",
    "known_gap_py_nondefault_name",
    "known_gap_py_no_count",
    "known_gap_rs_allowlisted_gate",
    "known_gap_rs_uncompiled_file",
    "known_gap_rs_same_name_items",
    "known_gap_scan_scope",
    "known_gap_py_outcome_exception",
    "known_gap_py_empty_parametrize",
    "known_gap_py_process_exit",
    "known_gap_py_uncollectable_class",
    "known_gap_py_fixture_on_test",
    "known_gap_py_assigned_hook",
    "known_gap_rs_verdict_attribute",
)

PY_GAP_BASE = (
    "import pytest\nimport unittest\n\n\ndef helper():\n    pytest.skip('x')\n\n\n"
    "def test_a():\n    assert 1\n\n\nclass Checks(unittest.TestCase):\n"
    "    def test_m(self):\n        pass\n"
)


def py_gap(tmp_path: Path, head: str) -> list[ratchet.Finding]:
    base, h = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", PY_GAP_BASE)
    write(h, "tools/tests/test_a.py", head)
    return ratchet.source_findings(base, h, CFG)


def rs_gap(tmp_path: Path, base_files: dict[str, str], head_files: dict[str, str]) -> list:  # type: ignore[type-arg]
    base, h = tmp_path / "b", tmp_path / "h"
    for root, files in ((base, base_files), (h, head_files)):
        write(root, "crates/c/Cargo.toml", CARGO)
        for rel, text in files.items():
            write(root, rel, text)
    return ratchet.source_findings(base, h, CFG)


# Verifies: TST-006
def test_known_gaps_are_documented() -> None:
    """Every gap pinned here is named in TST-006 notes and the ratchet.py docstring, and back."""
    tst = (ROOT / "docs" / "requirements" / "TST.md").read_text(encoding="utf-8")
    doc = ratchet.__doc__ or ""
    named = set(re.findall(r"known_gap_\w+", tst)) | set(re.findall(r"known_gap_\w+", doc))
    assert named == set(KNOWN_GAPS)
    for gap in KNOWN_GAPS:
        assert gap in tst, gap
        assert gap in doc, gap
        assert f"def test_{gap}" in Path(__file__).read_text(encoding="utf-8"), gap


# Verifies: TST-006
@pytest.mark.parametrize(
    "added",
    [
        "getattr(pytest, 'sk' + 'ip')('x')\n",
        "setattr(test_a, '__test__', False)\n",
        "globals()['__test__'] = False\n",
    ],
)
def test_known_gap_py_runtime_gate(tmp_path: Path, added: str) -> None:
    """A skip or `__test__` built at run time is not seen by the AST scan."""
    assert py_gap(tmp_path, PY_GAP_BASE + added) == []


# Verifies: TST-006
def test_known_gap_py_existing_helper(tmp_path: Path) -> None:
    """A new call to a helper that already existed (and already skipped) is not a new gate."""
    assert py_gap(tmp_path, PY_GAP_BASE.replace("    assert 1", "    helper()")) == []


DEAD_RS = "#[test]\nfn t() {\n    if false {\n        assert!(f());\n    }\n}\n"


# Verifies: TST-006
@pytest.mark.parametrize("body", ["    return\n    assert 1", "    pass"])
def test_known_gap_hollowed_test(tmp_path: Path, body: str) -> None:
    """A test that returns early or whose body is emptied still exists, so it is not deleted."""
    assert py_gap(tmp_path, PY_GAP_BASE.replace("    assert 1", body)) == []
    rs = rs_gap(
        tmp_path / "rs",
        {"crates/c/tests/t.rs": "#[test]\nfn t() {\n    assert!(f());\n}\n"},
        {"crates/c/tests/t.rs": "#[test]\nfn t() {\n    return;\n}\n"},
    )
    assert rs == []
    dead = rs_gap(
        tmp_path / "dead",
        {"crates/c/tests/t.rs": "#[test]\nfn t() {\n    assert!(f());\n}\n"},
        {"crates/c/tests/t.rs": DEAD_RS},
    )
    assert dead == []


# Verifies: TST-006
def test_known_gap_py_nondefault_name(tmp_path: Path) -> None:
    """A unittest.TestCase method in a class not named Test* is collected by pytest, not by us."""
    head = PY_GAP_BASE.replace("    def test_m(self):\n        pass\n", "    pass\n")
    assert py_gap(tmp_path, head) == []
    assert "Checks::test_m" not in ratchet.python_test_names(PY_GAP_BASE)


# Verifies: TST-006
def test_known_gap_py_no_count() -> None:
    """There is no executed-test count for pytest: the count job lists only cargo nextest."""
    wf = yaml.safe_load(
        (ROOT / ".github" / "workflows" / "ratchet.yml").read_text(encoding="utf-8")
    )
    runs = " ".join(
        str(step.get("run", ""))
        for job in ("list-merge-base", "list-head", "test-count")
        for step in wf["jobs"][job]["steps"]
    )
    assert "cargo nextest list" in runs
    assert "pytest" not in runs


# Verifies: TST-006
@pytest.mark.parametrize(
    "attr",
    [
        '#[cfg(not(feature = "std"))]',
        '#![cfg(not(feature = "std"))]',
        '#[cfg(feature = "serde")]',
        '#![cfg(not(target_arch = "wasm32"))]',
        '#[ignore = "data: not really"]',
        '#[ignore = "network"]',
    ],
)
def test_known_gap_rs_allowlisted_gate(tmp_path: Path, attr: str) -> None:
    """An allowlisted gate (declared-feature cfg, data/network ignore) can hide a PR-tier test.

    Only `ratchet (test count)` sees it, and its head listing is produced by PR code.
    """
    found = rs_gap(
        tmp_path,
        {"crates/c/tests/t.rs": "#[test]\nfn t() {}\n"},
        {"crates/c/tests/t.rs": f"{attr}\n#[test]\nfn t() {{}}\n"},
    )
    assert found == []


# Verifies: TST-006
def test_known_gap_rs_uncompiled_file(tmp_path: Path) -> None:
    """A test file moved out of the cargo build, or a dropped `mod` line, keeps its names."""
    moved = rs_gap(
        tmp_path / "moved",
        {"crates/c/tests/t.rs": "#[test]\nfn t() {}\n"},
        {"crates/c/tests/off/t.rs": "#[test]\nfn t() {}\n"},
    )
    assert moved == []
    dropped = rs_gap(
        tmp_path / "dropped",
        {"crates/c/src/lib.rs": "mod unit;\n", "crates/c/src/unit.rs": "#[test]\nfn u() {}\n"},
        {"crates/c/src/lib.rs": "", "crates/c/src/unit.rs": "#[test]\nfn u() {}\n"},
    )
    assert dropped == []


# Verifies: TST-006
def test_known_gap_rs_same_name_items(tmp_path: Path) -> None:
    """Two same-named fns of one module (two impl blocks) are one item, so a gate can move."""
    two = (
        "struct A;\nstruct B;\n"
        "impl A {{\n    #[test]\n{0}    fn t() {{}}\n}}\n"
        "impl B {{\n    #[test]\n{1}    fn t() {{}}\n}}\n"
    )
    ign = '    #[ignore = "nightly: slow"]\n'
    found = rs_gap(
        tmp_path,
        {"crates/c/tests/t.rs": two.format(ign, "")},
        {"crates/c/tests/t.rs": two.format("", ign)},
    )
    assert found == []


# Verifies: TST-006
def test_known_gap_scan_scope(tmp_path: Path) -> None:
    """Only Rust under crates/ and Python under tools/ are scanned."""
    base, h = tmp_path / "b", tmp_path / "h"
    write(base, "tests/test_x.py", "def test_x():\n    pass\n")
    write(base, "other/t.rs", "#[test]\nfn t() {}\n")
    write(h, "tests/test_x.py", "import pytest\n\n\n@pytest.mark.skip\ndef test_x():\n    pass\n")
    write(h, "other/t.rs", "#[test]\n#[ignore]\nfn t() {}\n")
    assert ratchet.source_findings(base, h, CFG) == []


# Verifies: TST-006
@pytest.mark.parametrize(
    "imported",
    [
        "from _pytest.outcomes import Skipped",
        "from _pytest.outcomes import XFailed as Skipped",
        "from _pytest.outcomes import Exit as Skipped",
        "from _pytest.outcomes import OutcomeException as Skipped",
    ],
)
def test_known_gap_py_outcome_exception(tmp_path: Path, imported: str) -> None:
    """Raising pytest's outcome exceptions directly skips or stops a test without a gate name."""
    head = PY_GAP_BASE.replace("import pytest\n", f"import pytest\n{imported}\n", 1).replace(
        "    assert 1", "    raise Skipped('x')"
    )
    assert py_gap(tmp_path, head) == []


# Verifies: TST-006
def test_known_gap_py_empty_parametrize(tmp_path: Path) -> None:
    """An empty parameter set makes pytest skip the test (empty_parameter_set_mark default)."""
    head = PY_GAP_BASE.replace("def test_a():", "@pytest.mark.parametrize('x', [])\ndef test_a(x):")
    assert py_gap(tmp_path, head) == []


# Verifies: TST-006
@pytest.mark.parametrize("stop", ["import os\nos._exit(0)\n", "import sys\nsys.exit(0)\n"])
def test_known_gap_py_process_exit(tmp_path: Path, stop: str) -> None:
    """A new test file that ends the process at import stops pytest with exit 0 and no summary."""
    base, h = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", PY_GAP_BASE)
    write(h, "tools/tests/test_a.py", PY_GAP_BASE)
    write(h, "tools/tests/test_0.py", stop)
    assert ratchet.source_findings(base, h, CFG) == []


# Verifies: TST-006
@pytest.mark.parametrize(
    "ctor",
    ["    def __init__(self):\n        pass\n", "    def __new__(cls):\n        return None\n"],
)
def test_known_gap_py_uncollectable_class(tmp_path: Path, ctor: str) -> None:
    """pytest does not collect a Test* class with __init__/__new__ (a warning only)."""
    base_text = PY_GAP_BASE + "\n\nclass TestK:\n    def test_k(self):\n        assert 1\n"
    head = base_text.replace("class TestK:\n", f"class TestK:\n{ctor}\n", 1)
    base, h = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", base_text)
    write(h, "tools/tests/test_a.py", head)
    assert ratchet.source_findings(base, h, CFG) == []


# Verifies: TST-006
def test_known_gap_py_fixture_on_test(tmp_path: Path) -> None:
    """A fixture decorator on a test* function stops its collection; it still counts as a test."""
    head = PY_GAP_BASE.replace("def test_a():", "@pytest.fixture\ndef test_a():")
    assert py_gap(tmp_path, head) == []
    assert "test_a" in ratchet.python_test_names(head)


# Verifies: TST-006
def test_known_gap_py_assigned_hook(tmp_path: Path) -> None:
    """A pytest hook bound by assignment (not `def pytest_*`), e.g. in a subdirectory conftest."""
    base, h = tmp_path / "b", tmp_path / "h"
    write(base, "tools/tests/test_a.py", PY_GAP_BASE)
    write(h, "tools/tests/test_a.py", PY_GAP_BASE)
    write(
        h,
        "tools/tests/sub/conftest.py",
        "pytest_collection_modifyitems = lambda session, config, items: items.clear()\n",
    )
    assert ratchet.source_findings(base, h, CFG) == []


# Verifies: TST-006
@pytest.mark.parametrize("attr", ["#[should_panic]", '#[should_panic(expected = "")]'])
def test_known_gap_rs_verdict_attribute(tmp_path: Path, attr: str) -> None:
    """`should_panic` turns a failing assertion into a pass; the test still runs and is counted."""
    found = rs_gap(
        tmp_path,
        {"crates/c/tests/t.rs": "#[test]\nfn t() {\n    assert_eq!(1, 2);\n}\n"},
        {"crates/c/tests/t.rs": f"#[test]\n{attr}\nfn t() {{\n    assert_eq!(1, 2);\n}}\n"},
    )
    assert found == []
