# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Crate layering check over ``cargo metadata`` (CRATE-PLAN §1, ARCHITECTURE "Layering").

Rules, applied to dependencies between workspace crates:

* every workspace crate has a layer in :data:`LAYERS` (a new crate must be placed explicitly);
* a normal or build dependency points to a strictly lower layer;
* a dev-dependency points to a strictly lower layer or to a test-support crate
  (:data:`TEST_SUPPORT`);
* a test-support crate is never a normal or build dependency, except for the pairs listed in
  :data:`EXCEPTIONS`, each with its reason;
* the graph of all workspace edges (normal, build and dev) has no cycle.

Exit code 0 when the workspace obeys the rules; 1 otherwise.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent

# Layers of CRATE-PLAN §1 (L0 foundation ... L12 surfaces). Planned crates are listed so that the
# table does not have to change when they arrive.
LAYERS: dict[str, int] = {
    "ferroastro-real": 0,
    "ferroastro-core": 1,
    "ferroastro-erfa": 1,
    "ferroastro-time": 2,
    "ferroastro-constants": 2,
    "ferroastro-vectors": 2,
    "ferroastro-testdata": 2,
    "ferroastro-frames": 3,
    "ferroastro-ephem": 4,
    "ferroastro-horizons": 4,
    "ferroastro-analytic": 4,
    "ferroastro-integrators": 4,
    "ferroastro-system": 5,
    "ferroastro-precession": 5,
    "ferroastro-scenario": 5,
    "ferroastro-forces": 6,
    "ferroastro-abm": 6,
    "ferroastro-eih": 7,
    "ferroastro-symplectic": 7,
    "ferroastro-propagate": 8,
    "ferroastro-diffrun": 9,
    "ferroastro-ladder": 10,
    "ferroastro-report": 11,
    "ferroastro-bench": 11,
    "ferroastro-oracle": 11,
    "ferroastro-cli": 12,
    "ferroastro-mcp": 12,
    "ferroastro-wasm": 12,
    "ferroastro-py": 12,
}

# Dev-dependency-only crates (CRATE-PLAN: "test-support, dev-dependency only").
TEST_SUPPORT: frozenset[str] = frozenset({"ferroastro-vectors", "ferroastro-testdata"})

# (crate, dependency) -> reason. Each entry is a deliberate, documented deviation.
EXCEPTIONS: dict[tuple[str, str], str] = {
    ("ferroastro-cli", "ferroastro-vectors"): (
        "the CLI ladder report compares each rung with its cited test vector at run time "
        "(WP4); NEEDS HUMAN REVIEW in IMPLEMENTATION-NOTES WP6"
    ),
}

Edge = tuple[str, str, str]  # (crate, dependency, kind: "normal" | "build" | "dev")


def workspace_edges(metadata: Mapping[str, Any]) -> tuple[list[str], list[Edge]]:
    """Return the workspace crate names and the edges between them from ``cargo metadata``."""
    members = set(metadata.get("workspace_members", []))
    packages = [p for p in metadata.get("packages", []) if p.get("id") in members]
    names = sorted(str(p["name"]) for p in packages)
    local = set(names)
    edges: list[Edge] = []
    for p in packages:
        for d in p.get("dependencies", []):
            dep = str(d["name"])  # the package name, also for renamed dependencies
            if dep in local:
                kind = d.get("kind") or "normal"
                if dep == p["name"] and kind == "dev":
                    # A self dev-dependency only turns on the crate's own features for its tests
                    # (e.g. ferroastro-time's `hifitime`); it is not an edge between crates.
                    continue
                edges.append((str(p["name"]), dep, str(kind)))
    return names, sorted(set(edges))


def _cycles(names: Iterable[str], edges: Iterable[Edge]) -> list[list[str]]:
    graph: dict[str, list[str]] = {n: [] for n in names}
    for src, dst, _ in edges:
        graph.setdefault(src, []).append(dst)
        graph.setdefault(dst, [])
    white, grey, black = 0, 1, 2
    colour = dict.fromkeys(graph, white)
    found: list[list[str]] = []
    stack: list[str] = []

    def visit(node: str) -> None:
        colour[node] = grey
        stack.append(node)
        for nxt in sorted(graph[node]):
            if colour[nxt] == grey:
                found.append([*stack[stack.index(nxt) :], nxt])
            elif colour[nxt] == white:
                visit(nxt)
        stack.pop()
        colour[node] = black

    for n in sorted(graph):
        if colour[n] == white:
            visit(n)
    return found


def check(
    names: Iterable[str],
    edges: Iterable[Edge],
    layers: Mapping[str, int] = LAYERS,
    exceptions: Mapping[tuple[str, str], str] = EXCEPTIONS,
) -> list[str]:
    """Return one finding per violated rule (empty when the graph is clean)."""
    names = list(names)
    edges = list(edges)
    findings = [
        f"{n}: no layer assigned in tools/check_layers.py LAYERS" for n in names if n not in layers
    ]
    for src, dst, kind in edges:
        if src not in layers or dst not in layers:
            continue
        ls, ld = layers[src], layers[dst]
        if (src, dst) in exceptions:
            continue
        if kind == "dev":
            if dst not in TEST_SUPPORT and ld >= ls:
                findings.append(f"{src} (L{ls}) dev-depends on {dst} (L{ld}): upward or same layer")
            continue
        if dst in TEST_SUPPORT:
            findings.append(f"{src} has a {kind} dependency on test-support crate {dst}")
        elif ld >= ls:
            findings.append(f"{src} (L{ls}) depends on {dst} (L{ld}): upward or same layer")
    findings += [f"dependency cycle: {' -> '.join(c)}" for c in _cycles(names, edges)]
    return findings


def cargo_metadata(root: Path = ROOT) -> dict[str, Any]:
    """Run ``cargo metadata --no-deps`` in ``root`` and parse its JSON."""
    cargo = shutil.which("cargo")
    if cargo is None:
        msg = "cargo not found on PATH"
        raise RuntimeError(msg)
    out = subprocess.run(  # noqa: S603 - fixed argument list, no shell
        [cargo, "metadata", "--format-version", "1", "--no-deps", "--offline"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    data: dict[str, Any] = json.loads(out.stdout)
    return data


def check_repo(root: Path = ROOT) -> list[str]:
    """Check the workspace at ``root``."""
    names, edges = workspace_edges(cargo_metadata(root))
    return check(names, edges)


def main() -> int:
    """Entry point; returns the process exit code."""
    names, edges = workspace_edges(cargo_metadata(ROOT))
    findings = check(names, edges)
    for (src, dst), why in sorted(EXCEPTIONS.items()):
        if any(e[0] == src and e[1] == dst for e in edges):
            print(f"note: allowed exception {src} -> {dst}: {why}")
    for f in findings:
        print(f"error: {f}", file=sys.stderr)
    print(
        f"check_layers: {len(names)} crates, {len(edges)} workspace edges, {len(findings)} error(s)"
    )
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
