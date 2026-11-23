# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Checks on the repository's own governance files: workflows, AGENTS.md, issue forms, labels."""

from __future__ import annotations

import re
import tomllib
from pathlib import Path
from typing import Any

import pytest
import yaml

import governance as gov
import lint_workflows
from oracles import drift

ROOT = Path(__file__).resolve().parents[2]
FORMS = ROOT / ".github" / "ISSUE_TEMPLATE"
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
# A link target with a URI scheme (https:, mailto:, ...) is not a file; it is not checked.
SCHEME_RE = re.compile(r"[A-Za-z][A-Za-z0-9+.-]*:")
# Element types of the GitHub issue-form schema.
FORM_TYPES = {"markdown", "input", "textarea", "dropdown", "checkboxes"}
AGENT_DOCS = ("AGENTS.md", "CLAUDE.md", "CONTRIBUTING.md", "README.md")
# The crate that the DEP-016 acceptance names, and the diffsol features that enable it.
NATIVE_CANARY_CRATE = "suitesparse_sys"
NATIVE_CANARY_FEATURES = {"suitesparse", "sundials"}


def load_yaml(path: Path) -> Any:
    return yaml.safe_load(path.read_text(encoding="utf-8"))


def label_names() -> set[str]:
    return {entry["name"] for entry in load_yaml(ROOT / ".github" / "labels.yml")}


# Verifies: STD-012
def test_real_workflows_pass_the_workflow_lint() -> None:
    assert sorted((ROOT / ".github" / "workflows").glob("*.y*ml")), "no workflow found"
    assert lint_workflows.lint_repo(ROOT) == []


# Verifies: AGT-001
@pytest.mark.parametrize("name", AGENT_DOCS)
def test_relative_links_of_the_root_docs_resolve(name: str) -> None:
    doc = ROOT / name
    targets = [
        t
        for t in LINK_RE.findall(doc.read_text(encoding="utf-8"))
        if not SCHEME_RE.match(t) and not t.startswith("#")
    ]
    broken = [t for t in targets if not (doc.parent / t.split("#", 1)[0]).exists()]
    assert broken == []


# Verifies: AGT-001
def test_agents_md_is_canonical_and_lists_the_commands() -> None:
    text = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
    for command in (
        "cargo nextest run",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        "uv run ruff check",
        "trace.py --check",
        "cargo deny check",
    ):
        assert command in text, command
    assert "@AGENTS.md" in (ROOT / "CLAUDE.md").read_text(encoding="utf-8")


# Verifies: AGT-003
def test_agent_task_form_requires_its_four_fields() -> None:
    form = load_yaml(FORMS / "agent-task.yml")
    required = {
        block["id"]
        for block in form["body"]
        if block.get("validations", {}).get("required") is True
    }
    assert {"scope", "files", "done-definition", "commands"} <= required


# Verifies: ORG-006
def test_issue_forms_exist_and_are_well_formed() -> None:
    names = {path.name for path in FORMS.glob("*.yml")}
    assert {
        "bug.yml",
        "requirement.yml",
        "physics-discrepancy.yml",
        "adr-proposal.yml",
        "agent-task.yml",
    } <= names
    for path in sorted(FORMS.glob("*.yml")):
        form = load_yaml(path)
        assert form["name"], path.name
        assert form["description"], path.name
        ids = [block["id"] for block in form["body"] if "id" in block]
        assert len(ids) == len(set(ids)), path.name
        assert any(block["type"] != "markdown" for block in form["body"]), path.name


# Verifies: ORG-006
def test_issue_form_elements_follow_the_schema_basics() -> None:
    for path in sorted(FORMS.glob("*.yml")):
        for block in load_yaml(path)["body"]:
            where = f"{path.name}: {block.get('id', block.get('type'))}"
            assert block["type"] in FORM_TYPES, where
            attributes = block["attributes"]
            if block["type"] == "markdown":
                assert attributes["value"], where
                assert "id" not in block, where
                continue
            assert re.fullmatch(r"[A-Za-z0-9_-]+", block["id"]), where
            assert attributes["label"], where
            if block["type"] in ("dropdown", "checkboxes"):
                assert attributes["options"], where
            assert isinstance(block.get("validations", {}).get("required", False), bool), where


# Verifies: ORG-006
def test_labels_file_is_well_formed() -> None:
    entries = load_yaml(ROOT / ".github" / "labels.yml")
    names = [entry["name"] for entry in entries]
    assert len(names) == len(set(names))
    for entry in entries:
        assert set(entry) == {"name", "color", "description"}, entry
        assert re.fullmatch(r"[0-9a-f]{6}", entry["color"]), entry
        assert entry["description"], entry


# Verifies: ORG-006
def test_labels_used_by_forms_and_checkers_are_defined() -> None:
    defined = label_names()
    used = {gov.HOLD_LABEL, gov.GOLDEN_LABEL, *drift.ISSUE_LABELS}
    for path in sorted(FORMS.glob("*.yml")):
        used |= set(load_yaml(path).get("labels", []))
    assert used <= defined, sorted(used - defined)


# Verifies: DEP-016
def test_native_library_crates_are_banned_and_absent_from_the_lockfile() -> None:
    bans = tomllib.loads((ROOT / "deny.toml").read_text(encoding="utf-8"))["bans"]
    banned = {entry["crate"] for entry in bans["deny"]}
    assert NATIVE_CANARY_CRATE in banned
    diffsol = [entry for entry in bans["features"] if entry["crate"] == "diffsol"]
    assert len(diffsol) == 1
    assert set(diffsol[0]["deny"]) >= NATIVE_CANARY_FEATURES
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    locked = {package["name"] for package in lock["package"]}
    assert locked & banned == set()
