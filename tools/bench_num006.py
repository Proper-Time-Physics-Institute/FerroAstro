# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""NUM-006 "f64 path unchanged": benchmark the f64 IAS15 step before and after a refactor.

The script checks out two revisions into detached git worktrees under
``target/bench-num006/``: a baseline (default: the last commit before the ``Real`` refactor
of the integrator) and a candidate (default: ``HEAD``). It copies the baseline's
``crates/ferroastro-ladder/benches/two_body.rs`` into both, so the measured code path is the
same benchmark source, and then runs the Criterion benchmark
``ias15_sun_mercury/propagate_1_julian_year`` (about 317 IAS15 steps of the Newtonian
Sun + Mercury problem) alternately, baseline then candidate, ``--rounds`` times (default 5).
Each round reads Criterion's median estimate from ``estimates.json``.

The verdict compares the median of the baseline medians with the median of the candidate
medians. The pass band comes from vector ``NUM006-F64-BENCH-RATIO`` in
``tests/vectors/integrator-checks.toml`` (2 %, PROVISIONAL: the WP7 reading of NUM-006
"unchanged").

Usage::

    uv run python bench_num006.py [--baseline REV] [--candidate REV] [--rounds N]

The output is a log and a JSON summary in ``target/bench-num006/``.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tomllib
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "target" / "bench-num006"
BENCH_FILE = Path("crates/ferroastro-ladder/benches/two_body.rs")
BENCH_ID = "ias15_sun_mercury/propagate_1_julian_year"
DEFAULT_BASELINE = "c5fd5a0"


def git(*args: str, cwd: Path = ROOT) -> str:
    """Run git and return its stripped stdout."""
    return subprocess.run(  # noqa: S603
        ["git", *args],  # noqa: S607
        cwd=cwd,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def worktree(rev: str, name: str) -> Path:
    """Create (or recreate) a detached worktree of ``rev`` at ``target/bench-num006/<name>``."""
    path = OUT / name
    if path.exists():
        git("worktree", "remove", "--force", str(path))
    git("worktree", "add", "--detach", str(path), rev)
    return path


def run_bench(tree: Path, target: Path) -> float:
    """Run the benchmark once in ``tree`` and return Criterion's median (ns per iteration)."""
    cmd = [
        "cargo",
        "bench",
        "-p",
        "ferroastro-ladder",
        "--bench",
        "two_body",
        "--",
        BENCH_ID,
        "--exact",
        "--noplot",
        "--warm-up-time",
        "3",
        "--measurement-time",
        "8",
    ]
    env_target = {"CARGO_TARGET_DIR": str(target)}
    subprocess.run(  # noqa: S603
        cmd,
        cwd=tree,
        check=True,
        env={**os.environ, **env_target},
        capture_output=True,
        text=True,
    )
    est = target / "criterion" / BENCH_ID / "new" / "estimates.json"
    data = json.loads(est.read_text(encoding="utf-8"))
    return float(data["median"]["point_estimate"])


def band(vectors: Path = ROOT / "tests" / "vectors" / "integrator-checks.toml") -> float:
    """Relative pass band of the ratio, from vector ``NUM006-F64-BENCH-RATIO``."""
    doc = tomllib.loads(vectors.read_text(encoding="utf-8"))
    for v in doc["vector"]:
        if v["id"] == "NUM006-F64-BENCH-RATIO":
            return float(v["tolerance"]["rel"])
    msg = "vector NUM006-F64-BENCH-RATIO not found"
    raise KeyError(msg)


def verdict(
    baseline_ns: list[float], candidate_ns: list[float], tolerance: float
) -> tuple[float, bool]:
    """Ratio of the candidate median to the baseline median, and whether it is within the band."""
    ratio = statistics.median(candidate_ns) / statistics.median(baseline_ns)
    return ratio, abs(ratio - 1.0) <= tolerance


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--baseline", default=DEFAULT_BASELINE)
    ap.add_argument("--candidate", default="HEAD")
    ap.add_argument("--rounds", type=int, default=5)
    ap.add_argument("--tolerance", type=float, default=None, help="override the vector band")
    args = ap.parse_args(argv)

    tolerance = band() if args.tolerance is None else args.tolerance
    OUT.mkdir(parents=True, exist_ok=True)
    base_sha = git("rev-parse", args.baseline)
    cand_sha = git("rev-parse", args.candidate)
    trees = {
        "baseline": worktree(base_sha, "wt-baseline"),
        "candidate": worktree(cand_sha, "wt-candidate"),
    }
    shutil.copyfile(trees["baseline"] / BENCH_FILE, trees["candidate"] / BENCH_FILE)
    medians: dict[str, list[float]] = {"baseline": [], "candidate": []}
    for k in range(args.rounds):
        for name in ("baseline", "candidate"):
            ns = run_bench(trees[name], OUT / f"target-{name}")
            medians[name].append(ns)
            print(f"round {k + 1}: {name:9} median {ns / 1e6:.4f} ms", flush=True)
    b = statistics.median(medians["baseline"])
    c = statistics.median(medians["candidate"])
    ratio, ok = verdict(medians["baseline"], medians["candidate"], tolerance)
    summary = {
        "date_utc": datetime.now(UTC).isoformat(timespec="seconds"),
        "benchmark": BENCH_ID,
        "baseline": base_sha,
        "candidate": cand_sha,
        "rounds": args.rounds,
        "baseline_medians_ns": medians["baseline"],
        "candidate_medians_ns": medians["candidate"],
        "baseline_median_ns": b,
        "candidate_median_ns": c,
        "ratio": ratio,
        "tolerance": tolerance,
        "pass": ok,
        "host": f"{platform.node()} {platform.machine()} {platform.processor()}",
        "rustc": subprocess.run(
            ["rustc", "-V"],  # noqa: S607
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip(),
    }
    (OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))
    label = "PASS" if ok else "FAIL"
    print(
        f"NUM-006 f64 IAS15 benchmark: candidate/baseline = {ratio:.4f} ({label}, ±{tolerance:.0%})"
    )
    for tree in trees.values():
        git("worktree", "remove", "--force", str(tree))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
