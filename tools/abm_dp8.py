# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""DP8 start-up coefficients of ``ferroastro-abm`` (NUM-002, WP24): verify, emit the Rust table.

The formula is the 12-stage, 8th-order Runge-Kutta formula of Dormand & Prince's DOP853, as
published in Hairer, Norsett & Wanner, *Solving Ordinary Differential Equations I* (2nd ed. 1993),
Sec. II.10, with the coefficient table of Hairer's ``dop853.f``. The 30-significant-digit decimals
below were transcribed from SciPy's BSD-3 ``scipy/integrate/_ivp/dop853_coefficients.py``
(``C[0:12]``, ``A[1:12, 0:12]`` and the weights ``B = A[12, 0:12]``), which reproduces that table.
No ephemeris or N-body integrator source was consulted (DEP-008).

The transcription is checked here, independently of any source, by the Butcher order conditions:
for every rooted tree ``t`` with at most 8 vertices (200 trees), ``sum_i b_i Phi_i(t) = 1/gamma(t)``
in 50-digit arithmetic, plus the row-sum conditions ``c_i = sum_j a_ij``. With 30-digit inputs the
residuals are ~1e-29; the check fails above 1e-25.

Usage::

    uv run python abm_dp8.py            # verify and print the residuals
    uv run python abm_dp8.py --write    # verify and (re)write the Rust table
    uv run python abm_dp8.py --check    # verify and fail if the committed table differs
"""

from __future__ import annotations

import argparse
import sys
from functools import cache
from pathlib import Path

import mpmath

ROOT = Path(__file__).resolve().parent.parent
TABLE = ROOT / "crates" / "ferroastro-abm" / "src" / "dp8_table.rs"
STAGES = 12
ORDER = 8
TOLERANCE = mpmath.mpf("1e-25")
DIGITS = 50

C: list[str] = [
    "0.0",
    "0.526001519587677318785587544488e-01",
    "0.789002279381515978178381316732e-01",
    "0.118350341907227396726757197510",
    "0.281649658092772603273242802490",
    "0.333333333333333333333333333333",
    "0.25",
    "0.307692307692307692307692307692",
    "0.651282051282051282051282051282",
    "0.6",
    "0.857142857142857142857142857142",
    "1.0",
]

# Row i: {j: a_ij}; rows 1..11 of A and row 12 = the 8th-order weights b.
A_ROWS: dict[int, dict[int, str]] = {
    1: {0: "5.26001519587677318785587544488e-2"},
    2: {0: "1.97250569845378994544595329183e-2", 1: "5.91751709536136983633785987549e-2"},
    3: {0: "2.95875854768068491816892993775e-2", 2: "8.87627564304205475450678981324e-2"},
    4: {
        0: "2.41365134159266685502369798665e-1",
        2: "-8.84549479328286085344864962717e-1",
        3: "9.24834003261792003115737966543e-1",
    },
    5: {
        0: "3.7037037037037037037037037037e-2",
        3: "1.70828608729473871279604482173e-1",
        4: "1.25467687566822425016691814123e-1",
    },
    6: {
        0: "3.7109375e-2",
        3: "1.70252211019544039314978060272e-1",
        4: "6.02165389804559606850219397283e-2",
        5: "-1.7578125e-2",
    },
    7: {
        0: "3.70920001185047927108779319836e-2",
        3: "1.70383925712239993810214054705e-1",
        4: "1.07262030446373284651809199168e-1",
        5: "-1.53194377486244017527936158236e-2",
        6: "8.27378916381402288758473766002e-3",
    },
    8: {
        0: "6.24110958716075717114429577812e-1",
        3: "-3.36089262944694129406857109825",
        4: "-8.68219346841726006818189891453e-1",
        5: "2.75920996994467083049415600797e1",
        6: "2.01540675504778934086186788979e1",
        7: "-4.34898841810699588477366255144e1",
    },
    9: {
        0: "4.77662536438264365890433908527e-1",
        3: "-2.48811461997166764192642586468",
        4: "-5.90290826836842996371446475743e-1",
        5: "2.12300514481811942347288949897e1",
        6: "1.52792336328824235832596922938e1",
        7: "-3.32882109689848629194453265587e1",
        8: "-2.03312017085086261358222928593e-2",
    },
    10: {
        0: "-9.3714243008598732571704021658e-1",
        3: "5.18637242884406370830023853209",
        4: "1.09143734899672957818500254654",
        5: "-8.14978701074692612513997267357",
        6: "-1.85200656599969598641566180701e1",
        7: "2.27394870993505042818970056734e1",
        8: "2.49360555267965238987089396762",
        9: "-3.0467644718982195003823669022",
    },
    11: {
        0: "2.27331014751653820792359768449",
        3: "-1.05344954667372501984066689879e1",
        4: "-2.00087205822486249909675718444",
        5: "-1.79589318631187989172765950534e1",
        6: "2.79488845294199600508499808837e1",
        7: "-2.85899827713502369474065508674",
        8: "-8.87285693353062954433549289258",
        9: "1.23605671757943030647266201528e1",
        10: "6.43392746015763530355970484046e-1",
    },
    12: {
        0: "5.42937341165687622380535766363e-2",
        5: "4.45031289275240888144113950566",
        6: "1.89151789931450038304281599044",
        7: "-5.8012039600105847814672114227",
        8: "3.1116436695781989440891606237e-1",
        9: "-1.52160949662516078556178806805e-1",
        10: "2.01365400804030348374776537501e-1",
        11: "4.47106157277725905176885569043e-2",
    },
}

Tree = tuple["Tree", ...]


def coefficients() -> tuple[list[mpmath.mpf], list[list[mpmath.mpf]], list[mpmath.mpf]]:
    """Return ``(c, A, b)`` as 50-digit numbers (``A`` strictly lower triangular, 12 x 12)."""
    mpmath.mp.dps = DIGITS
    c = [mpmath.mpf(s) for s in C]
    a = [[mpmath.mpf(0)] * STAGES for _ in range(STAGES)]
    for i in range(1, STAGES):
        for j, s in A_ROWS[i].items():
            a[i][j] = mpmath.mpf(s)
    b = [mpmath.mpf(0)] * STAGES
    for j, s in A_ROWS[STAGES].items():
        b[j] = mpmath.mpf(s)
    return c, a, b


@cache
def trees(order: int) -> list[Tree]:
    """All rooted trees with ``order`` vertices, each a sorted tuple of its subtrees."""
    if order == 1:
        return [()]
    out: list[Tree] = []
    # Multisets of subtrees with total order ``order - 1``, generated in non-increasing order.
    catalogue = [t for n in range(1, order) for t in trees(n)]
    index = {t: k for k, t in enumerate(catalogue)}

    def extend(remaining: int, max_index: int, acc: list[Tree]) -> None:
        if remaining == 0:
            out.append(tuple(sorted(acc, key=lambda t: index[t])))
            return
        for k in range(max_index, -1, -1):
            t = catalogue[k]
            n = size(t)
            if n <= remaining:
                extend(remaining - n, k, [*acc, t])

    extend(order - 1, len(catalogue) - 1, [])
    return sorted(set(out), key=repr)


def size(t: Tree) -> int:
    """Return the number of vertices."""
    return 1 + sum(size(s) for s in t)


def density(t: Tree) -> int:
    """Butcher's gamma(t) = |t| * prod gamma(subtrees)."""
    g = size(t)
    for s in t:
        g *= density(s)
    return g


def residuals() -> tuple[int, mpmath.mpf, mpmath.mpf]:
    """Check every order condition up to ``ORDER``; return (tree count, max residual, row-sum)."""
    c, a, b = coefficients()

    @cache
    def phi(t: Tree) -> tuple[mpmath.mpf, ...]:
        vals = [mpmath.mpf(1)] * STAGES
        for s in t:
            inner = phi(s)
            for i in range(STAGES):
                vals[i] *= mpmath.fsum(a[i][j] * inner[j] for j in range(STAGES))
        return tuple(vals)

    worst = mpmath.mpf(0)
    count = 0
    for n in range(1, ORDER + 1):
        for t in trees(n):
            count += 1
            lhs = mpmath.fsum(b[i] * phi(t)[i] for i in range(STAGES))
            worst = max(worst, abs(lhs - mpmath.mpf(1) / density(t)))
    rowsum = max(abs(mpmath.fsum(a[i]) - c[i]) for i in range(STAGES))
    return count, worst, rowsum


def split(x: mpmath.mpf) -> tuple[float, float]:
    """``(hi, lo)``: ``hi`` the nearest binary64 to ``x`` and ``lo`` the nearest to ``x - hi``."""
    hi = float(x)
    lo = float(x - mpmath.mpf(hi))
    return hi, lo


def rust_table() -> str:
    """Return the generated Rust module text."""
    c, a, b = coefficients()

    def pair(x: mpmath.mpf) -> str:
        hi, lo = split(x)
        return f"({hi!r}_f64, {lo!r}_f64)"

    lines = [
        "// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "// SPDX-License-Identifier: Apache-2.0",
        "// @generated by tools/abm_dp8.py --write; do not edit (check: abm_dp8.py --check).",
        "//! DP8 (DOP853 8th-order, 12-stage) coefficients as binary64 `(hi, lo)` pairs whose",
        "//! sum is the 30-digit published value (Hairer, Norsett & Wanner I, Sec. II.10;",
        "//! `dop853.f`). Every order condition up to order 8 was verified by the generator to",
        "//! <= 1e-25.",
        "",
        "/// A coefficient as an unevaluated binary64 pair `hi + lo`.",
        "pub(crate) type Pair = (f64, f64);",
        "",
        "/// Number of stages.",
        f"pub(crate) const STAGES: usize = {STAGES};",
        "",
        "/// Nodes `c_i`.",
        "pub(crate) const C: [Pair; STAGES] = [",
        *[f"    {pair(x)}," for x in c],
        "];",
        "",
        "/// Weights `b_i` of the 8th-order solution.",
        "pub(crate) const B: [Pair; STAGES] = [",
        *[f"    {pair(x)}," for x in b],
        "];",
        "",
        "/// Strictly lower-triangular `a_ij`, row `i` holding `j < i` (row 0 is empty).",
        "pub(crate) const A: [[Pair; STAGES]; STAGES] = [",
    ]
    for i in range(STAGES):
        lines.append("    [")
        lines.extend(f"        {pair(a[i][j])}," for j in range(STAGES))
        lines.append("    ],")
    lines.append("];")
    return "\n".join(lines) + "\n"


def main(argv: list[str] | None = None) -> int:
    """Verify the order conditions; optionally write or check the Rust table."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="write the Rust table")
    mode.add_argument("--check", action="store_true", help="fail if the Rust table differs")
    args = parser.parse_args(argv)
    count, worst, rowsum = residuals()
    print(
        f"DP8: {count} order conditions (order <= {ORDER}), max residual "
        f"{mpmath.nstr(worst, 3)}; row-sum residual {mpmath.nstr(rowsum, 3)}"
    )
    if worst > TOLERANCE or rowsum > TOLERANCE:
        print("DP8: order conditions FAIL", file=sys.stderr)
        return 1
    text = rust_table()
    if args.write:
        TABLE.write_text(text, encoding="utf-8")
        print(f"wrote {TABLE.relative_to(ROOT)}")
    elif args.check:
        if not TABLE.exists() or TABLE.read_text(encoding="utf-8") != text:
            print(f"{TABLE.relative_to(ROOT)} is out of date", file=sys.stderr)
            return 1
        print(f"{TABLE.relative_to(ROOT)} is up to date")
    return 0


if __name__ == "__main__":
    sys.exit(main())
