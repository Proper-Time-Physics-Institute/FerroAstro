# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""REBOUND/REBOUNDx oracle runner (REF-005, stage A; REF-011 refusals).

Runs inside ``ci/oracles/rebound`` only. REBOUND 5.2.2 and REBOUNDx 5.1.0 are GPL-3.0: they are
used as black boxes through their documented Python API (``Simulation``, ``add``, ``integrate``,
``particles``, ``integrator.epsilon`` / ``adaptive_mode`` (REBOUND 5 changelog: the ``ri_*``
structs are gone), ``energy``) and their source is never read (DEP-008). The container
receives a scenario and ``header.440`` and returns plain files (LIC-001); it never contains a
FerroAstro build (red-team L4).

Implemented (everything else is refused by name, exit status 3):

* ``mode = "n-body"``, ``[forces] newton = true`` only (stage A). ``ppn{beta,gamma}`` is refused
  permanently: REBOUNDx ``gr_full`` has only the parameter ``c`` (beta = gamma = 1, REF-005 notes);
  ``gr_eih`` arrives with stage C (WP21) and is refused until then;
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
#: Switches this runner implements (stage A).
SUPPORTED_SWITCHES = frozenset({"newton"})
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
    return [
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
    "gr_eih": "REBOUNDx gr_full is wired with stage C (WP21)",
    "gr_1pn_central": "REBOUNDx gr/gr_potential are wired with stage C (WP21)",
    "sun_j2": "REBOUNDx gravitational_harmonics is not wired yet",
    "lense_thirring": "REBOUNDx has no solar Lense-Thirring term in this runner",
    "asteroids": "the REF-003 small-body states are not an input of this runner",
    "kbos": "the REF-003 small-body states are not an input of this runner",
    "kbo_ring": "the DE440 KBO ring is not implemented in this runner",
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


def fmt(x: float) -> str:
    """Shortest round-trip decimal of a binary64 (``repr``)."""
    return repr(float(x))


def run(
    scenario: Mapping[str, Any], header: Header440
) -> tuple[list[list[str]], dict[str, str], str]:
    """Integrate; return the CSV rows, extra manifest fields and the engine version string."""
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
    }
    version = f"rebound {rebound.__version__}, reboundx {reboundx.__version__}"
    return rows, extra, version


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        scenario = common.load_scenario(args.scenario)
        check(scenario)
        digest, runner_sha = common.runner_identity()
        header = Header440.read(args.header)
        started = time.perf_counter()
        rows, extra, version = run(scenario, header)
        wall = time.perf_counter() - started
    except common.Refusal as r:
        print(r, file=sys.stderr)
        return common.REFUSAL_EXIT
    except (common.SetupError, KeyError, ValueError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    args.out_dir.mkdir(parents=True, exist_ok=True)
    name = common.output_file(scenario, "rebound")
    path = args.out_dir / name
    # REUSE-IgnoreStart (licence lines written into the CSV, not this file's licence)
    lines = [
        "# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute",
        "# SPDX-License-Identifier: Apache-2.0",
        # REUSE-IgnoreEnd
        f"# REBOUND IAS15 states of scenario {scenario['name']} (REF-005 stage A oracle output)",
        f"# engine: {version}",
        f"# header_sha256: {common.sha256_file(args.header)}",
        f"# epoch_jd_tdb: {fmt(header.value('JDEPOC'))}",
        f"# au_km: {fmt(header.value('AU'))}",
        "# units: t_days TDB days since epoch; position km; velocity km/s; SSB-centred ICRF axes",
        ",".join(STATE_COLUMNS),
    ]
    path.write_text("\n".join(lines + [",".join(r) for r in rows]) + "\n", encoding="utf-8")
    here = Path(__file__).resolve().parent
    entry = common.ManifestEntry(
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
    common.write_manifest(args.out_dir, [entry])
    print(f"wrote {path} ({len(rows)} rows, {wall:.2f} s); {extra}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
