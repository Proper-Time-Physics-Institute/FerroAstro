# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Orekit oracle runner (REF-006, stage B: a particle in a DE440-driven field; REF-011).

Runs inside ``ci/oracles/orekit`` only. Orekit 13.1.9 and Hipparchus 4.0.3 are Apache-2.0; the
image follows the oracle conventions anyway (plain files in and out, no FerroAstro build). The
Java side is ``ci/oracles/orekit/StageB.java`` (documented Orekit API only).

Implemented (everything else is refused by name, exit status 3):

* ``mode = "ephemeris-field"`` with ``newton`` only: the particle (first body after the Sun) is
  propagated about the Sun (``NewtonianAttraction(GM_sun)``) with ``ThirdBodyAttraction`` for every
  other listed body, positions from the JPL binary DE440 file (Orekit reads binary DE files, not
  ``.bsp``: OI-27). ``gr_1pn_central`` (Orekit ``Relativity``), ``sun_j2`` (``J2OnlyPerturbation``)
  and ``lense_thirring`` are refused until a later stage wires them;
* ``initial_conditions.source = "de440-bsp"`` with an epoch inside the file (1550-2650): Orekit
  takes the particle's initial state from the same ephemeris;
* ``f64``, forward spans, ``output.cadence_days`` (fixed-step handler on DP853 dense output).

The scenario's ``[integrator]`` names FerroAstro's IAS15; Orekit runs the settings of
``ci/oracles/orekit/settings.toml`` (DormandPrince853; WP17 plan, NEEDS HUMAN REVIEW).
Output: heliocentric (``center_naif: 10``) ICRF positions (km) and velocities (km/s).

Usage (inside the container; the driver is ``tools/oracles/run_oracles.sh``)::

    python -m oracles.orekit_runner --scenario S.toml --jpl-dir DIR --out-dir OUT
"""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Any

from oracles import common, nbody_io

TOOL = "orekit"
SUPPORTED_SWITCHES = frozenset({"newton"})
SETTINGS = Path("/opt/fa/settings.toml")
OREKIT_DATA = Path("/opt/orekit/data")
CLASSPATH = "/opt/orekit/lib/*:/opt/orekit/classes"
JPL_FILE = "linux_p1550p2650.440"
#: The JPL binary file's coverage, TDB JD (header.440 GROUP 1010: 1549-12-21 .. 2650-01-25).
COVERAGE_JD = (2287184.5, 2688976.5)
NAIF = {
    "Sun": 10,
    "Mercury": 1,
    "Venus": 2,
    "Earth": 399,
    "Moon": 301,
    "Mars": 4,
    "Jupiter": 5,
    "Saturn": 6,
    "Uranus": 7,
    "Neptune": 8,
    "Pluto": 9,
}

SWITCH_REASONS: dict[str, str] = {
    "gr_1pn_central": "Orekit Relativity is not wired in stage B yet",
    "gr_eih": "Orekit has no EIH N-body equations (single-body propagator, REF-006)",
    "ppn": "Orekit Relativity has beta = gamma = 1 built in (REF-006 notes)",
    "sun_j2": "J2OnlyPerturbation is not wired in stage B yet",
    "lense_thirring": "LenseThirringRelativity is written for Earth satellites (OI-26)",
    "asteroids": "the field holds the DE440 Major bodies only",
    "kbos": "the field holds the DE440 Major bodies only",
    "kbo_ring": "the field holds the DE440 Major bodies only",
}


def unsupported_options(s: Mapping[str, Any]) -> list[tuple[str, bool, str]]:
    """``(item, supported, reason)`` for the non-switch options of a scenario."""
    ic = s.get("initial_conditions", {})
    integ = s.get("integrator", {})
    span = s.get("span", {})
    out = s.get("output", {})
    bodies = [str(b) for b in s.get("bodies", [])]
    jd = ic.get("epoch_jd_tdb")
    years = span.get("years")
    days = float(years) * nbody_io.DAYS_PER_JULIAN_YEAR if isinstance(years, (int, float)) else 0.0
    inside = isinstance(jd, (int, float)) and (
        COVERAGE_JD[0] <= float(jd) and float(jd) + days <= COVERAGE_JD[1]
    )
    cadence = out.get("cadence_days")
    return [
        (
            "forces.newton",
            s.get("forces", {}).get("newton") is True,
            "Orekit always applies the central attraction",
        ),
        ("mode", s.get("mode") == "ephemeris-field", "only ephemeris-field mode (stage B)"),
        (
            "initial_conditions.source",
            ic.get("source") == "de440-bsp",
            "the particle starts from the DE440 ephemeris itself (de440-bsp)",
        ),
        (
            "initial_conditions.epoch_jd_tdb",
            inside,
            f"the span must lie inside {JPL_FILE} (JD {COVERAGE_JD[0]} .. {COVERAGE_JD[1]})",
        ),
        ("constants.gm_set", s.get("constants", {}).get("gm_set") == "de440", "DE440 GMs only"),
        ("integrator.name", integ.get("name") == "ias15", "only the IAS15 mapping is defined"),
        ("integrator.precision", integ.get("precision") == "f64", "Orekit integrates in f64"),
        ("central_body", s.get("central_body") == "Sun" and bodies[:1] == ["Sun"], "the Sun first"),
        (
            "bodies",
            len(bodies) >= 3 and all(b in NAIF for b in bodies),
            "the Sun, the particle and >= 1 field body, each an Orekit DE body (no EMB)",
        ),
        ("span.years", days > 0.0, "forward spans only"),
        ("span.stop_after_passages", "stop_after_passages" not in span, "no passage events"),
        ("output.perihelion", not out.get("perihelion"), "no passage events in this runner"),
        (
            "output.cadence_days",
            isinstance(cadence, (int, float))
            and nbody_io.steps_per(days, float(cadence)) is not None,
            "a cadence that divides the span",
        ),
    ]


def check(scenario: Mapping[str, Any]) -> None:
    """Raise :class:`common.Refusal` unless the runner implements everything requested."""
    requested = common.requested_switches(scenario.get("forces", {}))
    switches = [
        (sw, False, SWITCH_REASONS.get(sw, "not a CI-002 switch"))
        for sw in requested
        if sw not in SUPPORTED_SWITCHES
    ]
    common.refuse_unsupported(
        TOOL, scenario, frozenset(requested), switches + unsupported_options(scenario)
    )


def params(scenario: Mapping[str, Any], settings: Mapping[str, Any], jpl_dir: Path) -> str:
    """Return the ``StageB`` properties file."""
    bodies = [str(b) for b in scenario["bodies"]]
    values = {
        "orekit_data": str(OREKIT_DATA),
        "jpl_dir": str(jpl_dir),
        "epoch_jd_tdb": nbody_io.fmt(float(scenario["initial_conditions"]["epoch_jd_tdb"])),
        "span_days": nbody_io.fmt(nbody_io.span_days(scenario)),
        "cadence_days": nbody_io.fmt(float(scenario["output"]["cadence_days"])),
        "central": bodies[0],
        "particle": bodies[1],
        "field": ",".join(bodies[2:]),
        "min_step_s": nbody_io.fmt(float(settings["min_step_s"])),
        "max_step_s": nbody_io.fmt(float(settings["max_step_s"])),
        "abs_tol_m": nbody_io.fmt(float(settings["abs_tol_m"])),
        "rel_tol": nbody_io.fmt(float(settings["rel_tol"])),
    }
    return "".join(f"{k}={v}\n" for k, v in values.items())


def run(
    scenario: Mapping[str, Any], settings: Mapping[str, Any], jpl_dir: Path, work: Path
) -> tuple[list[list[str]], dict[str, str], str]:
    """Run ``StageB``; return the CSV rows, extra manifest fields and the engine version."""
    (work / "stage-b.properties").write_text(params(scenario, settings, jpl_dir), encoding="utf-8")
    raw = work / "stage-b.out"
    r = subprocess.run(  # noqa: S603
        ["java", "-cp", CLASSPATH, "StageB", str(work / "stage-b.properties"), str(raw)],  # noqa: S607
        capture_output=True,
        text=True,
        check=False,
    )
    if r.returncode != 0:
        msg = f"StageB exit {r.returncode}: {r.stderr[-1500:]}"
        raise ValueError(msg)
    rows: list[list[str]] = []
    gms: dict[str, str] = {}
    max_dt = 0.0
    for line in raw.read_text(encoding="utf-8").splitlines():
        if line.startswith("#gm "):
            _, body, gm = line.split()
            gms[f"orekit_gm_{body.lower()}_m3_s2"] = gm
            continue
        if line.startswith("#rotation_to_icrf_rad "):
            gms["integration_frame_rotation_to_icrf_rad"] = line.split()[1]
            continue
        f = line.split(",")
        max_dt = max(max_dt, abs(float(f[8].removeprefix("dt="))))
        rows.append([nbody_io.fmt(float(f[0])), f[1]] + [nbody_io.fmt(float(x)) for x in f[2:8]])
    n_out = nbody_io.steps_per(
        nbody_io.span_days(scenario), float(scenario["output"]["cadence_days"])
    )
    if n_out is None or len(rows) != n_out + 1:
        msg = f"{len(rows)} output rows, expected {n_out} + 1"
        raise ValueError(msg)
    java = Path("/opt/orekit/java-version.txt").read_text(encoding="utf-8").strip()
    extra = {
        "orekit": "13.1.9",
        "hipparchus": "4.0.3",
        "orekit_data_commit": "2422a7d06ee2cc3b5a503a47271203b0706f3e82",
        "java": java,
        "ephemeris_file": JPL_FILE,
        "integrator": str(settings["integrator"]),
        "min_step_s": nbody_io.fmt(float(settings["min_step_s"])),
        "max_step_s": nbody_io.fmt(float(settings["max_step_s"])),
        "abs_tol_m": nbody_io.fmt(float(settings["abs_tol_m"])),
        "rel_tol": nbody_io.fmt(float(settings["rel_tol"])),
        "max_output_time_deviation_s": nbody_io.fmt(max_dt),
        "integrator_mapping": nbody_io.mapped_integrator_note(scenario, settings),
        "force_models": "NewtonianAttraction(GM_sun) + ThirdBodyAttraction per field body",
        **gms,
    }
    return rows, extra, "orekit 13.1.9, hipparchus 4.0.3"


def main(argv: Sequence[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--jpl-dir", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--settings", type=Path, default=SETTINGS)
    args = parser.parse_args(argv)
    try:
        scenario = common.load_scenario(args.scenario)
        settings = nbody_io.load_settings(args.settings, str(scenario.get("name", "")))
        check(scenario)
        digest, runner_sha = common.runner_identity()
        started = time.perf_counter()
        with tempfile.TemporaryDirectory(prefix="orekit-") as work:
            rows, extra, version = run(scenario, settings, args.jpl_dir, Path(work))
        wall = time.perf_counter() - started
    except common.Refusal as r:
        print(r, file=sys.stderr)
        return common.REFUSAL_EXIT
    except (common.SetupError, KeyError, ValueError, IndexError) as e:
        print(f"SETUP ERROR: {e}", file=sys.stderr)
        return common.SETUP_EXIT
    args.out_dir.mkdir(parents=True, exist_ok=True)
    name = common.output_file(scenario, TOOL)
    path = args.out_dir / name
    jpl = args.jpl_dir / JPL_FILE
    nbody_io.write_states_csv(
        path,
        nbody_io.StatesHeader(
            title=f"Orekit states of scenario {scenario['name']} (REF-006 stage B oracle output)",
            engine=version,
            header_sha256="n/a (initial state and field from " + JPL_FILE + ")",
            epoch_jd_tdb=float(scenario["initial_conditions"]["epoch_jd_tdb"]),
            au_km=0.0,
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
            JPL_FILE: common.sha256_file(jpl),
            "settings.toml": common.sha256_file(args.settings),
        },
        cpu=common.cpu_model(),
        wall_clock_s=round(wall, 3),
        output_sha256=common.sha256_file(path),
        units="t_days: TDB days since epoch_jd_tdb; position: km; velocity: km/s",
        frame="ICRF axes, heliocentric (particle minus Sun; center_naif 10)",
        time_scale="TDB",
        scenario=str(scenario["name"]),
        generated_utc=common.utc_now(),
        command="python -m oracles.orekit_runner "
        + " ".join(sys.argv[1:] if argv is None else argv),
        extra=extra,
    )
    common.write_manifest(args.out_dir, [entry])
    print(f"wrote {path} ({len(rows)} rows, {wall:.2f} s); {extra}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
