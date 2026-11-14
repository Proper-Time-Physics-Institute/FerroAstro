# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""The live header.440 cross-check tooling (#22) on synthetic inputs: no header value is used.

The real comparison (FerroAstro vs tools/oracles/header440.py in the data tier, vs Orekit in the
oracle tier) needs the fetched JPL file and is run by ci.yml ``data`` and oracles.yml.
"""

from __future__ import annotations

import contextlib
import json
import struct
import tomllib
from pathlib import Path

import pytest

from oracles import header_crosscheck, orekit_constants
from oracles.header440 import Header440, bits_hex, constants_json
from oracles.header440 import main as header440_main

ROOT = Path(__file__).resolve().parents[2]
METADATA = ROOT / "crates" / "ferroastro-constants" / "data" / "de440-metadata.toml"
# Synthetic header: structural fixture with arbitrary tokens (not physics).
TOY = """GROUP   1010

Toy Ephemeris

GROUP   1040

     3
  AA      BB      CC

GROUP   1041

     3
  0.500000000000000000D+00  -0.250000000000000000D+00  0.300000000000000000D+01

GROUP   1050
"""


def _bits(x: float) -> str:
    return struct.pack(">d", x).hex()


def _doc(values: dict[str, float], reader: str = "test") -> dict[str, object]:
    return {
        "reader": reader,
        "constants": {k: {"repr": repr(v), "bits": _bits(v)} for k, v in values.items()},
    }


# Verifies: REF-002, PHY-002
def test_ulp_distance_counts_representable_steps_across_zero() -> None:
    one = _bits(1.0)
    up = struct.pack(">q", struct.unpack(">q", bytes.fromhex(one))[0] + 1).hex()
    assert header_crosscheck.ulp_distance(one, one) == 0
    assert header_crosscheck.ulp_distance(one, up) == 1
    assert header_crosscheck.ulp_distance(_bits(0.0), _bits(-0.0)) == 0
    tiny = _bits(5e-324)  # lint: allow-literal(smallest subnormal, an arithmetic probe)
    neg_tiny = _bits(-5e-324)  # lint: allow-literal(smallest subnormal, an arithmetic probe)
    assert header_crosscheck.ulp_distance(tiny, neg_tiny) == 2
    with pytest.raises(ValueError, match="16 hex digits"):
        header_crosscheck.ulp_distance("00", one)


# Verifies: REF-002, PHY-002
def test_allowed_distance_comes_from_the_committed_vector() -> None:
    doc = tomllib.loads((ROOT / "tests" / "vectors" / "system.toml").read_text(encoding="utf-8"))
    v = next(x for x in doc["vector"] if x["id"] == header_crosscheck.VECTOR_ID)
    assert set(v["requirement"]) >= {"REF-002", "PHY-002"}
    assert header_crosscheck.allowed_ulps() == float(v["value"]) + float(v["tolerance"]["abs"])


# Verifies: REF-002, PHY-002
def test_crosscheck_passes_identical_and_fails_shifted_or_missing(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    ours = {"AA": 0.5, "BB": -0.25}
    paths = {}
    for name, values in {
        "fa": ours,
        "same": ours,
        "shifted": {"AA": 0.5, "BB": -0.5},
        "missing": {"AA": 0.5},
    }.items():
        paths[name] = tmp_path / f"{name}.json"
        paths[name].write_text(json.dumps(_doc(values, name)), encoding="utf-8")
    assert header_crosscheck.main([str(paths["fa"]), str(paths["same"])]) == 0
    out = capsys.readouterr().out
    assert "2/2 within 0 ulp" in out
    assert "PASS" in out
    # The log names constants and distances only, never a value.
    assert "0.5" not in out
    assert header_crosscheck.main([str(paths["fa"]), str(paths["shifted"])]) == 1
    assert "MISMATCH" in capsys.readouterr().out
    assert header_crosscheck.main([str(paths["fa"]), str(paths["missing"])]) == 1
    assert "missing" in capsys.readouterr().out
    assert header_crosscheck.main([str(paths["fa"])]) == 2
    assert header_crosscheck.main([str(paths["fa"]), str(tmp_path / "nope.json")]) == 2


# Verifies: REF-002
def test_header440_json_round_trips_the_binary64_bits(tmp_path: Path) -> None:
    h = Header440.parse(TOY)
    doc = json.loads(constants_json(h, ["BB", "AA"], "toy"))
    assert list(doc["constants"]) == ["BB", "AA"]
    for name in ("AA", "BB"):
        entry = doc["constants"][name]
        assert entry["bits"] == bits_hex(h.value(name)) == _bits(float(entry["repr"]))
    header = tmp_path / "toy.440"
    header.write_text(TOY, encoding="utf-8")
    names_from = tmp_path / "fa.json"
    names_from.write_text(json.dumps(_doc({"CC": 3.0, "AA": 0.5})), encoding="utf-8")
    out = tmp_path / "out.json"
    with out.open("w", encoding="utf-8") as fh, contextlib.redirect_stdout(fh):
        assert header440_main(["--header", str(header), "--names-from", str(names_from)]) == 0
    assert list(json.loads(out.read_text(encoding="utf-8"))["constants"]) == ["CC", "AA"]
    assert header440_main(["--header", str(header), "--names", "ZZ"]) == 1


# Verifies: REF-002, PHY-002
def test_orekit_constants_reads_names_only_from_the_committed_metadata() -> None:
    names = orekit_constants.metadata_names(METADATA)
    doc = tomllib.loads(METADATA.read_text(encoding="utf-8"))
    assert names == [c["name"] for c in doc["constant"]]
    assert len(set(names)) == len(names)
    # No value travels with the names (#22): the metadata has no token/value key at all.
    for c in doc["constant"]:
        assert not {"token", "value"} & set(c), c["name"]
    assert orekit_constants.DE_FILE == "lnxp1990.440"


# Verifies: REF-002
def test_orekit_image_keeps_the_bundled_de440_binary_pinned_and_compiles_header_constants() -> None:
    dockerfile = (ROOT / "ci" / "oracles" / "orekit" / "Dockerfile").read_text(encoding="utf-8")
    assert "ARG LNXP1990_SHA256=" in dockerfile
    assert "sha256sum -c -" in dockerfile
    assert str(orekit_constants.DE_DIR) in dockerfile
    assert "HeaderConstants.java" in dockerfile
    java = (ROOT / "ci" / "oracles" / "orekit" / "HeaderConstants.java").read_text(encoding="utf-8")
    assert "getLoadedConstant" in java
    assert "isNaN" in java
