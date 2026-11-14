# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Unit tests of the WP26 P7 identity tool's verdict (no builds run here).

# Verifies: NUM-021, TST-005
"""

from __future__ import annotations

import simd_identity as si


def _r(name: str, digest: str) -> si.Result:
    return si.Result(name, digest, 1.0, "")


def test_identical_required_digests_pass() -> None:
    ok, lines = si.verdict([_r("scalar", "aa"), _r("simd", "aa")])
    assert ok
    assert any("IDENTICAL" in line for line in lines)


def test_a_differing_simd_digest_fails() -> None:
    ok, _ = si.verdict([_r("scalar", "aa"), _r("simd", "ab")])
    assert not ok


def test_a_missing_required_variant_fails() -> None:
    ok, _ = si.verdict([_r("scalar", "aa")])
    assert not ok


def test_the_native_build_is_reported_not_required() -> None:
    ok, lines = si.verdict([_r("scalar", "aa"), _r("simd", "aa"), _r("simd-native", "bb")])
    assert ok
    assert "reported simd-native: DIFFERENT" in lines


def test_variants_add_the_native_build_on_request() -> None:
    assert [v.name for v in si.variants(False)] == ["scalar", "simd"]
    native = si.variants(True)[-1]
    assert native.name == "simd-native"
    assert "target-cpu=native" in native.rustflags
    assert native.target_dir is not None
