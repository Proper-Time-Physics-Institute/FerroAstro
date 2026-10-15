<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# AGENTS.md

Canonical instructions for AI coding agents (and humans) working on FerroAstro (ADR-0007).
Crate-level `AGENTS.md` files add local physics conventions; the nearest file wins.

## Project in one paragraph
FerroAstro is an Apache-2.0 Rust astrodynamics ecosystem, agentic-first, that rebuilds the Fortran/Java
scientific-computing stack. Its flagship is the **Mercury perihelion precession ladder**
([docs/strategy/MISSION.md](docs/strategy/MISSION.md)). Each rung is a requirement with a cited value and tolerance.

## Status
Pre-M0: strategy, requirements and ADRs only. The Cargo workspace and CI land in milestone M0
([docs/strategy/ROADMAP.md](docs/strategy/ROADMAP.md)). The commands below are the contract that M0 implements.

## Commands (all must pass before a PR)
```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace && cargo test --doc --workspace
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace
(cd tools && uv run ruff check && uv run ruff format --check && uv run mypy .)
reuse lint
(cd tools && uv run python trace.py --check)
```

## Rules
1. **No bare physical literals in tests.** Load expected values by `id` from `tests/vectors/*.toml`, each with
   source, citation, tolerance and rationale (ADR-0004). Never edit a golden value or tolerance without
   updating its provenance; such PRs need the `golden-change` label.
2. **Cite requirements.** Tests carry `/// Verifies: PHY-004` (Rust) or `# Verifies: PHY-004` (Python).
   Requirement IDs live in [docs/requirements/](docs/requirements/).
3. **Types carry physics.** Use the frame, centre, time-scale and unit newtypes (ADR-0001). Never strip them to
   "make it compile".
4. **State the gauge and frame.** Every force model declares its gauge (PPN isotropic/harmonic by default),
   and every precession result declares its frame, epoch, span and fit model.
5. **No `unsafe`**, no `unwrap` in library code, no panics on user input. Errors use stable `FA-Exxxx` codes.
6. **Dependencies:** run `cargo deny check`. Never add GPL, LGPL or AGPL crates, `nyx-space`, or REBOUND/ASSIST
   bindings. Do not vendor and modify MPL files.
7. **Clean-room:** do not read REBOUND, REBOUNDx, ASSIST or Swiftest source when implementing integrators or force
   models. Work from the papers (DEP-008).
8. **UNVERIFIED values** listed in [docs/requirements/OPEN-ITEMS.md](docs/requirements/OPEN-ITEMS.md) must not
   be used as test oracles.
9. **Python** under `tools/` must be ruff-clean (lint + format) and pass mypy strict (STD-009).
10. **Commits:** Conventional Commits, with a DCO `Signed-off-by:` from the responsible human or organisation (ADR-0005).
    Agent-authored PRs need human approval before merge.

## Where things are
| Path | What |
|---|---|
| `docs/strategy/` | Mission, roadmap, architecture, research journal |
| `docs/requirements/` | Requirement catalogue (YAML blocks), open items, generated TRACE.md |
| `docs/adr/` | Decisions (MADR 4.0.0) |
| `docs/standards/` | Rust, Python and testing standards |
| `crates/` | Rust workspace (from M0) |
| `tests/vectors/` | Golden values with provenance (from M0) |
| `ci/` | Oracle containers, scenarios, goldens (from M3) |
| `tools/` | Python tooling (uv project) |
