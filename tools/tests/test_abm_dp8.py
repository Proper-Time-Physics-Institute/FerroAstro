# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""DP8 start-up coefficients (WP24 check A2): order conditions and the generated Rust table."""

from __future__ import annotations

import mpmath
import pytest

import abm_dp8


# Verifies: NUM-002
def test_tree_counts_and_order_conditions() -> None:
    assert [len(abm_dp8.trees(n)) for n in range(1, 9)] == [1, 1, 2, 4, 9, 20, 48, 115]
    count, worst, rowsum = abm_dp8.residuals()
    assert count == 200
    assert worst <= abm_dp8.TOLERANCE
    assert rowsum <= abm_dp8.TOLERANCE


# Verifies: NUM-002
def test_a_perturbed_coefficient_breaks_the_order_conditions(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rows = {i: dict(r) for i, r in abm_dp8.A_ROWS.items()}
    # Change the 8th-order weight b_6 in its 20th significant digit.
    b6 = mpmath.mpf(rows[12][6])
    rows[12][6] = mpmath.nstr(b6 * (1 + mpmath.mpf("1e-19")), 30)
    monkeypatch.setattr(abm_dp8, "A_ROWS", rows)
    _, worst, _ = abm_dp8.residuals()
    assert worst > abm_dp8.TOLERANCE


# Verifies: NUM-002
def test_rust_table_is_up_to_date() -> None:
    assert abm_dp8.main(["--check"]) == 0


# Verifies: NUM-002, DEP-007
def test_split_is_exact_to_double_double() -> None:
    _, _, b = abm_dp8.coefficients()
    for x in b:
        hi, lo = abm_dp8.split(x)
        assert abs(mpmath.mpf(hi) + mpmath.mpf(lo) - x) <= abs(x) * mpmath.mpf(2) ** -104
