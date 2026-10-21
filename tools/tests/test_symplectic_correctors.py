# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Exact checks of the derived symplectic-corrector coefficients (NUM-004, WP29)."""

from __future__ import annotations

from fractions import Fraction

import symplectic_correctors as sc


# Verifies: NUM-004
def test_series_coefficients_of_phi() -> None:
    """``(x/2)/sinh(x/2) = 1 - x^2/24 + 7 x^4/5760 - 31 x^6/967680 + ...`` (Bernoulli form)."""
    assert [sc.d_coefficient(k) for k in range(3)] == [
        Fraction(-1, 24),
        Fraction(7, 5760),  # lint: allow-literal(series coefficient, exact rational)
        Fraction(-31, 967680),  # lint: allow-literal(series coefficient, exact rational)
    ]


# Verifies: NUM-004
def test_corrector_coefficients_solve_the_matching_conditions_exactly() -> None:
    for order, n in sc.ORDERS.items():
        beta = sc.solve(n)
        assert len(beta) == n, order
        assert sc.residual(n, beta) == 0, order
    assert sc.solve(1) == [Fraction(-1, 48)]


# Verifies: NUM-004
def test_committed_rust_table_is_current() -> None:
    assert sc.main(["--check"]) == 0
