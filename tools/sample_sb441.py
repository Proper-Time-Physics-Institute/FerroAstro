# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Sample the asteroid/KBO SPK sb441-n373s.bsp at JDEPOC (REF-003, provisional while OI-24 is open).

Writes ``tests/goldens/sb441-n373s-jdepoc-states.csv``: the Sun-centred ICRF state of every
target of the pinned ``sb441-n373s.bsp`` at the header.440 epoch ``JDEPOC`` (TDB), evaluated with
jplephem 2.24 (the REF-004 reader). Both inputs are checked against their SHA-256 pins in
``tests/data/MANIFEST.toml`` first; a mismatch stops the script.

Columns: ``naif`` (NAIF id), ``symbol`` (the header.440 GM symbol ``MAnnnn`` for numbered
asteroid ``nnnn`` = ``naif - 2000000``; empty for the 30 trans-Neptunian targets, whose header
symbols ``MA8001...MA8030`` are not named by JPL, OI-24), then ``x, y, z`` (km) and
``vx, vy, vz`` (km/s), written with ``repr`` (shortest round-trip form). Where several segments of
a target cover JDEPOC, the last one in file order is used (NAIF precedence, as
``ferroastro-ephem``).

Usage::

    uv run python sample_sb441.py [--data-dir $FA_DATA_DIR] [--out PATH]
"""

from __future__ import annotations

import argparse
import platform
import subprocess
import tomllib
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import jplephem
import numpy as np
from jplephem.spk import SPK

import fetch_jpl

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "tests" / "goldens" / "sb441-n373s-jdepoc-states.csv"
SPK_PRODUCT = "sb441-n373s-bsp"
HEADER_PRODUCT = "de440-header"
JPLEPHEM_PIN = "2.24"
S_PER_DAY = 86400.0
ASTEROID_NAIF_BASE = 2_000_000
MA_UNIDENTIFIED_FROM = 8000
SUN = 10
COLUMNS = ["naif", "symbol", "x", "y", "z", "vx", "vy", "vz"]


class SampleError(Exception):
    """A pin mismatch or an input the script cannot interpret."""


def git_head() -> str:
    """Return ``<sha>`` of HEAD, with ``-dirty`` if this script has uncommitted changes."""
    sha = subprocess.run(  # noqa: S603
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],  # noqa: S607
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    status = subprocess.run(  # noqa: S603
        ["git", "-C", str(ROOT), "status", "--porcelain", "--", str(Path(__file__))],  # noqa: S607
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return f"{sha}-dirty" if status else sha


def header_constants(text: str) -> dict[str, float]:
    """GROUP 1040 names and GROUP 1041 values of a header.440 text."""
    try:
        names = text.split("GROUP   1040")[1].split("GROUP   1041", maxsplit=1)[0].split()[1:]
        values = text.split("GROUP   1041")[1].split("GROUP", maxsplit=1)[0].split()[1:]
    except IndexError as e:
        msg = "header.440 lacks GROUP 1040/1041"
        raise SampleError(msg) from e
    if len(names) != len(values):
        msg = f"header.440: {len(names)} names but {len(values)} values"
        raise SampleError(msg)
    return {n: float(v.replace("D", "E")) for n, v in zip(names, values, strict=True)}


def asteroid_symbols(constants: dict[str, float]) -> dict[int, str]:
    """NAIF id -> ``MAnnnn`` for the numbered asteroids of the header (nnnn below 8000)."""
    out: dict[int, str] = {}
    for name in constants:
        if name.startswith("MA") and int(name[2:]) < MA_UNIDENTIFIED_FROM:
            out[ASTEROID_NAIF_BASE + int(name[2:])] = name
    return out


def covering_segment(kernel: Any, target: int, jd: float) -> Any:  # noqa: ANN401 - jplephem
    """Return the last segment of ``target`` (Sun-centred) in file order covering ``jd``."""
    found = None
    for seg in kernel.segments:
        if seg.target == target and seg.start_jd <= jd <= seg.end_jd:
            if seg.center != SUN or seg.data_type != 2 or seg.frame != 1:
                msg = (
                    f"target {target}: centre {seg.center}, type {seg.data_type}, frame {seg.frame}"
                )
                raise SampleError(msg)
            found = seg
    if found is None:
        msg = f"no segment of target {target} covers JD {jd}"
        raise SampleError(msg)
    return found


def sample(bsp: Path, jd: float, symbols: dict[int, str]) -> list[list[str]]:
    """One row per target: Sun-centred state at ``jd`` TDB (km, km/s)."""
    rows: list[list[str]] = []
    with SPK.open(str(bsp)) as kernel:
        targets = sorted({int(s.target) for s in kernel.segments})
        for t in targets:
            seg = covering_segment(kernel, t, jd)
            pos, rate = seg.compute_and_differentiate(jd, 0.0)
            p = [float(x) for x in np.asarray(pos, dtype=np.float64)]
            v = [float(x) / S_PER_DAY for x in np.asarray(rate, dtype=np.float64)]
            rows.append([str(t), symbols.get(t, ""), *map(repr, p + v)])
    return rows


def provenance(directory: Path, product: fetch_jpl.Product) -> dict[str, Any]:
    """Read the cached product's provenance sidecar (URL, SHA-256, retrieval date)."""
    side = directory / f"{product.file}.provenance.toml"
    if not side.exists():
        msg = f"{side} missing: run `uv run python fetch_jpl.py --products {product.name}`"
        raise SampleError(msg)
    with side.open("rb") as f:
        return tomllib.load(f)


def verified(directory: Path, product: fetch_jpl.Product) -> Path:
    """Return the cached file after checking its SHA-256 against the MANIFEST pin."""
    path = directory / product.file
    if not path.exists():
        msg = f"{path} missing: run `uv run python fetch_jpl.py --products {product.name}`"
        raise SampleError(msg)
    sha = fetch_jpl.sha256_file(path)
    if sha != product.sha256:
        msg = f"{path}: SHA-256 {sha} != pinned {product.sha256} (FA-E0204)"
        raise SampleError(msg)
    return path


def render(  # noqa: PLR0913, PLR0917 - every input is provenance written into the file
    rows: Sequence[Sequence[str]],
    spk: fetch_jpl.Product,
    spk_prov: dict[str, Any],
    header: fetch_jpl.Product,
    jd: float,
    n_asteroids: int,
) -> str:
    """Render the CSV text: provenance header lines, then the column line and the rows."""
    if jplephem.__version__ != JPLEPHEM_PIN:
        msg = f"jplephem {jplephem.__version__} != pinned {JPLEPHEM_PIN} (REF-004)"
        raise SampleError(msg)
    # REUSE-IgnoreStart (header text written into the CSV, not this file's licence)
    lines = [
        "SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "SPDX-License-Identifier: Apache-2.0",
        # REUSE-IgnoreEnd
        "REF-003 sampled initial states: every target of sb441-n373s.bsp at header.440 JDEPOC "
        "(provisional, OI-24)",
        f"generated_by: tools/sample_sb441.py@{git_head()}",
        f"engine: jplephem {jplephem.__version__}, numpy {np.__version__}, "
        f"Python {platform.python_version()} on {platform.machine()}",
        f"generated_utc: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"source_file: {spk.file}",
        f"source_url: {spk.url}",
        f"source_sha256: {spk.sha256}",
        f"source_retrieved: {spk_prov.get('retrieved', 'unknown')}",
        f"header_file: {header.file} sha256 {header.sha256}",
        f"jdepoc_tdb: {jd!r}",
        "centre: Sun (NAIF 10); frame: ICRF (SPK frame 1, J2000)",
        "units: x, y, z km; vx, vy, vz km/s (jplephem km/day divided by 86400)",
        f"rows: {len(rows)} ({n_asteroids} header-numbered asteroids, "
        f"{len(rows) - n_asteroids} other targets)",
    ]
    text = "".join(f"# {line}\n" for line in lines) + ",".join(COLUMNS) + "\n"
    return text + "".join(",".join(r) + "\n" for r in rows)


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    ap.add_argument("--data-dir", type=Path, default=None, help="JPL data cache (FA_DATA_DIR)")
    ap.add_argument("--out", type=Path, default=OUT)
    args = ap.parse_args(argv)
    directory: Path = args.data_dir or fetch_jpl.data_dir()
    products = fetch_jpl.load_manifest()
    spk, hdr = products[SPK_PRODUCT], products[HEADER_PRODUCT]
    try:
        bsp = verified(directory, spk)
        constants = header_constants(verified(directory, hdr).read_text(encoding="ascii"))
        jd = constants["JDEPOC"]
        symbols = asteroid_symbols(constants)
        rows = sample(bsp, jd, symbols)
        missing = sorted(set(symbols) - {int(r[0]) for r in rows})
        if missing:
            msg = f"header asteroids without a state in {spk.file}: {missing}"
            raise SampleError(msg)
        text = render(rows, spk, provenance(directory, spk), hdr, jd, len(symbols))
    except SampleError as e:
        print(f"error: {e}")
        return 1
    args.out.write_text(text, encoding="utf-8")
    print(f"wrote {args.out} ({len(rows)} rows, sha256 {fetch_jpl.sha256_file(args.out)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
