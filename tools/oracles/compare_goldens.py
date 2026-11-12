# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Compare regenerated golden CSVs with the committed ones, data rows only (REF-009).

The ``#`` header lines record generation time, generator commit and environment, so they differ
between runs by design. Every other line (the column line and the data rows) must be identical,
byte for byte: the values are written with ``repr`` (shortest round-trip binary64), so equal text
means bit-identical numbers.

Usage (host side, after the ERFA image ran)::

    uv run python -m oracles.compare_goldens NEW_DIR ../tests/goldens FILE [FILE ...]

Exit status 0 when every file matches, 1 otherwise.
"""

from __future__ import annotations

import hashlib
import sys
from collections.abc import Sequence
from pathlib import Path


def data_lines(path: Path) -> list[str]:
    """Return the non-comment lines of a golden CSV."""
    return [
        line for line in path.read_text(encoding="utf-8").splitlines() if not line.startswith("#")
    ]


def first_difference(a: Sequence[str], b: Sequence[str]) -> str | None:
    """Describe the first differing data line of two files, or ``None`` when identical."""
    for i, (x, y) in enumerate(zip(a, b, strict=False)):
        if x != y:
            return f"data line {i}: {x!r} != {y!r}"
    if len(a) != len(b):
        return f"{len(a)} data lines != {len(b)}"
    return None


def digest(lines: Sequence[str]) -> str:
    r"""SHA-256 of the data lines, ``\\n``-joined."""
    return hashlib.sha256("\n".join(lines).encode()).hexdigest()


def main(argv: Sequence[str]) -> int:
    """Compare ``argv[2:]`` in directories ``argv[0]`` (new) and ``argv[1]`` (committed)."""
    if len(argv) < 3:
        print(__doc__, file=sys.stderr)
        return 2
    new_dir, old_dir = Path(argv[0]), Path(argv[1])
    bad = 0
    for name in argv[2:]:
        new, old = data_lines(new_dir / name), data_lines(old_dir / name)
        diff = first_difference(new, old)
        status = "IDENTICAL" if diff is None else f"DIFFERENT ({diff})"
        print(f"{name}: {len(new) - 1} data rows, data sha256 {digest(new)}: {status}")
        bad += diff is not None
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
