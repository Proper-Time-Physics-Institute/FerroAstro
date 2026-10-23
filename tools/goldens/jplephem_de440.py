# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Generate the jplephem 2.24 SPK goldens (REF-004, DEP-011, TST-003).

Three CSV files are written to ``tests/goldens/`` (the default directory):

``jplephem-de440.csv``
    Every segment of ``de440.bsp`` (14 type-2 segments), raw (target relative to the segment's own
    centre), at its first and last epoch, at 4 record boundaries and at 50 quasi-random epochs
    spread over the coverage (golden-ratio sequence). Each epoch is a two-part TDB Julian date
    ``jd1 + jd2`` with ``jd1`` a half-integer (0 h TDB) and ``0 <= jd2 < 1``, evaluated with
    ``Segment.compute_and_differentiate(jd1, jd2)``.
``jplephem-synthetic.csv``
    A synthetic SPK (no JPL data) with one type-2 and one type-3 segment whose coefficients are
    exact dyadic numbers from :func:`coefficient`, written in both byte orders (``LTL-IEEE`` and
    ``BIG-IEEE``) by :func:`synthetic_spk` and evaluated by jplephem. The header records the
    SHA-256 of both files; ``crates/ferroastro-ephem/tests/common`` rebuilds the same bytes and
    must reproduce those digests (the files themselves are never committed, LIC-003).
``jplephem-sb441.csv`` (only with ``--sb441``)
    Ceres, Pallas and Vesta (2000001, 2000002, 2000004) from ``sb441-n373s.bsp`` at 50 epochs.

Positions are in km and velocities in km/s (jplephem returns km/day for type 2; the value is
divided by 86400). Floats are written with ``repr`` (shortest round-trip form).

Usage::

    uv run python goldens/jplephem_de440.py --bsp $FA_DATA_DIR/de440.bsp
        [--sb441 $FA_DATA_DIR/sb441-n373s.bsp] [--out-dir DIR]  # one line
"""

from __future__ import annotations

import argparse
import hashlib
import math
import platform
import struct
import subprocess
import tempfile
from collections.abc import Iterator, Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

import jplephem
import numpy as np
from jplephem.spk import SPK

ROOT = Path(__file__).resolve().parents[2]
JPLEPHEM_PIN = "2.24"
S_PER_DAY = 86400.0
J2000 = 2451545.0
GOLDEN_RATIO_FRAC = (math.sqrt(5.0) - 1.0) / 2.0
N_RANDOM = 50
FTPSTR = b"FTPSTR:\r:\n:\r\n:\r\x00:\x81:\x10\xce:ENDFTP"
SB441_BODIES = (2000001, 2000002, 2000004)
COLUMNS = ["seg", "target", "center", "kind", "jd1", "jd2", "x", "y", "z", "vx", "vy", "vz"]

# Synthetic file layout (mirrored in crates/ferroastro-ephem/tests/common/mod.rs):
# (target, center, type, ncoef, n_records, init_days_from_j2000, intlen_days)
SYNTH_SEGMENTS = ((1001, 0, 2, 5, 4, -8, 4), (1002, 1001, 3, 4, 3, -6, 4))
SYNTH_IFN = b"FERROASTRO SYNTHETIC SPK (WP10)"


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


def sha256_file(path: Path) -> str:
    """Lowercase hex SHA-256 of a file."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def evaluate(seg: Any, jd1: float, jd2: float) -> list[float]:  # noqa: ANN401 - jplephem Segment
    """Position (km) and velocity (km/s) of a type-2/3 segment at ``jd1 + jd2`` TDB."""
    pos, rate = seg.compute_and_differentiate(jd1, jd2)
    p = [float(x) for x in np.asarray(pos, dtype=np.float64)]
    if seg.data_type == 3:
        return p[:3] + p[3:6]
    return p + [float(v) / S_PER_DAY for v in np.asarray(rate, dtype=np.float64)]


def segment_epochs(seg: Any) -> Iterator[tuple[str, float, float]]:  # noqa: ANN401
    """Yield ``(kind, jd1, jd2)``: both ends, 4 record boundaries, then quasi-random epochs."""
    init, intlen, _rsize, n = (float(x) for x in seg.daf.read_array(seg.end_i - 3, seg.end_i))
    start, end = float(seg.start_jd), float(seg.end_jd)
    yield "start", start, 0.0
    yield "end", end, 0.0
    for k in (1, int(n) // 3, (2 * int(n)) // 3, int(n) - 1):
        et = init + k * intlen
        days = et / S_PER_DAY
        whole = math.floor(days + 0.5) - 0.5  # half-integer day offset from J2000 (0 h TDB)
        jd1 = J2000 + whole
        yield "boundary", jd1, days - whole
    span = end - start
    for i in range(N_RANDOM):
        u = math.fmod((i + 1) * GOLDEN_RATIO_FRAC, 1.0)
        whole = math.floor(u * (span - 1.0))
        frac = math.fmod((i + 1) * 0.7548776662466927, 1.0)  # plastic-number sequence
        yield "random", start + whole, frac


def de440_rows(bsp: Path) -> list[list[str]]:
    """Golden rows for every de440 segment."""
    out: list[list[str]] = []
    with SPK.open(str(bsp)) as kernel:
        for i, seg in enumerate(kernel.segments):
            for kind, jd1, jd2 in segment_epochs(seg):
                vals = evaluate(seg, jd1, jd2)
                out.append(
                    [str(i), str(seg.target), str(seg.center), kind, *map(repr, (jd1, jd2, *vals))]
                )
    return out


def sb441_rows(bsp: Path) -> list[list[str]]:
    """Golden rows for three asteroids of sb441-n373s.bsp at 50 epochs each."""
    out: list[list[str]] = []
    with SPK.open(str(bsp)) as kernel:
        for i, seg in enumerate(kernel.segments):
            if seg.target not in SB441_BODIES:
                continue
            for kind, jd1, jd2 in segment_epochs(seg):
                if kind == "boundary":
                    continue
                vals = evaluate(seg, jd1, jd2)
                out.append(
                    [str(i), str(seg.target), str(seg.center), kind, *map(repr, (jd1, jd2, *vals))]
                )
    return out


def coefficient(seg: int, rec: int, comp: int, k: int) -> float:
    """Synthetic coefficient: a small integer times 1024 / 2**k (exact in binary64)."""
    m = (seg * 1000 + rec * 100 + comp * 10 + k * 7) % 17 - 8
    return m * 1024.0 / 2.0**k


def synthetic_spk(endian: str) -> bytes:
    """Build the synthetic SPK in byte order ``"<"`` (LTL-IEEE) or ``">"`` (BIG-IEEE)."""
    data: list[float] = []
    summaries: list[bytes] = []
    names: list[bytes] = []
    address = 3 * 128 + 1  # first word of record 4
    for si, (target, center, dtype, ncoef, n, init_d, intlen_d) in enumerate(SYNTH_SEGMENTS):
        ncomp = 3 if dtype == 2 else 6
        init, intlen = init_d * S_PER_DAY, intlen_d * S_PER_DAY
        rsize = 2 + ncomp * ncoef
        words: list[float] = []
        for r in range(n):
            words += [init + (r + 0.5) * intlen, intlen / 2.0]
            words += [coefficient(si, r, c, k) for c in range(ncomp) for k in range(ncoef)]
        words += [init, intlen, float(rsize), float(n)]
        first, last = address, address + len(words) - 1
        summaries.append(
            struct.pack(
                endian + "2d6i", init, init + n * intlen, target, center, 1, dtype, first, last
            )
        )
        names.append(f"SYNTHETIC TYPE {dtype} SEGMENT {si}".encode().ljust(40, b" "))
        data += words
        address = last + 1
    free = address
    rec1 = bytearray(1024)
    struct.pack_into(
        endian + "8sii60siii8s",
        rec1,
        0,
        b"DAF/SPK ",
        2,
        6,
        SYNTH_IFN.ljust(60, b" "),
        2,
        2,
        free,
        b"LTL-IEEE" if endian == "<" else b"BIG-IEEE",
    )
    rec1[699 : 699 + len(FTPSTR)] = FTPSTR
    rec2 = bytearray(1024)
    struct.pack_into(endian + "3d", rec2, 0, 0.0, 0.0, float(len(summaries)))
    for k, s in enumerate(summaries):
        rec2[24 + 40 * k : 24 + 40 * (k + 1)] = s
    rec3 = bytearray(1024)
    for k, nm in enumerate(names):
        rec3[40 * k : 40 * (k + 1)] = nm
    body = struct.pack(endian + f"{len(data)}d", *data)
    body += bytes(-len(body) % 1024)
    return bytes(rec1 + rec2 + rec3) + body


def synthetic_rows(tmp: Path) -> tuple[list[list[str]], dict[str, str]]:
    """Evaluate both byte orders of the synthetic file with jplephem."""
    rows: list[list[str]] = []
    digests: dict[str, str] = {}
    for tag, endian in (("LTL", "<"), ("BIG", ">")):
        path = tmp / f"synthetic-{tag}.bsp"
        path.write_bytes(synthetic_spk(endian))
        digests[tag] = sha256_file(path)
        with SPK.open(str(path)) as kernel:
            for i, seg in enumerate(kernel.segments):
                _t, _c, _ty, _nc, n_rec, init_d, intlen_d = SYNTH_SEGMENTS[i]
                n = n_rec * intlen_d  # coverage in days
                start = J2000 + init_d
                # Integer-day steps through the coverage, plus a fraction, and both ends.
                pts = [(start, 0.0), (start + n, 0.0)]
                pts += [(start + d, (d * 0.3819660112501051) % 1.0) for d in range(n)]
                for jd1, jd2 in pts:
                    vals = evaluate(seg, jd1, jd2)
                    rows.append(
                        [
                            str(i),
                            str(seg.target),
                            str(seg.center),
                            tag,
                            *map(repr, (jd1, jd2, *vals)),
                        ]
                    )
    return rows, digests


def header(title: str, columns: Sequence[str], n: int, extra: Sequence[str] = ()) -> str:
    """Provenance header lines (``#``-prefixed) and the column line."""
    if jplephem.__version__ != JPLEPHEM_PIN:
        msg = f"jplephem {jplephem.__version__} != pinned {JPLEPHEM_PIN} (REF-004)"
        raise SystemExit(msg)
    # REUSE-IgnoreStart (header text written into the CSV, not this file's licence)
    lines = [
        "SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "SPDX-License-Identifier: Apache-2.0",
        # REUSE-IgnoreEnd
        title,
        f"generated_by: tools/goldens/jplephem_de440.py@{git_head()}",
        f"engine: jplephem {jplephem.__version__}, numpy {np.__version__}, "
        f"Python {platform.python_version()} on {platform.machine()}",
        f"generated_utc: {datetime.now(UTC).strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"rows: {n}",
        "units: jd1, jd2 two-part TDB Julian date (days); x, y, z km; vx, vy, vz km/s",
        *extra,
    ]
    return "".join(f"# {line}\n" for line in lines) + ",".join(columns) + "\n"


def write(path: Path, head: str, rows: list[list[str]]) -> None:
    """Write a golden CSV and report it."""
    path.write_text(head + "".join(",".join(r) + "\n" for r in rows), encoding="utf-8")
    print(f"wrote {path} ({len(rows)} rows, sha256 {sha256_file(path)})")


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--bsp", type=Path, required=True, help="pinned de440.bsp")
    parser.add_argument("--sb441", type=Path, help="pinned sb441-n373s.bsp (optional)")
    parser.add_argument("--out-dir", type=Path, default=ROOT / "tests" / "goldens")
    args = parser.parse_args(argv)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    rows = de440_rows(args.bsp)
    extra = [f"source: de440.bsp sha256 {sha256_file(args.bsp)}"]
    write(
        args.out_dir / "jplephem-de440.csv",
        header(
            "jplephem SPK goldens: every de440.bsp segment, raw (REF-004)",
            COLUMNS,
            len(rows),
            extra,
        ),
        rows,
    )
    with tempfile.TemporaryDirectory() as tmp:
        srows, digests = synthetic_rows(Path(tmp))
    write(
        args.out_dir / "jplephem-synthetic.csv",
        header(
            "jplephem SPK goldens: synthetic type-2 and type-3 SPK, both byte orders (DEP-011)",
            COLUMNS,
            len(srows),
            [f"synthetic_sha256_{k}: {v}" for k, v in digests.items()],
        ),
        srows,
    )
    if args.sb441 is not None:
        brows = sb441_rows(args.sb441)
        write(
            args.out_dir / "jplephem-sb441.csv",
            header(
                "jplephem SPK goldens: Ceres, Pallas, Vesta from sb441-n373s.bsp (DEP-011)",
                COLUMNS,
                len(brows),
                [f"source: sb441-n373s.bsp sha256 {sha256_file(args.sb441)}"],
            ),
            brows,
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
