# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
r"""Nightly golden-drift check, refusal ledger and speed-table stub (CI-006; WP17).

Host side only (no oracle engine is imported). Inputs are the plain files the oracle containers
wrote under a candidate directory ``<cand>/<tool>/<name>/`` with their ``MANIFEST.json``:

* **drift**: every committed golden ``ci/golden/<tool>/<name>/`` must have a candidate whose
  manifest is complete (CI-003) and whose data rows are byte-identical (``repr`` binary64, so
  equal text is equal bits); JSON goldens are compared without their run-date keys. The ERFA
  candidate is compared with ``tests/goldens/``. A missing
  candidate is drift too: an oracle that stopped running cannot vouch for its golden.
* **report**: a markdown report (drift, refusals, per-run status) and, when there is drift, the
  body of a **blocking** issue (red-team H3 / control 5: drift is not only flagged). The issue is
  opened with ``gh issue create`` only when ``--open-issue`` is given; otherwise the body is
  printed (dry run). Labels: ``golden-drift``, ``blocking``.
* **speed table** (stub, CI-006/CI-008): wall-clock seconds and CPU per tool and scenario from the
  manifests. The fixed runner class of CI-008 is not defined yet, so the table is informational.

Usage::

    python -m oracles.drift --candidate target/oracle-out --status target/oracle-out/status.tsv \\
        --report target/oracle-out/report.md [--open-issue]

Exit status 0 without drift, 1 with drift (whether or not an issue was opened).
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from pathlib import Path

from oracles import common, compare_goldens

ROOT = Path(__file__).resolve().parents[2]
ERFA_GOLDENS = ("erfa-time-pyerfa.csv", "erfa-dat-pyerfa.csv", "erfa-frames-pyerfa.csv")
ISSUE_LABELS = ("golden-drift", "blocking")
#: Keys of JSON goldens that record when or how a run happened, not what it found.
#: ``last_modified`` is the HTTP header of a fresh download: the JPL runner records it only when
#: its cache is cold (``fetched``), so a warm-cache run lacks it (first full nightly, WP19). The
#: product itself is still compared through its ``sha256`` and ``pin_ok``.
VOLATILE_JSON_KEYS = frozenset(
    {"date_utc", "retrieved_utc", "fetched", "generated_utc", "last_modified"}
)


def stable_json(path: Path) -> object:
    """Return a JSON golden without its :data:`VOLATILE_JSON_KEYS` (recursively)."""

    def strip(x: object) -> object:
        if isinstance(x, dict):
            return {k: strip(v) for k, v in x.items() if k not in VOLATILE_JSON_KEYS}
        if isinstance(x, list):
            return [strip(v) for v in x]
        return x

    return strip(json.loads(path.read_text(encoding="utf-8")))


@dataclass(frozen=True)
class Finding:
    """One drift finding."""

    golden: str
    problem: str


def committed_goldens(golden_root: Path) -> list[Path]:
    """Every ``<tool>/<name>/`` directory under ``ci/golden``."""
    return sorted(d for d in golden_root.glob("*/*") if d.is_dir())


def check_drift(candidate: Path, golden_root: Path, tests_goldens: Path) -> list[Finding]:
    """Compare the candidates with the committed goldens; return the drift findings."""
    out: list[Finding] = []
    for g in committed_goldens(golden_root):
        rel = g.relative_to(golden_root).as_posix()
        c = candidate / rel
        if not c.is_dir():
            out.append(Finding(rel, "no candidate: the oracle did not produce this golden"))
            continue
        out.extend(Finding(rel, f"manifest: {p}") for p in common.validate_manifest(c))
        for f in sorted(p.name for p in g.iterdir() if p.name != common.MANIFEST_NAME):
            if not (c / f).exists():
                out.append(Finding(rel, f"{f}: missing in the candidate"))
            elif f.endswith(".json"):
                if stable_json(c / f) != stable_json(g / f):
                    out.append(Finding(rel, f"{f}: content differs (volatile keys ignored)"))
            elif compare_goldens.data_lines(c / f) != compare_goldens.data_lines(g / f):
                out.append(Finding(rel, f"{f}: data rows differ from the committed golden"))
    erfa = candidate / "erfa" / "erfa-goldens"
    if not erfa.is_dir():
        out.append(Finding("tests/goldens (erfa)", "no candidate: the erfa image did not run"))
    else:
        for f in ERFA_GOLDENS:
            if compare_goldens.data_lines(erfa / f) != compare_goldens.data_lines(
                tests_goldens / f
            ):
                out.append(Finding(f"tests/goldens/{f}", "data rows differ (REF-009)"))
    return out


def speed_rows(candidate: Path) -> list[tuple[str, str, float, str]]:
    """``(tool, scenario, wall_clock_s, cpu)`` of every manifest entry under the candidate."""
    rows: list[tuple[str, str, float, str]] = []
    for m in sorted(candidate.glob("*/*/" + common.MANIFEST_NAME)):
        doc = json.loads(m.read_text(encoding="utf-8"))
        rows.extend(
            (
                str(e.get("tool")),
                str(e.get("scenario")),
                float(e.get("wall_clock_s", 0.0)),
                str(e.get("cpu")),
            )
            for e in doc.get("entries", [])
        )
    return rows


def read_status(path: Path | None) -> list[list[str]]:
    """Read the driver's ``tool<TAB>scenario<TAB>exit<TAB>note`` lines (refusals are exit 3)."""
    if path is None or not path.exists():
        return []
    return [ln.split("\t") for ln in path.read_text(encoding="utf-8").splitlines() if ln.strip()]


def render(findings: Sequence[Finding], status: Sequence[Sequence[str]], candidate: Path) -> str:
    """Render the nightly report (markdown)."""
    lines = ["# Oracle nightly report (CI-006)", ""]
    lines += [f"Candidate directory: `{candidate}`", ""]
    if findings:
        lines += ["## DRIFT (blocking)", "", "| golden | problem |", "|---|---|"]
        lines += [f"| `{f.golden}` | {f.problem} |" for f in findings]
    else:
        lines += ["## No drift", "", "Every committed golden was reproduced bit for bit."]
    lines += ["", "## Runs", "", "| tool | scenario | exit | note |", "|---|---|---|---|"]
    for s in status:
        tool, scen, code, note = (*s, "", "", "", "")[:4]
        what = {"0": "ok", "3": "refused (REF-011)"}.get(code, "FAILED")
        lines.append(f"| {tool} | {scen} | {code} {what} | {note} |")
    lines += [
        "",
        "## Speed table (stub; CI-008 runner class not defined, informational)",
        "",
        "| tool | scenario | wall clock (s) | CPU |",
        "|---|---|---|---|",
    ]
    lines += [f"| {t} | {s} | {w:.2f} | {c} |" for t, s, w, c in speed_rows(candidate)]
    return "\n".join(lines) + "\n"


def open_issue(body: str, title: str) -> str:
    """Open the blocking drift issue with ``gh``; return its URL."""
    gh = shutil.which("gh")
    if gh is None:
        msg = "gh is not installed; cannot open the drift issue"
        raise RuntimeError(msg)
    cmd = [gh, "issue", "create", "--title", title, "--body", body]
    for label in ISSUE_LABELS:
        cmd += ["--label", label]
    r = subprocess.run(cmd, capture_output=True, text=True, check=True)  # noqa: S603
    return r.stdout.strip()


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    p.add_argument("--candidate", type=Path, required=True)
    p.add_argument("--golden", type=Path, default=ROOT / "ci" / "golden")
    p.add_argument("--tests-goldens", type=Path, default=ROOT / "tests" / "goldens")
    p.add_argument("--status", type=Path, default=None)
    p.add_argument("--report", type=Path, default=None)
    p.add_argument("--open-issue", action="store_true")
    p.add_argument("--date", default=common.utc_now()[:10])
    args = p.parse_args(argv)
    findings = check_drift(args.candidate, args.golden, args.tests_goldens)
    report = render(findings, read_status(args.status), args.candidate)
    if args.report is not None:
        args.report.write_text(report, encoding="utf-8")
    print(report)
    if not findings:
        return 0
    title = f"BLOCKING: oracle golden drift ({args.date}, {len(findings)} finding(s))"
    if args.open_issue:
        print(f"opened {open_issue(report, title)}")
    else:
        print(f"DRY RUN (no --open-issue): would open issue {title!r} labelled {ISSUE_LABELS}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
