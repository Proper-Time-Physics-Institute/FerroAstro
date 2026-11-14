# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""An independent reader of JPL's ASCII ``header.440`` for the oracle runners.

It shares no code with FerroAstro (the runners must not contain a FerroAstro build, red-team
L4). It reads GROUP 1040 (names) and GROUP 1041 (values, Fortran ``D`` exponents) and assembles
the SSB-centred initial conditions under the interpretation FerroAstro documents as
``HEADER_STATE_INTERPRETATION`` (OI-23): ``CENTER = 0``; ``X/Y/Z`` in au and ``XD/YD/ZD`` in au/day
at ``JDEPOC`` (TDB), ICRF-aligned axes; suffixes ``1, 2, 4…9`` are planetary-system barycentres,
``B`` the Earth-Moon barycentre, ``S`` the Sun and ``M`` the Moon relative to the Earth.

As a command it prints the constants it parsed as JSON (shortest round-trip decimal and binary64
bits), the format of ``ferroastro-constants``' ``header_constants_json`` example. That is oracle (a)
of the live cross-check ``SYSTEM-HEADER-LIVE-CROSSCHECK-MAX-ULP`` (#22; compared by
:mod:`oracles.header_crosscheck`); the output is never committed::

    uv run python -m oracles.header440 --header H.440 --names-from FERROASTRO.json
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from collections.abc import Iterable, Sequence
from dataclasses import dataclass
from pathlib import Path

#: Scenario body name -> (header state suffix, GM symbol). Earth and Moon are split from ``B``
#: and ``M`` with ``EMRAT`` (see :meth:`Header440.state`).
BODIES: dict[str, tuple[str, str]] = {
    "Sun": ("S", "GMS"),
    "Mercury": ("1", "GM1"),
    "Venus": ("2", "GM2"),
    "EMB": ("B", "GMB"),
    "Mars": ("4", "GM4"),
    "Jupiter": ("5", "GM5"),
    "Saturn": ("6", "GM6"),
    "Uranus": ("7", "GM7"),
    "Neptune": ("8", "GM8"),
    "Pluto": ("9", "GM9"),
}

#: Scenario body name -> NAIF id (as ``ferroastro_scenario::BodyName::naif``).
NAIF: dict[str, int] = {
    "Sun": 10,
    "Mercury": 1,
    "Venus": 2,
    "EMB": 3,
    "Earth": 399,
    "Moon": 301,
    "Mars": 4,
    "Jupiter": 5,
    "Saturn": 6,
    "Uranus": 7,
    "Neptune": 8,
    "Pluto": 9,
}

Vec3 = tuple[float, float, float]


def fortran_real(token: str) -> float:
    """Parse a Fortran real (``0.1D+01``), correctly rounded."""
    return float(token.replace("D", "E").replace("d", "e"))


@dataclass(frozen=True)
class Header440:
    """The constants of a ``header.440`` file, by name (GROUP 1040 order)."""

    names: tuple[str, ...]
    values: dict[str, float]

    @classmethod
    def parse(cls, text: str) -> Header440:
        """Parse the ASCII header text."""
        tokens = text.split()
        try:
            i40 = next(i for i in range(len(tokens) - 1) if tokens[i : i + 2] == ["GROUP", "1040"])
            i41 = next(i for i in range(len(tokens) - 1) if tokens[i : i + 2] == ["GROUP", "1041"])
        except StopIteration as exc:
            msg = "header.440: GROUP 1040 or GROUP 1041 missing"
            raise ValueError(msg) from exc
        n = int(tokens[i40 + 2])
        names = tuple(tokens[i40 + 3 : i40 + 3 + n])
        count = int(tokens[i41 + 2])
        raw = tokens[i41 + 3 : i41 + 3 + count]
        if count != n or len(names) != n or len(raw) != n:
            msg = f"header.440: {n} names but {count} values ({len(raw)} read)"
            raise ValueError(msg)
        return cls(names, {k: fortran_real(v) for k, v in zip(names, raw, strict=True)})

    @classmethod
    def read(cls, path: Path) -> Header440:
        """Read and parse a file."""
        return cls.parse(path.read_text(encoding="utf-8"))

    def value(self, name: str) -> float:
        """Return the value of a constant (``KeyError`` naming it when absent)."""
        try:
            return self.values[name]
        except KeyError as exc:
            msg = f"header.440 has no constant {name!r}"
            raise KeyError(msg) from exc

    def block(self, suffix: str) -> tuple[Vec3, Vec3]:
        """Position (au) and velocity (au/day) of state block ``suffix``."""
        p = (self.value(f"X{suffix}"), self.value(f"Y{suffix}"), self.value(f"Z{suffix}"))
        v = (self.value(f"XD{suffix}"), self.value(f"YD{suffix}"), self.value(f"ZD{suffix}"))
        return p, v

    def state(self, body: str) -> tuple[float, Vec3, Vec3]:
        """``(GM au^3/d^2, position au, velocity au/day)`` of a scenario body, SSB-centred."""
        if self.value("CENTER") != 0.0:
            msg = "header.440 CENTER is not 0; the documented interpretation does not apply"
            raise ValueError(msg)
        if body in BODIES:
            suffix, gm = BODIES[body]
            p, v = self.block(suffix)
            return self.value(gm), p, v
        if body in ("Earth", "Moon"):
            emrat = self.value("EMRAT")
            gmb = self.value("GMB")
            bp, bv = self.block("B")
            mp, mv = self.block("M")  # Moon relative to the Earth
            if body == "Earth":
                w = 1.0 / (1.0 + emrat)
                return (
                    gmb * emrat / (1.0 + emrat),
                    (bp[0] - w * mp[0], bp[1] - w * mp[1], bp[2] - w * mp[2]),
                    (bv[0] - w * mv[0], bv[1] - w * mv[1], bv[2] - w * mv[2]),
                )
            w = emrat / (1.0 + emrat)
            return (
                gmb / (1.0 + emrat),
                (bp[0] + w * mp[0], bp[1] + w * mp[1], bp[2] + w * mp[2]),
                (bv[0] + w * mv[0], bv[1] + w * mv[1], bv[2] + w * mv[2]),
            )
        msg = f"unknown scenario body {body!r}"
        raise KeyError(msg)


def bits_hex(value: float) -> str:
    """Return the binary64 bits of ``value`` as 16 lowercase hex digits."""
    return struct.pack(">d", value).hex()


def constants_json(header: Header440, names: Iterable[str], source: str) -> str:
    """JSON of the named constants (``repr`` and binary64 bits), in the order given."""
    doc = {
        "reader": "tools/oracles/header440.py (independent Python reader)",
        "source": source,
        "constants": {
            n: {"repr": repr(header.value(n)), "bits": bits_hex(header.value(n))} for n in names
        },
    }
    return json.dumps(doc, indent=2) + "\n"


def main(argv: Sequence[str] | None = None) -> int:
    """Print the constants named in ``--names`` / ``--names-from`` (default: all) as JSON."""
    ap = argparse.ArgumentParser(description="Print header.440 constants as JSON (oracle a).")
    ap.add_argument("--header", type=Path, required=True)
    ap.add_argument("--names", default="", help="comma-separated GROUP 1040 names")
    ap.add_argument(
        "--names-from", type=Path, help="a header_constants_json document whose names to print"
    )
    args = ap.parse_args(argv)
    header = Header440.read(args.header)
    names: list[str] = [n for n in args.names.split(",") if n]
    if args.names_from is not None:
        names += list(json.loads(args.names_from.read_text(encoding="utf-8"))["constants"])
    try:
        text = constants_json(header, names or header.names, str(args.header))
    except KeyError as e:
        print(f"header440: {e}", file=sys.stderr)
        return 1
    sys.stdout.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
