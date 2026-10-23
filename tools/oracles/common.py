# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Shared runner plumbing: scenarios, refusals (REF-011), hashes and ``MANIFEST.json`` (CI-003).

A runner reads a scenario with :func:`load_scenario`, calls :func:`refuse_unsupported` with the
switches and options it implements, and exits with :data:`REFUSAL_EXIT` and a message that names
every unsupported item when anything else is requested. It never omits a force silently.

Every output file gets one :class:`ManifestEntry` in ``<out-dir>/MANIFEST.json``:

``file``, ``tool``, ``version``, ``image_digest``, ``runner_sha``, ``runner_sha256``,
``input_sha256`` (file name -> SHA-256), ``cpu``, ``wall_clock_s``, ``output_sha256``,
``units``, ``frame``, ``time_scale``, ``scenario``, ``generated_utc``, ``command``.

``image_digest`` and ``runner_sha`` come from the host driver (``tools/oracles/run_oracles.sh``)
through the environment variables :data:`IMAGE_DIGEST_ENV` and :data:`RUNNER_SHA_ENV`, because a
container cannot see its own image digest or the repository's git commit. A runner refuses to
write a manifest without them (exit :data:`SETUP_EXIT`), so no golden carries an empty field.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import re
import tomllib
from collections.abc import Collection, Iterable, Mapping, Sequence
from dataclasses import asdict, dataclass, field
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

#: Exit status of a refused scenario (REF-011): non-zero and distinct from crashes (1) and usage
#: errors (2).
REFUSAL_EXIT = 3
#: Exit status of a runner that cannot write a valid golden (missing identity, bad input).
SETUP_EXIT = 4
#: Host-supplied image digest (``sha256:…`` of the image the runner executes in).
IMAGE_DIGEST_ENV = "FA_ORACLE_IMAGE_DIGEST"
#: Host-supplied git commit of the runner sources.
RUNNER_SHA_ENV = "FA_ORACLE_RUNNER_SHA"
#: The manifest file name in every output directory.
MANIFEST_NAME = "MANIFEST.json"
#: Manifest format version.
MANIFEST_SCHEMA_VERSION = 1

#: The CI-002 force switches, in the order of ``ferroastro_scenario::SWITCHES``.
SWITCHES: tuple[str, ...] = (
    "newton",
    "sun_j2",
    "gr_1pn_central",
    "gr_eih",
    "ppn",
    "lense_thirring",
    "asteroids",
    "kbos",
    "kbo_ring",
)

#: Fields that must be present and non-empty in every manifest entry (CI-003).
REQUIRED_FIELDS: tuple[str, ...] = (
    "file",
    "tool",
    "version",
    "image_digest",
    "runner_sha",
    "runner_sha256",
    "input_sha256",
    "cpu",
    "wall_clock_s",
    "output_sha256",
    "units",
    "frame",
    "time_scale",
    "scenario",
    "generated_utc",
    "command",
)


class Refusal(Exception):  # noqa: N818 - a refusal is the runner's answer, not an error
    """The scenario requests something this runner does not implement (REF-011)."""

    def __init__(self, tool: str, scenario: str, items: Sequence[tuple[str, str]]) -> None:
        self.tool = tool
        self.scenario = scenario
        self.items = list(items)
        named = "; ".join(f"`{item}` ({why})" for item, why in self.items)
        super().__init__(
            f"REFUSED: {tool} cannot run scenario {scenario!r}: it requests {named}. "
            "The runner never omits a force or option silently (REF-011)."
        )


class SetupError(Exception):
    """The runner cannot produce a valid golden (missing identity or malformed input)."""


def load_scenario(path: Path) -> dict[str, Any]:
    """Return the scenario TOML as a table (``schema_version`` 1, ``name`` a plain file stem)."""
    data = tomllib.loads(path.read_text(encoding="utf-8"))
    if data.get("schema_version") != 1:
        msg = f"{path}: schema_version {data.get('schema_version')!r} is not 1"
        raise SetupError(msg)
    safe_stem(data.get("name"))
    return data


SAFE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")


def safe_stem(name: object) -> str:
    """Return ``name`` if it is a plain file stem, else raise :class:`SetupError`.

    Runners name their output files after the scenario. A name with a path separator, ``..``, a
    leading dot or other characters could write outside ``--out-dir`` (PR #13 judge review
    R13-CODE-6), so it is refused before anything runs.
    """
    if not isinstance(name, str) or not SAFE_NAME.fullmatch(name) or ".." in name:
        msg = (
            f"scenario name {name!r} is not a plain file stem ([A-Za-z0-9][A-Za-z0-9._-]*, "
            "no '..'); it names the output file"
        )
        raise SetupError(msg)
    return name


def output_file(scenario: Mapping[str, Any], tool: str) -> str:
    """Return ``<scenario name>.<tool>.states.csv`` for a name that :func:`safe_stem` accepts."""
    return f"{safe_stem(scenario.get('name'))}.{tool}.states.csv"


def requested_switches(forces: Mapping[str, Any]) -> list[str]:
    """Return the switches a ``[forces]`` table turns on, unknown keys included (by name).

    ``ppn`` counts when present (any beta, gamma), ``asteroids`` when greater than 0, the others
    when true. Unknown keys are returned too, so a runner refuses them by name.
    """
    on: list[str] = []
    for key, value in forces.items():
        if key == "ppn":
            on.append(key)
        elif key == "asteroids":
            if isinstance(value, int) and value > 0:
                on.append(key)
        elif key not in SWITCHES or value is True:
            on.append(key)
    order = {s: i for i, s in enumerate(SWITCHES)}
    return sorted(on, key=lambda s: (order.get(s, len(SWITCHES)), s))


def refuse_unsupported(
    tool: str,
    scenario: Mapping[str, Any],
    supported_switches: Collection[str],
    checks: Iterable[tuple[str, bool, str]] = (),
) -> None:
    """Raise :class:`Refusal` naming every unsupported switch and every failed option check.

    ``checks`` are ``(item, supported, reason)`` triples for non-switch options (mode,
    integrator, precision, IC source, outputs).
    """
    forces = scenario.get("forces", {})
    items = [
        (s, f"{tool} implements only {', '.join(sorted(supported_switches)) or 'no switch'}")
        for s in requested_switches(forces)
        if s not in supported_switches
    ]
    items += [(item, why) for item, ok, why in checks if not ok]
    if items:
        raise Refusal(tool, str(scenario.get("name", "?")), items)


def sha256_file(path: Path) -> str:
    """Lower-case hex SHA-256 of a file."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_files(paths: Iterable[Path]) -> str:
    r"""SHA-256 over ``name\0sha256\n`` lines of the files, sorted by name (runner sources)."""
    lines = sorted(f"{p.name}\0{sha256_file(p)}\n" for p in paths)
    return hashlib.sha256("".join(lines).encode()).hexdigest()


def cpu_model() -> str:
    """Return the CPU model string (``/proc/cpuinfo``; the platform otherwise)."""
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                return f"{line.split(':', 1)[1].strip()} ({os.cpu_count()} logical CPUs)"
    except OSError:
        pass
    return platform.processor() or platform.machine() or "unknown"


def utc_now() -> str:
    """Return the current UTC time, ISO 8601 to the second."""
    return datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def runner_identity() -> tuple[str, str]:
    """``(image_digest, runner_sha)`` from the host driver's environment, or :class:`SetupError`."""
    digest = os.environ.get(IMAGE_DIGEST_ENV, "").strip()
    sha = os.environ.get(RUNNER_SHA_ENV, "").strip()
    missing = [n for n, v in ((IMAGE_DIGEST_ENV, digest), (RUNNER_SHA_ENV, sha)) if not v]
    if missing:
        msg = (
            f"{', '.join(missing)} not set: run the oracle through tools/oracles/run_oracles.sh, "
            "which passes the image digest and the runner commit (CI-003 fields must be non-empty)"
        )
        raise SetupError(msg)
    return digest, sha


@dataclass(frozen=True)
class ManifestEntry:
    """One output file's provenance record (CI-003)."""

    file: str
    tool: str
    version: str
    image_digest: str
    runner_sha: str
    runner_sha256: str
    input_sha256: dict[str, str]
    cpu: str
    wall_clock_s: float
    output_sha256: str
    units: str
    frame: str
    time_scale: str
    scenario: str
    generated_utc: str
    command: str
    extra: dict[str, str] = field(default_factory=dict)


def write_manifest(out_dir: Path, entries: Iterable[ManifestEntry]) -> Path:
    """Merge ``entries`` into ``out_dir/MANIFEST.json`` (one entry per file, sorted by file)."""
    path = out_dir / MANIFEST_NAME
    current: dict[str, dict[str, Any]] = {}
    if path.exists():
        old = json.loads(path.read_text(encoding="utf-8"))
        current = {str(e["file"]): e for e in old.get("entries", [])}
    for e in entries:
        current[e.file] = asdict(e)
    doc = {
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "entries": [current[k] for k in sorted(current)],
    }
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def validate_manifest(out_dir: Path) -> list[str]:
    """Return the CI-003 problems of ``out_dir`` (empty when valid).

    Every file but the manifest has one entry with every required field non-empty, and its
    ``output_sha256`` matches the file.
    """
    path = out_dir / MANIFEST_NAME
    if not path.exists():
        return [f"{out_dir}: no {MANIFEST_NAME}"]
    doc = json.loads(path.read_text(encoding="utf-8"))
    problems: list[str] = []
    if doc.get("schema_version") != MANIFEST_SCHEMA_VERSION:
        problems.append(f"{path}: schema_version {doc.get('schema_version')!r}")
    entries = {str(e.get("file")): e for e in doc.get("entries", [])}
    files = sorted(p.name for p in out_dir.iterdir() if p.is_file() and p.name != MANIFEST_NAME)
    for name in files:
        e = entries.get(name)
        if e is None:
            problems.append(f"{name}: no manifest entry")
            continue
        problems.extend(
            f"{name}: field `{k}` missing or empty"
            for k in REQUIRED_FIELDS
            if e.get(k) in (None, "", {}, [])
        )
        if e.get("output_sha256") != sha256_file(out_dir / name):
            problems.append(f"{name}: output_sha256 does not match the file")
    problems.extend(
        f"{name}: manifest entry without a file" for name in entries if name not in files
    )
    return problems
