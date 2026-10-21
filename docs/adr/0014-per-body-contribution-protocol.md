---
status: accepted
date: 2026-10-20
decision-makers: maintainers (decision C of 2026-10-20)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Per-body contribution protocol (P-remove)

## Context and Problem Statement
PHY-012, NUM-013(b) and PHY-019 compare FerroAstro's per-body Newtonian contributions to Mercury's perihelion rate with Park et al. 2017 (AJ 153, 121) Table 3, within ±0.005 ″/cy (±0.0003 ″/cy for the asteroids). A "contribution" depends on how a body is switched on or off, because the bodies interact: Park's own pair rows reach 0.0411 ″/cy (Jupiter+Saturn), about 8× the tolerance. Two protocols are possible:

* **P-add**: contribution(X) = ϖ̇(Sun + Mercury + X) − ϖ̇(Sun + Mercury).
* **P-remove**: contribution(X) = ϖ̇(full system) − ϖ̇(full system without X).

Which protocol Park 2017 used is OI-38. Until 2026-10-20 the requirements fixed P-add as the provisional protocol, and the build pre-registered P-add as the primary, asserted protocol before any run (WP14 plan; the `tolerance_rationale` of every row in `tests/vectors/park2017-table3.toml` on `m3-newtonian-nbody`). Which protocol should the gate assert?

## Decision Drivers
* The gate must test FerroAstro's dynamics, not a protocol mismatch.
* The protocol must not be tuned to make a test pass (PHY-012 notes).
* Evidence about Park's protocol exists but is not yet confirmed by a maintainer against the Park 2017 text (OI-38).

## Considered Options
* Assert P-add (the pre-registered protocol); report P-remove.
* Assert P-remove; report P-add ("report both").
* Assert nothing until OI-38 is closed.

## Decision Outcome
Chosen option: **"report both"**. The harness computes both protocols from identical initial conditions with one estimator. It **asserts P-remove** and **reports P-add as information only**. The same rule applies to the N-AST rung (PHY-019). Cross terms are reported in a labelled sign convention: the harness's inclusion–exclusion second difference is the negative of Park's "pair removal minus single removals".

**This choice was made after the results were known.** P-add was the protocol written down before any run. P-remove was chosen after the WP14 and WP17 results below had been seen. This ADR states that plainly. The choice is therefore **PROVISIONAL** until a maintainer confirms OI-38 against the Park 2017 text.

### Evidence (build branch `m3-newtonian-nbody`, `docs/strategy/IMPLEMENTATION-NOTES.md`)
WP14, `crates/ferroastro-diffrun/tests/nightly_phy012.rs::nightly_phy012_park17_table3_newtonian_contributions` (Newtonian, header.440 initial conditions at JDEPOC, IAS15 f64, Park2017 estimator over J2000 ± 1000 Julian yr, shared IC hash `6d875b67…`). Recomputed identically by the progress review after WP18.

| row | Park17 Table 3 | P-add (pre-registered) | P-add − Park | P-remove | P-remove − Park |
|---|---|---|---|---|---|
| Venus | 277.4176 | 277.394894 | **−0.022706 (miss)** | 277.418142 | +0.000542 |
| Earth/Moon | 90.8881 | 90.869103 | **−0.018997 (miss)** | 90.888795 | +0.000695 |
| Mars | 2.4814 | 2.479384 | −0.002016 | 2.481426 | +0.000026 |
| Jupiter | 153.9899 | 154.028732 | **+0.038832 (miss)** | 153.990192 | +0.000292 |
| Saturn | 7.3227 | 7.365749 | **+0.043049 (miss)** | 7.322772 | +0.000072 |
| Uranus | 0.1425 | 0.139602 | −0.002898 | 0.142531 | +0.000031 |
| Neptune | 0.0424 | 0.042068 | −0.000332 | 0.042378 | −0.000022 |

* **P-add misses 4 of 7 rows** (Venus, Earth/Moon, Jupiter, Saturn) by 0.019–0.043 ″/cy, up to 8.6× the tolerance. P-remove passes all 7 within 0.0007 ″/cy.
* The P-add misses are not an integrator error. On the WP17 forward window, FerroAstro (IAS15), REBOUND 5.2.2 (IAS15), Mercury6 (RA15) and Swiftest (WHM) give the same P-add Venus value to 2e-8 ″/cy through one estimator (`crates/ferroastro-oracle/tests/nightly_oracle_differentials.rs`); every tool misses Park by −0.0298 on that window. Two independent scipy integrations in the progress reviews after WP15 and WP18 reproduce FerroAstro's P-add Venus to ≤ 1.7e-4 ″/cy.
* Park 2017 §4 (quoted verbatim in the WP14 notes) describes rows obtained by setting a body's GM to zero in the nominal ephemeris and differencing.
* Park's six pair rows equal P-remove's joint removal minus the two single removals to ≤ 1.2e-4 ″/cy (progress review after WP18, R1). That comparison was also made after the results were known.
* N-AST (PHY-019): P-add 0.001242, P-remove 0.001245 ″/cy (WP15, `nightly_phy013.rs`). The protocol dependence of about 3e-6 ″/cy is 1% of ±0.0003 (progress review after WP15, F9), so PHY-019 gates independently of OI-38.

### Consequences
* Good: the gate measures FerroAstro's dynamics against Park under the protocol that the evidence points to, and P-add stays visible in every report.
* Good: PHY-012, NUM-013(b) and PHY-019 are no longer blocked on OI-38. OI-38 stays open with the narrower question "confirm against the Park 2017 text".
* Bad: the asserted protocol was chosen after seeing which one passes. A pass therefore establishes only "**consistent with Park 2017 under P-remove**", not agreement independent of the protocol. The ±0.005 ″/cy tolerance remains tighter than the protocol ambiguity (cross terms up to 0.041 ″/cy).
* **Revert condition.** If a maintainer's reading of the Park 2017 text says P-add, this decision reverts: the gate asserts P-add, and the 4 P-add misses above are treated as real discrepancies, filed as `type:physics-discrepancy`.
* The build vectors still carry `blocked_by = "OI-38"` (ADR-0004 revision). Switching the asserted protocol in the harness and unblocking the vectors is build-PR work. Ticking OI-38 alone would arm the P-add rows and turn the nightly red (WP15 review F13), so it must land together with that switch.

### Confirmation
* PROVISIONAL until OI-38 is closed against the Park 2017 text by a maintainer; until then this ADR may be reverted under the condition above.
* Nightly `nightly_phy012_park17_table3_newtonian_contributions` asserts P-remove for the 7 rows of PHY-012 within ±0.005 ″/cy and prints P-add next to them; `nightly_phy013_n_sum_and_asteroid_differential` does the same for N-AST within ±0.0003 ″/cy.
* Reports label the protocol of every row and the sign convention of every cross-term column.

## Pros and Cons of the Options
### Assert P-add; report P-remove
* Good: it is the pre-registered protocol, so no post-hoc choice.
* Bad: four tools agree that it misses Park by up to 0.043 ″/cy for a reason that is not FerroAstro's dynamics; the gate would fail on the protocol, not on the physics.

### Assert P-remove; report P-add (chosen)
* Good: matches all 7 rows and all 6 pair rows; consistent with the Park 2017 §4 text as quoted.
* Bad: chosen after the results; provisional until OI-38 is confirmed.

### Assert nothing until OI-38 is closed
* Good: no post-hoc choice.
* Bad: the M3 per-planet rungs stay tests that cannot fail.

## More Information
Requirements: PHY-012, NUM-013, PHY-019, CI-009. Open items: OI-38 (protocol, revert condition), OI-09 (Park's "Mercury" row, never an oracle). Related: ADR-0004 (`blocked_by`), ADR-0011 (precession observable).
