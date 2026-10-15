# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Generate the pyerfa 2.0.1.5 time-scale goldens (REF-009, DEP-010, TST-003).

Two CSV files are written to ``tests/goldens/`` (the defaults):

``erfa-time-pyerfa.csv``
    At 223 TT epochs spanning 1550-2650 (every 5 years plus J2000.0 and the 1977 TCB/TCG
    origin), each as a two-part Julian date "0 h + fraction of day": TAI, TDB, TCB and TCG from
    pyerfa (``tttai``, ``dtdb`` with u = v = 0 then ``tttdb``, ``tdbtcb``, ``tttcg``), the
    inverse of each (``taitt``, ``tdbtt`` with ``dtdb`` evaluated at the TDB date, ``tcbtdb``,
    ``tcgtt``), a topocentric ``dtdb`` at a deterministic site, and ``jd2cal`` of the TT date.
``erfa-dat-pyerfa.csv``
    TAI - UTC from ``erfa.dat`` at 0 h of every January 1 and July 1, 1972-2026, and one second
    before each of those instants (``fd = 86399/86400`` of the previous day).

Every float is written with ``repr`` (shortest round-trip form), so Rust parses the exact
binary64 value. The header records the generator, pyerfa/ERFA/SOFA/numpy versions, the git
commit of this script and the UTC generation time. Consumers record the file SHA-256 in the
vector that cites it.

Usage::

    uv run --group goldens python goldens/erfa_time.py [--out-dir DIR]

Only the ``goldens`` dependency group installs pyerfa (``pyerfa==2.0.1.5``).
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
GOLDEN_RATIO_FRAC = (math.sqrt(5.0) - 1.0) / 2.0
FIRST_YEAR, LAST_YEAR, STEP_YEARS = 1550, 2650, 5
EARTH_EQ_RADIUS_KM = 6378.137  # GRS80/WGS84 equatorial radius (site geometry only)
EARTH_POLAR_RADIUS_KM = 6356.752  # GRS80 polar radius (site geometry only)

TIME_COLUMNS = [
    "k",
    "iy",
    "im",
    "id",
    "tt1",
    "tt2",
    "tai1",
    "tai2",
    "dtdb_geo",
    "tdb1",
    "tdb2",
    "tcb1",
    "tcb2",
    "tcg1",
    "tcg2",
    "tt_from_tai1",
    "tt_from_tai2",
    "tt_from_tdb1",
    "tt_from_tdb2",
    "tdb_from_tcb1",
    "tdb_from_tcb2",
    "tt_from_tcg1",
    "tt_from_tcg2",
    "ut",
    "elong",
    "u",
    "v",
    "dtdb_topo",
    "cal_iy",
    "cal_im",
    "cal_id",
    "cal_fd",
]
DAT_COLUMNS = ("iy", "im", "id", "fd", "dat")


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


def epochs() -> Iterator[tuple[int, int, int, int, float]]:
    """Yield ``(k, year, month, day, fraction_of_day)`` for the golden TT epochs."""
    k = 0
    for year in range(FIRST_YEAR, LAST_YEAR + 1, STEP_YEARS):
        month = 1 + (7 * k) % 12
        day = 1 + (11 * k) % 28
        frac = math.fmod((k + 1) * GOLDEN_RATIO_FRAC, 1.0)
        yield k, year, month, day, frac
        k += 1
    # J2000.0 (2000 January 1, 12h TT) and the TCB/TCG origin 1977 January 1, 00:00:32.184 TT.
    yield k, 2000, 1, 1, 0.5
    yield k + 1, 1977, 1, 1, 32.184 / 86400.0


def time_rows() -> Iterator[list[str]]:
    """Compute one golden row per epoch."""
    for k, iy, im, iday, frac in epochs():
        djm0, djm = erfa.cal2jd(iy, im, iday)
        tt1, tt2 = scalar(djm0) + scalar(djm), frac
        tai1, tai2 = (scalar(x) for x in erfa.tttai(tt1, tt2))
        dtr = scalar(erfa.dtdb(tt1, tt2, 0.0, 0.0, 0.0, 0.0))
        tdb1, tdb2 = (scalar(x) for x in erfa.tttdb(tt1, tt2, dtr))
        tcb1, tcb2 = (scalar(x) for x in erfa.tdbtcb(tdb1, tdb2))
        tcg1, tcg2 = (scalar(x) for x in erfa.tttcg(tt1, tt2))
        bt1, bt2 = (scalar(x) for x in erfa.taitt(tai1, tai2))
        dtr_b = scalar(erfa.dtdb(tdb1, tdb2, 0.0, 0.0, 0.0, 0.0))
        bd1, bd2 = (scalar(x) for x in erfa.tdbtt(tdb1, tdb2, dtr_b))
        bc1, bc2 = (scalar(x) for x in erfa.tcbtdb(tcb1, tcb2))
        bg1, bg2 = (scalar(x) for x in erfa.tcgtt(tcg1, tcg2))
        # Deterministic topocentric site: latitude sweeps -80..80 deg, longitude the circle.
        lat = math.radians(-80.0 + (160.0 * k) / 222.0)
        elong = math.fmod(k * 2.399963229728653, 2.0 * math.pi) - math.pi
        u = EARTH_EQ_RADIUS_KM * math.cos(lat)
        v = EARTH_POLAR_RADIUS_KM * math.sin(lat)
        ut = frac
        topo = scalar(erfa.dtdb(tt1, tt2, ut, elong, u, v))
        cy, cm, cd, cfd = erfa.jd2cal(tt1, tt2)
        values: list[object] = [k, iy, im, iday, tt1, tt2, tai1, tai2, dtr, tdb1, tdb2]
        values += [tcb1, tcb2, tcg1, tcg2, bt1, bt2, bd1, bd2, bc1, bc2, bg1, bg2]
        values += [ut, elong, u, v, topo, int(cy), int(cm), int(cd), scalar(cfd)]
        yield [repr(x) if isinstance(x, float) else str(x) for x in values]


def dat_rows() -> Iterator[list[str]]:
    """TAI-UTC at each Jan 1 / Jul 1 0h and one second before it, 1972-2026."""
    last_second = 86399.0 / 86400.0
    for iy in range(1972, 2027):
        for im in (1, 7):
            djm0, djm = erfa.cal2jd(iy, im, 1)
            py, pm, pd, _ = erfa.jd2cal(scalar(djm0), scalar(djm) - 1.0)
            for y, m, d, fd in ((int(py), int(pm), int(pd), last_second), (iy, im, 1, 0.0)):
                if (y, m) < (1972, 1):
                    continue  # before 1972 UTC had fractional offsets and rate changes
                yield [str(y), str(m), str(d), repr(fd), repr(scalar(erfa.dat(y, m, d, fd)))]


def header(title: str, units: str, columns: Sequence[str], rows: int) -> str:
    """Provenance header lines (``#``-prefixed) and the column line."""
    if erfa.__version__ != PYERFA_PIN:
        msg = f"pyerfa {erfa.__version__} != pinned {PYERFA_PIN} (REF-009)"
        raise SystemExit(msg)
    lines = [
        "SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "SPDX-License-Identifier: Apache-2.0",
        title,
        f"generated_by: tools/goldens/erfa_time.py@{git_head()}",
        f"engine: pyerfa {erfa.__version__} (ERFA {erfa.version.erfa_version}, "
        f"SOFA {erfa.version.sofa_version}), numpy {np.__version__}, "
        f"Python {platform.python_version()} on {platform.machine()}",
        f"generated_utc: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"rows: {rows}",
        f"units: {units}",
    ]
    return "".join(f"# {line}\n" for line in lines) + ",".join(columns) + "\n"


def write(
    path: Path, title: str, units: str, columns: Sequence[str], rows: list[list[str]]
) -> None:
    """Write one CSV file."""
    body = "".join(",".join(r) + "\n" for r in rows)
    path.write_text(header(title, units, columns, len(rows)) + body, encoding="utf-8")
    print(f"wrote {path} ({len(rows)} rows)")


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out-dir", type=Path, default=ROOT / "tests" / "goldens")
    args = parser.parse_args(argv)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    write(
        args.out_dir / "erfa-time-pyerfa.csv",
        "pyerfa time-scale goldens: TT -> TAI/TDB/TCB/TCG and back, 1550-2650 (REF-009)",
        "two-part Julian dates in days; dtdb in seconds; ut fraction of day; elong rad; u, v km",
        TIME_COLUMNS,
        list(time_rows()),
    )
    write(
        args.out_dir / "erfa-dat-pyerfa.csv",
        "pyerfa TAI-UTC (erfa.dat) at the 1972-2026 half-year boundaries (DEP-010)",
        "UTC calendar date; fd fraction of the UTC day; dat = TAI-UTC in seconds",
        DAT_COLUMNS,
        list(dat_rows()),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
