# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Offline tests of tools/fetch_jpl.py (no network: the downloader is replaced)."""

from __future__ import annotations

import hashlib
import subprocess
from pathlib import Path

import pytest

import fetch_jpl

ROOT = Path(__file__).resolve().parents[2]
PAYLOAD = b"not a real ephemeris\n"
PAYLOAD_SHA = hashlib.sha256(PAYLOAD).hexdigest()


def _product(sha: str = PAYLOAD_SHA) -> fetch_jpl.Product:
    return fetch_jpl.Product("toy", "https://example.invalid/toy.bin", "toy.bin", sha)


def _fake(calls: list[str]) -> fetch_jpl.Downloader:
    def download(url: str, dest: Path) -> fetch_jpl.Download:
        calls.append(url)
        dest.write_bytes(PAYLOAD)
        return fetch_jpl.Download(dest, "Tue, 22 Dec 2020 00:56:26 GMT")

    return download


# Verifies: REF-001
def test_manifest_pins_every_jpl_product() -> None:
    products = fetch_jpl.load_manifest()
    for name in fetch_jpl.DEFAULT_PRODUCTS:
        assert name in products
    for p in products.values():
        assert p.url.startswith("https://ssd.jpl.nasa.gov/")
        assert len(p.sha256) == 64


# Verifies: LIC-003
def test_fetch_verifies_pin_and_writes_sidecar(tmp_path: Path) -> None:
    calls: list[str] = []
    assert fetch_jpl.fetch(_product(), tmp_path, _fake(calls)) == "downloaded"
    assert (tmp_path / "toy.bin").read_bytes() == PAYLOAD
    side = (tmp_path / "toy.bin.provenance.toml").read_text()
    assert f'sha256 = "{PAYLOAD_SHA}"' in side
    assert 'url = "https://example.invalid/toy.bin"' in side
    assert "last_modified" in side
    # A matching cached copy is reused without a download.
    assert fetch_jpl.fetch(_product(), tmp_path, _fake(calls)) == "hit"
    assert len(calls) == 1
    assert fetch_jpl.verify(_product(), tmp_path) == "ok"


# Verifies: LIC-003
def test_checksum_mismatch_is_refused(tmp_path: Path) -> None:
    with pytest.raises(fetch_jpl.FetchError, match="FA-E0201"):
        fetch_jpl.fetch(_product("0" * 64), tmp_path, _fake([]))
    assert not (tmp_path / "toy.bin").exists()
    assert list(tmp_path.iterdir()) == []
    assert fetch_jpl.verify(_product(), tmp_path) == "MISSING"


# Verifies: LIC-003
def test_non_https_and_bad_pins_are_rejected(tmp_path: Path) -> None:
    m = tmp_path / "MANIFEST.toml"
    m.write_text(
        'schema_version = 1\n[[product]]\nname = "x"\nurl = "http://x"\nfile = "x"\n'
        f'sha256 = "{PAYLOAD_SHA}"\n'
    )
    with pytest.raises(fetch_jpl.FetchError, match="https"):
        fetch_jpl.load_manifest(m)
    m.write_text(
        'schema_version = 1\n[[product]]\nname = "x"\nurl = "https://x"\nfile = "x"\n'
        'sha256 = "ABC"\n'
    )
    with pytest.raises(fetch_jpl.FetchError, match="hex"):
        fetch_jpl.load_manifest(m)


# Verifies: LIC-003, REF-001, DEP-017
def test_no_jpl_sofa_or_novas_file_is_tracked() -> None:
    files = subprocess.run(
        ["git", "ls-files"],  # noqa: S607
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    banned = []
    for f in files:
        name = Path(f).name.lower()
        if (
            name.endswith((".bsp", ".bpc", ".tpc", ".tf", ".tls"))
            or name.startswith(("header.4", "testpo.4", "ascp", "ascm", "sofa", "novas"))
            or name in ("testeph.f", "jplsubs.f")
        ):
            banned.append(f)
    assert banned == []
    # The data crate never ships: it is test infrastructure only (DEP-017).
    cargo = (ROOT / "crates" / "ferroastro-testdata" / "Cargo.toml").read_text()
    assert "publish = false" in cargo
