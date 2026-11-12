# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""ERFA oracle runner (REF-009): regenerate the WP8/WP9 pyerfa goldens with MANIFEST entries.

Runs inside ``ci/oracles/erfa`` (pyerfa 2.0.1.5, BSD-3-Clause). It executes the committed
generators ``tools/goldens/erfa_time.py`` and ``tools/goldens/erfa_frames.py`` unchanged (they are
copied into the image) and writes their three CSV files plus ``MANIFEST.json`` to ``--out-dir``.
The data rows must equal the committed ``tests/goldens/erfa-*-pyerfa.csv`` bit for bit; the
``#`` header lines carry the generation time and environment and differ by design. The host
compares with ``python -m oracles.compare_goldens``.

This job takes no scenario: time scales and frame rotations have no force switches. A
``--scenario`` argument is refused (REF-011) rather than ignored.

Usage (inside the container)::

    python -m oracles.erfa_runner --out-dir OUT
"""

from __future__ import annotations

import argparse
import importlib.util
import os
import sys
import time
from collections.abc import Sequence
from pathlib import Path
from types import ModuleType

from oracles import common

TOOL = "erfa"
JOB = "erfa-goldens"
GOLDENS = Path(__file__).resolve().parent.parent / "goldens"

#: Output file -> (generator, units, frame, time scale).
OUTPUTS: dict[str, tuple[str, str, str, str]] = {
    "erfa-time-pyerfa.csv": (
        "erfa_time.py",
        "two-part Julian dates in days; dtdb in seconds; ut fraction of day; elong rad; u, v km",
        "none (time scales); topocentric dtdb at a geocentric site (u, v)",
        "TT, TAI, TDB, TCB, TCG",
    ),
    "erfa-dat-pyerfa.csv": (
        "erfa_time.py",
        "calendar date and day fraction; TAI-UTC in seconds",
        "none (time scales)",
        "UTC, TAI",
    ),
    "erfa-frames-pyerfa.csv": (
        "erfa_frames.py",
        "tt1, tt2: two-part TT Julian date in days; angles in radians; matrices dimensionless",
        "GCRS/ICRS, IAU 2006 mean equator and equinox of date, mean ecliptic of date, ECLIPJ2000",
        "TT",
    ),
}


def load(name: str) -> ModuleType:
    """Load a generator module by path (``tools/goldens`` is not a package)."""
    spec = importlib.util.spec_from_file_location(name.removesuffix(".py"), GOLDENS / name)
    if spec is None or spec.loader is None:
        msg = f"cannot load {GOLDENS / name}"
        raise common.SetupError(msg)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--scenario", type=Path, help="refused: this job takes no scenario")
    args = parser.parse_args(argv)
    try:
        if args.scenario is not None:
            raise common.Refusal(
                TOOL,
                args.scenario.name,
                [("--scenario", "the ERFA job regenerates fixed time/frame goldens")],
            )
        digest, runner_sha = common.runner_identity()
    except common.Refusal as r:
        print(r, file=sys.stderr)
        return common.REFUSAL_EXIT
    except common.SetupError as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    os.environ["FA_GENERATOR_SHA"] = runner_sha
    args.out_dir.mkdir(parents=True, exist_ok=True)
    walls: dict[str, float] = {}
    version = "?"
    for gen in ("erfa_time.py", "erfa_frames.py"):
        module = load(gen)
        started = time.perf_counter()
        status = int(module.main(["--out-dir", str(args.out_dir)]))
        walls[gen] = time.perf_counter() - started
        if status != 0:
            print(f"{gen} exited with {status}", file=sys.stderr)
            return status
        erfa = module.erfa
        version = (
            f"pyerfa {erfa.__version__} (ERFA {erfa.version.erfa_version}, "
            f"SOFA {erfa.version.sofa_version}), numpy {module.np.__version__}"
        )
    here = Path(__file__).resolve().parent
    runner_files = sorted(here.glob("*.py")) + [
        GOLDENS / g for g in ("erfa_time.py", "erfa_frames.py")
    ]
    entries = []
    for name, (gen, units, frame, scales) in OUTPUTS.items():
        path = args.out_dir / name
        entries.append(
            common.ManifestEntry(
                file=name,
                tool=TOOL,
                version=version,
                image_digest=digest,
                runner_sha=runner_sha,
                runner_sha256=common.sha256_files(runner_files),
                input_sha256={gen: common.sha256_file(GOLDENS / gen)},
                cpu=common.cpu_model(),
                wall_clock_s=round(walls[gen], 3),
                output_sha256=common.sha256_file(path),
                units=units,
                frame=frame,
                time_scale=scales,
                scenario=JOB,
                generated_utc=common.utc_now(),
                command=f"python -m oracles.erfa_runner (runs tools/goldens/{gen})",
            )
        )
    common.write_manifest(args.out_dir, entries)
    print(f"wrote {len(entries)} goldens to {args.out_dir} with {version}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
