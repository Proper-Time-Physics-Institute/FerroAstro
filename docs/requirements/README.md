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

`TRACE.md` is generated by `tools/trace.py`, a ~150-line Python script run with `uv run` and built in **M0**. It parses the YAML blocks in this directory, greps `Verifies:` citations and vector `requirement` fields, and writes the requirement → tests → vectors → last-CI-run matrix. Per Journal #6, ADR-0010 (`docs/adr/0010-requirements-tooling.md`) evaluates **StrictDoc** and **OpenFastTrace** as replacements, revisited at **M2**.

## Summary

| Prefix | Count | File |
|---|---:|---|
| ORG | 10 | [ORG.md](ORG.md) |
| STD | 12 | [STD.md](STD.md) |
| PHY | 19 | [PHY.md](PHY.md) |
| NUM | 25 | [NUM.md](NUM.md) |
| TST | 5 | [TST.md](TST.md) |
| API | 3 | [API.md](API.md) |
| AGT | 4 | [AGT.md](AGT.md) |
| DEP | 17 | [DEP.md](DEP.md) |
| REF | 11 | [REF.md](REF.md) |
| CI | 10 | [CI.md](CI.md) |
| LIC | 5 | [LIC.md](LIC.md) |
| **Total** | **121** | |

Criticality split: 74 high, 32 medium, 15 low.

Counts re-checked on 2026-10-20 after the build-evidence sync: unchanged (no requirement added, retired or re-rated; every status is still `draft`). Open items: 78 in total (OI-01…OI-78), 2 closed (OI-05, OI-23), 76 open; OI-73…OI-78 are new.

## Journal #6 revisions applied

| ID | Change |
|---|---|
| ORG-008 | Revised: every commit carries a DCO `Signed-off-by:` checked in CI. Agent commits are signed off by the operating human or org. |
| ORG-010 | New: CLA drafted, counsel-reviewed, then enforced (deferred). |
| CI-008 | Refined: benchmarks run on one fixed GitHub paid larger-runner class and are compared only within the same run. |
| CI-010 | New: long jobs checkpoint and resume across the 6 h per-job limit. |
| API-001 | Fixed: home-grown newtypes generic over `Real`, no `uom` (ADR-0001), compile-fail tests via `trybuild`. |
| ORG-003/004 | Unchanged in substance. `trace.py` now, with an ADR evaluating StrictDoc/OpenFastTrace (revisited at M2). |

## Journal #9 (red-team review) revisions applied

Applied 2026-10-14. Open items raised by the review are in [OPEN-ITEMS.md](OPEN-ITEMS.md) (OI-72 and caveats on existing items).

| ID | Change |
|---|---|
| PHY-019 | New: rung N-AST asteroid differential, 0.0012 ± 0.0003 ″/cy (provisional protocol; blocked on OI-38, OI-24). |
| LIC-005 | New: a GPL oracle container never contains a FerroAstro build; exchange is files between separate jobs. |
| PHY-006, PHY-009, PHY-012, PHY-014, NUM-013, NUM-015, NUM-017 | Acceptance corrected (ratio test, per-unit coefficients, provisional protocol, J2-dependent expected value, absolute floor, labelled gauge exemptions). |
| DEP-002, DEP-003, DEP-008, DEP-009, DEP-015, LIC-001, LIC-002, LIC-004 | Licensing fixes: MPL only by exception, anise deferred to M5, rug scope, session clean-room rule, per-file ERFA notices, hash-pinned clarify entries, private GPL oracle images. |
| REF-003, PHY-016, PHY-017 | Criticality raised to high. |
| STD-005, STD-007 | MSRV wording aligned with ADR-0006; semver gating from M6. |

## Build-evidence sync 2026-10-20

Applied 2026-10-20 from the build branches `m0-foundation` and `m3-newtonian-nbody`. Evidence for every row is in `docs/strategy/IMPLEMENTATION-NOTES.md` on `m3-newtonian-nbody` (work packages WP1–WP18) and the file or test named in each requirement's `notes`. Maintainer decisions of 2026-10-20 (A–D) are applied. Statuses stay `draft`. No tolerance was loosened. Where an acceptance changed, the requirement's `notes` give the reason and the evidence.

| ID | Change | Evidence (WP) |
|---|---|---|
| REF-002, PHY-002 | OI-23 closed: header states are SSB-centred ICRF in au and au/day; the state part of REF-002 is no longer blocked. OI-05 closed (exact au). PHY-002/REF-002 now wait only on OI-04 (GMSDOT units). | WP11 check 1 (`ref002_header_initial_conditions_equal_de440_at_jdepoc`), WP16 check 3, WP1 review |
| PHY-012 | **Acceptance changed**: "report both" protocol; P-remove asserted, P-add reported (ADR-0014). Chosen after the results were known; PROVISIONAL until OI-38 is confirmed against the Park 2017 text, with a revert condition. Tolerance and expected values unchanged. | WP14 (`nightly_phy012.rs`), WP17 (four-tool P-add agreement), progress reviews after WP15/WP18 |
| NUM-013 | **Acceptance changed** for part (b), as PHY-012; cross-term sign convention must be labelled; no longer blocked on OI-38. | WP14, WP17, progress review after WP18 (R1) |
| PHY-019 | **Acceptance changed**: protocol as PHY-012, and the rung gates independently of OI-38 (protocol dependence ≈ 3e-6 ″/cy, 1% of ±0.0003). Tolerance unchanged. | WP15 (`nightly_phy013.rs`), progress review after WP15 (F9) |
| DEP-011 | **Acceptance changed**: until the M5 anise decision (OI-72), the ANISE comparison is replaced by DE440 ASCII coefficient identity, the geocentric-Moon check and a Horizons DE441 gross bound; the ANISE clause stays conditional on adoption. | WP10 |
| NUM-008 | **Acceptance tightened**: in the q-formulation the default 12-term series must match a double-double difference to ≤ 4 ε (4 terms leave 6.4e6 ε). | WP12 (U3) |
| NUM-014 | Acceptance names the reference plane (IAU 2006 J2000 ecliptic) and tolerances (1e-3 vs p_A, 1e-5 vs exact geometry); no longer blocked on OI-02 (p_A from the verified `p06e` port). | WP9 |
| NUM-006, NUM-010, DEP-010, DEP-012, REF-009, REF-011, NUM-009 | Acceptance clarified (benchmark band ±2 % PROVISIONAL; 1e-6 s is the location error and round-off is backend-dependent; time-scale tolerances stated; u² = 2⁻¹⁰⁶ named; REF-009 split into three evidenced parts; FerroAstro's own FA-E1201/FA-E1202 refusals; the < 2× bound applies to EIH Majors). | WP7, WP7, WP8, WP7, WP8/9/16, WP13, WP15 |
| REF-001, REF-003, REF-005, REF-006, REF-007, REF-008 | Notes: fetch tool, MANIFEST pins and testeph threshold; sb441-n373s (DE441) and the DE440-asteroid-IC gap; oracle packaging, goldens and post-measurement oracle changes (NEEDS HUMAN REVIEW). | WP10, WP15, WP16, WP17 |
| CI-001, CI-002, CI-004, CI-005, CI-006, CI-010 | Notes: local unpushed images with digest in each manifest; scenario schema, new switches/mode and per-tool integrator mapping; golden locations and local generation; `.fa-data` cache; drift-issue mechanism and incomplete nightly; resume tested in a new process. | WP16, WP13/15/17, WP16, WP10, WP17/18, WP13 |
| PHY-004, PHY-010, PHY-011, NUM-011, NUM-015, NUM-018, NUM-019, NUM-022 | Notes: μ convention (OI-77); default WGCCRE 2015 solar pole and J2 bodies (OI-76); OI-35 evidence pending sign-off; open NUM-011 decisions (OI-74); μ* re-read (OI-75); J2 reaction evidence; scaling not testable on a shared machine. | WP11/13, WP9/12, WP4, WP5, WP15, WP12, WP13 |
| DEP-009, LIC-004 | Notes: ERFA 2.0.1 source, naming without ERFA's `era` or SOFA's `iau` prefix, and the list of ported routines. | WP8, WP9 |
| TST-002, ORG-004 | Notes: the `blocked_by` vector field and its auto-gating rule; a proposal that `trace.py` accept CI-gate evidence (ADR-0010, pending judge review). | WP14, WP6 |

ADRs: new ADR-0014 (per-body contribution protocol); ADR-0004 revision (`blocked_by`, proposed); ADR-0010 proposal (CI-gate evidence). Strategy: ARCHITECTURE.md now lists the 21 crates that exist on the build branches and their layers; ROADMAP.md's M0 exit depends only on OI-04, and the M3 exit names the provisional protocol.
