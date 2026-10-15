# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Symbolic derivation of the 1PN test-particle equations of motion (PHY-009, NUM-017).

The script derives, with sympy, the facts the Rust force models in
``crates/ferroastro-forces/src/relativity.rs`` rely on:

1. The test-particle Lagrangian per unit mass, ``L = -c^2 dtau/dt``, expanded to ``O(c^-2)``,
   has the form ``1/2 v^2 + U + eps [A v^4 + B U v^2 + C U^2 + D U (n.v)^2]`` with

   * PPN metric (isotropic gauge): ``A, B, C, D = 1/8, gamma + 1/2, 1/2 - beta, 0``;
   * Schwarzschild metric in areal coordinates: ``1/8, 1/2, 1/2, 1``.

2. The Euler-Lagrange equations of the general Lagrangian, reduced to first order in ``eps``
   by substituting the Newtonian acceleration into the ``O(eps)`` terms, give

   ``delta_a = (eps mu / r^2) [((4A - B - 2D) v^2 + 2(B - C + D) U + 3D rdot^2) n
   + (8A + 2B) rdot v]``.

3. With the PPN coefficients this equals the IERS-form Schwarzschild term (Journal #4 R3)
   ``mu/(c^2 r^3) [(2(beta+gamma) mu/r - gamma v^2) r + 2(1+gamma)(r.v) v]``.

Run ``uv run python pn_lagrangian.py`` to print the results; it exits non-zero if any identity
fails. ``tests/test_pn_lagrangian.py`` runs the same checks under pytest.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from typing import Any

import sympy as sp  # type: ignore[import-untyped]

Expr = Any  # sympy ships no type information (no py.typed marker)

EPS, MU, BETA, GAMMA = sp.symbols("epsilon mu beta gamma", positive=True)
A, B, C, D = sp.symbols("A B C D")
X = sp.symbols("x y z", real=True)
V = sp.symbols("vx vy vz", real=True)


@dataclass(frozen=True)
class Coefficients:
    """The four 1PN coefficients of the test-particle Lagrangian."""

    a: Expr
    b: Expr
    c: Expr
    d: Expr


def _dot(p: tuple[Expr, ...], q: tuple[Expr, ...]) -> Expr:
    return sum((pi * qi for pi, qi in zip(p, q, strict=True)), sp.Integer(0))


def _kinematics() -> tuple[Expr, Expr, Expr, Expr]:
    """Return ``r``, ``U = mu/r``, ``v^2`` and ``(n.v)^2`` as sympy expressions."""
    r = sp.sqrt(_dot(X, X))
    return r, MU / r, _dot(V, V), (_dot(X, V) / r) ** 2


def _first_order(expr: Expr) -> Expr:
    """Return the ``eps^1`` coefficient of ``expr`` expanded about ``eps = 0``."""
    series = sp.series(expr, EPS, 0, 2).removeO()
    return sp.expand(series).coeff(EPS, 1)


def _read_coefficients(l1: Expr) -> Coefficients:
    """Identify ``A..D`` in an ``O(eps)`` Lagrangian written with symbols ``U, v2, nv2``."""
    u, v2, nv2 = sp.symbols("U v2 nv2", positive=True)
    poly = sp.Poly(sp.expand(l1), u, v2, nv2)
    return Coefficients(
        a=poly.coeff_monomial(v2**2),
        b=poly.coeff_monomial(u * v2),
        c=poly.coeff_monomial(u**2),
        d=poly.coeff_monomial(u * nv2),
    )


def metric_coefficients(metric: str) -> Coefficients:
    """Expand ``-c^2 dtau/dt`` (per unit mass, ``eps = 1/c^2``) for ``metric``.

    ``metric`` is ``"ppn"`` (isotropic PPN with beta, gamma) or ``"areal"`` (Schwarzschild in
    standard coordinates). The result is expressed with abstract ``U``, ``v^2``, ``(n.v)^2``.
    """
    u, v2, nv2 = sp.symbols("U v2 nv2", positive=True)
    if metric == "ppn":
        # dtau^2/dt^2 = (1 - 2 eps U + 2 beta eps^2 U^2) - eps (1 + 2 gamma eps U) v^2
        dtau2 = 1 - 2 * EPS * u + 2 * BETA * EPS**2 * u**2 - EPS * (1 + 2 * GAMMA * EPS * u) * v2
    elif metric == "areal":
        # g_tt = -(1 - 2 eps U); g_rr = 1/(1 - 2 eps U); angular part unchanged:
        # spatial line element / dt^2 = v^2 + (g_rr - 1) (n.v)^2
        g_rr = 1 / (1 - 2 * EPS * u)
        dtau2 = (1 - 2 * EPS * u) - EPS * (v2 + (g_rr - 1) * nv2)
    else:
        msg = f"unknown metric {metric!r}"
        raise ValueError(msg)
    lagrangian = -sp.sqrt(dtau2) / EPS
    zeroth = sp.limit(lagrangian + 1 / EPS, EPS, 0)
    if sp.simplify(zeroth - (v2 / 2 + u)) != 0:
        msg = f"Newtonian limit of the {metric} Lagrangian is {zeroth}"
        raise ArithmeticError(msg)
    return _read_coefficients(_first_order(lagrangian + 1 / EPS))


def reduced_correction(k: Coefficients) -> tuple[Expr, ...]:
    """Euler-Lagrange ``delta_a`` of the general Lagrangian, to first order in ``eps``.

    ``a = grad U + eps [dL1/dr - d/dt(dL1/dv)]``, with ``d/dt`` taken along the Newtonian
    motion: ``d/dt f = sum_j (df/dx_j) v_j + (df/dv_j) a_N,j``.
    """
    _, u, v2, nv2 = _kinematics()
    l1 = k.a * v2**2 + k.b * u * v2 + k.c * u**2 + k.d * u * nv2
    a_newton = tuple(sp.diff(u, xi) for xi in X)
    out = []
    for i in range(3):
        p_i = sp.diff(l1, V[i])
        ddt = sum(
            (sp.diff(p_i, X[j]) * V[j] + sp.diff(p_i, V[j]) * a_newton[j] for j in range(3)),
            sp.Integer(0),
        )
        out.append(sp.diff(l1, X[i]) - ddt)
    return tuple(out)


def closed_form(k: Coefficients) -> tuple[Expr, ...]:
    """Return the closed form of ``PnLagrangian::add_acceleration`` (without ``eps``)."""
    r, u, v2, _ = _kinematics()
    rdot = _dot(X, V) / r
    radial = (4 * k.a - k.b - 2 * k.d) * v2 + 2 * (k.b - k.c + k.d) * u + 3 * k.d * rdot**2
    along = (8 * k.a + 2 * k.b) * rdot
    return tuple(MU / r**2 * (radial * xi / r + along * vi) for xi, vi in zip(X, V, strict=True))


def iers_r3() -> tuple[Expr, ...]:
    """Return the Journal #4 R3 correction (without ``eps``), PPN isotropic/harmonic gauge."""
    r, _, v2, _ = _kinematics()
    rv = _dot(X, V)
    radial = 2 * (BETA + GAMMA) * MU / r - GAMMA * v2
    return tuple(
        MU / r**3 * (radial * xi + 2 * (1 + GAMMA) * rv * vi) for xi, vi in zip(X, V, strict=True)
    )


def vectors_equal(p: tuple[Expr, ...], q: tuple[Expr, ...]) -> bool:
    """Return True if two symbolic 3-vectors simplify to the same expression."""
    return all(sp.simplify(pi - qi) == 0 for pi, qi in zip(p, q, strict=True))


def check() -> list[str]:
    """Run every identity; return a list of failures (empty when all hold)."""
    failures: list[str] = []
    ppn = metric_coefficients("ppn")
    half = sp.Rational(1, 2)
    want_ppn = Coefficients(sp.Rational(1, 8), GAMMA + half, half - BETA, 0)
    if any(sp.simplify(g - w) != 0 for g, w in zip(_tuple(ppn), _tuple(want_ppn), strict=True)):
        failures.append(f"PPN coefficients {ppn} != {want_ppn}")
    areal = metric_coefficients("areal")
    want_areal = Coefficients(sp.Rational(1, 8), sp.Rational(1, 2), sp.Rational(1, 2), 1)
    if any(sp.simplify(g - w) != 0 for g, w in zip(_tuple(areal), _tuple(want_areal), strict=True)):
        failures.append(f"areal coefficients {areal} != {want_areal}")
    generic = Coefficients(A, B, C, D)
    if not vectors_equal(reduced_correction(generic), closed_form(generic)):
        failures.append("reduced Euler-Lagrange correction != closed form")
    if not vectors_equal(closed_form(want_ppn), iers_r3()):
        failures.append("PPN closed form != Journal #4 R3")
    return failures


def _tuple(k: Coefficients) -> tuple[Expr, Expr, Expr, Expr]:
    return (k.a, k.b, k.c, k.d)


def main() -> int:
    """Print the derived coefficients and the result of every identity check."""
    for metric in ("ppn", "areal"):
        k = metric_coefficients(metric)
        print(f"{metric:5s}: A = {k.a}, B = {k.b}, C = {k.c}, D = {k.d}")
    failures = check()
    for f in failures:
        print(f"FAIL: {f}")
    if not failures:
        print("OK: reduced Euler-Lagrange closed form and PPN == R3 verified symbolically")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
