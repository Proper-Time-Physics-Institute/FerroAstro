# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Unit tests of the NUM-006 benchmark comparison (the benchmark itself runs locally)."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("bench_num006", HERE / "bench_num006.py")
assert spec is not None
assert spec.loader is not None
bench = importlib.util.module_from_spec(spec)
sys.modules["bench_num006"] = bench
spec.loader.exec_module(bench)

# Synthetic timings (ns): only their ratios matter.
BASE = [10.0, 11.0, 9.0]  # lint: allow-literal(synthetic timings)
CAND = [10.1, 30.0, 10.0]  # lint: allow-literal(synthetic timings)
SLOW = [11.0, 11.0, 11.0]  # lint: allow-literal(synthetic timings, 10 % slower)
EXPECTED_RATIO = 10.1 / 10.0  # lint: allow-literal(median of CAND over median of BASE)


# Verifies: NUM-006
def test_band_comes_from_the_vector_and_verdict_uses_medians() -> None:
    tol = bench.band()
    assert 0.0 < tol < 0.1  # lint: allow-literal(sanity range of a relative band)
    ratio, ok = bench.verdict(BASE, CAND, tol)
    assert abs(ratio - EXPECTED_RATIO) < 1e-12  # lint: allow-literal(float round-off)
    assert ok
    _, bad = bench.verdict([10.0, 10.0, 10.0], SLOW, tol)
    assert not bad
