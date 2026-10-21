<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Architecture

## Layering

The crates are built bottom-up. Each layer depends only on the layers below it, and everything up to
`ferroastro-ladder` builds on stable Rust for `wasm32-unknown-unknown` with no C or Fortran toolchain
(DEP-006). The layer of every crate is fixed in `tools/check_layers.py` (`LAYERS`), which runs inside
`trace.py --check` and fails on an unplaced crate, an upward or same-layer edge, or a cycle.

**Status (build-evidence sync 2026-10-20).** The diagram and table below follow the crates that exist on the
build branches (`m0-foundation`, `m3-newtonian-nbody`; 21 crates, 119 edges, 0 layer errors in the progress
review after WP18) and `tools/check_layers.py` there. Crates marked *(planned)* do not exist yet. Deviations
from the original plan are listed under the table, each with its work package (WP) in
`docs/strategy/IMPLEMENTATION-NOTES.md` on `m3-newtonian-nbody`.

```
 L12 surfaces   ferroastro-cli   ferroastro-mcp* ferroastro-wasm* (ferroastro-py*, tooling only)
 L11 tooling    ferroastro-oracle   ferroastro-bench   ferroastro-report*
 L10 showcase   ferroastro-ladder                       single-body rungs, rung catalogue
 L9  analysis   ferroastro-diffrun                      differential-run harness (P-remove / P-add, ADR-0014)
 L8  driver     ferroastro-propagate                    scenario runs, dense output, passages, checkpoint/resume
 L7             ferroastro-eih*   ferroastro-symplectic*
 L6  dynamics   ferroastro-forces   ferroastro-abm*     force terms, N-body model, tiers
 L5             ferroastro-system   ferroastro-precession   ferroastro-scenario
                (catalogue, ICs, SSB)  (estimators only)     (scenario spec, IC hash)
 L4             ferroastro-ephem   ferroastro-analytic   ferroastro-integrators   ferroastro-horizons*
 L3  reference  ferroastro-frames                       ICRF, ECLIPJ2000, IAU 2006, mean-orbit, solar equator
 L2             ferroastro-time   ferroastro-constants  (test support: ferroastro-vectors, ferroastro-testdata)
 L1  foundation ferroastro-core   ferroastro-erfa       newtypes, errors; ERFA port
 L0             ferroastro-real                         Real trait, f64 / double-double / f128
 (* = planned)
```

| Crate (layer) | Responsibility | Key requirements | Reuse |
|---|---|---|---|
| `ferroastro-real` (L0) | `Real` trait; `f64`, own double-double, optional nightly `f128`; compensated summation; `no_std` with `libm` | DEP-007, NUM-006, DEP-012 | own DD (chosen by benchmark against `qd`/`twofloat`, WP7); `astro-float` as test oracle |
| `ferroastro-core` (L1) | Newtypes: `Length`, `Velocity`, `GravParam`, `Angle`; `Epoch<S: TimeScale>`; `State<F: Frame, C: Center>`; frame/centre/time-scale markers; the shared `FA-Exxxx` error codes; compile-fail tests | API-001, API-002 | home-grown (ADR-0001) |
| `ferroastro-erfa` (L1) | Ports of ERFA time and precession routines, generic over `Real`, `no_std`; licence `Apache-2.0 AND BSD-3-Clause` | DEP-009, LIC-004, REF-009 | ERFA 2.0.1 (BSD-3) |
| `ferroastro-time` (L2) | TAI/TT/TDB/TCB/TCG conversions; UTC only through the optional `hifitime` feature | DEP-010, REF-009 | `hifitime` (MPL-2.0, unmodified, optional) + `ferroastro-erfa` |
| `ferroastro-constants` (L2) | Versioned DE440 constants generated from `header.440`; IAU 2015 nominal values quarantined in `nominal` | PHY-002/003, REF-002 | own |
| `ferroastro-vectors`, `ferroastro-testdata` (L2, test support) | Test-vector loader (refuses `FA_VECTORS_DIR` for canonical runs); pinned data fetch and cache verification (`publish = false`) | TST-002/003, CI-005 | own |
| `ferroastro-frames` (L3) | ICRF ↔ ECLIPJ2000, IAU 2006 ecliptic of date, Mercury mean-orbit frame, solar equator (configurable pole) | NUM-014, PHY-010, PHY-015 | `ferroastro-erfa` |
| `ferroastro-ephem` (L4) | DAF/SPK type 2/3 reader generic over `Real`; testpo parser | DEP-011, REF-001, REF-004 | own; validated against `jplephem` (`anise` is not a dependency; decision deferred to M5, OI-72) |
| `ferroastro-analytic` (L4) | Analytic oracles (GR, PPN, SR exact, J2 planar/inclined, LT, Kepler) | PHY-004, NUM-015 | own |
| `ferroastro-integrators` (L4) | IAS15 (default) with dense output, event location and snapshots; later ABM, IRKGL16, WH/Saha–Tremaine | NUM-001…005, NUM-010, CI-010 | own (clean-room) |
| `ferroastro-system` (L5) | Body catalogue and tiers, initial conditions from `header.440`/SPK/states, small-body placement, relativistic SSB | REF-002, REF-003, NUM-009, NUM-018 | own |
| `ferroastro-precession` (L5) | Estimators on sampled data only (Park2017, passages fit, LRL) and rate reports; no propagation | NUM-011, NUM-012, PHY-015 | own |
| `ferroastro-scenario` (L5) | Scenario spec (TOML + committed JSON Schema), refusal of unsupported switches, canonical IC hash | CI-002, REF-011, NUM-013 | `schemars` |
| `ferroastro-forces` (L6) | Force terms with declared gauge; test-particle and N-body models; tiers (Major/Minor/Ring); geocentric Moon; J2 with reaction | NUM-008/009/017/019 | own (clean-room, DEP-008) |
| `ferroastro-propagate` (L8) | Runs a scenario: ICs, model, IAS15, cadence samples, perihelion events, checkpoint/resume, batches | CI-010, NUM-010, NUM-022, TST-005 | `postcard`, `rayon` (optional) |
| `ferroastro-diffrun` (L9) | Differential-run harness: identical-IC check, checkpointed legs, protocol combinations | NUM-013, PHY-012, PHY-013, PHY-019, CI-009 | own |
| `ferroastro-ladder` (L10) | Single-body rungs, rung catalogue, examples per rung | ORG-002, API-003 | own |
| `ferroastro-oracle` (L11) | Oracle golden manifests and state comparison (`publish = false`) | CI-003, REF-005…008 | own |
| `ferroastro-bench` (L11) | Speed-table assembly and accuracy columns (`publish = false`) | CI-008, NUM-024 | own |
| `ferroastro-cli` (L12) | `ferroastro ladder …`, `ferroastro scenario validate`, `run`, `resume` | REF-011, CI-010 | hand-written argument parsing (no `clap` yet) |
| `ferroastro-mcp` (L12, planned) | MCP tools with `outputSchema` / `structuredContent` | DEP-013, AGT-004 | `rmcp` 3.x |
| `ferroastro-wasm` (L12, planned) | Browser bindings for the demo | DEP-006 | `wasm-bindgen` |

**Deviations from the original plan (all recorded in IMPLEMENTATION-NOTES):**
- `ferroastro-units` was never created. Its M0 subset (quantities, frame/centre and time-scale markers, `Epoch`) lives in
  `ferroastro-core`, which also holds the shared error codes (WP1 "Crate naming"); `Real` lives in `ferroastro-real`
  (WP7). `ferroastro-core` is the name to use.
- `ferroastro-precession` contains no propagation. The analytic oracles moved to `ferroastro-analytic`, the passage
  run to `ferroastro-propagate::passages` (re-exported by `ferroastro-ladder`), and the differential harness is
  `ferroastro-diffrun` (WP6, WP13, WP14).
- Forces and integrators are separate layers: `ferroastro-forces` (L6) implements `SecondOrderSystem` from
  `ferroastro-integrators` (L4), a downward edge (WP6).
- Edges beyond the original plan, all downward: `propagate → precession, analytic, frames` (passage search, solar
  pole), `cli → propagate, scenario, ephem` (WP13 deviation 6), `forces → ephem, system`, `system → frames`
  (WP12, WP15), `precession → frames` (WP9).
- `cli → vectors` is a runtime edge to a test-support crate, the single listed exception in `check_layers.py`;
  NEEDS HUMAN REVIEW (WP6).
- The CI `wasm32` job builds real, core, erfa, time, integrators, forces, analytic, frames, ephem, system, scenario,
  propagate (without `parallel`) and diffrun; real and erfa also as `no_std` + `libm`, and time with `hifitime`.

## Design rules

1. **Scalar genericity is non-negotiable in the dynamics path.** Integrators, forces, the ephemeris evaluator and
   time arithmetic are generic over `Real`, and monomorphisation keeps f64 cost-free. Ephemeris mode follows the
   Subbotin–Kodukov–Pavlov mixed scheme: the state and accumulators are double-double and the forces are f64 (NUM-007).
2. **Types carry the physics.** Frames, centres, time scales and units are type parameters. Converting between
   them is an explicit function that cites its source (ERFA routine, IERS chapter).
3. **No silent physics.** Each force term declares its gauge (PPN isotropic/harmonic by default) and its
   parameters (β, γ, J2, S☉). Scenario runners refuse unsupported switches instead of ignoring them (REF-011).
4. **Library code never panics on input.** Errors are `thiserror` enums with stable `FA-Exxxx` codes and remediation
   text (API-002).
5. **Determinism.** Fixed reduction order, an explicit FMA policy, and bit-reproducible results across thread counts (NUM-021).
6. **Clean-room rule.** IAS15, WH and the EIH equations are implemented from the papers. GPL sources (REBOUND,
   REBOUNDx, ASSIST, Swiftest) are never read for implementation (DEP-008, LIC-001).

## Repository layout

```
crates/            Rust workspace members (above)
tests/vectors/     TOML golden values with provenance + schema.json        (TST-002/003)
tests/goldens/     pinned reader/library goldens: pyerfa, jplephem, sb441 states (REF-003/004/009)
tests/data/        MANIFEST.toml: SHA-256 pins of fetched JPL/NAIF/IERS products (never the files; REF-001)
ci/oracles/<tool>/ Dockerfiles for Orekit, REBOUND(x), Swiftest, Mercury6, ERFA, JPL fetch (CI-001)
ci/scenarios/      tool-agnostic scenario specs + committed scenario.schema.json (CI-002)
ci/golden/         MANIFEST.json + small oracle goldens per <tool>/<scenario>/; large ones as artifacts (CI-003)
tools/             Python (uv project, ruff + mypy strict): trace.py, report.py, oracle runners
docs/              strategy/, requirements/, adr/, standards/, physics/, validation/
```

## Verification architecture

| Tier | Trigger | Content |
|---|---|---|
| PR-fast (< 10 min) | every PR | Unit and property tests; analytic rungs; cached goldens only (no oracle containers) |
| PR-oracle | `oracle` label | Re-run the affected oracle containers on the PR's scenarios |
| Nightly | short run on GitHub runners; full run on the local machine (Journal #10) | All oracles × all scenarios, 1000-yr spans, speed table, golden-drift check |
| Weekly / release | local machine, checkpointed (CI-010) | Stage D: DE440 reproduction 1550–2650; published report |

The independent oracles are: analytic formulas (every single-body rung, and β/γ ≠ 1, which no external code
supports); Park 2017 published values; REBOUNDx `gr_full` (coupled 1PN); Orekit (Mercury in a DE-driven field);
Swiftest and Mercury6 (Fortran-ecosystem Newtonian); ERFA (time/frames); `jplephem` and DE440 (truth).
