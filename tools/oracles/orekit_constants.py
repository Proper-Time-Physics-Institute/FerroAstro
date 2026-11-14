# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Orekit reading of the DE440 header constants: oracle (b) of the live cross-check (#22).

Runs inside ``ci/oracles/orekit`` only (``tools/oracles/run_oracles.sh header-constants OUT``).
The Java side, ``ci/oracles/orekit/HeaderConstants.java``, asks Orekit 13.1.9
(``JPLEphemeridesLoader.getLoadedConstant``) for every constant named in
``crates/ferroastro-constants/data/de440-metadata.toml`` (mounted read-only; names only, no values),
from the binary DE440 file ``DE-440-ephemerides/lnxp1990.440`` bundled in the pinned orekit-data
snapshot (verified by SHA-256 when the image is built). Output: ``orekit-header-constants.json``
and its ``MANIFEST.json`` entry (CI-003). The output is compared with FerroAstro by
:mod:`oracles.header_crosscheck` and is never committed.

Usage (inside the container)::

    python -m oracles.orekit_constants --metadata /in/de440-metadata.toml --out-dir OUT
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import tomllib
from collections.abc import Sequence
from pathlib import Path

from oracles import common

TOOL = "orekit"
OREKIT_DATA = Path("/opt/orekit/data")
#: Where the Dockerfile keeps the orekit-data DE440 binary (outside the data tree).
DE_DIR = Path("/opt/orekit/de440-bundled")
DE_FILE = "lnxp1990.440"
CLASSPATH = "/opt/orekit/lib/*:/opt/orekit/classes"
OUTPUT = "orekit-header-constants.json"
OREKIT_DATA_COMMIT = "2422a7d06ee2cc3b5a503a47271203b0706f3e82"


def metadata_names(path: Path) -> list[str]:
    """Return the constant names of the FerroAstro metadata file, in file order."""
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    names = [str(c["name"]) for c in doc.get("constant", [])]
    if not names:
        msg = f"{path}: no [[constant]] names"
        raise ValueError(msg)
    return names


def run(names: Sequence[str], out: Path, de_dir: Path = DE_DIR) -> None:
    """Run ``HeaderConstants`` for ``names``; raise ``ValueError`` on any failure."""
    cmd = [
        "java",
        "-cp",
        CLASSPATH,
        "HeaderConstants",
        str(OREKIT_DATA),
        str(de_dir),
        str(out),
        *names,
    ]
    r = subprocess.run(  # noqa: S603
        cmd,
        capture_output=True,
        text=True,
        check=False,
    )
    if r.returncode != 0:
        msg = f"HeaderConstants exit {r.returncode}: {r.stderr[-1500:]}"
        raise ValueError(msg)
    got = json.loads(out.read_text(encoding="utf-8"))["constants"]
    if list(got) != list(names):
        msg = f"HeaderConstants returned {list(got)}, expected {list(names)}"
        raise ValueError(msg)


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        names = metadata_names(args.metadata)
        digest, runner_sha = common.runner_identity()
        args.out_dir.mkdir(parents=True, exist_ok=True)
        path = args.out_dir / OUTPUT
        started = time.perf_counter()
        run(names, path)
        wall = time.perf_counter() - started
    except (common.SetupError, KeyError, ValueError, OSError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    java = Path("/opt/orekit/java-version.txt").read_text(encoding="utf-8").strip()
    here = Path(__file__).resolve().parent
    entry = common.ManifestEntry(
        file=OUTPUT,
        tool=TOOL,
        version="orekit 13.1.9, hipparchus 4.0.3",
        image_digest=digest,
        runner_sha=runner_sha,
        runner_sha256=common.sha256_files(sorted(here.glob("*.py"))),
        input_sha256={
            DE_FILE: common.sha256_file(DE_DIR / DE_FILE),
            args.metadata.name: common.sha256_file(args.metadata),
        },
        cpu=common.cpu_model(),
        wall_clock_s=round(wall, 3),
        output_sha256=common.sha256_file(path),
        units="as stored in the DE file (the units of de440-metadata.toml)",
        frame="n/a (constants)",
        time_scale="n/a (constants)",
        scenario="header440-constants",
        generated_utc=common.utc_now(),
        command="python -m oracles.orekit_constants "
        + " ".join(sys.argv[1:] if argv is None else argv),
        extra={
            "orekit": "13.1.9",
            "orekit_data_commit": OREKIT_DATA_COMMIT,
            "ephemeris_file": f"DE-440-ephemerides/{DE_FILE}",
            "java": java,
            "api": "JPLEphemeridesLoader.getLoadedConstant",
            "constants": str(len(names)),
        },
    )
    common.write_manifest(args.out_dir, [entry])
    print(f"wrote {path} ({len(names)} constants, {wall:.2f} s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
