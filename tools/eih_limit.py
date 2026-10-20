# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Symbolic R5 -> R3 limit of the EIH equations (NUM-017, WP19 check C1).

``eq27_correction`` is an independent sympy transcription of Park et al. 2021 (DE440), Eq. 27,
minus its Newtonian part (the c^-2 terms, with ``eps = 1/c^2``), for any number of point masses
with positions, velocities and right-hand-side accelerations. ``check`` shows that, for a
massless body ``i`` and a single static source ``j`` (``v_j = a_j = 0``), the correction equals
the 1PN Schwarzschild test-particle correction in PPN isotropic/harmonic gauge (Journal #4 R3,
``ferroastro_forces::SchwarzschildPpn``):

``delta_a = mu/(c^2 r^3) [(2(beta+gamma) mu/r - gamma v^2) r + 2(1+gamma)(r.v) v]``

for general PPN beta and gamma. Clean-room: the equation was read from the typeset PDF only.

``check_lagrangian`` is an independent second check of every term for two bodies at beta = gamma
= 1. It derives the order-reduced Euler-Lagrange equations of the Einstein-Infeld-Hoffmann
two-body Lagrangian (Landau & Lifshitz, *The Classical Theory of Fields*, 4th ed., Eq. 106.17,
G = 1, ``eps = 1/c^2``)

``L = sum m v^2/2 + m_a m_b/r + eps [sum m v^4/8 + m_a m_b/(2r) (3(v_a^2 + v_b^2) - 7 v_a.v_b
- (n.v_a)(n.v_b)) - m_a m_b (m_a + m_b)/(2 r^2)]``

and compares them with ``eq27_correction`` at random rational points (60 significant digits). The
same function checks that the energy ``E = sum v.dL/dv - L`` is conserved at O(eps) under Eq. 27;
``ferroastro-eih``'s binary test integrates that energy.

Run ``uv run python eih_limit.py``; it exits non-zero if an identity fails.
``tests/test_eih_limit.py`` runs the same checks under pytest, with a negative control.
"""

from __future__ import annotations

import dataclasses
import random
import sys
from collections.abc import Sequence
from dataclasses import dataclass
from typing import Any

import sympy as sp  # type: ignore[import-untyped]

Expr = Any  # sympy ships no type information (no py.typed marker)
Vec = tuple[Expr, Expr, Expr]

BETA, GAMMA = sp.symbols("beta gamma", real=True)
MU = sp.symbols("mu", positive=True)
X: Vec = sp.symbols("x y z", real=True)
V: Vec = sp.symbols("vx vy vz", real=True)
ZERO: Vec = (sp.Integer(0), sp.Integer(0), sp.Integer(0))


@dataclass(frozen=True)
class Body:
    """A point mass: GM, barycentric position, velocity and right-hand-side acceleration."""

    gm: Expr
    r: Vec
    v: Vec
    a: Vec


@dataclass(frozen=True)
class Coefficients:
    """The numeric coefficients of Eq. 27, so a negative control can perturb one."""

    ui: Expr  # -2(beta + gamma), multiplies U_i
    uj: Expr  # -(2 beta - 1), multiplies U_j
    vi2: Expr  # gamma, multiplies v_i^2
    vj2: Expr  # 1 + gamma, multiplies v_j^2
    vivj: Expr  # -2(1 + gamma), multiplies v_i . v_j
    proj: Expr  # -3/2, multiplies [(r_i - r_j) . v_j / r_ij]^2
    aj: Expr  # 1/2, multiplies (r_j - r_i) . a_j
    s2i: Expr  # 2 + 2 gamma
    s2j: Expr  # 1 + 2 gamma
    s3: Expr  # (3 + 4 gamma)/2


def eq27() -> Coefficients:
    """Return the coefficients as typeset in Park et al. 2021, Eq. 27."""
    return Coefficients(
        ui=-2 * (BETA + GAMMA),
        uj=-(2 * BETA - 1),
        vi2=GAMMA,
        vj2=1 + GAMMA,
        vivj=-2 * (1 + GAMMA),
        proj=sp.Rational(-3, 2),
        aj=sp.Rational(1, 2),
        s2i=2 + 2 * GAMMA,
        s2j=1 + 2 * GAMMA,
        s3=(3 + 4 * GAMMA) / 2,
    )


def _dot(p: Sequence[Expr], q: Sequence[Expr]) -> Expr:
    return sum((pi * qi for pi, qi in zip(p, q, strict=True)), sp.Integer(0))


def _sub(p: Vec, q: Vec) -> Vec:
    return (p[0] - q[0], p[1] - q[1], p[2] - q[2])


def _norm(p: Vec) -> Expr:
    return sp.sqrt(_dot(p, p))


def _potential(k: int, bodies: Sequence[Body]) -> Expr:
    """``sum_{l != k} GM_l / r_kl``."""
    return sum(
        (b.gm / _norm(_sub(bodies[k].r, b.r)) for n, b in enumerate(bodies) if n != k),
        sp.Integer(0),
    )


def eq27_correction(i: int, bodies: Sequence[Body], k: Coefficients) -> Vec:
    """Return the c^-2 coefficient of Eq. 27 for body ``i`` (the acceleration minus Newton)."""
    bi = bodies[i]
    out = [sp.Integer(0), sp.Integer(0), sp.Integer(0)]
    for j, bj in enumerate(bodies):
        if j == i:
            continue
        d_ji = _sub(bj.r, bi.r)
        d_ij = _sub(bi.r, bj.r)
        rij = _norm(d_ij)
        brace = (
            k.ui * _potential(i, bodies)
            + k.uj * _potential(j, bodies)
            + k.vi2 * _dot(bi.v, bi.v)
            + k.vj2 * _dot(bj.v, bj.v)
            + k.vivj * _dot(bi.v, bj.v)
            + k.proj * (_dot(d_ij, bj.v) / rij) ** 2
            + k.aj * _dot(d_ji, bj.a)
        )
        w = tuple(k.s2i * bi.v[c] - k.s2j * bj.v[c] for c in range(3))
        s2 = bj.gm / rij**3 * _dot(d_ij, w)
        for c in range(3):
            out[c] += (
                bj.gm * d_ji[c] / rij**3 * brace
                + s2 * (bi.v[c] - bj.v[c])
                + k.s3 * bj.gm * bj.a[c] / rij
            )
    return (out[0], out[1], out[2])


def schwarzschild_r3() -> Vec:
    """Return the R3 test-particle correction times c^2 (source at the origin, GM = mu)."""
    r = _norm(X)
    radial = 2 * (BETA + GAMMA) * MU / r - GAMMA * _dot(V, V)
    along = 2 * (1 + GAMMA) * _dot(X, V)
    return (
        MU / r**3 * (radial * X[0] + along * V[0]),
        MU / r**3 * (radial * X[1] + along * V[1]),
        MU / r**3 * (radial * X[2] + along * V[2]),
    )


def limit_pair() -> list[Body]:
    """Massless body 0 at ``X`` with velocity ``V``; static source 1 (GM = mu) at the origin."""
    return [Body(sp.Integer(0), X, V, ZERO), Body(MU, ZERO, ZERO, ZERO)]


def vectors_equal(p: Vec, q: Vec) -> bool:
    """Return True if every component of ``p - q`` simplifies to zero."""
    return all(sp.simplify(sp.expand(a - b)) == 0 for a, b in zip(p, q, strict=True))


MA, MB = sp.symbols("m_a m_b", positive=True)
RA: Vec = sp.symbols("xa ya za", real=True)
RB: Vec = sp.symbols("xb yb zb", real=True)
VA: Vec = sp.symbols("vxa vya vza", real=True)
VB: Vec = sp.symbols("vxb vyb vzb", real=True)
#: Random test points of ``check_lagrangian`` (deterministic) and the working precision.
LAGRANGIAN_POINTS = 3
LAGRANGIAN_DIGITS = 60


EPS = sp.Symbol("eps")


def _two_body() -> tuple[Expr, Expr, Vec, Vec]:
    """Return the EIH two-body ``eps`` Lagrangian, the energy and the Newtonian accelerations.

    Landau & Lifshitz Eq. 106.17 with G = 1. The energy is ``sum v.dL/dv - L`` of
    ``L = L0 + eps L1``, so it carries ``eps`` explicitly.
    """
    rv = _sub(RA, RB)
    r = _norm(rv)
    n = (rv[0] / r, rv[1] / r, rv[2] / r)
    va2, vb2 = _dot(VA, VA), _dot(VB, VB)
    l1 = (
        (MA * va2**2 + MB * vb2**2) / 8
        + MA * MB / (2 * r) * (3 * (va2 + vb2) - 7 * _dot(VA, VB) - _dot(VA, n) * _dot(VB, n))
        - MA * MB * (MA + MB) / (2 * r**2)
    )
    lag = MA * va2 / 2 + MB * vb2 / 2 + MA * MB / r + EPS * l1
    energy = sum((v * sp.diff(lag, v) for v in (*VA, *VB)), sp.Integer(0)) - lag
    an_a: Vec = (-MB * rv[0] / r**3, -MB * rv[1] / r**3, -MB * rv[2] / r**3)
    an_b: Vec = (MA * rv[0] / r**3, MA * rv[1] / r**3, MA * rv[2] / r**3)
    return l1, energy, an_a, an_b


def _ddt(f: Expr, acc_a: Sequence[Expr], acc_b: Sequence[Expr]) -> Expr:
    """Return the time derivative of ``f(r_a, r_b, v_a, v_b)`` along the given accelerations."""
    return sum(
        (
            sp.diff(f, RA[c]) * VA[c]
            + sp.diff(f, RB[c]) * VB[c]
            + sp.diff(f, VA[c]) * acc_a[c]
            + sp.diff(f, VB[c]) * acc_b[c]
            for c in range(3)
        ),
        sp.Integer(0),
    )


def _small(x: Expr, scale: Expr) -> bool:
    """Return True if ``|x|`` is below ``scale`` times 1e-50 (60-digit evaluation)."""
    tol = sp.Float(10, LAGRANGIAN_DIGITS) ** (10 - LAGRANGIAN_DIGITS)
    return bool(abs(sp.N(x, LAGRANGIAN_DIGITS)) <= (abs(sp.N(scale, LAGRANGIAN_DIGITS)) + 1) * tol)


def check_lagrangian(k: Coefficients | None = None) -> list[str]:
    """Compare Eq. 27 (beta = gamma = 1, two bodies) with the EIH Lagrangian; check its energy."""
    base = eq27() if k is None else k
    unit = {BETA: 1, GAMMA: 1}
    coeffs = Coefficients(
        **{f.name: sp.sympify(getattr(base, f.name)).subs(unit) for f in dataclasses.fields(base)}
    )
    l1, energy, an_a, an_b = _two_body()
    euler_lagrange = tuple(
        (sp.diff(l1, RA[c]) - _ddt(sp.diff(l1, VA[c]), an_a, an_b)) / MA for c in range(3)
    )
    bodies = [Body(MA, RA, VA, an_a), Body(MB, RB, VB, an_b)]
    delta_a = eq27_correction(0, bodies, coeffs)
    delta_b = eq27_correction(1, bodies, coeffs)
    full_a = tuple(an_a[c] + EPS * delta_a[c] for c in range(3))
    full_b = tuple(an_b[c] + EPS * delta_b[c] for c in range(3))
    de_dt = _ddt(energy, full_a, full_b)
    rng = random.Random(20261020)  # noqa: S311 (deterministic test points, not cryptography)
    fails: list[str] = []
    for trial in range(LAGRANGIAN_POINTS):
        point = {s: sp.Rational(rng.randint(-900, 900), 97) for s in (*RA, *RB, *VA, *VB)}
        point[MA] = sp.Rational(rng.randint(1, 50), 7)
        point[MB] = sp.Rational(rng.randint(1, 50), 11)
        for c in range(3):
            diff = (euler_lagrange[c] - delta_a[c]).subs(point)
            if not _small(diff, delta_a[c].subs(point)):
                fails.append(f"Lagrangian vs Eq. 27, point {trial}, component {c}: {sp.N(diff, 8)}")
        rate = sp.expand(de_dt.subs(point))
        scale = _ddt(energy, an_a, an_b).subs(point).subs(EPS, 0) + 1
        for order in (0, 1):
            coeff = rate.coeff(EPS, order)
            if not _small(coeff, scale):
                fails.append(
                    f"energy: dE/dt has an eps^{order} part at point {trial}: {sp.N(coeff, 8)}"
                )
    return fails


def check(k: Coefficients | None = None) -> list[str]:
    """Run the checks; return the failures (empty when all identities hold)."""
    coeffs = eq27() if k is None else k
    bodies = limit_pair()
    fails = []
    if not vectors_equal(eq27_correction(0, bodies, coeffs), schwarzschild_r3()):
        fails.append("R5 -> R3: the massless body's EIH correction is not the R3 correction")
    if not vectors_equal(eq27_correction(1, bodies, coeffs), ZERO):
        fails.append("R5 -> R3: the source receives a correction from a massless body")
    return fails


def main() -> int:
    """Print the results; return the process exit status."""
    fails = check() + check_lagrangian()
    for f in fails:
        print("FAIL", f)
    if not fails:
        print("R5 -> R3 limit: EIH (Park et al. 2021 Eq. 27) equals the R3 correction for")
        print("general beta, gamma; the static source receives no correction.")
        print("Two bodies, beta = gamma = 1: Eq. 27 equals the order-reduced Euler-Lagrange")
        print("equations of the EIH Lagrangian (Landau & Lifshitz 106.17), and its energy is")
        print("conserved at O(1/c^2).")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
