# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Swiftest oracle runner (REF-007, stages A and C-lite; REF-011 refusals).

Runs inside ``ci/oracles/swiftest`` only. Swiftest 2026.7.1 is GPL-3.0-or-later: it is used as a
black box through its documented Python API (readthedocs "Basic simulation", "Detailed
simulation setup", API reference: ``Simulation``, ``set_unit_system``, ``set_parameter``,
``add_body(name, rh, vh, Gmass, radius)``, ``run``, ``data``), and its source is never read
(DEP-008). The container receives a scenario and ``header.440`` and returns plain files (LIC-001);
it never contains a FerroAstro build (red-team L4).

Implemented (everything else is refused by name, exit status 3):

* ``mode = "n-body"`` with ``newton`` (stage A, the PHY-012 P-add configurations): every body is a
  massive Swiftest body, the first (``central_body``, the Sun) is Swiftest's central body;
* ``mode = "test-particle"`` with ``newton`` and optionally ``gr_1pn_central`` (stage C-lite): the
  Sun is the central body and the other body a massless test particle; ``gr_1pn_central`` turns
  on Swiftest's ``general_relativity`` (a central-body 1PN term, not EIH: REF-007 notes);
* ``initial_conditions.source = "de440-header"``, DE440 GMs, ``f64``; ``output.cadence_days`` a
  whole number of steps, the span a whole number of outputs, forward in time.

The scenario's ``[integrator]`` names FerroAstro's IAS15, which Swiftest does not have: the run uses
the integrator and fixed step of ``ci/oracles/swiftest/settings.toml`` (WHM, dt = 1/128 d; WP17
plan, NEEDS HUMAN REVIEW), and the mapping is recorded in the manifest.

Units inside Swiftest: au, day (TDB), GM in au^3/d^2 (bodies are added by ``Gmass``, so neither G
nor a solar mass enters). Initial conditions are heliocentric (body minus Sun, header.440 at
``JDEPOC``). The output CSV is heliocentric (``center_naif: 10``), ICRF axes, km and km/s.

Usage (inside the container; the driver is ``tools/oracles/run_oracles.sh``)::

    python -m oracles.swiftest_runner --scenario S.toml --header header.440 --out-dir OUT
"""

from __future__ import annotations

import argparse
import math
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from oracles import common, nbody_io
from oracles.header440 import BODIES, NAIF, Header440

TOOL = "swiftest"
SUPPORTED_SWITCHES = frozenset({"newton", "gr_1pn_central"})
SETTINGS = Path("/opt/fa/settings.toml")

SWITCH_REASONS: dict[str, str] = {
    "ppn": "Swiftest's general_relativity has no beta, gamma parameters",
    "gr_eih": "Swiftest's general_relativity is a central-body 1PN term, not EIH (REF-007)",
    "sun_j2": "the plain j2rp2 input is not wired (OI-31)",
    "lense_thirring": "Swiftest has no solar Lense-Thirring term",
    "asteroids": "the REF-003 small-body states are not an input of this runner",
    "kbos": "the REF-003 small-body states are not an input of this runner",
    "kbo_ring": "the DE440 KBO ring is not implemented in this runner",
}


def unsupported_options(
    s: Mapping[str, Any], settings: Mapping[str, Any]
) -> list[tuple[str, bool, str]]:
    """``(item, supported, reason)`` for the non-switch options of a scenario."""
    ic = s.get("initial_conditions", {})
    integ = s.get("integrator", {})
    span = s.get("span", {})
    out = s.get("output", {})
    forces = s.get("forces", {})
    mode = s.get("mode")
    bodies = [str(b) for b in s.get("bodies", [])]
    known = set(BODIES) | {"Earth", "Moon"}
    dt = float(settings.get("dt_days", 0.0))
    years = span.get("years")
    days = float(years) * nbody_io.DAYS_PER_JULIAN_YEAR if isinstance(years, (int, float)) else 0.0
    cadence = out.get("cadence_days")
    whole_outputs = (
        isinstance(cadence, (int, float))
        and nbody_io.steps_per(float(cadence), dt) is not None
        and nbody_io.steps_per(days, float(cadence)) is not None
    )
    return [
        (
            "forces.newton",
            forces.get("newton") is True,
            "Swiftest always applies Newtonian gravity",
        ),
        ("mode", mode in ("n-body", "test-particle"), "only n-body and test-particle modes"),
        (
            "forces.gr_1pn_central",
            not forces.get("gr_1pn_central") or mode == "test-particle",
            "Swiftest's GR is compared with the central-body 1PN term in test-particle mode only",
        ),
        (
            "initial_conditions.source",
            ic.get("source") == "de440-header",
            "only de440-header initial conditions are implemented",
        ),
        ("constants.gm_set", s.get("constants", {}).get("gm_set") == "de440", "DE440 GMs only"),
        ("integrator.name", integ.get("name") == "ias15", "only the IAS15 mapping is defined"),
        ("integrator.precision", integ.get("precision") == "f64", "Swiftest integrates in f64"),
        ("central_body", s.get("central_body") == "Sun" and bodies[:1] == ["Sun"], "the Sun first"),
        (
            "bodies",
            all(b in known for b in bodies)
            and not ("EMB" in bodies and {"Earth", "Moon"} & set(bodies))
            and (mode != "test-particle" or len(bodies) == 2),
            "header.440 bodies, no EMB next to Earth or Moon, two bodies in test-particle mode",
        ),
        ("span.years", days > 0.0, "Swiftest integrates forwards only (tstop > tstart)"),
        ("span.stop_after_passages", "stop_after_passages" not in span, "no passage events"),
        ("output.perihelion", not out.get("perihelion"), "no passage events in this runner"),
        (
            "output.cadence_days",
            whole_outputs,
            "the cadence must be a whole number of steps and the span a whole number of outputs",
        ),
    ]


def check(scenario: Mapping[str, Any], settings: Mapping[str, Any]) -> None:
    """Raise :class:`common.Refusal` unless the runner implements everything requested."""
    requested = common.requested_switches(scenario.get("forces", {}))
    switches = [
        (sw, False, SWITCH_REASONS.get(sw, "not a CI-002 switch"))
        for sw in requested
        if sw not in SUPPORTED_SWITCHES
    ]
    common.refuse_unsupported(
        TOOL,
        scenario,
        frozenset(requested),
        switches + unsupported_options(scenario, settings),
    )


def heliocentric(header: Header440, body: str) -> tuple[float, list[float], list[float]]:
    """``(GM, r, v)`` of ``body`` relative to the Sun (au, au/day)."""
    gm, p, v = header.state(body)
    _, ps, vs = header.state("Sun")
    return gm, [p[k] - ps[k] for k in range(3)], [v[k] - vs[k] for k in range(3)]


Vec = Sequence[float]


def energy(gm: Sequence[float], rh: Sequence[Vec], vh: Sequence[Vec], test_particle: bool) -> float:
    """Return the total energy x G (GM-weighted) of one output frame, barycentric frame.

    In test-particle mode, the particle's specific energy v^2/2 - GM_sun/r instead. Computed by
    this runner from Swiftest's heliocentric output (``rh``, ``vh``; row 0 is the Sun), not by
    Swiftest.
    """
    n = len(gm)
    r = [[0.0, 0.0, 0.0]] + [[float(rh[i][k]) for k in range(3)] for i in range(1, n)]
    v = [[0.0, 0.0, 0.0]] + [[float(vh[i][k]) for k in range(3)] for i in range(1, n)]
    if test_particle:
        rr = math.sqrt(sum(x * x for x in r[1]))
        return 0.5 * sum(x * x for x in v[1]) - gm[0] / rr
    m = sum(gm)
    vc = [sum(gm[i] * v[i][k] for i in range(n)) / m for k in range(3)]
    e = 0.0
    for i in range(n):
        e += 0.5 * gm[i] * sum((v[i][k] - vc[k]) ** 2 for k in range(3))
        for j in range(i + 1, n):
            dij = math.sqrt(sum((r[i][k] - r[j][k]) ** 2 for k in range(3)))
            e -= gm[i] * gm[j] / dij
    return e


def unit_system(sim: Any, au_km: float) -> float:  # noqa: ANN401 - swiftest.Simulation
    """Set au, day and a mass unit with G * MU = 1 au^3/d^2; return Swiftest's G (SI).

    With GU = 1 Swiftest's mass <-> Gmass conversions are exact (with MU2KG = 1 kg, Venus's GM came
    back one ulp off). Swiftest's G is read back from its documented ``GU``, not from its source.
    """
    du2m = au_km * 1000.0
    tu2s = nbody_io.SECONDS_PER_DAY
    sim.set_unit_system(MU2KG=1.0, DU2M=du2m, TU2S=tu2s, verbose=False)
    g_si = float(sim.GU) * du2m**3 / tu2s**2
    sim.set_unit_system(MU2KG=du2m**3 / (g_si * tu2s**2), DU2M=du2m, TU2S=tu2s, verbose=False)
    if float(sim.GU) != 1.0:
        msg = f"cannot make Swiftest's GU exactly 1 (got {float(sim.GU)!r})"
        raise ValueError(msg)
    return g_si


def add_bodies(sim: Any, scenario: Mapping[str, Any], header: Header440) -> list[float]:  # noqa: ANN401
    """Add the Sun (central body) and the other bodies; return each body's GM as added."""
    bodies = [str(b) for b in scenario["bodies"]]
    test_particle = scenario["mode"] == "test-particle"
    au_km = header.value("AU")
    gms, _, _ = header.state("Sun")
    sim.add_body(
        name="Sun", Gmass=gms, radius=header.value("ASUN") / au_km, rh=[0.0] * 3, vh=[0.0] * 3
    )
    gm_in = [gms]
    for b in bodies[1:]:
        gm, r, v = heliocentric(header, b)
        if test_particle:
            sim.add_body(name=b, rh=r, vh=v)
            gm_in.append(0.0)
        else:
            sim.add_body(name=b, Gmass=gm, rh=r, vh=v)
            gm_in.append(gm)
    return gm_in


#: Swiftest 2026.7.1's driver segfaults intermittently in-process (observed in WP17 for Sun +
#: one planet: 3 of 4 identical runs crashed, and forked retries of a crashed process crashed
#: again). Each attempt therefore runs in a freshly spawned interpreter; a crashed attempt is
#: retried, and the number of attempts is recorded in the manifest.
MAX_ATTEMPTS = 8


def build_and_run(
    scenario: Mapping[str, Any], header_path: Path, settings: Mapping[str, Any], workdir: Path
) -> None:
    """Set up and run Swiftest, then save the arrays the runner uses (in a spawned child)."""
    import numpy as np  # noqa: PLC0415 - present in the oracle image only
    import swiftest  # noqa: PLC0415 - GPL engine, present only inside the oracle image

    header = Header440.read(header_path)
    days = nbody_io.span_days(scenario)
    dt = float(settings["dt_days"])
    istep_out = nbody_io.steps_per(float(scenario["output"]["cadence_days"]), dt)
    sim = swiftest.Simulation(
        simdir=str(workdir / "sim"), integrator=str(settings["integrator"]), verbose=False
    )
    g_si = unit_system(sim, header.value("AU"))
    sim.set_parameter(
        general_relativity=bool(scenario["forces"].get("gr_1pn_central", False)),
        close_encounter_check=False,
        rotation=False,
        compute_conservation_values=False,
        output_format="XV",
    )
    gm_in = add_bodies(sim, scenario, header)
    sim.run(tstop=days, dt=dt, istep_out=istep_out, dump_cadence=1000)
    d = sim.data
    np.savez(
        workdir / "fa-output.npz",
        name=np.array([str(n) for n in d["name"].values]),
        status=d["status"].values,
        gmass=d["Gmass"].values[0],
        gm_in=np.array(gm_in),
        g_si=np.array(g_si),
        time=d["time"].values,
        rh=d["rh"].values,
        vh=d["vh"].values,
    )


def run_isolated(
    scenario: Mapping[str, Any], header_path: Path, settings: Mapping[str, Any], workdir: Path
) -> tuple[Any, int]:
    """Run in spawned children until one exits cleanly; return its arrays and the attempt count."""
    import multiprocessing  # noqa: PLC0415

    import numpy as np  # noqa: PLC0415

    ctx = multiprocessing.get_context("spawn")
    codes = []
    for attempt in range(1, MAX_ATTEMPTS + 1):
        work = workdir / f"attempt-{attempt}"
        work.mkdir(parents=True)
        proc = ctx.Process(
            target=build_and_run, args=(dict(scenario), header_path, dict(settings), work)
        )
        proc.start()
        proc.join()
        codes.append(proc.exitcode)
        npz = work / "fa-output.npz"
        if proc.exitcode == 0 and npz.exists():
            return np.load(npz), attempt
    msg = f"Swiftest failed {MAX_ATTEMPTS} times (child exit codes {codes})"
    raise ValueError(msg)


def check_output(data: Any, bodies: Sequence[str], gm_in: Sequence[float], n_out: int) -> None:  # noqa: ANN401
    """Refuse an output with other bodies, a discard, changed GMs or a wrong frame count."""
    names = [str(n) for n in data["name"]]
    if names != list(bodies):
        msg = f"Swiftest returned bodies {names}, expected {list(bodies)}"
        raise ValueError(msg)
    if (data["status"] != 0).any():
        msg = "a body was discarded during the run (status != 0)"
        raise ValueError(msg)
    gmass = data["gmass"]
    for i, b in enumerate(names):
        if gm_in[i] != 0.0 and float(gmass[i]) != gm_in[i]:
            msg = f"Swiftest Gmass of {b} is {gmass[i]!r}, header {gm_in[i]!r}"
            raise ValueError(msg)
    if len(data["time"]) != n_out + 1:
        msg = f"{len(data['time'])} output frames, expected {n_out} + 1"
        raise ValueError(msg)


def run(
    scenario: Mapping[str, Any], header_path: Path, settings: Mapping[str, Any], workdir: Path
) -> tuple[list[list[str]], dict[str, str], str]:
    """Integrate; return the CSV rows, extra manifest fields and the engine version string."""
    import importlib.metadata  # noqa: PLC0415

    header = Header440.read(header_path)
    version = importlib.metadata.version("swiftest")
    bodies = [str(b) for b in scenario["bodies"]]
    test_particle = scenario["mode"] == "test-particle"
    gr = bool(scenario["forces"].get("gr_1pn_central", False))
    au_km = header.value("AU")
    dt = float(settings["dt_days"])
    cadence = float(scenario["output"]["cadence_days"])
    days = nbody_io.span_days(scenario)
    istep_out = nbody_io.steps_per(cadence, dt)
    n_out = nbody_io.steps_per(days, cadence)
    if istep_out is None or n_out is None:
        msg = "the cadence or the span is not a whole number of steps or outputs"
        raise ValueError(msg)
    data, attempts = run_isolated(scenario, header_path, settings, workdir)
    gm_in = [float(x) for x in data["gm_in"]]
    g_si = float(data["g_si"])
    check_output(data, bodies, gm_in, n_out)
    times = [float(t) for t in data["time"]]
    rh = data["rh"]
    vh = data["vh"]
    e0 = energy(gm_in, rh[0], vh[0], test_particle)
    e1 = energy(gm_in, rh[-1], vh[-1], test_particle)

    keep = [str(b) for b in scenario.get("output", {}).get("sample_bodies", bodies)]
    out_idx = [(i, b) for i, b in enumerate(bodies) if i > 0 and b in keep]
    rows = [
        nbody_io.state_row(k * cadence, NAIF[b], rh[k, i], vh[k, i], au_km)
        for k in range(len(times))
        for i, b in out_idx
    ]
    extra = {
        "swiftest_version": version,
        "integrator": str(settings["integrator"]),
        "dt_days": nbody_io.fmt(dt),
        "istep_out": str(istep_out),
        "general_relativity": str(gr),
        "test_particle_mode": str(test_particle),
        "frames": str(len(times)),
        "max_output_time_deviation_days": nbody_io.fmt(
            max(abs(t - k * cadence) for k, t in enumerate(times))
        ),
        "gmass_bitwise_header": "true",
        "relative_energy_change": nbody_io.fmt((e1 - e0) / abs(e0)),
        "integrator_mapping": nbody_io.mapped_integrator_note(scenario, settings),
        "omp_num_threads": "1",
        "attempts": str(attempts),
        "swiftest_g_si": nbody_io.fmt(g_si),
    }
    return rows, extra, f"swiftest {version}"


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--header", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--settings", type=Path, default=SETTINGS)
    args = parser.parse_args(argv)
    try:
        scenario = common.load_scenario(args.scenario)
        settings = nbody_io.load_settings(args.settings, str(scenario.get("name", "")))
        check(scenario, settings)
        digest, runner_sha = common.runner_identity()
        header = Header440.read(args.header)
        started = time.perf_counter()
        with tempfile.TemporaryDirectory(prefix="swiftest-") as work:
            rows, extra, version = run(scenario, args.header, settings, Path(work))
        wall = time.perf_counter() - started
    except common.Refusal as r:
        print(r, file=sys.stderr)
        return common.REFUSAL_EXIT
    except (common.SetupError, KeyError, ValueError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    args.out_dir.mkdir(parents=True, exist_ok=True)
    name = common.output_file(scenario, TOOL)
    path = args.out_dir / name
    nbody_io.write_states_csv(
        path,
        nbody_io.StatesHeader(
            title=f"Swiftest states of scenario {scenario['name']} (REF-007 oracle output)",
            engine=version,
            header_sha256=common.sha256_file(args.header),
            epoch_jd_tdb=header.value("JDEPOC"),
            au_km=header.value("AU"),
            center_naif=NAIF["Sun"],
            frame_note="heliocentric (body minus Sun), ICRF axes",
        ),
        rows,
    )
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
            "settings.toml": common.sha256_file(args.settings),
        },
        cpu=common.cpu_model(),
        wall_clock_s=round(wall, 3),
        output_sha256=common.sha256_file(path),
        units="t_days: TDB days since epoch_jd_tdb; position: km; velocity: km/s",
        frame="ICRF axes, heliocentric (body minus Sun; center_naif 10)",
        time_scale="TDB",
        scenario=str(scenario["name"]),
        generated_utc=common.utc_now(),
        command="python -m oracles.swiftest_runner "
        + " ".join(sys.argv[1:] if argv is None else argv),
        extra=extra,
    )
    common.write_manifest(args.out_dir, [entry])
    print(f"wrote {path} ({len(rows)} rows, {wall:.2f} s); {extra}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
