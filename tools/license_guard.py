# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Licence guard for the ``tools/`` uv environment (LIC-001, LIC-005, STD-009).

Fails when any distribution installed in the current environment (the one ``uv sync --locked``
built) declares a GPL, LGPL or AGPL licence. Installed distribution metadata is read with the
standard library only (``importlib.metadata``), so the guard needs no third-party package.

Exemptions (LIC-005): the GPL oracle container images under ``ci/oracles/`` are not part of this
environment and are never scanned; GPL oracle code lives only there. ``rebound`` and friends are
never exempt here. The only in-environment exemptions are the REUSE command-line tool and its
GPL-licensed dependency, used as a CLI (``reuse lint``) and never imported (STD-009). Each
exemption is listed in :data:`EXEMPT` with its reason.

Usage::

    uv run python license_guard.py          # CI gate; exit 1 on any finding
"""

from __future__ import annotations

import re
import sys
from collections.abc import Iterable
from importlib import metadata

# A copyleft licence the project forbids: GPL, LGPL, AGPL (any version), by SPDX id or by name.
COPYLEFT_RE = re.compile(
    r"\b(?:A|L)?GPL(?:[-\s]?v?\d(?:\.\d)?)?\b"
    r"|GNU\s+(?:Affero\s+|Lesser\s+|Library\s+)?General\s+Public\s+Licen[sc]e",
    re.IGNORECASE,
)

# Distribution names (PEP 503 normalised) exempt from the guard, each with its reason.
EXEMPT: dict[str, str] = {
    "reuse": "REUSE lint CLI (STD-009): run as a tool, never imported; parts are GPL-3.0-or-later.",
    "python-debian": "GPL-2.0-or-later dependency of `reuse`, reached only through that CLI.",
}

# GPL oracle packages that live only in ci/oracles/ images (LIC-001, LIC-005): never exemptable.
ORACLE_NAMES = frozenset({"rebound", "reboundx", "assist", "swiftest"})


def normalise(name: str) -> str:
    """Return the PEP 503 normalised distribution name."""
    return re.sub(r"[-_.]+", "-", name).lower()


def license_strings(md: metadata.PackageMetadata) -> list[str]:
    """Collect every licence declaration in one distribution's metadata."""
    out = [*(md.get_all("License-Expression") or []), *(md.get_all("License") or [])]
    out += [c for c in md.get_all("Classifier") or [] if c.startswith("License ::")]
    return [s for s in out if s]


def find_violations(
    dists: Iterable[metadata.Distribution],
    exempt: dict[str, str] | None = None,
) -> list[str]:
    """Return one finding per distribution that declares a GPL/LGPL/AGPL licence."""
    exempt = EXEMPT if exempt is None else exempt
    findings: list[str] = []
    for dist in dists:
        name = normalise(dist.metadata["Name"] or "")
        if name in ORACLE_NAMES:
            findings.append(f"{name}: GPL oracle package installed in tools/ (LIC-001, LIC-005)")
            continue
        hits = [s for s in license_strings(dist.metadata) if COPYLEFT_RE.search(s)]
        if hits and name not in exempt:
            findings.append(f"{name}: copyleft licence declared: {hits[0][:80]!r} (LIC-001)")
    return sorted(findings)


def main() -> int:
    """Scan the current environment; print findings and return the exit code."""
    findings = find_violations(metadata.distributions())
    for f in findings:
        print(f"license_guard: {f}", file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
