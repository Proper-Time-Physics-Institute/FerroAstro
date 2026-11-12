# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Symbolic identity of three Lense-Thirring forms (NUM-019, PHY-011, OI-35; WP20 check L2).

Clean-room transcriptions, each read from a page image of the typeset source:

* DE440 (Park et al. 2021, AJ 161, 105), Sec. 3.4, Eqs. 37-39:
  ``a = 2 Omega x v``, ``Omega = (1+gamma) G/(2 c^2 r^3) [-J + 3 (J.r) r/r^2]``, ``J = S p``;
* INPOP21a (Fienga et al. 2021, IMCCE scientific note), Eq. 1:
  ``a = (gamma+1) G/(c^2 r^3) S [3 (k.r)/r^2 (r x v) - (k x v)]``;
* Journal #4 R3b (``ferroastro_forces::LenseThirring``):
  ``a = (1+gamma) G S/(c^2 r^3) [(3/r^2)(r x v)(r.s) + v x s]``.

``check`` returns the list of pairs that differ (empty when all three are identical for general
gamma, G S, c, r, v and spin axis). ``tests/test_lt_forms.py`` runs it with a negative control.
Run ``uv run python lt_forms.py``; it exits non-zero if an identity fails.
"""

from __future__ import annotations

import sys
from collections.abc import Callable
from typing import Any

import sympy as sp  # type: ignore[import-untyped]

Vec = Any  # a sympy Matrix; sympy ships no type information (no py.typed marker)
Form = Callable[[], Vec]

R = sp.Matrix(sp.symbols("x y z", real=True))
V = sp.Matrix(sp.symbols("vx vy vz", real=True))
K = sp.Matrix(sp.symbols("kx ky kz", real=True))
GAMMA, GS, C = sp.symbols("gamma GS c", positive=True)


def de440(sign_j: int = -1) -> Vec:
    """DE440 Eqs. 37-38; ``sign_j = +1`` is the negative control (the ``-J`` term flipped)."""
    r, v, k, gamma, gs, c = R, V, K, GAMMA, GS, C
    rn2 = r.dot(r)
    rn3 = rn2 * sp.sqrt(rn2)
    j = gs * k
    omega = (1 + gamma) / (2 * c**2 * rn3) * (sign_j * j + 3 * j.dot(r) * r / rn2)
    return 2 * omega.cross(v)


def inpop21a() -> Vec:
    """INPOP21a Eq. 1."""
    r, v, k, gamma, gs, c = R, V, K, GAMMA, GS, C
    rn2 = r.dot(r)
    rn3 = rn2 * sp.sqrt(rn2)
    return (gamma + 1) * gs / (c**2 * rn3) * (3 * k.dot(r) / rn2 * r.cross(v) - k.cross(v))


def r3b() -> Vec:
    """Journal #4 R3b, as ``LenseThirring`` evaluates it."""
    r, v, k, gamma, gs, c = R, V, K, GAMMA, GS, C
    rn2 = r.dot(r)
    rn3 = rn2 * sp.sqrt(rn2)
    return (1 + gamma) * gs / (c**2 * rn3) * (3 / rn2 * r.cross(v) * r.dot(k) + v.cross(k))


def check(first: Form | None = None) -> list[str]:
    """Names of the form pairs that are not identically equal (empty list: all identical)."""
    forms: list[tuple[str, Form]] = [
        ("DE440 Eq. 37-38", first or de440),
        ("INPOP21a Eq. 1", inpop21a),
        ("R3b", r3b),
    ]
    values = [(name, f()) for name, f in forms]
    failures = []
    for i, (na, a) in enumerate(values):
        for nb, b in values[i + 1 :]:
            if any(sp.simplify(d) != 0 for d in a - b):
                failures.append(f"{na} != {nb}")
    return failures


def main() -> int:
    """Print the result; non-zero exit on failure."""
    failures = check()
    print("LT forms identical (general gamma):", not failures, failures)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
