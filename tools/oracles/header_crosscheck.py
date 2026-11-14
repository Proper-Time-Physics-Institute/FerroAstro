# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Live cross-check of FerroAstro's header.440 constants against independent readers (#22).

The repository commits no header.440 value (maintainer decision 2026-11-14). Instead the values
FerroAstro parses at run time are compared, bit for bit, with

* (a) ``tools/oracles/header440.py`` on the same file (data tier, ci.yml ``data`` job), and
* (b) AUTHORITATIVELY, Orekit 13.1.9, which reads the same constants from the binary DE440 file
  bundled in the pinned orekit-data snapshot (oracle tier, ``tools/oracles/run_oracles.sh
  header-constants``; oracles.yml).

Each input is a JSON document ``{"constants": {NAME: {"repr": ..., "bits": "<16 hex>"}}}``:
FerroAstro's from ``cargo run -p ferroastro-constants --example header_constants_json``, the
oracles' from :mod:`oracles.header440` and :mod:`oracles.orekit_constants`. The allowed distance
is the vector ``SYSTEM-HEADER-LIVE-CROSSCHECK-MAX-ULP`` (tests/vectors/system.toml). Only names,
distances and verdicts are printed, so the log reproduces no value either.

Usage::

    uv run python -m oracles.header_crosscheck FERROASTRO.json ORACLE.json [ORACLE.json ...]

Exit status 0 when every oracle agrees on every FerroAstro constant, 1 otherwise, 2 on bad input.
"""

from __future__ import annotations

import json
import sys
import tomllib
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
VECTORS = ROOT / "tests" / "vectors" / "system.toml"
VECTOR_ID = "SYSTEM-HEADER-LIVE-CROSSCHECK-MAX-ULP"
SIGN = 1 << 63
HEX_DIGITS = 16


def allowed_ulps(path: Path = VECTORS, vector_id: str = VECTOR_ID) -> float:
    """``value + tolerance.abs`` of the cross-check vector (ulps)."""
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    for v in doc.get("vector", []):
        if v.get("id") == vector_id:
            if v.get("unit") != "ulp":
                msg = f"{vector_id}: unit {v.get('unit')!r}, expected 'ulp'"
                raise ValueError(msg)
            return float(v["value"]) + float(v["tolerance"]["abs"])
    msg = f"{path}: no vector {vector_id}"
    raise KeyError(msg)


def ordered(bits: int) -> int:
    """Map binary64 bits to an integer whose order and differences count ulps."""
    magnitude = bits & (SIGN - 1)
    return -magnitude if bits & SIGN else magnitude


def ulp_distance(a_hex: str, b_hex: str) -> int:
    """Distance in ulps between two binary64 values given as 16-hex-digit bit patterns."""
    if len(a_hex) != HEX_DIGITS or len(b_hex) != HEX_DIGITS:
        msg = f"bit patterns must have {HEX_DIGITS} hex digits: {a_hex!r}, {b_hex!r}"
        raise ValueError(msg)
    return abs(ordered(int(a_hex, 16)) - ordered(int(b_hex, 16)))


def constants(doc: Mapping[str, Any]) -> dict[str, str]:
    """NAME -> bits of a cross-check document."""
    return {str(k): str(v["bits"]) for k, v in doc["constants"].items()}


def compare(
    ours: Mapping[str, str], oracle: Mapping[str, str], allowed: float
) -> list[tuple[str, int | None, bool]]:
    """``(name, ulps or None if the oracle lacks it, ok)`` for every FerroAstro constant."""
    rows: list[tuple[str, int | None, bool]] = []
    for name, bits in ours.items():
        if name not in oracle:
            rows.append((name, None, False))
            continue
        d = ulp_distance(bits, oracle[name])
        rows.append((name, d, d <= allowed))
    return rows


def main(argv: Sequence[str]) -> int:
    """Compare ``argv[0]`` (FerroAstro) with each of ``argv[1:]`` (oracles)."""
    if len(argv) < 2:  # FerroAstro plus at least one oracle
        print(__doc__, file=sys.stderr)
        return 2
    try:
        allowed = allowed_ulps()
        docs = [json.loads(Path(p).read_text(encoding="utf-8")) for p in argv]
        ours = constants(docs[0])
    except (OSError, KeyError, ValueError, TypeError) as e:
        print(f"header_crosscheck: bad input: {e}", file=sys.stderr)
        return 2
    if not ours:
        print("header_crosscheck: the FerroAstro document lists no constant", file=sys.stderr)
        return 2
    bad = 0
    for path, doc in zip(argv[1:], docs[1:], strict=True):
        reader = doc.get("reader", path)
        rows = compare(ours, constants(doc), allowed)
        failed = [r for r in rows if not r[2]]
        print(f"## {reader}: {len(rows) - len(failed)}/{len(rows)} within {allowed:g} ulp")
        for name, d, ok in rows:
            shown = "missing" if d is None else f"{d} ulp"
            print(f"   {name:8} {shown:>10}  {'OK' if ok else 'MISMATCH'}")
        bad += bool(failed)
    print(f"{VECTOR_ID}: {'PASS' if not bad else 'FAIL'}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
