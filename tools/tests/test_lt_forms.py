# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""DE440 Eq. 37-38, INPOP21a Eq. 1 and R3b Lense-Thirring forms are identical (WP20 check L2)."""

from __future__ import annotations

from typing import Any

import lt_forms


# Verifies: NUM-019, PHY-011
def test_de440_inpop21a_and_r3b_lense_thirring_forms_are_identical() -> None:
    assert lt_forms.check() == []


# Verifies: NUM-019
def test_negative_control_detects_a_flipped_spin_term() -> None:
    def wrong() -> Any:
        return lt_forms.de440(sign_j=1)

    assert lt_forms.check(wrong) != []
