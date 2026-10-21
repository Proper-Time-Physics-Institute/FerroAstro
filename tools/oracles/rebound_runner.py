# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""REBOUND/REBOUNDx oracle runner (REF-005, stages A and C; REF-011 refusals).

Runs inside ``ci/oracles/rebound`` only. REBOUND 5.2.2 and REBOUNDx 5.1.0 are GPL-3.0: they are
used as black boxes through their documented Python API (``Simulation``, ``add``, ``integrate``,
``particles``, ``integrator.epsilon`` / ``adaptive_mode`` (REBOUND 5 changelog: the ``ri_*``
structs are gone), ``energy``) and their source is never read (DEP-008). The container
receives a scenario and ``header.440`` and returns plain files (LIC-001); it never contains a
FerroAstro build (red-team L4).

Implemented (everything else is refused by name, exit status 3):

* ``mode = "n-body"`` with ``[forces] newton = true`` (stage A), plus (stage C, WP21) ``gr_eih``,
  mapped to the REBOUNDx effect ``gr_full`` ("first-order post-newtonian effects from all bodies",
  Newhall et al. 1983), and ``gr_1pn_central``, mapped to ``gr`` (single dominant mass,
  Anderson et al. 1975; the Sun must be ``particles[0]``). A ``gr_1pn_central`` scenario also
  yields a second series with ``gr_potential`` (Nobili & Roxburgh 1986), written next to the
  primary one as ``<scenario>.rebound.gr_potential.states.csv`` with its own manifest entry. The
  speed of light is ``c = CLIGHT * 86400 / AU`` au/day from header.440, and the runner refuses a
  header whose BETA or GAMMA is not 1. Only the documented API is used: ``reboundx.Extras``,
  ``load_force``, ``add_force`` and ``params["c"]`` (REBOUNDx docs, "Astrophysical Effects" and
  the Python quick-start);
* ``ppn{beta,gamma}`` is refused permanently: REBOUNDx ``gr_full`` has only the parameter ``c``
  (beta = gamma = 1, REF-005 notes; beta, gamma != 1 is validated analytically, REF-010).
  ``lense_thirring``, ``sun_j2`` and the small-body switches are refused, and so is test-particle
  mode with any switch;
* ``initial_conditions.source = "de440-header"`` (the runner reads ``header.440`` itself);
* ``integrator.name = "ias15"`` in ``f64`` with ``epsilon`` and ``dt0_days``;
* ``output.cadence_days`` samples (exact output times) and ``output.sample_bodies``.

Units inside REBOUND: G = 1, masses are GM in au^3/d^2, lengths au, time days (TDB). The output
CSV converts with the header ``AU`` (km) and 86400 s/day: ``t_days`` (TDB days since ``JDEPOC``),
``naif``, position (km) and velocity (km/s), SSB-centred, ICRF axes.

Usage (inside the container; the driver is ``tools/oracles/run_oracles.sh``)::

    python -m oracles.rebound_runner --scenario S.toml --header header.440 --out-dir OUT
"""

from __future__ import annotations

import argparse
import math
import sys
import time
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from oracles import common
from oracles.header440 import BODIES, NAIF, Header440

TOOL = "rebound"
#: Switches this runner implements (stage A: newton; stage C: the two relativistic switches).
SUPPORTED_SWITCHES = frozenset({"newton", "gr_eih", "gr_1pn_central"})
#: REBOUNDx effect of each relativistic switch (primary series), and the extra variants per switch.
REBOUNDX_EFFECT: dict[str, str] = {"gr_eih": "gr_full", "gr_1pn_central": "gr"}
EXTRA_VARIANTS: dict[str, tuple[str, ...]] = {"gr_eih": (), "gr_1pn_central": ("gr_potential",)}
SECONDS_PER_DAY = 86400.0
DAYS_PER_JULIAN_YEAR = 365.25
STATE_COLUMNS = ("t_days", "naif", "x_km", "y_km", "z_km", "vx_km_s", "vy_km_s", "vz_km_s")


def unsupported_options(s: Mapping[str, Any]) -> list[tuple[str, bool, str]]:
    """``(item, supported, reason)`` for the non-switch options of a scenario."""
    ic = s.get("initial_conditions", {})
    integ = s.get("integrator", {})
    span = s.get("span", {})
    out = s.get("output", {})
    known_bodies = set(BODIES) | {"Earth", "Moon"}
    bodies = [str(b) for b in s.get("bodies", [])]
    forces = s.get("forces", {})
    central_gr = forces.get("gr_1pn_central") is True
    return [
        (
            "forces.gr_1pn_central",
            not (central_gr and forces.get("gr_eih") is True),
            "gr_eih already contains the Sun's 1PN field; REBOUNDx gr and gr_full are not combined",
        ),
        (
            "forces.gr_1pn_central",
            not central_gr or (bodies[:1] == ["Sun"] and s.get("central_body") == "Sun"),
            "REBOUNDx gr treats particles[0] as the dominant mass: the Sun must be the first body "
            "and the central body",
        ),
        (
            "forces.newton",
            forces.get("newton") is True,
            "REBOUND always applies Newtonian point-mass gravity; newton must be true",
        ),
        ("mode", s.get("mode") == "n-body", "only n-body mode is implemented"),
        (
            "initial_conditions.source",
            ic.get("source") == "de440-header",
            "only de440-header initial conditions are implemented (no SPK or Horizons input)",
        ),
        ("constants.gm_set", s.get("constants", {}).get("gm_set") == "de440", "DE440 GMs only"),
        ("integrator.name", integ.get("name") == "ias15", "only REBOUND's IAS15 is wired"),
        ("integrator.precision", integ.get("precision") == "f64", "REBOUND integrates in f64"),
        (
            "integrator.pc_tolerance",
            "pc_tolerance" not in integ,
            "REBOUND's documented API has no predictor-corrector threshold setting",
        ),
        (
            "span.stop_after_passages",
            "stop_after_passages" not in span,
            "no perihelion-passage events in this runner",
        ),
        (
            "output.perihelion",
            not out.get("perihelion"),
            "no perihelion-passage events in this runner",
        ),
        (
            "bodies",
            all(b in known_bodies for b in bodies)
            and not ("EMB" in bodies and {"Earth", "Moon"} & set(bodies)),
            "bodies must be header.440 bodies, without EMB next to Earth or Moon",
        ),
    ]


#: Why each switch other than ``newton`` is refused (named in the refusal message).
SWITCH_REASONS: dict[str, str] = {
    "ppn": "REBOUNDx gr_full has only the parameter c (beta = gamma = 1); "
    "beta, gamma != 1 is validated analytically (REF-010)",
    "sun_j2": "REBOUNDx gravitational_harmonics is not wired yet",
    "lense_thirring": "REBOUNDx has no solar Lense-Thirring term in this runner",
    "asteroids": "the REF-003 small-body states are not an input of this runner",
    "kbos": "the REF-003 small-body states are not an input of this runner",
    "kbo_ring": "the DE440 KBO ring is not implemented in this runner",
    "major_asteroids": "the REF-003 small-body states are not an input of this runner",
}


def check(scenario: Mapping[str, Any]) -> None:
    """Raise :class:`common.Refusal` unless the runner implements everything requested."""
    switches = [
        (sw, False, SWITCH_REASONS.get(sw, "not a CI-002 switch"))
        for sw in common.requested_switches(scenario.get("forces", {}))
        if sw not in SUPPORTED_SWITCHES
    ]
    # The switch triples above carry the per-switch reasons, so no switch is left to the generic
    # check (every requested switch counts as "handled" there).
    requested = frozenset(common.requested_switches(scenario.get("forces", {})))
    common.refuse_unsupported(TOOL, scenario, requested, switches + unsupported_options(scenario))


def sample_times(span_days: float, cadence_days: float | None) -> list[float]:
    """Output times (days from the epoch): 0, ±cadence, … within the span, and the span end."""
    times = [0.0]
    if cadence_days is not None:
        n = math.floor(abs(span_days) / cadence_days + 1e-12)
        sign = 1.0 if span_days >= 0 else -1.0
        times += [sign * k * cadence_days for k in range(1, n + 1)]
    if times[-1] != span_days:
        times.append(span_days)
    return times


def relativity_switch(scenario: Mapping[str, Any]) -> str | None:
    """Return the relativistic switch a (checked) scenario requests, if any."""
    forces = scenario.get("forces", {})
    on = [sw for sw in REBOUNDX_EFFECT if forces.get(sw) is True]
    return on[0] if on else None


def speed_of_light_au_per_day(header: Header440) -> float:
    """``c`` in au/day from header.440 (CLIGHT km/s, AU km), refusing BETA, GAMMA != 1."""
    for name in ("BETA", "GAMMA"):
        if header.value(name) != 1.0:
            raise common.Refusal(
                TOOL,
                "header.440",
                [
                    (
                        name,
                        f"REBOUNDx gr effects assume {name} = 1; "
                        f"header.440 has {header.value(name)}",
                    )
                ],
            )
    return header.value("CLIGHT") * SECONDS_PER_DAY / header.value("AU")


def fmt(x: float) -> str:
    """Shortest round-trip decimal of a binary64 (``repr``)."""
    return repr(float(x))


def run(
    scenario: Mapping[str, Any], header: Header440, effect: str | None = None
) -> tuple[list[list[str]], dict[str, str], str]:
    """Integrate with the REBOUNDx ``effect`` (``None``: Newtonian).

    Return the CSV rows, the extra manifest fields and the engine version string.
    """
    import rebound  # noqa: PLC0415 - GPL engine, present only inside the oracle image
    import reboundx  # noqa: PLC0415

    bodies = [str(b) for b in scenario["bodies"]]
    au_km = header.value("AU")
    integ = scenario.get("integrator", {})
    sim = rebound.Simulation()
    sim.G = 1.0
    sim.integrator = "ias15"
    if "epsilon" in integ:
        sim.integrator.epsilon = float(integ["epsilon"])
    sim.dt = float(integ.get("dt0_days", 0.1))  # FerroAstro's N-body default first trial step
    span_days = float(scenario["span"]["years"]) * DAYS_PER_JULIAN_YEAR
    if span_days < 0:
        sim.dt = -abs(sim.dt)
    for b in bodies:
        gm, p, v = header.state(b)
        sim.add(m=gm, x=p[0], y=p[1], z=p[2], vx=v[0], vy=v[1], vz=v[2])
    c_au_day: float | None = None
    rebx: Any = None
    if effect is not None:
        c_au_day = speed_of_light_au_per_day(header)
        rebx = reboundx.Extras(sim)
        force = rebx.load_force(effect)
        rebx.add_force(force)
        force.params["c"] = c_au_day
    out = scenario.get("output", {})
    keep = [str(b) for b in out.get("sample_bodies", bodies)]
    cadence = out.get("cadence_days")
    e0 = sim.energy()
    rows: list[list[str]] = []
    vel = au_km / SECONDS_PER_DAY
    for t in sample_times(span_days, None if cadence is None else float(cadence)):
        if t != 0.0:
            sim.integrate(t, exact_finish_time=1)
        for i, b in enumerate(bodies):
            if b not in keep:
                continue
            q = sim.particles[i]
            rows.append(
                [
                    fmt(t),
                    str(NAIF[b]),
                    fmt(q.x * au_km),
                    fmt(q.y * au_km),
                    fmt(q.z * au_km),
                    fmt(q.vx * vel),
                    fmt(q.vy * vel),
                    fmt(q.vz * vel),
                ]
            )
    e1 = sim.energy()
    extra = {
        "rebound_version": str(rebound.__version__),
        "reboundx_version": str(reboundx.__version__),
        "ias15_epsilon": fmt(sim.integrator.epsilon),
        "ias15_adaptive_mode": str(sim.integrator.adaptive_mode),
        "ias15_min_dt": fmt(sim.integrator.min_dt),
        "steps_done": str(getattr(sim, "steps_done", "n/a")),
        "relative_energy_change": fmt((e1 - e0) / abs(e0)),
        "gravity": str(getattr(sim, "gravity", "n/a")),
        "reboundx_effect": effect or "none",
        "c_au_per_day": "n/a" if c_au_day is None else fmt(c_au_day),
        "force_is_velocity_dependent": str(getattr(sim, "force_is_velocity_dependent", "n/a")),
    }
    del rebx  # the Extras object must outlive the integration
    version = f"rebound {rebound.__version__}, reboundx {reboundx.__version__}"
    return rows, extra, version


def series_name(scenario_name: str, variant: str | None) -> str:
    """File name of a series: the primary one, or a named REBOUNDx variant next to it."""
    infix = "" if variant is None else f".{variant}"
    return f"{scenario_name}.rebound{infix}.states.csv"


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    # (file variant, REBOUNDx effect): the primary series first, then the extra variants.
    runs: list[tuple[str | None, str | None]]
    results: list[tuple[str | None, list[list[str]], dict[str, str], str, float]] = []
    try:
        scenario = common.load_scenario(args.scenario)
        check(scenario)
        digest, runner_sha = common.runner_identity()
        header = Header440.read(args.header)
        sw = relativity_switch(scenario)
        runs = [(None, None if sw is None else REBOUNDX_EFFECT[sw])]
        runs += [(v, v) for v in (EXTRA_VARIANTS[sw] if sw is not None else ())]
        for variant, effect in runs:
            started = time.perf_counter()
            rows, extra, version = run(scenario, header, effect)
            results.append((variant, rows, extra, version, time.perf_counter() - started))
    except common.Refusal as r:
        print(r, file=sys.stderr)
        return common.REFUSAL_EXIT
    except (common.SetupError, KeyError, ValueError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    args.out_dir.mkdir(parents=True, exist_ok=True)
    here = Path(__file__).resolve().parent
    entries: list[common.ManifestEntry] = []
    for variant, rows, extra, version, wall in results:
        name = series_name(str(scenario["name"]), variant)
        path = args.out_dir / name
        stage = "C" if extra["reboundx_effect"] != "none" else "A"
        # REUSE-IgnoreStart (licence lines written into the CSV, not this file's licence)
        lines = [
            "# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
            "# SPDX-License-Identifier: Apache-2.0",
            # REUSE-IgnoreEnd
            f"# REBOUND IAS15 states of scenario {scenario['name']} "
            f"(REF-005 stage {stage} oracle output)",
            f"# engine: {version}",
            f"# reboundx_effect: {extra['reboundx_effect']}",
            f"# header_sha256: {common.sha256_file(args.header)}",
            f"# epoch_jd_tdb: {fmt(header.value('JDEPOC'))}",
            f"# au_km: {fmt(header.value('AU'))}",
            "# units: t_days TDB days since epoch; position km; velocity km/s; "
            "SSB-centred ICRF axes",
            ",".join(STATE_COLUMNS),
        ]
        path.write_text("\n".join(lines + [",".join(r) for r in rows]) + "\n", encoding="utf-8")
        entries.append(
            common.ManifestEntry(
                file=name,
                tool=TOOL,
                version=version,
                image_digest=digest,
                runner_sha=runner_sha,
                runner_sha256=common.sha256_files(sorted(here.glob("*.py"))),
                input_sha256={
                    args.scenario.name: common.sha256_file(args.scenario),
                    args.header.name: common.sha256_file(args.header),
                },
                cpu=common.cpu_model(),
                wall_clock_s=round(wall, 3),
                output_sha256=common.sha256_file(path),
                units="t_days: TDB days since epoch_jd_tdb; position: km; velocity: km/s",
                frame="ICRF axes, solar-system-barycentric (DE440 header.440 frame)",
                time_scale="TDB",
                scenario=str(scenario["name"]),
                generated_utc=common.utc_now(),
                command="python -m oracles.rebound_runner "
                + " ".join(sys.argv[1:] if argv is None else argv),
                extra=extra,
            )
        )
        print(f"wrote {path} ({len(rows)} rows, {wall:.2f} s); {extra}")
    common.write_manifest(args.out_dir, entries)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
