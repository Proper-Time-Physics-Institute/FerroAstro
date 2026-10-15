# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Symbolic checks of the 1PN test-particle equations used by ferroastro-forces."""

from __future__ import annotations

import sympy as sp  # type: ignore[import-untyped]

import pn_lagrangian as pn


# Verifies: PHY-009, NUM-017
def test_metric_coefficients_and_reduced_equations() -> None:
    assert pn.check() == []


# Verifies: PHY-009
def test_negative_control_detects_a_wrong_coefficient() -> None:
    generic = pn.Coefficients(pn.A, pn.B, pn.C, pn.D)
    wrong = pn.Coefficients(pn.A, pn.B + sp.Rational(1, 2), pn.C, pn.D)
    assert not pn.vectors_equal(pn.reduced_correction(generic), pn.closed_form(wrong))
