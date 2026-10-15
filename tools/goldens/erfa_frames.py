# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Generate the pyerfa 2.0.1.5 precession and ecliptic goldens (REF-009, NUM-014, TST-003).

One CSV file is written to ``tests/goldens/erfa-frames-pyerfa.csv`` (the default directory). At
113 TT epochs (1550-2650 every 10 years, plus J2000.0 and the DE440 epoch JDEPOC 2440400.5), each
a two-part Julian date "0 h + fraction of day", it holds:

* ``obl06``, all 16 angles of ``p06e`` (including the general precession p_A), and the
  ``pmat06`` and ``ecm06`` matrices (row-major);
* ``eqec06`` of a deterministic ICRS direction and ``eceq06`` of a deterministic ecliptic one;
* the ECLIPJ2000 matrix ``rx(84381.448 arcsec, ir)`` (constant; the JPL/NAIF ecliptic of J2000).

Every float is written with ``repr`` (shortest round-trip form). The header records the
generator, pyerfa/ERFA/SOFA/numpy versions, the git commit of this script and the UTC generation
time. Consumers pin the file SHA-256 in the vector that cites it.

Usage::

    uv run --group goldens python goldens/erfa_frames.py [--out-dir DIR]
"""

from __future__ import annotations

import argparse
import math
import platform
import subprocess
from collections.abc import Iterator, Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import erfa
import numpy as np

ROOT = Path(__file__).resolve().parents[2]
PYERFA_PIN = "2.0.1.5"
FIRST_YEAR, LAST_YEAR, STEP_YEARS = 1550, 2650, 10
GOLDEN_RATIO_FRAC = (math.sqrt(5.0) - 1.0) / 2.0
ECLIPJ2000_OBLIQUITY_ARCSEC = 84381.448  # JPL Horizons manual, "Ecliptic of Standard Epoch"
DE440_JDEPOC = 2440400.5  # header.440 JDEPOC (initial epoch of the NUM-014 run)

P06E_NAMES = (
    "eps0",
    "psia",
    "oma",
    "bpa",
    "bqa",
    "pia",
    "bpia",
    "epsa",
    "chia",
    "za",
    "zetaa",
    "thetaa",
    "pa",
    "gam",
    "phi",
    "psi",
)
MATRIX = tuple(f"{i}{j}" for i in (1, 2, 3) for j in (1, 2, 3))
COLUMNS = [
    "k",
    "tt1",
    "tt2",
    "obl06",
    *P06E_NAMES,
    *(f"pmat06_{e}" for e in MATRIX),
    *(f"ecm06_{e}" for e in MATRIX),
    "eq_ra",
    "eq_dec",
    "eqec06_lon",
    "eqec06_lat",
    "ec_lon",
    "ec_lat",
    "eceq06_ra",
    "eceq06_dec",
    *(f"eclj2000_{e}" for e in MATRIX),
]


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


def scalar(x: Any) -> float:  # noqa: ANN401 - pyerfa returns numpy scalars or 0-d arrays
    """Convert a pyerfa result to a Python float, exactly."""
    return float(np.asarray(x, dtype=np.float64))


def flat(m: Any) -> list[float]:  # noqa: ANN401 - numpy 3x3 array
    """Row-major elements of a 3x3 matrix."""
    return [float(x) for x in np.asarray(m, dtype=np.float64).reshape(9)]


def epochs() -> Iterator[tuple[int, float, float]]:
    """Yield ``(k, tt1, tt2)`` for the golden TT epochs."""
    k = 0
    for year in range(FIRST_YEAR, LAST_YEAR + 1, STEP_YEARS):
        month = 1 + (5 * k) % 12
        day = 1 + (13 * k) % 28
        djm0, djm = erfa.cal2jd(year, month, day)
        yield k, scalar(djm0) + scalar(djm), math.fmod((k + 1) * GOLDEN_RATIO_FRAC, 1.0)
        k += 1
    yield k, 2451545.0, 0.0  # J2000.0
    yield k + 1, DE440_JDEPOC, 0.0  # header.440 JDEPOC


def rows() -> Iterator[list[str]]:
    """Compute one golden row per epoch."""
    eclj2000 = flat(erfa.rx(ECLIPJ2000_OBLIQUITY_ARCSEC * erfa.DAS2R, erfa.ir()))
    for k, tt1, tt2 in epochs():
        p = [scalar(x) for x in erfa.p06e(tt1, tt2)]
        # Deterministic directions: right ascension / longitude sweeps the circle, declination /
        # latitude sweeps -85..85 deg.
        ra = math.fmod(k * 2.399963229728653, 2.0 * math.pi)
        dec = math.radians(-85.0 + (170.0 * k) / 112.0)
        lon, lat = (scalar(x) for x in erfa.eqec06(tt1, tt2, ra, dec))
        elon = math.fmod(1.0 + k * 1.1, 2.0 * math.pi)
        elat = math.radians(85.0 - (170.0 * k) / 112.0)
        bra, bdec = (scalar(x) for x in erfa.eceq06(tt1, tt2, elon, elat))
        values: list[object] = [k, tt1, tt2, scalar(erfa.obl06(tt1, tt2)), *p]
        values += flat(erfa.pmat06(tt1, tt2)) + flat(erfa.ecm06(tt1, tt2))
        values += [ra, dec, lon, lat, elon, elat, bra, bdec, *eclj2000]
        yield [repr(x) if isinstance(x, float) else str(x) for x in values]


def header(title: str, units: str, columns: Sequence[str], n: int) -> str:
    """Provenance header lines (``#``-prefixed) and the column line."""
    if erfa.__version__ != PYERFA_PIN:
        msg = f"pyerfa {erfa.__version__} != pinned {PYERFA_PIN} (REF-009)"
        raise SystemExit(msg)
    # REUSE-IgnoreStart (header text written into the CSV, not this file's licence)
    lines = [
        "SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "SPDX-License-Identifier: Apache-2.0",
        # REUSE-IgnoreEnd
        title,
        f"generated_by: tools/goldens/erfa_frames.py@{git_head()}",
        f"engine: pyerfa {erfa.__version__} (ERFA {erfa.version.erfa_version}, "
        f"SOFA {erfa.version.sofa_version}), numpy {np.__version__}, "
        f"Python {platform.python_version()} on {platform.machine()}",
        f"generated_utc: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"rows: {n}",
        f"units: {units}",
    ]
    return "".join(f"# {line}\n" for line in lines) + ",".join(columns) + "\n"


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out-dir", type=Path, default=ROOT / "tests" / "goldens")
    args = parser.parse_args(argv)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    body = list(rows())
    path = args.out_dir / "erfa-frames-pyerfa.csv"
    text = header(
        "pyerfa IAU 2006 precession/ecliptic goldens: obl06, p06e, pmat06, ecm06, eqec06, "
        "eceq06 and the ECLIPJ2000 matrix, 1550-2650 (REF-009)",
        "tt1, tt2: two-part TT Julian date in days; angles in radians; matrices dimensionless",
        COLUMNS,
        len(body),
    )
    path.write_text(text + "".join(",".join(r) + "\n" for r in body), encoding="utf-8")
    print(f"wrote {path} ({len(body)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
