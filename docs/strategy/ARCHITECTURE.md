<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Architecture

## Layering

The crates are built bottom-up. Each layer depends only on the layers below it, and everything up to
`ferroastro-ladder` builds on stable Rust for `wasm32-unknown-unknown` with no C or Fortran toolchain
(DEP-006).

```
 surfaces     ferroastro-cli   ferroastro-mcp   ferroastro-wasm   (ferroastro-py, tooling only)
                    \               |               /
 showcase                  ferroastro-ladder         scenarios, rungs, reports
                                  |
 analysis               ferroastro-precession        estimators (Park2017, LRL, osculating), differential runs
                                  |
 dynamics     ferroastro-forces ── ferroastro-integrators
              (Newton, J2, SR, PPN, LT, EIH, tiers)   (IAS15, ABM, IRKGL16, WH/Saha–Tremaine)
                                  |
 data         ferroastro-ephem   ferroastro-constants   SPK type-2 + header.440 readers; DE440 constants
                                  |
 reference    ferroastro-time    ferroastro-frames      TT/TDB/TCB/TCG/UTC; ICRF, ecliptic, mean-orbit
                                  |
 foundation   ferroastro-units   ferroastro-real        newtypes for quantities/epochs/states; Real trait
```

| Crate | Responsibility | Key requirements | Reuse |
|---|---|---|---|
| `ferroastro-real` | `Real` trait; `f64`, double-double, optional nightly `f128`; compensated summation | DEP-007, NUM-006, DEP-012 | DD backend chosen by benchmark (`qd` / `twofloat` / own); `astro-float` as test oracle |
| `ferroastro-units` | Newtypes: `Length`, `Velocity`, `GravParam`, `Angle`; `Epoch<S: TimeScale>`; `State<F: Frame, C: Center>`; compile-fail tests | API-001 | home-grown (ADR-0001) |
| `ferroastro-time` | Time scales and conversions, leap seconds | DEP-010, REF-009 | `hifitime` (MPL-2.0, unmodified) plus an ERFA (BSD-3) port subset |
| `ferroastro-frames` | ICRF, J2000 ecliptic, Mercury mean-orbit frame, IAU 2006 precession (for the historical comparison only) | NUM-014, PHY-015 | ERFA port |
| `ferroastro-constants` | Versioned constant sets: DE440 (loaded from `header.440`); IAU 2015 nominal values in a separate namespace | PHY-002/003, REF-002 | own |
| `ferroastro-ephem` | DAF/SPK type-2 reader generic over `Real`; DE ASCII header parser | DEP-011, REF-004 | own; validated against `anise` and `jplephem` |
| `ferroastro-forces` | Force models as composable terms; body tiers (Major/Minor/Ring); every model labels its gauge | NUM-008/009/017/018/019 | own (clean-room, DEP-008) |
| `ferroastro-integrators` | IAS15 (default), ABM PECE(C) ephemeris mode, IRKGL16 reference, WH + Saha–Tremaine | NUM-001…005, NUM-010 | own (clean-room) |
| `ferroastro-precession` | Event location (r·v = 0), Park-2017 estimator, alternatives, differential harness, frame/epoch/fit metadata | NUM-011…013, PHY-015 | own |
| `ferroastro-ladder` | Scenario TOML → runs → structured results; the rung catalogue | ORG-002, CI-002 | own |
| `ferroastro-cli` | `ferroastro ladder …`, report generation | — | `clap`, `plotters`/`plotly` |
| `ferroastro-mcp` | MCP tools with `outputSchema` / `structuredContent` | DEP-013, AGT-004 | `rmcp` 3.x |
| `ferroastro-wasm` | Browser bindings for the demo | DEP-006 | `wasm-bindgen` |

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
ci/oracles/<tool>/ Dockerfiles for Orekit, REBOUND(x), Swiftest, Mercury6, ERFA, JPL fetch (CI-001)
ci/scenarios/      tool-agnostic scenario specs driving FerroAstro and every oracle (CI-002)
ci/golden/         MANIFEST.json + small goldens; large ones as release/OCI artifacts (CI-003)
tools/             Python (uv project, ruff + mypy strict): trace.py, report.py, oracle runners
docs/              strategy/, requirements/, adr/, standards/, physics/, validation/
```

## Verification architecture

| Tier | Trigger | Content |
|---|---|---|
| PR-fast (< 10 min) | every PR | Unit and property tests; analytic rungs; cached goldens only (no oracle containers) |
| PR-oracle | `oracle` label | Re-run the affected oracle containers on the PR's scenarios |
| Nightly | cron, larger runner | All oracles × all scenarios, 1000-yr spans, speed table, golden-drift check |
| Weekly / release | cron + tag, larger runner, checkpointed (CI-010) | Stage D: DE440 reproduction 1550–2650; published report |

The independent oracles are: analytic formulas (every single-body rung, and β/γ ≠ 1, which no external code
supports); Park 2017 published values; REBOUNDx `gr_full` (coupled 1PN); Orekit (Mercury in a DE-driven field);
Swiftest and Mercury6 (Fortran-ecosystem Newtonian); ERFA (time/frames); `jplephem` and DE440 (truth).
