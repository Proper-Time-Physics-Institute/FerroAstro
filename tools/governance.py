# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Maintainer-owned merge controls (ORG-009, ORG-011, TST-004, TST-006; ADR-0015).

The ``hold`` and ``ratchet`` workflows run this script from a checkout of the **base** SHA, never
from the PR head, so a PR cannot change the rules it is judged by. It reads the GitHub REST API
with the workflow's read-only ``GITHUB_TOKEN`` and never writes. Standard library only, so the
``hold`` job needs no ``uv sync``.

Who counts as a maintainer is ``.github/governance/maintainers.txt`` in the base checkout (one
GitHub login per line, ``#`` comments). A label or review by anyone else has no effect, because
agent accounts can write labels whenever they can open PRs.

Subcommands::

    python governance.py hold  --maintainers FILE  # exit 1 while the PR is on hold
    python governance.py flags --maintainers FILE  # approved=… golden_change=… to $GITHUB_OUTPUT

Rules:

* **hold** (restrictive label). The PR is held while the ``hold`` label is on it, whoever set it
  (fail-safe), and after the label is removed until a maintainer has removed it: the hold clears
  only when the most recent ``unlabeled`` event by a maintainer comes after the most recent
  ``labeled`` event by anyone. So only a maintainer can clear any hold. If a non-maintainer removed
  the label first, a maintainer clears the hold by adding and removing the label again.
* **golden-change** (permissive label). It counts only while it is on the PR and its most recent
  ``labeled`` event was by a maintainer (TST-004).
* **approved**. Some maintainer's most recent decisive review (approved, changes requested or
  dismissed) is ``APPROVED`` and was given on the PR's current head SHA (TST-006 override).

Any API failure fails closed: the PR is held, and approval and labels count as absent.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.request
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

API = "https://api.github.com"
HOLD_LABEL = "hold"
GOLDEN_LABEL = "golden-change"
QUEUE_REF_RE = re.compile(r"/pr-(\d+)-[0-9a-f]+$")
DECISIVE = {"APPROVED", "CHANGES_REQUESTED", "DISMISSED"}


def load_maintainers(path: Path) -> frozenset[str]:
    """Return the maintainer logins listed in ``path`` (case-folded, ``@`` stripped)."""
    logins = set()
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            logins.add(line.lstrip("@").casefold())
    return frozenset(logins)


def is_maintainer(login: str | None, maintainers: frozenset[str]) -> bool:
    """Whether ``login`` is a maintainer."""
    return login is not None and login.casefold() in maintainers


def _label_events(events: Iterable[Mapping[str, Any]], label: str) -> list[Mapping[str, Any]]:
    """Return the labeled/unlabeled events for ``label``, in API (chronological) order."""
    return [
        e
        for e in events
        if e.get("event") in ("labeled", "unlabeled")
        and (e.get("label") or {}).get("name") == label
    ]


def hold_active(
    events: Iterable[Mapping[str, Any]],
    current_labels: Iterable[str],
    maintainers: frozenset[str],
) -> bool:
    """Whether the PR is held (see the module docstring)."""
    if HOLD_LABEL in set(current_labels):
        return True
    last_set = last_clear = -1
    for i, e in enumerate(_label_events(events, HOLD_LABEL)):
        if e.get("event") == "labeled":
            last_set = i
        elif is_maintainer((e.get("actor") or {}).get("login"), maintainers):
            last_clear = i
    return last_set > last_clear


def maintainer_label(
    events: Iterable[Mapping[str, Any]],
    current_labels: Iterable[str],
    label: str,
    maintainers: frozenset[str],
) -> bool:
    """Whether ``label`` is on the PR and was last applied by a maintainer."""
    if label not in set(current_labels):
        return False
    applied = [e for e in _label_events(events, label) if e.get("event") == "labeled"]
    return bool(applied) and is_maintainer(
        (applied[-1].get("actor") or {}).get("login"), maintainers
    )


def maintainer_approved(
    reviews: Iterable[Mapping[str, Any]], head_sha: str, maintainers: frozenset[str]
) -> bool:
    """Whether a maintainer's latest decisive review approves ``head_sha``."""
    latest: dict[str, Mapping[str, Any]] = {}
    for review in reviews:
        login = (review.get("user") or {}).get("login")
        if is_maintainer(login, maintainers) and review.get("state") in DECISIVE:
            latest[str(login).casefold()] = review
    return any(
        r.get("state") == "APPROVED" and r.get("commit_id") == head_sha for r in latest.values()
    )


def pr_number(event_name: str, event: Mapping[str, Any]) -> int:
    """Return the PR number for a ``pull_request*`` or ``merge_group`` event.

    The merge queue must use group size 1 (ADR-0015), so the queue ref names exactly one PR.
    """
    if event_name == "merge_group":
        ref = str((event.get("merge_group") or {}).get("head_ref", ""))
        match = QUEUE_REF_RE.search(ref)
        if not match:
            raise ValueError(f"cannot read a PR number from merge-queue ref {ref!r}")
        return int(match.group(1))
    return int(event["pull_request"]["number"])


class GitHub:
    """Minimal read-only GitHub REST client."""

    def __init__(self, repo: str, token: str) -> None:
        """Bind to ``owner/name`` with ``token``."""
        self.repo = repo
        self.token = token

    def get(self, path: str) -> Any:  # noqa: ANN401 (JSON)
        """GET one API path (relative to the repository) and decode the JSON."""
        url = path if path.startswith("https://") else f"{API}/repos/{self.repo}/{path}"
        request = urllib.request.Request(  # noqa: S310 (fixed https API host)
            url,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310
            return json.load(response)

    def get_all(self, path: str) -> list[Any]:
        """GET every page of a list endpoint."""
        items: list[Any] = []
        sep = "&" if "?" in path else "?"
        for page in range(1, 51):
            batch = self.get(f"{path}{sep}per_page=100&page={page}")
            items.extend(batch)
            if len(batch) < 100:
                break
        return items


def _context() -> tuple[GitHub, int]:
    event_name = os.environ["GITHUB_EVENT_NAME"]
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    gh = GitHub(os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_TOKEN"])
    return gh, pr_number(event_name, event)


def cmd_hold(maintainers: frozenset[str]) -> int:
    """Exit 1 while the PR is held (ORG-009 acceptance 4)."""
    try:
        gh, number = _context()
        labels = [lab["name"] for lab in gh.get(f"issues/{number}")["labels"]]
        events = gh.get_all(f"issues/{number}/events")
    except Exception as exc:  # fail closed on any API problem
        print(f"::error::hold: cannot read PR state ({exc}); failing closed (ORG-009)")
        return 1
    if hold_active(events, labels, maintainers):
        print(f"::error::PR #{number} is on hold; only a maintainer can clear it (ORG-009).")
        return 1
    print(f"PR #{number} is not on hold.")
    return 0


def cmd_flags(maintainers: frozenset[str]) -> int:
    """Write ``approved=`` and ``golden_change=`` to ``$GITHUB_OUTPUT`` (and print them)."""
    approved = golden = False
    try:
        gh, number = _context()
        pr = gh.get(f"pulls/{number}")
        labels = [lab["name"] for lab in pr["labels"]]
        events = gh.get_all(f"issues/{number}/events")
        reviews = gh.get_all(f"pulls/{number}/reviews")
        approved = maintainer_approved(reviews, pr["head"]["sha"], maintainers)
        golden = maintainer_label(events, labels, GOLDEN_LABEL, maintainers)
    except Exception as exc:  # fail closed: no flag is granted
        print(f"::warning::governance: cannot read PR state ({exc}); no override granted")
    lines = f"approved={str(approved).lower()}\ngolden_change={str(golden).lower()}\n"
    print(lines, end="")
    if out := os.environ.get("GITHUB_OUTPUT"):
        with Path(out).open("a", encoding="utf-8") as fh:
            fh.write(lines)
    return 0


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=["hold", "flags"])
    parser.add_argument("--maintainers", type=Path, required=True)
    args = parser.parse_args(argv)
    maintainers = load_maintainers(args.maintainers)
    if not maintainers:
        print("::error::governance: the maintainer list is empty")
        return 1
    return cmd_hold(maintainers) if args.command == "hold" else cmd_flags(maintainers)


if __name__ == "__main__":
    sys.exit(main())
