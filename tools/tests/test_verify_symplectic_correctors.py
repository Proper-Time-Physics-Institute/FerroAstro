# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Independent check of the corrector coefficients against Wisdom (2006) (issue #43, NUM-004)."""

from __future__ import annotations

import pytest

import symplectic_correctors as sc
import verify_symplectic_correctors as vc

N = 20  # series terms: enough for the order-17 corrector's first mismatch at x^19


@pytest.fixture(scope="module")
def target() -> vc.Series:
    return vc.target_series(N)


# Verifies: NUM-004
def test_target_series_agrees_with_the_closed_form_of_wp29(target: vc.Series) -> None:
    """The first-order algebra and WP29's Bernoulli closed form give the same ``g``."""
    for m in range(N):
        assert target[m].q == 0
        expected = sc.d_coefficient((m - 1) // 2) if m % 2 == 1 else 0
        assert target[m].p == expected, m


# Verifies: NUM-004
def test_committed_rust_table_has_its_stated_order_and_is_the_unique_solution(
    target: vc.Series,
) -> None:
    table = vc.rust_table()
    assert sorted(table) == [3, 5, 7, 11]
    for order, beta in table.items():
        stages = vc.ferro_stages([vc.QA.of(b) for b in beta])
        sign, first_miss = vc.match_order(vc.corrector_series(stages, N), target)
        assert (sign, first_miss) == (1, order), order
        independent = vc.solve(vc.ferro_stages, len(beta), sign)
        assert [v.p for v in independent] == beta, order
        assert all(v.q == 0 for v in independent), order


# Verifies: NUM-004
@pytest.mark.parametrize("order", [3, 5, 7, 11, 17])
def test_published_wisdom_2006_tables_meet_the_same_conditions(
    order: int, target: vc.Series
) -> None:
    """Wisdom's stages (``a = k alpha``) satisfy the conditions with the opposite global sign
    (conjugation convention), have the stated order, and are reproduced by the exact solve."""
    sign, first_miss = vc.match_order(vc.corrector_series(vc.published_stages(order), N), target)
    assert (sign, first_miss) == (-1, order)
    assert vc.solve_wisdom_r(order, sign) == [r for _, r in vc.PUBLISHED[order]]


# Verifies: NUM-004
def test_order_17_b1_as_printed_has_a_sign_typo(target: vc.Series) -> None:
    """With the printed (positive) numerator of ``b_1`` not even the lowest condition holds;
    the sign of the printed decimal gives order 17 (previous test)."""
    rs = [r for _, r in vc.PUBLISHED[17]]
    assert rs[0] == -vc.PUBLISHED_17_B1_AS_PRINTED
    rs[0] = vc.PUBLISHED_17_B1_AS_PRINTED
    _, first_miss = vc.match_order(vc.corrector_series(vc.published_stages(17, rs), N), target)
    assert first_miss == 1
