<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# FerroAstro Mission

## Mission

FerroAstro rebuilds the scientific-computing foundations of astrodynamics, which today live in Fortran
(SOFA/ERFA, ODEPACK-era integrators, JPL ephemeris tooling) and Java (Orekit), as an
**Apache-2.0 Rust ecosystem that is agentic-first**. AI coding agents and human scientists should both
be able to use, extend and verify it with confidence. It treats Newtonian dynamics and
general relativity (including **proper time**) as equal first-class physics.

Every published result is tested against **real physical values with recorded provenance**
and benchmarked against the established Fortran, C and Java implementations in automated CI.
The aim is for FerroAstro to become the default foundation for other agentic astrodynamics projects
because its correctness is demonstrable rather than asserted.

## Flagship deliverable: the Mercury perihelion ladder

Mercury's perihelion precession is the showcase for both **speed** and **ease of use**.
It is built as a ladder of rungs. Each rung is a requirement with a cited expected value,
a tolerance with its rationale, and a CI test:

| # | Rung | Expected (″/Julian century; ± = published uncertainty) | Test tolerance (authoritative requirement) | Source |
|---|---|---|---|---|
| 1 | Newtonian Sun + Mercury (2-body) | 0 exactly | \|ϖ̇\| < 1×10⁻⁶ (PHY-001) | analytic (PHY-001) |
| 2 | + each planet (Venus, Earth/Moon, Mars, Jupiter, Saturn, Uranus, Neptune) | 277.4176, 90.8881, 2.4814, 153.9899, 7.3227, 0.1425, 0.0424 | ±0.005 each, provisional protocol (PHY-012) | Park et al. 2017 Table 3 (PHY-012) |
| 3 | + asteroids, cross terms → full Newtonian | 532.30 (asteroids alone 0.0012) | ±0.01 (PHY-013); asteroids ±0.0003 (PHY-019) | Park 2017 (PHY-013, PHY-019) |
| 4 | + solar J2 | 0.0286 ± 0.0011 | numerical vs analytic ±0.0005 (PHY-010) | Park 2017 (PHY-010) |
| 5a | Special relativity, SR-Kepler Lagrangian | 7.1634 (= GR/6), a *model* result | ±0.001 (PHY-007) | derived and checked (PHY-007) |
| 5b | "Velocity-dependent 1PN terms only" | 50.144 (harmonic gauge) / 35.817 (areal gauge), **not an observable** | ±0.002 (PHY-008) | derived (PHY-008) |
| 6 | Schwarzschild test particle, PPN (2+2γ−β)/3 | 42.9799 ± 0.0009 | analytic 42.9805 ± 0.0001 (PHY-004); numerical ±0.001 (PHY-005); β/γ ratio 10⁻⁶ relative (PHY-006) | Park 2017; analytic 42.9805 (PHY-004/005/006) |
| 7 | Lense–Thirring (solar spin) | −0.0020 ± 0.0002 | ±0.0002 (PHY-011) | Park 2017 (PHY-011) |
| 8 | Full EIH 1PN N-body + J2 + LT (DE440-class) | **575.3100 ± 0.0015**, in Mercury's mean-orbit frame | ±0.005; 575.3093 expected with DE440 J2 (PHY-014) | Park 2017 (PHY-014) |
| 9 | Reproduce JPL DE440 from its header initial conditions, faster | position residuals vs `de440.bsp` | see CI-007 | REF-001/002, CI-007 |

The ± values in the "Expected" column are the **published uncertainties** of the source (Park 2017 σ), not pass
criteria. Each rung's pass criterion is the test tolerance in its PHY requirement, which is authoritative.

The **headline number is 575.31″/cy**, which is inertial and fixed to the ICRF. The classical ≈5600″/cy
figure is mostly the precession of Earth's equinox. It is shown only as an explicitly constructed
historical comparison against Clemence (1947) (PHY-017). Every reported rate states its frame (PHY-015).

## Goals

1. **Correctness you can audit.** Every number has a source, a tolerance and a rationale (TST-002). Every
   requirement traces to a test (ORG-004).
2. **Speed you can reproduce.** Head-to-head timing against Orekit, REBOUND/REBOUNDx, Swiftest and Mercury6
   on identical scenarios, published from CI. No such comparison exists in the literature today (Journal #3).
3. **Agentic-first ergonomics.** Units, frames and time scales are types, so mixing them is a compile error
   (API-001). Errors are structured with stable codes (API-002). The repo has an `AGENTS.md`, an MCP server
   and runnable examples for every rung.
4. **Precision on demand.** f64 by default, with mixed double-double "ephemeris mode" and extended precision
   for multi-millennia runs, chosen by type parameter rather than by a separate code path (NUM-006/007).
5. **Open by construction.** Apache-2.0, a FOSS-only dependency graph enforced by `cargo-deny`, and GPL
   reference codes isolated in CI containers (LIC-001).

## Non-goals (for now)

- Fitting a new ephemeris to observations. Stage 1 reproduces DE440. An observational fit is a later,
  separately scoped program.
- Earth-satellite operations (SGP4, TLE handling, ground stations). Mature tools already cover these.
- Claiming NASA NPR 7150.2 or ECSS compliance. We borrow their tailoring ideas only (ORG-007).
- Bundling JPL ephemeris data in released artifacts until the redistribution terms are confirmed (DEP-017).

## Showcase surfaces

- **CLI + generated report**: one command runs the ladder and emits tables, plots and timings vs references.
- **WASM web demo**: the ladder runs live in a browser (core crates build for `wasm32-unknown-unknown`, DEP-006).
- **MCP server**: agents call the ladder as tools with typed, schema-declared outputs (DEP-013, AGT-004).

## Governance

- License: Apache-2.0, © Proper-Time Physics Institute.
- Contributions: DCO sign-off now. A CLA follows after legal review (ADR-0005).
- Decisions: MADR ADRs in [`docs/adr/`](../adr/). Requirements: [`docs/requirements/`](../requirements/).
- Research record: [`docs/strategy/journal/`](journal/), mirrored on issue #1.
