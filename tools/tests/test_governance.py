# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests for tools/governance.py: the hold rule, maintainer labels and approvals (ADR-0015)."""

from __future__ import annotations

import re
from pathlib import Path

import pytest

import governance as gov

ROOT = Path(__file__).resolve().parents[2]
M = frozenset({"alice", "bob"})


def ev(kind: str, label: str, actor: str) -> dict[str, object]:
    return {"event": kind, "label": {"name": label}, "actor": {"login": actor}}


# Verifies: ORG-009
def test_hold_label_present_holds_whoever_set_it() -> None:
    assert gov.hold_active([ev("labeled", "hold", "agent")], ["hold"], M)
    assert not gov.hold_active([], [], M)


# Verifies: ORG-009
def test_only_a_maintainer_can_clear_a_hold() -> None:
    events = [ev("labeled", "hold", "alice"), ev("unlabeled", "hold", "agent")]
    assert gov.hold_active(events, [], M)
    events.append(ev("unlabeled", "hold", "Bob"))
    assert not gov.hold_active(events, [], M)


# Verifies: ORG-009
def test_a_non_maintainer_hold_also_needs_a_maintainer_to_clear() -> None:
    events = [ev("labeled", "hold", "agent"), ev("unlabeled", "hold", "agent")]
    assert gov.hold_active(events, [], M)
    events += [ev("labeled", "hold", "alice"), ev("unlabeled", "hold", "alice")]
    assert not gov.hold_active(events, [], M)
    events.append(ev("labeled", "hold", "agent"))
    assert gov.hold_active(events, ["hold"], M)
    assert gov.hold_active(events, [], M)


# Verifies: TST-004
def test_golden_change_counts_only_when_a_maintainer_applied_it() -> None:
    lab = "golden-change"
    assert gov.maintainer_label([ev("labeled", lab, "alice")], [lab], lab, M)
    assert not gov.maintainer_label([ev("labeled", lab, "agent")], [lab], lab, M)
    assert not gov.maintainer_label([ev("labeled", lab, "alice")], [], lab, M)
    relabeled = [ev("labeled", lab, "alice"), ev("unlabeled", lab, "x"), ev("labeled", lab, "x")]
    assert not gov.maintainer_label(relabeled, [lab], lab, M)


# Verifies: TST-006
def test_approval_must_be_a_maintainer_on_the_current_head() -> None:
    def review(user: str, state: str, sha: str) -> dict[str, object]:
        return {"user": {"login": user}, "state": state, "commit_id": sha}

    assert gov.maintainer_approved([review("alice", "APPROVED", "h2")], "h2", M)
    assert not gov.maintainer_approved([review("alice", "APPROVED", "h1")], "h2", M)
    assert not gov.maintainer_approved([review("agent", "APPROVED", "h2")], "h2", M)
    later = [review("alice", "APPROVED", "h2"), review("alice", "CHANGES_REQUESTED", "h2")]
    assert not gov.maintainer_approved(later, "h2", M)
    commented = [review("alice", "APPROVED", "h2"), review("alice", "COMMENTED", "h2")]
    assert gov.maintainer_approved(commented, "h2", M)


# Verifies: ORG-009
def test_pr_number_from_events() -> None:
    assert gov.pr_number("pull_request", {"pull_request": {"number": 7}}) == 7
    queue = {"merge_group": {"head_ref": "refs/heads/gh-readonly-queue/main/pr-46-abc123"}}
    assert gov.pr_number("merge_group", queue) == 46
    with pytest.raises(ValueError, match="merge-queue ref"):
        gov.pr_number("merge_group", {"merge_group": {"head_ref": "refs/heads/main"}})


# Verifies: ORG-011
def test_codeowners_match_the_maintainer_list() -> None:
    maintainers = gov.load_maintainers(ROOT / ".github" / "governance" / "maintainers.txt")
    assert maintainers
    owners: set[str] = set()
    for raw in (ROOT / ".github" / "CODEOWNERS").read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].split()
        if line:
            assert len(line) > 1, f"pattern without owner: {raw}"
            assert {o.lstrip("@").casefold() for o in line[1:]} == maintainers, raw
            owners.update(line[1:])
    assert {o.lstrip("@").casefold() for o in owners} == maintainers


# Verifies: ORG-011
def test_codeowners_cover_the_org011_surface() -> None:
    patterns = {
        raw.split()[0]
        for raw in (ROOT / ".github" / "CODEOWNERS").read_text(encoding="utf-8").splitlines()
        if raw.split("#", 1)[0].strip()
    }
    required = {
        "/tests/vectors/", "/ci/", "/.github/", "/tools/trace.py", "/tools/lint_*.py",
        "/deny.toml", "/rust-toolchain.toml", "/REUSE.toml", "/LICENSE*", "/LICENSES/",
        "/NOTICE", "/CONTRIBUTING.md", "AGENTS.md", "/CLAUDE.md", "/docs/adr/",
        "/tools/ratchet.py", "/tools/governance.py", "/tools/verify_signatures.py",
    }  # fmt: skip
    assert required <= patterns, required - patterns
    org = (ROOT / "docs" / "requirements" / "ORG.md").read_text(encoding="utf-8")
    block = re.search(r"id: ORG-011\n.*?```", org, re.DOTALL)
    assert block is not None
    for pattern in patterns:
        assert pattern.strip("/") in block.group(0), f"{pattern} missing from ORG-011"
