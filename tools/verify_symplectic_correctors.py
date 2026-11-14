# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Independent check of the WHFast symplectic-corrector coefficients (issue #43, NUM-004).

``tools/symplectic_correctors.py`` (WP29) derived the corrector coefficients in-house through
the closed form ``(phi(x) - 1)/x`` and Bernoulli numbers. This script checks them by a separate
route and against the published tables. It reuses none of that script's code or arithmetic.

1. **First-order algebra, exact.** To first order in ``eps``, an element ``e^{tA}(1 + eps f B)``,
   where ``f`` is a power series in ``x = ad_A``, is a pair ``(t, f)``. The product is
   ``(t1, f1)(t2, f2) = (t1 + t2, e^{-t2 x} f1 + f2)``, because
   ``e^{-tA} M e^{tA} = e^{-t ad_A} M``. In this algebra:
   - the exact flow ``e^{h(A + eps B)}`` is ``(h, (1 - e^{-hx})/x)``;
   - the Wisdom-Holman step ``A(h/2) B(h) A(h/2)`` is a product of three pairs;
   - a corrector ``C = (0, g)`` that satisfies ``C W C^-1 = flow`` has
     ``g = (w - F)/(1 - e^{-hx})``, solved by power-series division.

   Every corrector, FerroAstro's and Wisdom's, is composed here literally from its stage list,
   ``X(a, b) = A(a) B(b) A(-a)`` and ``Z(a, b) = X(a, b) X(-a, -b)``. No closed form is used.
   An ``n``-pair corrector has order ``2n + 1`` (Wisdom's naming: the remaining error is
   ``O(eps (h ad_A)^{2n+1})``) when ``g_C`` matches ``s g`` through ``x^{2n-1}``.
   ``s = +1`` or ``-1`` is one global sign, fixed by the convention for the direction of
   conjugation. In FerroAstro that direction is fixed by test W3.
2. **Exact solve.** For given stage positions, solve the matching conditions for the ``b_i``
   in the field ``Q(alpha)``, with ``alpha^2 = 7/40``.
3. **Published tables.** Wisdom (2006, AJ 131, 2294, Appendix eqs. 29-33; PDF from
   web.mit.edu/wisdom/www/correctors2006.pdf, sha256 ``f2846ae6...b67eb``, fetched 2026-11-13)
   gives correctors of order 3, 5, 7, 11 and the corrected Wisdom, Holman & Touma (1996)
   order-17 corrector. Those stages sit at ``a = k alpha h``, ``alpha = (7/40)^{1/2}``, with
   ``b = r beta h``, ``beta = 1/(48 alpha)``, plus mirrored stages ``a_i = -a_{N+1-i}``,
   ``b_i = -b_{N+1-i}``. FerroAstro instead puts its stages at ``a = k h``, which is
   ``alpha = 1``. First-order correctors with different stage positions are equivalent but
   have different coefficients. The comparison therefore checks two things: that each
   published table meets the same conditions, and that the solve, run with Wisdom's positions,
   gives Wisdom's numbers.

REBOUND source was not consulted (AGENTS.md rule 7, DEP-008); the published papers are the
independent reference.

Usage::

    uv run python verify_symplectic_correctors.py
"""

from __future__ import annotations

import re
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from fractions import Fraction
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "crates" / "ferroastro-symplectic" / "src" / "corrector_table.rs"
#: ``alpha^2`` of Wisdom, Holman & Touma (1996) and Wisdom (2006).
ALPHA_SQ = Fraction(7, 40)


@dataclass(frozen=True)
class QA:
    """An element ``p + q alpha`` of ``Q(alpha)``, ``alpha^2 = 7/40`` (exact)."""

    p: Fraction = Fraction(0)
    q: Fraction = Fraction(0)

    @staticmethod
    def of(v: int | Fraction) -> QA:
        """Embed a rational."""
        return QA(Fraction(v))

    def __add__(self, o: QA) -> QA:
        """Sum."""
        return QA(self.p + o.p, self.q + o.q)

    def __sub__(self, o: QA) -> QA:
        """Difference."""
        return QA(self.p - o.p, self.q - o.q)

    def __neg__(self) -> QA:
        """Negation."""
        return QA(-self.p, -self.q)

    def __mul__(self, o: QA) -> QA:
        """Product."""
        return QA(self.p * o.p + self.q * o.q * ALPHA_SQ, self.p * o.q + self.q * o.p)

    def inv(self) -> QA:
        """Inverse (``alpha`` is irrational, so the norm vanishes only at zero)."""
        norm = self.p * self.p - self.q * self.q * ALPHA_SQ
        if norm == 0:
            raise ZeroDivisionError("QA inverse of zero")
        return QA(self.p / norm, -self.q / norm)

    def __truediv__(self, o: QA) -> QA:
        """Quotient."""
        return self * o.inv()

    def is_zero(self) -> bool:
        """Exactly zero."""
        return self.p == 0 and self.q == 0


ZERO = QA()
ONE = QA.of(1)
ALPHA = QA(Fraction(0), Fraction(1))

#: A truncated power series in ``x`` (coefficients of ``x^0 .. x^{N-1}``).
Series = list[QA]


def s_exp(t: QA, n: int) -> Series:
    """``e^{t x}``."""
    out = [ONE]
    for m in range(1, n):
        out.append(out[-1] * t / QA.of(m))
    return out


def s_add(f: Series, g: Series) -> Series:
    """``f + g``."""
    return [a + b for a, b in zip(f, g, strict=True)]


def s_scale(c: QA, f: Series) -> Series:
    """``c f``."""
    return [c * a for a in f]


def s_mul(f: Series, g: Series) -> Series:
    """``f g`` truncated."""
    n = len(f)
    out = [ZERO] * n
    for i, a in enumerate(f):
        if a.is_zero():
            continue
        for j in range(n - i):
            out[i + j] = out[i + j] + a * g[j]
    return out


def s_div(f: Series, g: Series) -> Series:
    """``f / g`` for ``g_0 != 0``, truncated."""
    n = len(f)
    out: Series = []
    for k in range(n):
        acc = f[k]
        for j in range(k):
            acc = acc - out[j] * g[k - j]
        out.append(acc / g[0])
    return out


@dataclass(frozen=True)
class Pair:
    """``e^{tA}(1 + eps f(ad_A) B)`` to first order in ``eps``."""

    t: QA
    f: Series

    def __mul__(self, o: Pair) -> Pair:
        """Group product to first order in ``eps``."""
        n = len(self.f)
        return Pair(self.t + o.t, s_add(s_mul(s_exp(-o.t, n), self.f), o.f))


def drift(t: QA, n: int) -> Pair:
    """``A(t) = e^{tA}``."""
    return Pair(t, [ZERO] * n)


def kick(b: QA, n: int) -> Pair:
    """``B(b) = e^{eps b B}``."""
    return Pair(ZERO, [b] + [ZERO] * (n - 1))


def stage_z(a: QA, b: QA, n: int) -> Pair:
    """``Z(a, b) = X(a, b) X(-a, -b)``, ``X(a, b) = A(a) B(b) A(-a)``."""
    x1 = drift(a, n) * kick(b, n) * drift(-a, n)
    x2 = drift(-a, n) * kick(-b, n) * drift(a, n)
    return x1 * x2


def corrector_series(stages: Sequence[tuple[QA, QA]], n: int) -> Series:
    """``g_C`` of ``C = Z(a_1, b_1) Z(a_2, b_2) ...``, composed literally (``h = 1``)."""
    c = Pair(ZERO, [ZERO] * n)
    for a, b in stages:
        c = c * stage_z(a, b, n)
    if not c.t.is_zero():
        raise ValueError("corrector has a net drift")
    return c.f


def target_series(n: int) -> Series:
    """``g`` with ``C W C^-1 = e^{A + eps B}`` for ``W = A(1/2) B(1) A(1/2)`` (``h = 1``)."""
    half = QA.of(Fraction(1, 2))
    w = (drift(half, n + 1) * kick(ONE, n + 1) * drift(half, n + 1)).f
    # (1 - e^{-x})/x: the first-order part of the exact flow.
    e = s_exp(-ONE, n + 2)
    flow = [-e[m + 1] for m in range(n + 1)]
    # (e^{-x} - 1) g = flow - w; both sides vanish at x^0, so divide after shifting by x.
    num = s_add(w, s_scale(-ONE, flow))
    den = [-e[m] for m in range(1, n + 2)]  # (1 - e^{-x})/x
    if not num[0].is_zero():
        raise ValueError("Wisdom-Holman step and flow differ at zeroth order")
    return s_div(num[1:], den)[:n]


def match_order(g_c: Series, g: Series) -> tuple[int, int]:
    """Global sign ``s`` and the first power ``x^m`` where ``g_C != s g``.

    ``m`` is the corrector order in Wisdom's naming (3 for one stage pair); ``m = 1`` means
    that not even the lowest condition holds, so the "corrector" corrects nothing.
    """
    sign = 1 if g_c[1] == g[1] else -1
    for m in range(len(g)):
        if not (g_c[m] - s_scale(QA.of(sign), [g[m]])[0]).is_zero():
            return sign, m
    raise ValueError("series too short to find the mismatch")


def solve(build: Callable[[Sequence[QA]], list[tuple[QA, QA]]], n_unk: int, sign: int) -> list[QA]:
    """Solve ``g_C(build(c))`` = ``sign * g`` at ``x^1, x^3, ... x^{2 n_unk - 1}`` for ``c``.

    ``g_C`` is linear in ``c``, so the columns come from unit vectors. Gauss-Jordan in ``Q(alpha)``.
    """
    nser = 2 * n_unk + 1
    g = target_series(nser)
    cols = []
    for k in range(n_unk):
        unit = [ONE if j == k else ZERO for j in range(n_unk)]
        cols.append(corrector_series(build(unit), nser))
    rows = [
        [cols[k][2 * j + 1] for k in range(n_unk)] + [QA.of(sign) * g[2 * j + 1]]
        for j in range(n_unk)
    ]
    for c in range(n_unk):
        p = next(r for r in range(c, n_unk) if not rows[r][c].is_zero())
        rows[c], rows[p] = rows[p], rows[c]
        piv = rows[c][c]
        rows[c] = [v / piv for v in rows[c]]
        for r in range(n_unk):
            if r != c and not rows[r][c].is_zero():
                f = rows[r][c]
                rows[r] = [a - f * b for a, b in zip(rows[r], rows[c], strict=True)]
    return [rows[i][n_unk] for i in range(n_unk)]


# --- FerroAstro (alpha = 1): C = prod_i Z(i h, beta_i h), i = 1 .. n -------------------------


def ferro_stages(beta: Sequence[QA]) -> list[tuple[QA, QA]]:
    """Stage list of the FerroAstro corrector (``wh.rs``: ``a_i = i h``, ``b_i = beta_i h``)."""
    return [(QA.of(i + 1), b) for i, b in enumerate(beta)]


def rust_table(path: Path = TABLE) -> dict[int, list[Fraction]]:
    """Parse ``corrector_table.rs`` (order -> beta_i), independently of the generator."""
    text = path.read_text(encoding="utf-8")
    out: dict[int, list[Fraction]] = {}
    for m in re.finditer(r"ORDER_(\d+): \[\(i64, i64\); \d+\] = \[(.*?)\];", text):
        pairs = re.findall(r"\((-?\d+), (\d+)\)", m.group(2))
        out[int(m.group(1))] = [Fraction(int(a), int(b)) for a, b in pairs]
    return out


# --- Wisdom (2006) Appendix: a = k alpha, b = r beta, beta = 1/(48 alpha) --------------------

#: Published ``(k, r)`` for the first half of the stages, in printed order ``i = 1 .. N/2``
#: (Wisdom 2006, eqs. 29-33). The second half is ``a_i = -a_{N+1-i}``, ``b_i = -b_{N+1-i}``.
#: Order 17 is the corrected WHT96 table, printed with ``b_1``'s numerator positive but its
#: decimal ``~ -0.0000870`` negative. The value below takes the sign of the decimal, and
#: ``PUBLISHED_17_B1_AS_PRINTED`` keeps the printed numerator so both can be tested.
PUBLISHED: dict[int, list[tuple[int, Fraction]]] = {
    3: [(-1, Fraction(-1, 2))],
    5: [(2, Fraction(-1, 6)), (1, Fraction(5, 6))],
    7: [(3, Fraction(12361, 246960)), (2, Fraction(-22651, 61740)), (1, Fraction(53521, 49392))],
    11: [
        (5, Fraction(2798927, 684573120)),
        (4, Fraction(-329447, 6985440)),
        (3, Fraction(895249, 3622080)),
        (2, Fraction(-14556229, 19015920)),
        (1, Fraction(3394141, 2328480)),
    ],
    17: [
        (8, Fraction(-682938344463443, 7846175667762432000)),
        (7, Fraction(26880679644439, 17513784972684000)),
        (6, Fraction(-33737961615779, 2641809989145600)),
        (5, Fraction(102433989269, 1539673404192)),
        (4, Fraction(-27170077124018711, 112088223825177600)),
        (3, Fraction(422297952838709, 648658702692000)),
        (2, Fraction(-104807478104929387, 80063017017984000)),
        (1, Fraction(45815578591785473, 24519298961757600)),
    ],
}
#: ``b_1`` of the order-17 table with the numerator sign exactly as printed.
PUBLISHED_17_B1_AS_PRINTED = Fraction(682938344463443, 7846175667762432000)
#: ``beta = 1/(48 alpha)``.
BETA = ONE / (QA.of(48) * ALPHA)


def wisdom_stages(half: Sequence[tuple[int, QA]]) -> list[tuple[QA, QA]]:
    """Full Wisdom stage list ``(a_i, b_i)``, ``i = 1 .. N``, from the first half (``b`` given)."""
    first = [(QA.of(k) * ALPHA, b) for k, b in half]
    mirror = [(-a, -b) for a, b in reversed(first)]
    return first + mirror


def published_stages(order: int, r: Sequence[Fraction] | None = None) -> list[tuple[QA, QA]]:
    """Stage list of a published corrector, optionally with the ``r`` replaced."""
    ks = [k for k, _ in PUBLISHED[order]]
    rs = list(r) if r is not None else [v for _, v in PUBLISHED[order]]
    return wisdom_stages([(k, QA.of(v) * BETA) for k, v in zip(ks, rs, strict=True)])


def solve_wisdom_r(order: int, sign: int) -> list[Fraction]:
    """Solve for the ``r`` of Wisdom's structure (his ``a = k alpha``); rational by construction."""
    ks = [k for k, _ in PUBLISHED[order]]

    def build(c: Sequence[QA]) -> list[tuple[QA, QA]]:
        return wisdom_stages([(k, ci * BETA) for k, ci in zip(ks, c, strict=True)])

    sol = solve(build, len(ks), sign)
    if any(v.q != 0 for v in sol):
        raise ValueError(f"order {order}: r not rational")
    return [v.p for v in sol]


def main() -> int:
    """Print the comparison."""
    n = 2 * 8 + 4
    g = target_series(n)
    print("g(x) odd coefficients:", [str(g[m].p) for m in range(1, 9, 2)])
    table = rust_table()
    for order, beta in sorted(table.items()):
        s, m = match_order(corrector_series(ferro_stages([QA.of(b) for b in beta]), n), g)
        ind = solve(ferro_stages, len(beta), s)
        same = [v.p for v in ind] == beta and all(v.q == 0 for v in ind)
        print(f"FerroAstro order {order}: sign {s:+d}, order {m}, independent solve equal: {same}")
    for order in sorted(PUBLISHED):
        s, m = match_order(corrector_series(published_stages(order), n), g)
        same = solve_wisdom_r(order, s) == [v for _, v in PUBLISHED[order]]
        print(f"Wisdom 2006 order {order}: sign {s:+d}, order {m}, solve reproduces r: {same}")
    rs = [v for _, v in PUBLISHED[17]]
    rs[0] = PUBLISHED_17_B1_AS_PRINTED
    s, m = match_order(corrector_series(published_stages(17, rs), n), g)
    print(f"Wisdom 2006 order 17, b_1 as printed: sign {s:+d}, order {m}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
