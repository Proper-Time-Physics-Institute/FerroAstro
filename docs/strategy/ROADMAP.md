<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap

Milestones follow the Mercury ladder ([MISSION.md](MISSION.md)). The **implementation order differs from
the presentation order**: the single-body relativistic rungs are cheap and come first, which gives an early
working 42.98″/cy demonstration. Every milestone exits only when its listed requirements are `verified`
in [`docs/requirements/TRACE.md`](../requirements/README.md).

| Milestone | Scope | Key requirements | Exit criterion |
|---|---|---|---|
| **M0 Foundations** | Cargo workspace; CI (fmt, clippy, nextest, doc, deny, ruff, mypy, reuse, DCO, trace); test-vector schema; `tools/trace.py`; constants loader for DE440 `header.440`; AGENTS.md | STD-001…012, TST-001…005, DEP-001…004, DEP-014…016, ORG-001…009, PHY-002/003, REF-002, AGT-001 | All gates green on `main`; DE440 constants are loaded from the header, never typed by hand. The PHY-002/REF-002 exit now depends only on closing OI-04 (GMSDOT units; being researched). OI-23 (header.440 CENTER/units) and OI-05 (exact au) were closed on 2026-10-20 |
| **M1 Core types & two-body** | `Real` trait (f64, double-double); newtypes for quantities, `Epoch<Scale>`, `State<Frame, Center>`; time scales (hifitime plus an ERFA-port subset); IAS15; analytic Kepler | API-001/002, DEP-007…010, DEP-012, LIC-004, NUM-001, NUM-006, NUM-010, NUM-023, REF-009 | Rung 1: Sun+Mercury precession below 1e-6″/cy over 1000 yr (PHY-001); Brouwer-law slopes (NUM-023) |
| **M2 Relativistic single-body rungs** | Force models SR-Kepler, Schwarzschild PPN(β,γ), J2, Lense–Thirring; analytic oracles; precession estimators (LRL, osculating, perihelion timing); first CLI report | PHY-004…011, NUM-003, NUM-012, NUM-015…017, NUM-019, API-003 | Rungs 4–7 reproduce 7.1634 / 50.144 & 35.817 (gauge-labelled) / 42.98 / 0.0286 / −0.0020″/cy within tolerance; `ferroastro ladder --rungs single-body` report |
| **M3 Newtonian N-body ladder** | Planets + Moon (geocentric) + asteroids (tiered); Park-2017 estimator and mean-orbit frame; differential-run harness; oracle containers (REBOUND, Swiftest, Mercury6, Orekit); goldens with manifests | PHY-012/013/015, NUM-004, NUM-008/009/011/013/014, REF-004…008/011, CI-001…006, CI-009, LIC-001…003, LIC-005 | Per-planet contributions within ±0.005″/cy of Park 2017 under the provisional P-remove protocol, with P-add reported (ADR-0014; reverts if OI-38 says P-add); first published speed table vs references |
| **M4 Full 1PN EIH N-body** | EIH with PPN β,γ; Sun-only 1PN rung; relativistic SSB; J2 & LT for all bodies; REBOUNDx `gr_full` oracle; MCP server alpha | PHY-014, PHY-019, NUM-005, NUM-018, NUM-020, REF-003, REF-010, DEP-013, AGT-004 | Total 575.3093 ± 0.005″/cy in the Park frame with DE440 J2 (Park 575.3100 shifted by the −0.0007 J2 offset); analytic β/γ scaling (PHY-006) |
| **M5 DE440 reproduction** | ABM13 + mixed double-double ephemeris mode; DE440 initial conditions from the header and `sb441-n373`; SPK type-2 reader generic over `Real`; checkpointed long runs on the local machine | NUM-002, NUM-007, NUM-021/022/024, DEP-011, REF-001, CI-007/008/010; `anise` adopt-or-not decision (OI-72) | Residuals vs `de440.bsp` over 1550–2650 published; wall time vs Subbotin et al. 2023 reference |
| **M6 Showcase & 0.x release** | WASM demo; full CLI report; MCP server beta; Horizons cross-check; equinox-referred historical comparison; Python bindings for tooling | PHY-016/017, DEP-005/006, DEP-017, ORG-010, AGT-002/003, STD-007 gating | Public demo and report; `cargo semver-checks` gating; THIRD_PARTY_LICENSES in every artifact |
| **Later** | Proper-time integration along trajectories (dτ/dt, clocks: GPS, Galileo 5/6, GP-A); multi-millennia DE441-span runs; Taylor/AD integrator (NUM-025); observational fitting | — | Scoped by future ADRs |

## Cross-cutting tracks

- **Open items**: every UNVERIFIED or secondary-source value is tracked in
  [`docs/requirements/OPEN-ITEMS.md`](../requirements/OPEN-ITEMS.md). None may serve as a test oracle until
  it is closed (PHY-018, NUM-020).
- **Legal**: draft the CLA for counsel review (ORG-010). The JPL DE, SOFA/`sofars` and NAIF redistribution terms are
  deferred until before the first release (see [OPEN-ITEMS](../requirements/OPEN-ITEMS.md)).
- **Proper time** is the long-term research thread. Time-scale correctness (TT/TDB/TCB/TCG) is built from
  M1 onward so that the proper-time milestone needs no rework.
