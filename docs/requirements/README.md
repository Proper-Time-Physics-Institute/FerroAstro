<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# FerroAstro requirements catalog

This directory holds FerroAstro's requirements: testable SHALL statements, each with an acceptance criterion and a source. They were consolidated from the strategy research journals in [`docs/strategy/journal/`](../strategy/journal/) (Journals #0–#6, 2026-10-14). All requirements start in `draft`. The decisions in Journal #6 (maintainer decisions) have already been applied.

- [OPEN-ITEMS.md](OPEN-ITEMS.md) lists every UNVERIFIED or secondary-source ([S]) item that still blocks a requirement.
- `TRACE.md` (requirement → tests → vector files → last CI run) is **generated** by `tools/trace.py`, which is built in milestone M0. Never edit it by hand (ORG-004).

## ID scheme

IDs have the form `<PREFIX>-NNN`. Each ID is unique and stable. **IDs are never renumbered or reused.** A requirement that is no longer wanted moves to `retired`. New requirements take the next free number in their prefix.

| Prefix | Meaning | Origin journal |
|---|---|---|
| `ORG` | Organization, governance, traceability, contribution policy | 05, revised by 06 |
| `STD` | Engineering standards: Rust, Python, SPDX/REUSE, commits, Actions | 05 |
| `PHY` | Physics truth data: expected values and tolerances for the Mercury ladder | 01 |
| `NUM` | Numerical methods: integrators, precision, precession estimator, EOMs, performance | 04 |
| `TST` | Test discipline and test-vector provenance (renamed from a colliding `NUM-` prefix) | 05 |
| `API` | Public library API: typed quantities, errors, examples | 05, revised by 06 |
| `AGT` | Agent-facing conventions: AGENTS.md, llms.txt, MCP output | 05 |
| `DEP` | Dependencies: crate reuse and dependency-graph licensing | 02 |
| `REF` | Reference implementations and ground-truth data (oracles) | 03 |
| `CI` | Oracle containers, goldens, job tiers, benchmarking | 03, revised by 06 |
| `LIC` | License isolation of GPL oracles and third-party sources | 03 |

## Requirement format

Each prefix file has an SPDX header, a scope paragraph, and one section per requirement:

````markdown
### PHY-004 — Rung GR-SCHW, analytic

```yaml
id: PHY-004
title: "…"
status: draft            # draft | approved | implemented | verified | retired
criticality: high        # high | medium | low
verification: test       # test | analysis | inspection | demonstration
statement: >-            # the SHALL text
  …
acceptance: >-           # concrete pass criterion, with numbers and tolerances
  …
source: >-               # journal file + primary citation
  …
parent: ORG-002          # parent requirement ID, or null
related: [PHY-005]       # cross-references (overlaps, dependencies)
notes: >-                # UNVERIFIED caveats, authority notes, revisions
  …
```
````

**Overlapping requirements.** Some streams state the same rule from different angles, for example PHY-012 and NUM-013, DEP-009 and LIC-004, REF-001 / LIC-003 / DEP-017, and DEP-007 and NUM-006. In each case both IDs are kept. They point at each other through `related:`, and the `notes:` field says which one is **AUTHORITATIVE** for which aspect. When two requirements disagree, the authoritative one wins until a maintainer reconciles them.

## Status lifecycle

```
draft → approved → implemented → verified → retired
```

| Status | Meaning | Who moves it |
|---|---|---|
| `draft` | Proposed. Wording, numbers or sources may still change. | Anyone, by PR |
| `approved` | Accepted by a maintainer. From this point `trace.py --check` requires at least one citing test when `verification: test`. | Maintainer review (CODEOWNERS on `docs/requirements/`) |
| `implemented` | Code exists and the citing tests are present. | PR that adds the implementation |
| `verified` | The acceptance criterion passed in CI (or the inspection/analysis/demonstration record exists), with evidence linked from TRACE.md. | CI evidence plus maintainer |
| `retired` | No longer applies. The ID stays reserved and is never reused. | Maintainer, with a reason in `notes` |

## Criticality tailoring

FerroAstro is research software. Its classification follows the spirit of **NASA NPR 7150.2 Class D/E** ("basic science, engineering design, and research and technology software" / "design concept, research, technology, and general-purpose software") and of **ECSS-E-ST-40C / Q-ST-80C criticality-based tailoring**. **FerroAstro makes no claim of compliance with either standard.** Details will live in `TAILORING.md` (ORG-007).

| Criticality | Applied to | What it demands |
|---|---|---|
| `high` | Physics correctness (every PHY rung, EOMs, estimator, constants, time scales, frames) and license/legal obligations (DEP license policy, LIC, DCO/CLA, SPDX) | Code implementing it needs a published-value test **and**, where an oracle exists, a reference-comparison test against an independent implementation. Two approving reviews on `crit:high` paths via CODEOWNERS. |
| `medium` | Infrastructure whose failure degrades evidence or reproducibility (CI tiers, oracles without unique coverage, diagnostics, API ergonomics) | At least one automated test or a recorded inspection. |
| `low` | Conveniences and documentation hygiene (mission doc, labels, llms.txt, stretch goals) | Inspection is enough. |

## How tests cite requirements

Tests name the requirement they verify in a doc comment or comment that `tools/trace.py` greps for:

```rust
/// Verifies: PHY-004
#[test]
fn gr_schwarzschild_analytic_rate() { /* loads vector MERC-GR-SCHW-ANALYTIC */ }
```

```python
# Verifies: PHY-016
def test_horizons_cross_check() -> None: ...
```

- Several IDs may be listed, comma-separated: `/// Verifies: PHY-010, NUM-015`.
- Test-vector files (`tests/vectors/*.toml`) also carry a `requirement = [...]` list (TST-002), which `trace.py` joins in.
- The form `// REQ: PHY-012` shown in Journal #5's TL;DR is **superseded** by `Verifies:`.
- CI fails when an `approved` requirement with `verification: test` has no citing test, or when a test cites an unknown ID (ORG-004).

## Traceability tooling

`TRACE.md` is generated by `tools/trace.py`, a ~150-line Python script run with `uv run` and built in **M0**. It parses the YAML blocks in this directory, greps `Verifies:` citations and vector `requirement` fields, and writes the requirement → tests → vectors → last-CI-run matrix. Per Journal #6, an ADR (`docs/adr/000x-requirements-tooling.md`) will evaluate **StrictDoc** and **OpenFastTrace** as replacements, revisited at **M2**.

## Summary

| Prefix | Count | File |
|---|---:|---|
| ORG | 10 | [ORG.md](ORG.md) |
| STD | 12 | [STD.md](STD.md) |
| PHY | 18 | [PHY.md](PHY.md) |
| NUM | 25 | [NUM.md](NUM.md) |
| TST | 5 | [TST.md](TST.md) |
| API | 3 | [API.md](API.md) |
| AGT | 4 | [AGT.md](AGT.md) |
| DEP | 17 | [DEP.md](DEP.md) |
| REF | 11 | [REF.md](REF.md) |
| CI | 10 | [CI.md](CI.md) |
| LIC | 4 | [LIC.md](LIC.md) |
| **Total** | **119** | |

Criticality split: 69 high, 35 medium, 15 low.

## Journal #6 revisions applied

| ID | Change |
|---|---|
| ORG-008 | Revised: every commit carries a DCO `Signed-off-by:` checked in CI. Agent commits are signed off by the operating human or org. |
| ORG-010 | New: CLA drafted, counsel-reviewed, then enforced (deferred). |
| CI-008 | Refined: benchmarks run on one fixed GitHub paid larger-runner class and are compared only within the same run. |
| CI-010 | New: long jobs checkpoint and resume across the 6 h per-job limit. |
| API-001 | Fixed: home-grown newtypes generic over `Real`, no `uom` (ADR-0001), compile-fail tests via `trybuild`. |
| ORG-003/004 | Unchanged in substance. `trace.py` now, with an ADR evaluating StrictDoc/OpenFastTrace (revisited at M2). |
