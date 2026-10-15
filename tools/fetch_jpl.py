# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Fetch pinned JPL data products into the FerroAstro data cache (REF-001, LIC-003, DEP-017).

The products, their URLs and their pinned SHA-256 digests are listed in
``tests/data/MANIFEST.toml`` (human-owned). Files are written to ``$FA_DATA_DIR``, else
``$CARGO_TARGET_DIR/fa-data``, else ``<repo>/target/fa-data`` -- the same lookup as the Rust crate
``ferroastro-testdata``, which locates and re-verifies them. Each download writes a
``<file>.provenance.toml`` sidecar (URL, UTC retrieval date, SHA-256, size, ``Last-Modified``) in
the format the Rust crate reads. Nothing is ever committed to git.

A cached copy whose SHA-256 matches its pin is reused; a mismatching cached copy is downloaded
again; a download that does not match its pin is discarded and the script exits with status 1.

Usage::

    uv run python fetch_jpl.py                 # the PR-fast set: header.440, de440.bsp, testpo.440
    uv run python fetch_jpl.py de440-ascp01950 # named products
    uv run python fetch_jpl.py --all           # every manifest product (about 1.2 GB)
    uv run python fetch_jpl.py --verify-only   # no network: check the cache against the pins
    uv run python fetch_jpl.py --list
"""

from __future__ import annotations

import argparse
import hashlib
import os
import sys
import time
import tomllib
import urllib.error
import urllib.request
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "tests" / "data" / "MANIFEST.toml"
DATA_DIR_ENV = "FA_DATA_DIR"
# The PR-fast cache of CI-005: products the non-ignored ephemeris tests read.
DEFAULT_PRODUCTS = ("de440-header", "de440-bsp", "de440-testpo")
RETRIES = 3
TIMEOUT_S = 300
CHUNK = 1 << 20


@dataclass(frozen=True)
class Product:
    """One ``[[product]]`` entry of the manifest."""

    name: str
    url: str
    file: str
    sha256: str


@dataclass(frozen=True)
class Download:
    """The bytes of one HTTP answer and its ``Last-Modified`` header."""

    path: Path
    last_modified: str | None


Downloader = Callable[[str, Path], Download]


class FetchError(Exception):
    """A product could not be fetched or does not match its pin."""


def load_manifest(path: Path = MANIFEST) -> dict[str, Product]:
    """Parse the manifest into products keyed by name."""
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        msg = f"{path}: unsupported schema_version {data.get('schema_version')!r}"
        raise FetchError(msg)
    products: dict[str, Product] = {}
    for raw in data.get("product", []):
        p = Product(raw["name"], raw["url"], raw["file"], raw["sha256"])
        if not p.url.startswith("https://"):
            msg = f"{p.name}: only https URLs are fetched, got {p.url!r}"
            raise FetchError(msg)
        if len(p.sha256) != 64 or any(c not in "0123456789abcdef" for c in p.sha256):
            msg = f"{p.name}: sha256 must be 64 lowercase hex digits"
            raise FetchError(msg)
        if p.name in products:
            msg = f"duplicate product {p.name!r}"
            raise FetchError(msg)
        products[p.name] = p
    return products


def data_dir() -> Path:
    """Return the cache directory (same lookup order as ``ferroastro_testdata::data_dir``)."""
    if env := os.environ.get(DATA_DIR_ENV):
        return Path(env)
    if target := os.environ.get("CARGO_TARGET_DIR"):
        return Path(target) / "fa-data"
    return ROOT / "target" / "fa-data"


def sha256_file(path: Path) -> str:
    """Lowercase hex SHA-256 of a file, read in chunks."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(CHUNK):
            h.update(chunk)
    return h.hexdigest()


def http_download(url: str, dest: Path) -> Download:
    """Download ``url`` to ``dest`` with retries on network errors (never on a checksum)."""
    last: Exception | None = None
    for attempt in range(1, RETRIES + 2):
        try:
            # The scheme is restricted to https in load_manifest.
            with urllib.request.urlopen(url, timeout=TIMEOUT_S) as r, dest.open("wb") as f:  # noqa: S310
                while chunk := r.read(CHUNK):
                    f.write(chunk)
                return Download(dest, r.headers.get("Last-Modified"))
        except (urllib.error.URLError, TimeoutError, ConnectionError) as e:
            last = e
            print(f"  attempt {attempt} failed: {e}", file=sys.stderr)
            time.sleep(min(2.0**attempt, 30.0))
    msg = f"download of {url} failed after {RETRIES + 1} attempts: {last}"
    raise FetchError(msg)


def sidecar_text(p: Product, sha: str, size: int, last_modified: str | None, day: str) -> str:
    """Render the provenance sidecar in the ``key = "value"`` form ``ferroastro-testdata`` reads."""
    lines = [
        f"url = {_toml_str(p.url)}",
        f"retrieved = {_toml_str(day)}",
        f"sha256 = {_toml_str(sha)}",
        f"bytes = {size}",
    ]
    if last_modified is not None:
        lines.append(f"last_modified = {_toml_str(last_modified)}")
    return "\n".join(lines) + "\n"


def _toml_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def fetch(p: Product, directory: Path, download: Downloader = http_download) -> str:
    """Make ``directory/p.file`` match its pin; return ``"hit"`` or ``"downloaded"``."""
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / p.file
    if path.is_file() and sha256_file(path) == p.sha256:
        return "hit"
    part = directory / f"{p.file}.{os.getpid()}.part"
    try:
        got = download(p.url, part)
        sha = sha256_file(part)
        if sha != p.sha256:
            msg = (
                f"FA-E0201: {p.url} has SHA-256 {sha}, but tests/data/MANIFEST.toml pins "
                f"{p.sha256}; the upstream product changed: review it and re-pin deliberately"
            )
            raise FetchError(msg)
        size = part.stat().st_size
        part.replace(path)
    finally:
        part.unlink(missing_ok=True)
    day = datetime.now(UTC).strftime("%Y-%m-%d")
    side = directory / f"{p.file}.provenance.toml"
    tmp = directory / f"{p.file}.{os.getpid()}.provenance.part"
    tmp.write_text(sidecar_text(p, sha, size, got.last_modified, day), encoding="utf-8")
    tmp.replace(side)
    return "downloaded"


def verify(p: Product, directory: Path) -> str:
    """Check the cached copy without network access; return a status word."""
    path = directory / p.file
    if not path.is_file():
        return "MISSING"
    return "ok" if sha256_file(path) == p.sha256 else "MISMATCH"


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    ap.add_argument("products", nargs="*", help="manifest product names (default: PR-fast set)")
    ap.add_argument("--all", action="store_true", help="every product in the manifest")
    ap.add_argument("--list", action="store_true", help="list the manifest and exit")
    ap.add_argument("--verify-only", action="store_true", help="no network; check the cache")
    ap.add_argument("--manifest", type=Path, default=MANIFEST)
    args = ap.parse_args(argv)

    products = load_manifest(args.manifest)
    if args.list:
        for p in products.values():
            print(f"{p.name:<20} {p.sha256[:12]}…  {p.url}")
        return 0
    names = list(products) if args.all else (args.products or list(DEFAULT_PRODUCTS))
    unknown = [n for n in names if n not in products]
    if unknown:
        print(f"unknown product(s): {', '.join(unknown)} (see --list)", file=sys.stderr)
        return 2
    directory = data_dir()
    print(f"data cache: {directory}")
    status = 0
    for n in names:
        p = products[n]
        try:
            word = verify(p, directory) if args.verify_only else fetch(p, directory)
        except FetchError as e:
            print(f"{n}: {e}", file=sys.stderr)
            status = 1
            continue
        if word in ("MISSING", "MISMATCH"):
            status = 1
        print(f"{n:<20} {word:<10} sha256={p.sha256} {p.url}")
    return status


if __name__ == "__main__":
    sys.exit(main())
