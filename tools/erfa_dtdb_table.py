# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Extract the Fairhead & Bretagnon (1990) coefficient table from ERFA's ``dtdb.c`` (DEP-009).

``ferroastro-erfa`` ports ``eraDtdb``. Its 787 x 3 coefficient table is copied mechanically,
literal text unchanged, so that a reviewer can regenerate the Rust file from the ERFA release
and diff it. Never run this on a SOFA file (LIC-004): the input must be ERFA's ``src/dtdb.c``.

Usage::

    uv run python erfa_dtdb_table.py <erfa>/src/dtdb.c > ../crates/ferroastro-erfa/src/dtdb_table.rs

The output starts with the SPDX header, the ERFA copyright and the SHA-256 of the input file.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path

EXPECTED_ROWS = 787
ROW = re.compile(r"\{\s*([-+0-9.eE]+)\s*,\s*([-+0-9.eE]+)\s*,\s*([-+0-9.eE]+)\s*\}")

# REUSE-IgnoreStart
HEADER = """\
// SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation
// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//! Fairhead & Bretagnon (1990) TDB-TT series: 787 sets of (amplitude s, frequency
//! rad per Julian millennium, phase rad), copied with their literal text from ERFA 2.0.1
//! `src/dtdb.c` (`fairhd`) by `tools/erfa_dtdb_table.py`. Do not edit by hand.
//!
//! Input `dtdb.c` SHA-256: {sha}.
//!
//! Rows 0-473 multiply T^0, 474-678 T^1, 679-763 T^2, 764-783 T^3 and 784-786 T^4, where T is
//! TDB in Julian millennia since J2000.0. The ERFA licence is at the end of this file.

/// The ERFA `fairhd` table, `[amplitude (s), frequency (rad / Julian millennium), phase (rad)]`.
#[allow(clippy::unreadable_literal, clippy::excessive_precision)]
pub(crate) static FAIRHD: [[f64; 3]; {n}] = [
"""
# REUSE-IgnoreEnd


def extract(text: str) -> list[tuple[str, str, str]]:
    """Return the coefficient rows of ``fairhd`` as literal strings, in source order."""
    start = text.index("fairhd[787][3]")
    end = text.index("};", start)
    rows = [(a, b, c) for a, b, c in ROW.findall(text[start:end])]
    if len(rows) != EXPECTED_ROWS:
        msg = f"expected {EXPECTED_ROWS} rows in fairhd, found {len(rows)}"
        raise ValueError(msg)
    return rows


def rust_float(literal: str) -> str:
    """Make a C double literal a valid Rust f64 literal (``.5`` -> ``0.5``, ``2`` -> ``2.0``)."""
    lit = literal.lstrip("+")
    sign = ""
    if lit.startswith("-"):
        sign, lit = "-", lit[1:]
    if lit.startswith("."):
        lit = "0" + lit
    mantissa = re.split("[eE]", lit, maxsplit=1)[0]
    if "." not in mantissa:
        # Rust accepts 1e-6 as f64, but keep an explicit fraction for uniformity.
        head, _, tail = lit.partition("e")
        lit = f"{head}.0" + (f"e{tail}" if tail else "")
    return sign + lit


def licence_block(text: str) -> str:
    """Return the ERFA licence comment at the end of the file as ``//`` comment lines.

    BSD-3-Clause condition 1 asks source redistributions to retain the notice, the conditions
    and the disclaimer, so every ported file carries them verbatim (red-team L8).
    """
    marker = "**  Copyright (C) 2013-2023, NumFOCUS Foundation."
    start = text.index(marker, text.index(marker) + 1)
    end = text.index("POSSIBILITY OF SUCH DAMAGE.", start) + len("POSSIBILITY OF SUCH DAMAGE.")
    lines = []
    for line in text[start:end].splitlines():
        body = line.removeprefix("**").removeprefix("  ").rstrip()
        lines.append(f"// {body}" if body else "//")
    return "\n".join(lines) + "\n"


def render(rows: list[tuple[str, str, str]], sha: str, licence: str) -> str:
    """Render the Rust module text."""
    out = [HEADER.replace("{sha}", sha).replace("{n}", str(len(rows)))]
    for a, b, c in rows:
        out.append(f"    [{rust_float(a)}, {rust_float(b)}, {rust_float(c)}],\n")
    out.append("];\n\n// ERFA licence, retained verbatim (BSD-3-Clause condition 1):\n//\n")
    out.append(licence)
    return "".join(out)


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("dtdb_c", type=Path, help="ERFA src/dtdb.c")
    args = parser.parse_args(argv)
    raw = args.dtdb_c.read_bytes()
    sha = hashlib.sha256(raw).hexdigest()
    text = raw.decode("utf-8")
    sys.stdout.write(render(extract(text), sha, licence_block(text)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
