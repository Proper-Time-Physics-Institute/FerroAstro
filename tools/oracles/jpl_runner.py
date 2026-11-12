# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""JPL oracle runner (REF-001): fetch and verify the pinned DE440 products.

Runs inside ``ci/oracles/jpl`` (jplephem 2.24, MIT). For each REF-001 product (``header.440``,
``testpo.440``, ``de440.bsp``) it takes the URL and SHA-256 pin from ``tests/data/MANIFEST.toml``
(mounted read-only), downloads the file into the mounted cache when absent, and verifies the
SHA-256. Nothing is baked into the image and nothing is committed (LIC-003, DEP-017).

It also cross-checks the two independent JPL products with jplephem: the SSB-centred
``header.440`` initial conditions at ``JDEPOC`` (read by :mod:`oracles.header440`, not by
FerroAstro) against ``de440.bsp`` evaluated at the same epoch, for the Sun, the planetary-system
barycentres and the Earth-Moon barycentre.

Output: ``ref001-verification.json`` plus ``MANIFEST.json``. Exit status 0 when every pin
matches, 5 when one does not (the JSON names it).

Usage (inside the container)::

    python -m oracles.jpl_runner --manifest MANIFEST.toml --cache DIR --out-dir OUT
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import tomllib
import urllib.request
from collections.abc import Sequence
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from oracles import common
from oracles.header440 import BODIES, NAIF, Header440

TOOL = "jpl"
JOB = "ref001-pins"
#: The REF-001 ground-truth products, by MANIFEST.toml name.
PRODUCTS = ("de440-header", "de440-testpo", "de440-bsp")
PIN_MISMATCH_EXIT = 5
OUTPUT = "ref001-verification.json"


def fetch(url: str, dest: Path) -> dict[str, str]:
    """Download ``url`` to ``dest`` (via a ``.part`` file); return response metadata."""
    part = dest.with_suffix(dest.suffix + ".part")
    req = urllib.request.Request(url, headers={"User-Agent": "FerroAstro-oracle-jpl/1"})  # noqa: S310
    with urllib.request.urlopen(req, timeout=600) as resp, part.open("wb") as f:  # noqa: S310
        while chunk := resp.read(1 << 20):
            f.write(chunk)
        meta = {"last_modified": str(resp.headers.get("Last-Modified", ""))}
    part.replace(dest)
    meta["retrieved_utc"] = common.utc_now()
    return meta


def verify(pins: dict[str, dict[str, Any]], cache: Path) -> list[dict[str, Any]]:
    """Fetch (when absent) and hash every REF-001 product; one report row each."""
    rows = []
    for name in PRODUCTS:
        pin = pins[name]
        path = cache / str(pin["file"])
        row: dict[str, Any] = {
            "name": name,
            "url": pin["url"],
            "file": pin["file"],
            "sha256_pinned": pin["sha256"],
        }
        if path.exists():
            row["fetched"] = False
        else:
            row["fetched"] = True
            row.update(fetch(str(pin["url"]), path))
        row["bytes"] = path.stat().st_size
        row["sha256"] = common.sha256_file(path)
        row["pin_ok"] = row["sha256"] == pin["sha256"]
        rows.append(row)
    return rows


def header_vs_bsp(header: Header440, bsp: Path) -> dict[str, Any]:
    """Max |Δr| (km) and |Δv| (km/s) of header.440 ICs vs de440.bsp at JDEPOC, per body."""
    from jplephem.spk import SPK  # noqa: PLC0415 - only inside the image

    jd = header.value("JDEPOC")
    au = header.value("AU")
    per_body: dict[str, dict[str, float]] = {}
    kernel = SPK.open(str(bsp))
    try:
        for body in BODIES:
            _, p, v = header.state(body)
            pos, vel = kernel[0, NAIF[body]].compute_and_differentiate(jd)
            dr = max(abs(float(pos[k]) - p[k] * au) for k in range(3))
            dv = max(abs(float(vel[k]) / 86400.0 - v[k] * au / 86400.0) for k in range(3))
            per_body[body] = {"max_abs_dr_km": dr, "max_abs_dv_km_s": dv}
    finally:
        kernel.close()
    return {
        "epoch_jd_tdb": jd,
        "per_body": per_body,
        "max_abs_dr_km": max(b["max_abs_dr_km"] for b in per_body.values()),
        "max_abs_dv_km_s": max(b["max_abs_dv_km_s"] for b in per_body.values()),
    }


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        digest, runner_sha = common.runner_identity()
        doc = tomllib.loads(args.manifest.read_text(encoding="utf-8"))
        pins = {str(p["name"]): p for p in doc.get("product", [])}
        missing = [n for n in PRODUCTS if n not in pins]
        if missing:
            msg = f"{args.manifest} has no pin for {missing}"
            raise common.SetupError(msg)
    except (common.SetupError, OSError, tomllib.TOMLDecodeError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    started = time.perf_counter()
    rows = verify(pins, args.cache)
    ok = all(r["pin_ok"] for r in rows)
    report: dict[str, Any] = {
        "job": JOB,
        "date_utc": datetime.now(UTC).strftime("%Y-%m-%d"),
        "products": rows,
        "all_pins_ok": ok,
    }
    if ok:
        files = {r["name"]: args.cache / str(r["file"]) for r in rows}
        report["header_vs_bsp_at_jdepoc"] = header_vs_bsp(
            Header440.read(files["de440-header"]), files["de440-bsp"]
        )
    wall = time.perf_counter() - started
    args.out_dir.mkdir(parents=True, exist_ok=True)
    path = args.out_dir / OUTPUT
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    import jplephem  # noqa: PLC0415 - only inside the image

    here = Path(__file__).resolve().parent
    entry = common.ManifestEntry(
        file=OUTPUT,
        tool=TOOL,
        version=f"jplephem {jplephem.__version__}",
        image_digest=digest,
        runner_sha=runner_sha,
        runner_sha256=common.sha256_files(sorted(here.glob("*.py"))),
        input_sha256={args.manifest.name: common.sha256_file(args.manifest)}
        | {str(r["file"]): str(r["sha256"]) for r in rows},
        cpu=common.cpu_model(),
        wall_clock_s=round(wall, 3),
        output_sha256=common.sha256_file(path),
        units="bytes; SHA-256 hex; header-vs-bsp differences in km and km/s",
        frame="ICRF axes, solar-system-barycentric (DE440)",
        time_scale="TDB (JDEPOC)",
        scenario=JOB,
        generated_utc=common.utc_now(),
        command="python -m oracles.jpl_runner",
    )
    common.write_manifest(args.out_dir, [entry])
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0 if ok else PIN_MISMATCH_EXIT


if __name__ == "__main__":
    raise SystemExit(main())
