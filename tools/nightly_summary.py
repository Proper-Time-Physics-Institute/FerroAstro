# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Label the failures of a nightly nextest log against the committed known-failures list.

WP28 (progress review W3 after WP26). The nightly tier carries pre-registered bounds that fail and
are disclosed as NEEDS HUMAN REVIEW items; they are listed in ``ci/nightly-known-failures.toml``.
This tool reads a nextest log (the output of ``tools/run_long_local.sh``) and prints, for every
failing test:

* ``known (value unchanged)``: the test and its failing checks are listed, and each listed value
  printed by the test equals the last measured value within the entry's ``rel_change``;
* ``known (value changed)``: listed, but a value moved (or was not printed);
* ``NEW``: the test is not listed, or it fails a check (a vector id or an inner ``FAIL`` line)
  that its entries do not cover.

It also reports listed tests that passed (``known failure now PASSES``) or did not run. It changes
no bound and no test. Exit status: 0 when every failure is known and unchanged, 1 otherwise, 2 for
a usage or input error.

Usage::

    uv run python nightly_summary.py <nextest log> [--known ../ci/nightly-known-failures.toml]
"""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_KNOWN = ROOT / "ci" / "nightly-known-failures.toml"

# A nextest status line: "        FAIL [  12.3s] (3/5) crate::binary test_name".
HEADER = re.compile(
    r"^\s*(PASS|FAIL|FLAKY|TIMEOUT|SIGSEGV|SIGABRT|SIGKILL|ABORT|LEAK)\s+\[\s*[0-9.]+s\]"
    r"(?:\s+\([^)]*\))?\s+(\S+)\s+(\S+)\s*$"
)
# A failing check inside a test's output (not a nextest status line).
INNER_FAIL = re.compile(r"^\s*FAIL (?!\[)(.*)$")
# Vector ids named by a failing tolerance check.
VECTOR_IDS = (
    re.compile(r'ToleranceExceeded\(Deviation \{ id: "([A-Z0-9-]+)"'),
    re.compile(r"FA-E0105: ([A-Z0-9][A-Z0-9-]+)"),
)
FAILING = {"FAIL", "TIMEOUT", "SIGSEGV", "SIGABRT", "SIGKILL", "ABORT", "LEAK"}


@dataclass(frozen=True)
class Known:
    """One entry of the known-failures list."""

    test: str
    vector: str
    review: str
    last_value: float
    unit: str
    value_regex: re.Pattern[str]
    rel_change: float


@dataclass
class TestRun:
    """A test's status and output text in the log."""

    status: str
    text: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class Line:
    """One summary line: the label, the test, and the detail."""

    label: str
    test: str
    detail: str


def load_known(path: Path) -> list[Known]:
    """Read the known-failures TOML.

    Raises
    ------
    ValueError
        For a missing field or a regex without exactly one capture group.
    """
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        raise ValueError(f"{path}: schema_version must be 1")
    out = []
    for i, e in enumerate(data.get("failure", [])):
        try:
            rx = re.compile(str(e["value_regex"]))
            if rx.groups != 1:
                raise ValueError(f"{path}: entry {i}: value_regex needs exactly one group")
            out.append(
                Known(
                    test=str(e["test"]),
                    vector=str(e["vector"]),
                    review=str(e["review"]),
                    last_value=float(e["last_value"]),
                    unit=str(e["unit"]),
                    value_regex=rx,
                    rel_change=float(e["rel_change"]),
                )
            )
        except KeyError as err:
            raise ValueError(f"{path}: entry {i}: missing field {err}") from err
    return out


def parse_log(text: str) -> dict[str, TestRun]:
    """Split a nextest log into each test's last status and its output lines."""
    runs: dict[str, TestRun] = {}
    current: TestRun | None = None
    for line in text.splitlines():
        m = HEADER.match(line)
        if m:
            status, _binary, test = m.groups()
            run = runs.setdefault(test, TestRun(status))
            run.status = status
            current = run
            continue
        if current is not None:
            current.text.append(line)
    return runs


def _value(entry: Known, text: str) -> float | None:
    m = entry.value_regex.search(text)
    if m is None:
        return None
    try:
        return float(m.group(1))
    except ValueError:
        return None


def summarise(runs: dict[str, TestRun], known: list[Known]) -> list[Line]:
    """Label every failing test and every listed test (see the module docstring)."""
    by_test: dict[str, list[Known]] = {}
    for k in known:
        by_test.setdefault(k.test, []).append(k)
    lines: list[Line] = []
    for test, run in sorted(runs.items()):
        if run.status not in FAILING:
            if test in by_test:
                lines.append(Line("known failure now PASSES", test, "listed; ask a human"))
            continue
        entries = by_test.get(test)
        if not entries:
            lines.append(Line("NEW", test, f"{run.status}; not in the known-failures list"))
            continue
        text = "\n".join(run.text)
        for k in entries:
            v = _value(k, text)
            if v is None:
                lines.append(
                    Line(
                        "known (value changed)",
                        test,
                        f"{k.vector}: value not found in the output (last {k.last_value:g} "
                        f"{k.unit}); {k.review}",
                    )
                )
                continue
            rel = abs(v - k.last_value) / max(abs(k.last_value), sys.float_info.min)
            label = "known (value unchanged)" if rel <= k.rel_change else "known (value changed)"
            lines.append(
                Line(
                    label,
                    test,
                    f"{k.vector}: {v:.10g} {k.unit} (last {k.last_value:.10g}, "
                    f"rel. change {rel:.2e}); {k.review}",
                )
            )
        listed_ids = {k.vector for k in entries}
        for rx in VECTOR_IDS:
            for vid in sorted(set(rx.findall(text)) - listed_ids):
                lines.append(Line("NEW", test, f"failing vector {vid} is not listed"))
        for raw in run.text:
            m = INNER_FAIL.match(raw)
            if m and not any(k.value_regex.search(raw) for k in entries):
                lines.append(Line("NEW", test, f"unlisted failing check: {m.group(1)[:160]}"))
    for test in sorted(set(by_test) - set(runs)):
        lines.append(Line("not run", test, "listed, but absent from this log"))
    return lines


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("log", type=Path, help="nextest log of a nightly run")
    ap.add_argument("--known", type=Path, default=DEFAULT_KNOWN)
    args = ap.parse_args(argv)
    try:
        known = load_known(args.known)
        runs = parse_log(args.log.read_text(encoding="utf-8", errors="replace"))
    except (OSError, ValueError, tomllib.TOMLDecodeError) as err:
        print(f"nightly_summary: {err}", file=sys.stderr)
        return 2
    lines = summarise(runs, known)
    print(f"## nightly summary against {args.known.name} ({len(known)} known failures)")
    for ln in lines:
        print(f"- {ln.label}: {ln.test}: {ln.detail}")
    bad = [ln for ln in lines if ln.label in {"NEW", "known (value changed)"}]
    failing = sum(1 for r in runs.values() if r.status in FAILING)
    print(
        f"failing tests: {failing}; NEW or changed: {len(bad)}; "
        f"verdict: {'ONLY KNOWN, UNCHANGED' if not bad else 'ATTENTION'}"
    )
    return 0 if not bad else 1


if __name__ == "__main__":
    raise SystemExit(main())
