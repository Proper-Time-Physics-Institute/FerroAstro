# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""WP26 check P7 (NUM-021, TST-005): one scenario, several builds, identical output bits.

Runs ``ferroastro scenario run`` on one scenario file with the CLI built

* ``scalar``: without the ``simd`` feature (baseline x86-64 target),
* ``simd``: with ``--features simd`` (the ``wide`` f64x4 lanes, SSE2 halves on the baseline),
* ``simd-native`` (with ``--native``): ``--features simd`` and ``RUSTFLAGS=-C target-cpu=native``
  (AVX lanes), in a separate target directory so the main build cache is not invalidated,

and compares the SHA-256 of the encoded outputs (``RunOutput``, postcard). The ``scalar`` and
``simd`` digests must be equal (exit 0); otherwise the exit status is 1. The native build is
reported; a mismatch there is printed but does not change the exit status (the plan registers
only the feature on/off comparison).

The data products are passed as paths (no network I/O here): header.440 from the cache
(``$FA_DATA_DIR`` or ``target/fa-data``) and the committed REF-003 sampled states. Their SHA-256
is printed with the results.

Usage: ``uv run python simd_identity.py [--scenario FILE] [--native] [--out DIR]``.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCENARIO = ROOT / "ci" / "scenarios" / "wp26-headline-substitute-20yr.toml"
SMALL_BODIES = ROOT / "tests" / "goldens" / "sb441-n373s-jdepoc-states.csv"
#: The variants whose digests must agree (the feature off and on).
REQUIRED = ("scalar", "simd")


@dataclass(frozen=True)
class Variant:
    """One CLI build."""

    name: str
    features: tuple[str, ...]
    rustflags: str
    target_dir: Path | None


@dataclass(frozen=True)
class Result:
    """One run of one variant."""

    variant: str
    sha256: str
    seconds: float
    summary: str


def sha256_file(path: Path) -> str:
    """Return the hex SHA-256 of a file."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def variants(native: bool) -> list[Variant]:
    """Return the builds to compare."""
    out = [
        Variant("scalar", (), "", None),
        Variant("simd", ("simd",), "", None),
    ]
    if native:
        out.append(
            Variant(
                "simd-native",
                ("simd",),
                "-C target-cpu=native",
                ROOT / "target" / "wp26-native",
            )
        )
    return out


def verdict(results: list[Result]) -> tuple[bool, list[str]]:
    """Return (the required digests agree, report lines)."""
    by_name = {r.variant: r.sha256 for r in results}
    lines = [f"{r.variant:<12} {r.sha256}  {r.seconds:8.1f} s" for r in results]
    required = [by_name[n] for n in REQUIRED if n in by_name]
    ok = len(required) == len(REQUIRED) and len(set(required)) == 1
    lines.append(
        f"required ({', '.join(REQUIRED)}): {'IDENTICAL' if ok else 'DIFFERENT or missing'}"
    )
    for r in results:
        if r.variant not in REQUIRED and required:
            same = r.sha256 == required[0]
            lines.append(f"reported {r.variant}: {'identical' if same else 'DIFFERENT'}")
    return ok, lines


def run_variant(v: Variant, scenario: Path, header: Path, small: Path, out_dir: Path) -> Result:
    """Build and run one variant; return its output digest."""
    env = dict(os.environ)
    if v.rustflags:
        env["RUSTFLAGS"] = v.rustflags
    if v.target_dir is not None:
        env["CARGO_TARGET_DIR"] = str(v.target_dir)
    cmd = ["cargo", "run", "--release", "-q", "-p", "ferroastro-cli"]
    if v.features:
        cmd += ["--features", ",".join(v.features)]
    build_cmd = ["cargo", "build", *cmd[2:]]
    build = subprocess.run(  # noqa: S603 (fixed argument list: cargo and our own paths)
        build_cmd, check=False, cwd=ROOT, env=env, capture_output=True, text=True
    )
    if build.returncode != 0:
        raise RuntimeError(f"{v.name}: build failed: {build.stderr}")
    output = out_dir / f"{scenario.stem}.{v.name}.postcard"
    args = [
        "--",
        "scenario",
        "run",
        str(scenario),
        "--header",
        str(header),
        "--small-bodies",
        str(small),
        "--output",
        str(output),
    ]
    start = time.monotonic()
    proc = subprocess.run(  # noqa: S603 (fixed argument list: cargo and our own paths)
        [*cmd, *args], check=False, cwd=ROOT, env=env, capture_output=True, text=True
    )
    seconds = time.monotonic() - start
    if proc.returncode != 0:
        raise RuntimeError(f"{v.name}: run failed ({proc.returncode}): {proc.stderr}")
    first = proc.stdout.splitlines()
    summary = next((line for line in first if line.startswith("scenario ")), "")
    return Result(v.name, sha256_file(output), seconds, summary)


def main(argv: list[str] | None = None) -> int:
    """Entry point."""
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--scenario", type=Path, default=DEFAULT_SCENARIO)
    p.add_argument("--header", type=Path, default=None)
    p.add_argument("--small-bodies", type=Path, default=SMALL_BODIES)
    p.add_argument("--native", action="store_true", help="also a -C target-cpu=native build")
    p.add_argument("--out", type=Path, default=ROOT / "target" / "wp26-identity")
    a = p.parse_args(argv)
    data_dir = Path(os.environ.get("FA_DATA_DIR", ROOT / "target" / "fa-data"))
    header = a.header or data_dir / "header.440"
    if not header.is_file():
        print(f"header.440 not found at {header}; fetch it first (tools/fetch_jpl.py)")
        return 2
    a.out.mkdir(parents=True, exist_ok=True)
    print(f"scenario     : {a.scenario} sha256 {sha256_file(a.scenario)}")
    print(f"header.440   : {header} sha256 {sha256_file(header)}")
    print(f"small bodies : {a.small_bodies} sha256 {sha256_file(a.small_bodies)}")
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"],  # noqa: S607
        check=False,
        cwd=ROOT,
        capture_output=True,
        text=True,
    ).stdout.strip()
    print(f"commit       : {commit}")
    results = []
    for v in variants(a.native):
        r = run_variant(v, a.scenario, header, a.small_bodies, a.out)
        print(f"{v.name}: {r.summary}")
        results.append(r)
    ok, lines = verdict(results)
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
