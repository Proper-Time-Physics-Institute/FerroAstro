<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Roadmap

Milestones follow the Mercury ladder ([MISSION.md](MISSION.md)). The **implementation order differs from
the presentation order**: the single-body relativistic rungs are cheap and come first, which gives an early
working 42.98″/cy demonstration. Every milestone exits only when its listed requirements are `verified`
in [`docs/requirements/TRACE.md`](../requirements/README.md).

| Milestone | Scope | Key requirements | Exit criterion |
|---|---|---|---|
| **M0 Foundations** | Cargo workspace; CI (fmt, clippy, nextest, doc, deny, ruff, mypy, reuse, DCO, trace); test-vector schema; `tools/trace.py`; constants loader for DE440 `header.440`; AGENTS.md | STD-001…006, STD-008…010, STD-012, TST-001…005, DEP-001…004, DEP-014…016, ORG-001, ORG-003…009, PHY-002/003, REF-002, AGT-001 | All gates green on `main`; DE440 constants are loaded from the header, never typed by hand. The PHY-002/REF-002 exit has no open-item dependency left: OI-04 (GMSDOT units) moved to M5 on 2026-10-20. OI-23 (header.440 CENTER/units) and OI-05 (exact au) were closed on 2026-10-20 |
| **M1 Core types & two-body** | `Real` trait (f64, double-double); newtypes for quantities, `Epoch<Scale>`, `State<Frame, Center>`; time scales (hifitime plus an ERFA-port subset); IAS15; analytic Kepler | API-001/002, DEP-007…010, DEP-012, LIC-004, NUM-001 (Kepler and Kozai tests; its ≥ 10⁹-step clause is in M5), NUM-006, NUM-010, NUM-023, REF-009 | Rung 1: Sun+Mercury precession below 1e-6″/cy over 1000 yr (PHY-001); Brouwer-law slopes (NUM-023) |
| **M2 Relativistic single-body rungs** | Force models SR-Kepler, Schwarzschild PPN(β,γ), J2, Lense–Thirring; analytic oracles; precession estimators (LRL, osculating, perihelion timing); first CLI report | PHY-004…011, NUM-003, NUM-012, NUM-015…017, NUM-019 | Rungs 4–7 reproduce 7.1634 / 50.144 & 35.817 (gauge-labelled) / 42.98 / 0.0286 / −0.0020″/cy within tolerance; `ferroastro ladder --rungs single-body` report |
| **M3 Newtonian N-body ladder** | Planets + Moon (geocentric) + asteroids (tiered); Park-2017 estimator and mean-orbit frame; differential-run harness; oracle containers (REBOUND, Swiftest, Mercury6, Orekit); goldens with manifests | PHY-012/013/015, NUM-004, NUM-008/009/011/013/014, NUM-023, REF-004…008/011, CI-001…006, CI-009, LIC-001…003, LIC-005 | Per-planet contributions within ±0.005″/cy of Park 2017 under the provisional P-remove protocol, with P-add reported (ADR-0014; reverts if OI-38 says P-add); first published speed table vs references; NUM-023 Brouwer-law slopes pass with an asserting test at ≥ 5e7 steps, not a known-failure guard (blocks M3 and #13; [maintainer ruling 2026-10-21](https://github.com/Proper-Time-Physics-Institute/FerroAstro/pull/13#issuecomment-6306750993)) |
| **M4 Full 1PN EIH N-body** | EIH with PPN β,γ; Sun-only 1PN rung; relativistic SSB; J2 & LT for all bodies; REBOUNDx `gr_full` oracle; MCP server alpha | PHY-014, PHY-019, NUM-005, NUM-018, NUM-020, REF-003, REF-010, DEP-013, AGT-004 | Total 575.3093 ± 0.005″/cy in the Park frame with DE440 J2 (Park 575.3100 shifted by the −0.0007 J2 offset); analytic β/γ scaling (PHY-006) |
| **M5 DE440 reproduction** | ABM13 + mixed double-double ephemeris mode; solar mass loss GM☉(t) with the GMSDOT convention fitted against de440.bsp (OI-04); DE440 initial conditions from the header and `sb441-n373`; SPK type-2 reader generic over `Real`; checkpointed long runs on the local machine | NUM-001 (≥ 10⁹-step Brouwer clause only, [#34](https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/34)), NUM-002, NUM-007, NUM-021/022/024, DEP-011, REF-001, CI-007/008/010; `anise` adopt-or-not decision (OI-72) | Residuals vs `de440.bsp` over 1550–2650 published; wall time vs Subbotin et al. 2023 reference |
| **M6 Showcase & 0.x release** | WASM demo; full CLI report; MCP server beta; Horizons cross-check; equinox-referred historical comparison; Python bindings for tooling | PHY-016/017, DEP-005/006, DEP-017, ORG-002, ORG-010, AGT-002/003, STD-007, STD-011, API-003 (doctest threshold to be set here) | Public demo and report; `cargo semver-checks` gating; THIRD_PARTY_LICENSES in every artifact |
| **Later** | Proper-time integration along trajectories (dτ/dt, clocks: GPS, Galileo 5/6, GP-A); multi-millennia DE441-span runs; Taylor/AD integrator (NUM-025); observational fitting | PHY-020, PHY-021, PHY-022 (comparison models and the three-clock diagnostic, ADR-0016) | Scoped by future ADRs |

## Key-list changes

Maintainer decisions of 2026-11-22 and 2026-11-23, taken after the milestone audit of `main` at `e86984d`
(decision record: issue #TBD-DECISION-ISSUE). Only the **Key requirements** column changed. Scope and exit
criteria are as before.

| Requirement | Was in | Now in | Why |
|---|---|---|---|
| ORG-002 (flagship ladder) | M0 | M6 | Its acceptance is a whole-ladder property: every PHY rung requirement verified by a test, with no uncovered PHY requirement in TRACE.md. The last rungs to land are M6's (PHY-016, and PHY-017, which waits on OI-01 and OI-02), so M6 is the first milestone that can meet it. |
| STD-007 (`cargo-semver-checks`) | M0, and "gating" in M6 | M6 | The check compares the public API "against the last release", and the first release is M6's. Its acceptance already makes the check required from M6. The advisory run before M6 stays in the requirement text; it is no longer an M0 exit condition. |
| STD-011 (Conventional Commits, release-please) | M0 | M6 | The PR-title lint exists (`pr-title.yml`). The other half, release PRs and the CHANGELOG from release-please or an ADR-chosen alternative, is release machinery, which belongs to the 0.x release. |
| NUM-001, ≥ 10⁹-step Brouwer clause | M1 | M5 | The maintainer deferred this clause on 2026-11-08 ([#34](https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/34)): the f64 energy error sits at its evaluation floor, so the check must be redesigned, and it runs for about two hours. The rest of NUM-001 (the integrator, and the Kepler and Kozai tests) stays in M1. |
| API-003 (doctests and per-rung examples) | M2 | M6 | Every implemented rung has an example that runs in CI, but the clause "every public function has a doctest" is far from met: CI reports 0 of 864, report-only. The threshold is to be set at M6. |
| PHY-020, PHY-021, PHY-022 (conformal-gravity and Proper Time (Gill) comparison models; three-clock diagnostic) | no key list | Later | Decided 2026-11-23. All three are "never in the ladder" (ADR-0016), so no ladder milestone M0–M6 can list them without making its exit depend on them, while "Later" is the row whose scope is proper time and clocks (GPS, Galileo 5/6) and work scoped by its own ADR. Statuses stay `draft`; the listing says where they are tracked, not when their code lands (it is already on `main`). |

NUM-012 stays in M2. Its statement no longer names the per-orbit mean-ellipse estimator, which was never
built (same decision; see the NUM-012 notes).

## Milestone status

No milestone has exited yet.

- **M2: not exited.** The exit criterion is met: every rung value is asserted by a test that passed in CI
  on `main` at `e86984d` (`ci` run 45030787159, 2026-11-22), and so is the
  `ferroastro ladder --rungs single-body` report. The tests are `phy007_sr_a.rs`, `phy008_sr_b.rs`,
  `phy005_gr_schw.rs`, `phy010_j2.rs` and `phy011_lt.rs` in `crates/ferroastro-ladder/tests/`, and
  `crates/ferroastro-cli/tests/cli_report.rs`. Twelve of the fourteen key requirements are `verified`.
  **What remains** (maintainer decision of 2026-11-23, decision record: issue #TBD-DECISION-ISSUE): two key
  requirements are `implemented`, not `verified`.
  - NUM-015: its acceptance is blocked by OI-37 (the citation for the inclined-orbit J₂ rates), which is
    still open.
  - NUM-003: its statement promises a "round-off-aware implementation", and the one test that measures
    that (`nightly_irk16_compensation_gain_pre_registered`, vector `IRK16-COMPENSATION-GAIN-MIN`) misses its
    pre-registered bound (gain 1.98 against ≥ 3). OI-80 blocks it: resolve the shortfall, and replace or
    retire the two pre-registered order tests whose design cannot measure the order. These three tests are
    registered known failures in `ci/nightly-known-failures.toml`.

  M2 exits when OI-37 and OI-80 are closed and NUM-015 and NUM-003 are `verified`.
  **Stated limits among the twelve** (same decision: both requirements stay `verified`, with the limit in
  the requirement's notes and OI-81 open to remove it; OI-81 qualifies them and does not block the exit):
  - NUM-019: the J₂ and LT contributions are asserted in CI for a two-body N-body run and for a single
    body. Its acceptance vector, the full N-body contribution under EIH, is asserted only in the nightly
    tier, not in per-PR CI.
  - NUM-012: the cross-estimator spread is reported for five of the eight single-body rungs.

## Cross-cutting tracks

- **Open items**: every UNVERIFIED or secondary-source value is tracked in
  [`docs/requirements/OPEN-ITEMS.md`](../requirements/OPEN-ITEMS.md). None may serve as a test oracle until
  it is closed (PHY-018, NUM-020).
- **Legal**: draft the CLA for counsel review (ORG-010). The JPL DE, SOFA/`sofars` and NAIF redistribution terms are
  deferred until before the first release (see [OPEN-ITEMS](../requirements/OPEN-ITEMS.md)).
- **Proper time** is the long-term research thread. Time-scale correctness (TT/TDB/TCB/TCG) is built from
  M1 onward so that the proper-time milestone needs no rework.
