# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Offline tests of tools/sample_sb441.py (no JPL data: toy inputs and the committed output)."""

from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

import fetch_jpl
import sample_sb441

TOY_HEADER = """KSIZE= 2036    NCOEFF= 1018

GROUP   1040

     4
  JDEPOC  AU      MA0001  MA8001

GROUP   1041

     4
  0.244040050000000000D+07  0.149597870700000000D+09  0.1D-12  0.2D-11

GROUP   1050
"""


# Verifies: REF-003
def test_header_constants_and_asteroid_symbols() -> None:
    c = sample_sb441.header_constants(TOY_HEADER)
    assert c["JDEPOC"] == 2440400.5
    assert c["MA8001"] == 0.2e-11
    # Only numbered asteroids (MAnnnn, nnnn < 8000) get a symbol; MA8001 is a KBO (OI-24).
    assert sample_sb441.asteroid_symbols(c) == {2_000_001: "MA0001"}


# Verifies: REF-003
def test_malformed_header_is_refused() -> None:
    with pytest.raises(sample_sb441.SampleError):
        sample_sb441.header_constants("no groups here")


# Verifies: REF-003, CI-005
def test_substituted_cache_file_is_refused(tmp_path: Path) -> None:
    payload = b"pinned bytes\n"
    p = fetch_jpl.Product(
        "toy", "https://example.invalid/t.bsp", "t.bsp", hashlib.sha256(payload).hexdigest()
    )
    (tmp_path / "t.bsp").write_bytes(payload)
    assert sample_sb441.verified(tmp_path, p) == tmp_path / "t.bsp"
    (tmp_path / "t.bsp").write_bytes(b"substituted\n")
    with pytest.raises(sample_sb441.SampleError, match="FA-E0204"):
        sample_sb441.verified(tmp_path, p)


# Verifies: REF-003
def test_committed_states_file_records_the_pinned_inputs() -> None:
    products = fetch_jpl.load_manifest()
    text = sample_sb441.OUT.read_text(encoding="utf-8")
    meta = {
        k.strip(): v.strip()
        for k, _, v in (
            line[2:].partition(":") for line in text.splitlines() if line.startswith("# ")
        )
    }
    spk = products[sample_sb441.SPK_PRODUCT]
    assert meta["source_file"] == spk.file
    assert meta["source_url"] == spk.url
    assert meta["source_sha256"] == spk.sha256
    assert meta["header_file"].endswith(products[sample_sb441.HEADER_PRODUCT].sha256)
    assert "jdepoc_tdb" in meta
    rows = [line for line in text.splitlines() if line and not line.startswith("#")]
    assert rows[0].split(",") == sample_sb441.COLUMNS
    assert len(rows) - 1 == int(meta["rows"].split()[0])
