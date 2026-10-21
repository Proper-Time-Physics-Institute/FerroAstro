# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Symbolic R5 -> R3 limit of the EIH equations (WP19 check C1)."""

from __future__ import annotations

import dataclasses

import eih_limit as eih


# Verifies: NUM-017, NUM-020
def test_eih_reduces_to_the_schwarzschild_test_particle_equations() -> None:
    assert eih.check() == []


# Verifies: NUM-017
def test_negative_control_detects_a_wrong_velocity_coefficient() -> None:
    wrong = dataclasses.replace(eih.eq27(), vi2=-eih.GAMMA)
    assert eih.check(wrong) != []


# Verifies: NUM-020, NUM-017
def test_eq27_equals_the_eih_two_body_lagrangian_and_conserves_its_energy() -> None:
    assert eih.check_lagrangian() == []


# Verifies: NUM-020
def test_lagrangian_check_detects_a_wrong_projection_coefficient() -> None:
    """The T6 coefficient (-3/2 -> -1) is invisible to the R5 -> R3 limit (WP19 probe E3)."""
    wrong = dataclasses.replace(eih.eq27(), proj=-1)
    assert eih.check(wrong) == []  # the R5 -> R3 limit cannot see T6
    assert eih.check_lagrangian(wrong) != []
