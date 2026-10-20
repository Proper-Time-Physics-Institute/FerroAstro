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
