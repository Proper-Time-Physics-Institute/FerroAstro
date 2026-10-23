# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Shared pieces of the N-body oracle runners (WP17): tool settings, output times, state CSVs.

A state CSV is the format of ``ferroastro_oracle::GoldenStates``: ``#`` header lines
(``key: value``), the column line :data:`STATE_COLUMNS` and one row per body and output time.
``center_naif`` in the header names the origin of the states: ``0`` for the solar-system
barycentre (stage A REBOUND), ``10`` for the Sun (heliocentric tools: Swiftest, Mercury6, Orekit).
Numbers are shortest round-trip decimals (``repr``), so a reader recovers the binary64 values.
"""

from __future__ import annotations

import tomllib
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SECONDS_PER_DAY = 86400.0
DAYS_PER_JULIAN_YEAR = 365.25
STATE_COLUMNS = ("t_days", "naif", "x_km", "y_km", "z_km", "vx_km_s", "vy_km_s", "vz_km_s")
#: Scenario integrator options that configure IAS15 only. A tool with its own integrator (mapped
#: through its settings file) records them as not applicable instead of honouring them.
IAS15_ONLY_OPTIONS = ("epsilon", "pc_tolerance", "dt0_days")


def fmt(x: float) -> str:
    """Shortest round-trip decimal of a binary64 (``repr``)."""
    return repr(float(x))


def load_settings(path: Path, scenario_name: str) -> dict[str, Any]:
    """Return the tool settings for a scenario: ``[default]`` overlaid by ``[scenario.<name>]``."""
    doc = tomllib.loads(path.read_text(encoding="utf-8"))
    out: dict[str, Any] = dict(doc.get("default", {}))
    out.update(doc.get("scenario", {}).get(scenario_name, {}))
    return out


def span_days(scenario: Mapping[str, Any]) -> float:
    """Return the scenario span in TDB days (Julian years of 365.25 d)."""
    return float(scenario["span"]["years"]) * DAYS_PER_JULIAN_YEAR


def steps_per(interval_days: float, dt_days: float) -> int | None:
    """``interval / dt`` when it is a positive integer (exactly, in binary64), else ``None``."""
    if dt_days <= 0.0 or interval_days <= 0.0:
        return None
    k = interval_days / dt_days
    n = round(k)
    return n if n >= 1 and k == n else None


def mapped_integrator_note(scenario: Mapping[str, Any], settings: Mapping[str, Any]) -> str:
    """Manifest note: what the scenario's ``[integrator]`` asked for and what the tool ran."""
    integ = scenario.get("integrator", {})
    ignored = [f"{k} = {integ[k]!r}" for k in IAS15_ONLY_OPTIONS if k in integ]
    ran = ", ".join(f"{k} = {v!r}" for k, v in sorted(settings.items()))
    return (
        f"scenario integrator {integ.get('name')!r} ({integ.get('precision')}) mapped to the "
        f"tool settings [{ran}]"
        + (f"; IAS15-only options not applicable: {', '.join(ignored)}" if ignored else "")
    )


@dataclass(frozen=True)
class StatesHeader:
    """The ``#`` header of a state CSV."""

    title: str
    engine: str
    header_sha256: str
    epoch_jd_tdb: float
    au_km: float
    center_naif: int
    frame_note: str


def write_states_csv(path: Path, head: StatesHeader, rows: Sequence[Sequence[str]]) -> None:
    """Write a state CSV (licence header lines first, as every committed golden has)."""
    # REUSE-IgnoreStart (licence lines written into the CSV, not this file's licence)
    lines = [
        "# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "# SPDX-License-Identifier: Apache-2.0",
        # REUSE-IgnoreEnd
        f"# {head.title}",
        f"# engine: {head.engine}",
        f"# header_sha256: {head.header_sha256}",
        f"# epoch_jd_tdb: {fmt(head.epoch_jd_tdb)}",
        f"# au_km: {fmt(head.au_km)}",
        f"# center_naif: {head.center_naif}",
        f"# units: t_days TDB days since epoch; position km; velocity km/s; {head.frame_note}",
        ",".join(STATE_COLUMNS),
    ]
    path.write_text("\n".join(lines + [",".join(r) for r in rows]) + "\n", encoding="utf-8")


def state_row(
    t_days: float, naif: int, r_au: Sequence[float], v_au_d: Sequence[float], au_km: float
) -> list[str]:
    """One CSV row from au and au/day (converted with the header ``AU`` and 86400 s/day)."""
    vel = au_km / SECONDS_PER_DAY
    return [
        fmt(t_days),
        str(naif),
        fmt(r_au[0] * au_km),
        fmt(r_au[1] * au_km),
        fmt(r_au[2] * au_km),
        fmt(v_au_d[0] * vel),
        fmt(v_au_d[1] * vel),
        fmt(v_au_d[2] * vel),
    ]
