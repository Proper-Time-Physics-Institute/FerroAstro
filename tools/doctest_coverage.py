# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""API-003 doctest coverage: which public functions carry a runnable example in their docs.

API-003 asks that every public function have a doctest. ``missing_doc_code_examples`` is an
unstable rustdoc lint, so this script reads the sources instead: for every ``pub fn`` /
``pub const fn`` in ``crates/*/src`` (outside ``#[cfg(test)]`` modules) it collects the
``///`` doc block directly above the item (attributes allowed in between) and counts the item
as covered when the block contains a fenced code block that rustdoc would run (```` ``` ```` or
```` ```rust ````, not ``text``/``ignore``/``no_run``). Crate- and module-level examples
(``//!``) do not count.

It is a heuristic over the source text, not a parser. Report-only by default; ``--min-ratio``
makes it fail below a coverage ratio, for when a maintainer sets one (WP7: NEEDS HUMAN REVIEW).

Usage::

    uv run python doctest_coverage.py [--min-ratio 0.0]
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PUB_FN = re.compile(r"^\s*pub(?:\(crate\))?\s+(?:const\s+)?(?:unsafe\s+)?fn\s+(\w+)")
FENCE = re.compile(r"^\s*///\s*```(\w*)")
RUNNABLE = {"", "rust"}


@dataclass(frozen=True)
class Item:
    """One public function."""

    crate: str
    path: str
    line: int
    name: str
    covered: bool


def items_in(text: str, crate: str, path: str) -> list[Item]:
    """Public functions of one source file, with their doctest coverage."""
    lines = text.splitlines()
    cut = next((k for k, s in enumerate(lines) if s.strip() == "#[cfg(test)]"), len(lines))
    out: list[Item] = []
    for i, line in enumerate(lines[:cut]):
        m = PUB_FN.match(line)
        if not m or "pub(crate)" in line:
            continue
        j = i - 1
        while j >= 0 and lines[j].strip().startswith("#["):
            j -= 1
        doc: list[str] = []
        while j >= 0 and lines[j].strip().startswith("///"):
            doc.append(lines[j])
            j -= 1
        covered = False
        inside = False
        for d in reversed(doc):  # source order; only opening fences carry a language
            f = FENCE.match(d)
            if f is None:
                continue
            if not inside and f.group(1) in RUNNABLE:
                covered = True
            inside = not inside
        out.append(Item(crate, path, i + 1, m.group(1), covered))
    return out


def scan(root: Path = ROOT) -> list[Item]:
    """All public functions of the workspace crates."""
    items: list[Item] = []
    for src in sorted((root / "crates").glob("*/src")):
        crate = src.parent.name
        for f in sorted(src.rglob("*.rs")):
            rel = f.relative_to(root).as_posix()
            items += items_in(f.read_text(encoding="utf-8"), crate, rel)
    return items


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--min-ratio", type=float, default=0.0)
    args = ap.parse_args(argv)
    items = scan()
    crates = sorted({i.crate for i in items})
    print(f"{'crate':28} {'pub fn':>7} {'doctest':>8} {'ratio':>6}")
    for c in crates:
        mine = [i for i in items if i.crate == c]
        cov = sum(i.covered for i in mine)
        print(f"{c:28} {len(mine):7} {cov:8} {cov / max(len(mine), 1):6.2f}")
    total = len(items)
    cov = sum(i.covered for i in items)
    ratio = cov / max(total, 1)
    print(f"{'total':28} {total:7} {cov:8} {ratio:6.2f}")
    if ratio < args.min_ratio:
        print(f"doctest coverage {ratio:.2f} below --min-ratio {args.min_ratio}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
