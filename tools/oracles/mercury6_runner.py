# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Mercury6 oracle runner (REF-008: Newtonian only; stage A and the P-add configurations; REF-011).

Runs inside ``ci/oracles/mercury6`` only. Mercury6 (the ``smirik/mercury`` mirror, labelled
GPL-3.0; the original licence is UNVERIFIED, OI-25) is used as a black box through the documented
input files of its manual (``mercury6.man``: ``param.in``, ``big.in``, ``small.in``, ``files.in``,
``message.in``; ``element.in`` for ``element6``) and its source is never read (DEP-008). The
container receives a scenario and ``header.440`` and returns plain files (LIC-001).

Implemented (everything else is refused by name, exit status 3):

* ``mode = "n-body"`` with ``newton`` only. **Every GR switch is refused** (``gr_1pn_central``,
  ``gr_eih``, ``ppn``): Mercury6 has no relativity, and adding it through ``mfo_user`` would make
  the oracle our own code (REF-008);
* the Sun as ``central_body`` and first body, the other bodies Big bodies; header.440 ICs (made
  heliocentric) at ``JDEPOC``, DE440 GMs, ``f64``, forward spans, a cadence that divides the span.

The scenario's ``[integrator]`` names FerroAstro's IAS15; Mercury6 runs the settings of
``ci/oracles/mercury6/settings.toml`` (RA15 ``radau``, accuracy 1e-12; WP17 plan, NEEDS HUMAN
REVIEW), and the mapping is recorded in the manifest.

Units: Mercury6 takes masses in solar masses and multiplies them by its own k^2 (au^3/d^2 per
solar mass). The runner enters m = GM / k^2 with the Gaussian k = 0.01720209895, and checks that
choice black-box with a one-body calibration run before every integration: the semi-major axis
that ``element6`` reports must give back GM_sun + GM_body to < 1e-13 (it is not read from the
source). Output: heliocentric (``center_naif: 10``) ICRF positions (km) and velocities (km/s)
converted from ``element6``'s x y z u v w (au, au/day) with the header ``AU``.

Usage (inside the container; the driver is ``tools/oracles/run_oracles.sh``)::

    python -m oracles.mercury6_runner --scenario S.toml --header header.440 --out-dir OUT
"""

from __future__ import annotations

import argparse
import math
import shutil
import subprocess
import sys
import tempfile
import time
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from oracles import common, nbody_io
from oracles.header440 import BODIES, NAIF, Header440

TOOL = "mercury6"
SUPPORTED_SWITCHES = frozenset({"newton"})
SETTINGS = Path("/opt/fa/settings.toml")
BIN = Path("/opt/mercury6/bin")
SHARE = Path("/opt/mercury6/share")
#: The Gaussian gravitational constant (IAU 1938, a defined constant), k^2 in au^3/d^2 per M_sun.
GAUSS_K2 = 0.01720209895**2
#: Largest relative misfit of the black-box k^2 check (element6 stores ~15 significant figures).
K2_CHECK_TOLERANCE = 1e-13
FILES_IN = (
    "big.in",
    "small.in",
    "param.in",
    "xv.out",
    "ce.out",
    "info.out",
    "big.dmp",
    "small.dmp",
    "param.dmp",
    "restart.dmp",
)
COLUMNS = "x25e y25e z25e u25e v25e w25e"

SWITCH_REASONS: dict[str, str] = {
    "gr_1pn_central": "Mercury6 has no relativity; GR from it is never used (REF-008)",
    "gr_eih": "Mercury6 has no relativity; GR from it is never used (REF-008)",
    "ppn": "Mercury6 has no relativity; GR from it is never used (REF-008)",
    "sun_j2": "the central J2 input is not wired in this runner",
    "lense_thirring": "Mercury6 has no Lense-Thirring term",
    "asteroids": "the REF-003 small-body states are not an input of this runner",
    "kbos": "the REF-003 small-body states are not an input of this runner",
    "kbo_ring": "the DE440 KBO ring is not implemented in this runner",
}


def unsupported_options(s: Mapping[str, Any]) -> list[tuple[str, bool, str]]:
    """``(item, supported, reason)`` for the non-switch options of a scenario."""
    ic = s.get("initial_conditions", {})
    integ = s.get("integrator", {})
    span = s.get("span", {})
    out = s.get("output", {})
    forces = s.get("forces", {})
    bodies = [str(b) for b in s.get("bodies", [])]
    known = set(BODIES) | {"Earth", "Moon"}
    years = span.get("years")
    days = float(years) * nbody_io.DAYS_PER_JULIAN_YEAR if isinstance(years, (int, float)) else 0.0
    cadence = out.get("cadence_days")
    whole = (
        isinstance(cadence, (int, float)) and nbody_io.steps_per(days, float(cadence)) is not None
    )
    return [
        (
            "forces.newton",
            forces.get("newton") is True,
            "Mercury6 always applies Newtonian gravity",
        ),
        ("mode", s.get("mode") == "n-body", "only n-body mode (Big bodies) is implemented"),
        (
            "initial_conditions.source",
            ic.get("source") == "de440-header",
            "only de440-header initial conditions are implemented",
        ),
        ("constants.gm_set", s.get("constants", {}).get("gm_set") == "de440", "DE440 GMs only"),
        ("integrator.name", integ.get("name") == "ias15", "only the IAS15 mapping is defined"),
        ("integrator.precision", integ.get("precision") == "f64", "Mercury6 runs in f64"),
        ("central_body", s.get("central_body") == "Sun" and bodies[:1] == ["Sun"], "the Sun first"),
        (
            "bodies",
            all(b in known for b in bodies)
            and not ("EMB" in bodies and {"Earth", "Moon"} & set(bodies))
            and len(bodies) >= 2,
            "header.440 bodies, no EMB next to Earth or Moon, at least one Big body",
        ),
        ("span.years", days > 0.0, "forward spans only"),
        ("span.stop_after_passages", "stop_after_passages" not in span, "no passage events"),
        ("output.perihelion", not out.get("perihelion"), "no passage events in this runner"),
        ("output.cadence_days", whole, "the span must be a whole number of outputs"),
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


def fortran(x: float) -> str:
    """Format a binary64 in Fortran double-precision notation (``D`` exponent, round trip)."""
    s = repr(float(x))
    if "e" in s:
        return s.replace("e", "D")
    return s + "D0"


def mercury_name(body: str) -> str:
    """Mercury6 body name (upper case, at most 8 characters as the manual asks)."""
    return body.upper()[:8]


@dataclass(frozen=True)
class Run:
    """The run-specific values of ``param.in`` (days, solar masses, au)."""

    start: float
    stop: float
    cadence: float
    central_mass: float
    central_radius: float


def param_in(settings: Mapping[str, Any], run: Run) -> str:
    """Return ``param.in`` in the layout of the manual and the sample file."""
    return f""")O+_06 Integration parameters  (WARNING: Do not delete this line!!)
) Lines beginning with `)' are ignored.
)---------------------------------------------------------------------
) Important integration parameters:
)---------------------------------------------------------------------
 algorithm (MVS, BS, BS2, RADAU, HYBRID etc) = {settings["algorithm"]}
 start time (days)= {fortran(run.start)}
 stop time (days) = {fortran(run.stop)}
 output interval (days) = {fortran(run.cadence)}
 timestep (days) = {fortran(float(settings["first_step_days"]))}
 accuracy parameter={fortran(float(settings["accuracy"]))}
)---------------------------------------------------------------------
) Integration options:
)---------------------------------------------------------------------
 stop integration after a close encounter = no
 allow collisions to occur = no
 include collisional fragmentation = no
 express time in days or years = days
 express time relative to integration start time = yes
 output precision = high
 < not used at present >
 include relativity in integration= no
 include user-defined force = no
)---------------------------------------------------------------------
) These parameters do not need to be adjusted often:
)---------------------------------------------------------------------
 ejection distance (AU)= 1000D0
 radius of central body (AU) = {fortran(run.central_radius)}
 central mass (solar) = {fortran(run.central_mass)}
 central J2 = 0D0
 central J4 = 0D0
 central J6 = 0D0
 < not used at present >
 < not used at present >
 Hybrid integrator changeover (Hill radii) = 3.D0
 number of timesteps between data dumps = 5000
 number of timesteps between periodic effects = 100
"""


Big = tuple[str, float, Sequence[float], Sequence[float]]


def big_in(epoch: float, bodies: Sequence[Big]) -> str:
    """``big.in``: Cartesian heliocentric states (au, au/day), masses in solar masses."""
    lines = [
        ")O+_06 Big-body initial data  (WARNING: Do not delete this line!!)",
        ") Lines beginning with `)' are ignored.",
        ")---------------------------------------------------------------------",
        " style (Cartesian, Asteroidal, Cometary) = Cartesian",
        f" epoch (in days) = {fortran(epoch)}",
        ")---------------------------------------------------------------------",
    ]
    for name, m, r, v in bodies:
        lines += [
            f" {name} m={fortran(m)} r=3.D0 d=1.D0",
            "  " + " ".join(fortran(x) for x in r),
            "  " + " ".join(fortran(x) for x in v),
            "  0.D0 0.D0 0.D0",
        ]
    return "\n".join(lines) + "\n"


SMALL_IN = """)O+_06 Small-body initial data  (WARNING: Do not delete this line!!)
) Lines beginning with `)' are ignored.
)---------------------------------------------------------------------
 style (Cartesian, Asteroidal, Cometary) = Cartesian
)---------------------------------------------------------------------
"""


def element_in(cadence: float, columns: str) -> str:
    """``element.in``: central-body (heliocentric) Cartesian output at every stored time."""
    return f""")O+_06 element  (WARNING: Do not delete this line!!)
) Lines beginning with `)' are ignored.
)---------------------------------------------------------------------
 number of input files = 1
)---------------------------------------------------------------------
) List the input files, one per line
 xv.out
)---------------------------------------------------------------------
 type of elements (central body, barycentric, Jacobi) = Central
 minimum interval between outputs (days) = {fortran(cadence / 2.0)}
 express time in days or years = days
 express time relative to integration start time = yes
)---------------------------------------------------------------------
) Output format? (e.g. a8.4 => semi-major axis with 8 digits & 4 dec. places)
 {columns}
)---------------------------------------------------------------------
) Which bodies do you want? (List one per line or leave blank for all bodies)
"""


def integrate(work: Path, param: str, big: str, columns: str, cadence: float) -> str:
    """Run ``mercury6`` then ``element6`` in ``work``; return ``info.out``."""
    work.mkdir(parents=True, exist_ok=True)
    shutil.copy(SHARE / "message.in.sample", work / "message.in")
    (work / "files.in").write_text("\n".join(FILES_IN) + "\n", encoding="utf-8")
    (work / "param.in").write_text(param, encoding="utf-8")
    (work / "big.in").write_text(big, encoding="utf-8")
    (work / "small.in").write_text(SMALL_IN, encoding="utf-8")
    (work / "element.in").write_text(element_in(cadence, columns), encoding="utf-8")
    for exe in ("mercury6", "element6"):
        r = subprocess.run([str(BIN / exe)], cwd=work, capture_output=True, text=True, check=False)  # noqa: S603
        if r.returncode != 0:
            msg = f"{exe} exit {r.returncode}: {r.stdout[-500:]} {r.stderr[-500:]}"
            raise ValueError(msg)
    return (work / "info.out").read_text(encoding="utf-8")


def read_aei(path: Path) -> list[list[float]]:
    """Rows of an ``element6`` ``.aei`` file (time, then the requested columns)."""
    rows = []
    for line in path.read_text(encoding="utf-8").splitlines():
        f = line.split()
        if not f:
            continue
        try:
            rows.append([float(x.replace("D", "E")) for x in f])
        except ValueError:
            continue
    return rows


def calibrate_k2(work: Path, header: Header440, settings: Mapping[str, Any]) -> float:
    """Black-box check of k^2: Sun + Mercury for one day; return GM_eff / (GM_sun + GM_Me) - 1."""
    gms, ps, vs = header.state("Sun")
    gm, p, v = header.state("Mercury")
    r = [p[k] - ps[k] for k in range(3)]
    w = [v[k] - vs[k] for k in range(3)]
    epoch = header.value("JDEPOC")
    param = param_in(settings, Run(epoch, epoch + 1.0, 1.0, gms / GAUSS_K2, 0.005))
    integrate(work, param, big_in(epoch, [("MERCURY", gm / GAUSS_K2, r, w)]), "a25e", 1.0)
    a = read_aei(work / "MERCURY.aei")[0][1]
    rr = math.sqrt(sum(x * x for x in r))
    v2 = sum(x * x for x in w)
    gm_eff = v2 / (2.0 / rr - 1.0 / a)
    return gm_eff / (gms + gm) - 1.0


def run(
    scenario: Mapping[str, Any], header: Header440, settings: Mapping[str, Any], work: Path
) -> tuple[list[list[str]], dict[str, str], str]:
    """Integrate; return the CSV rows, extra manifest fields and the engine version string."""
    misfit = calibrate_k2(work / "calibration", header, settings)
    if abs(misfit) > K2_CHECK_TOLERANCE:
        msg = f"Mercury6's k^2 is not the Gaussian value: GM misfit {misfit:.3e}"
        raise ValueError(msg)
    bodies = [str(b) for b in scenario["bodies"]]
    au_km = header.value("AU")
    epoch = header.value("JDEPOC")
    days = nbody_io.span_days(scenario)
    cadence = float(scenario["output"]["cadence_days"])
    gms, ps, vs = header.state("Sun")
    big = []
    for b in bodies[1:]:
        gm, p, v = header.state(b)
        r = [p[k] - ps[k] for k in range(3)]
        w = [v[k] - vs[k] for k in range(3)]
        big.append((mercury_name(b), gm / GAUSS_K2, r, w))
    param = param_in(
        settings,
        Run(epoch, epoch + days, cadence, gms / GAUSS_K2, header.value("ASUN") / au_km),
    )
    info = integrate(work / "run", param, big_in(epoch, big), COLUMNS, cadence)
    n_out = nbody_io.steps_per(days, cadence)
    keep = [str(b) for b in scenario.get("output", {}).get("sample_bodies", bodies)]
    series: dict[str, list[list[float]]] = {}
    for b in bodies[1:]:
        if b not in keep:
            continue
        rows_b = read_aei(work / "run" / f"{mercury_name(b)}.aei")
        if n_out is None or len(rows_b) != n_out + 1:
            msg = f"{b}: {len(rows_b)} element6 rows, expected {n_out} + 1"
            raise ValueError(msg)
        series[b] = rows_b
    rows: list[list[str]] = []
    max_dt = 0.0
    for k in range(n_out + 1 if n_out is not None else 0):
        t = k * cadence
        for b, rb in series.items():
            max_dt = max(max_dt, abs(rb[k][0] - t))
            rows.append(nbody_io.state_row(t, NAIF[b], rb[k][1:4], rb[k][4:7], au_km))
    if max_dt > 1e-5:
        msg = f"element6 output times deviate from k * cadence by {max_dt} d"
        raise ValueError(msg)
    energy_line = next(
        (ln for ln in info.splitlines() if "Fractional energy change due to integrator" in ln), ""
    )
    compiler = (SHARE / "compiler.txt").read_text(encoding="utf-8").strip()
    extra = {
        "mercury6_source": "github.com/smirik/mercury@aee9e0f6b8e4d359a9ed3607ee12e34a2e2dafac",
        "compiler": compiler,
        "algorithm": str(settings["algorithm"]),
        "accuracy": nbody_io.fmt(float(settings["accuracy"])),
        "first_step_days": nbody_io.fmt(float(settings["first_step_days"])),
        "output_precision": "high",
        "k2_used": nbody_io.fmt(GAUSS_K2),
        "k2_blackbox_gm_misfit": f"{misfit:.3e}",
        "relative_energy_change": energy_line.split(":")[-1].strip() or "n/a",
        "max_output_time_deviation_days": nbody_io.fmt(max_dt),
        "integrator_mapping": nbody_io.mapped_integrator_note(scenario, settings),
    }
    return rows, extra, "mercury6 (smirik/mercury aee9e0f6, Chambers 1999 package version 6)"


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
        check(scenario)
        digest, runner_sha = common.runner_identity()
        header = Header440.read(args.header)
        started = time.perf_counter()
        with tempfile.TemporaryDirectory(prefix="mercury6-") as work:
            rows, extra, version = run(scenario, header, settings, Path(work))
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
    nbody_io.write_states_csv(
        path,
        nbody_io.StatesHeader(
            title=f"Mercury6 states of scenario {scenario['name']} (REF-008 oracle output)",
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
        command="python -m oracles.mercury6_runner "
        + " ".join(sys.argv[1:] if argv is None else argv),
        extra=extra,
    )
    common.write_manifest(args.out_dir, [entry])
    print(f"wrote {path} ({len(rows)} rows, {wall:.2f} s); {extra}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
