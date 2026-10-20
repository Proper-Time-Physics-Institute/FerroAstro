## 📓 Journal #10 — Red-team fixes applied + maintainer decisions round 3 (2026-10-15)

### Maintainer decisions
| Question | Decision |
|---|---|
| Merge policy | Deferred to the M0 CI work in **#12**: data-gated merges with human-owned meta files, **no merge delay**, **registered agent signing keys**, and `cargo-mutants` required from M2. The written rule lands together with the rulesets that enforce it. A competing "zero reviews everywhere" version relayed from another session was **not** adopted. |
| `anise` | Not a dependency. The decision is deferred to **M5** (OI-72), and the exception is removed from `deny.toml`. |
| Long runs and benchmarks | The **maintainer's local machine for now**. GitHub runners do PR-fast work and a short nightly. Checkpoint/resume (CI-010) is kept. This supersedes the Journal #6 plan for larger runners (ADR-0013 revised). |
| PHY-014 oracle | **575.3093 ± 0.005″/cy with DE440 J2.** That is Park's 575.3100 shifted by the J2 term difference, 0.027917 − 0.028601 = −0.000684. |
| GPL oracle images | Private GHCR, or rebuilt in CI. Never published. New **LIC-005**: a GPL container never contains a FerroAstro build. |

### Red-team fixes applied (Journal #9)
- **PHY-006** asserts the ratio rate(β,γ)/rate(1,1) instead of comparing against `42.98×`.
- **PHY-009:** a unit of coefficient A gives an advance of 8, not 1.
- **42.981 → 42.980** (a rounding error, not the orbital elements).
- **The 0.0006 gap** is labelled a hypothesis.
- **PHY-001** names IAS15 and excludes Kepler-drift integrators.
- **PHY-012 / NUM-013(b):** protocol is (Sun+Mercury+planet) − (Sun+Mercury), with cross terms reported separately as pairs. **Blocked on OI-38** until Park's text is read.
- **New PHY-019:** asteroid differential, 0.0012 ± 0.0003″/cy. Missing Lense–Thirring or asteroids are now caught by differential rungs instead of passing PHY-014 silently.
- **NUM-015** gets an absolute floor, max(1e-6 relative, 1e-7″/cy), and the planar formulas are restricted to equatorial geometry.
- **NUM-017** exempts the areal gauge that the SR-b and D-term checks need.
- **Roadmap:** REF-003 moved to M4; 18 orphan requirements placed on milestones; the M0 exit depends on OI-04 and OI-23.
- **Toolchain and versioning:** MSRV follows ADR-0006; STD-007 now says M6; stale ADR-0010 references fixed; toolchain noted as 1.95.0.
- **Licensing:**
  - DEP-003 lists ten banned crates.
  - `rug` is banned in every graph, including dev dependencies.
  - DEP-015 requires hash-pinned `licenses.clarify` entries, since cargo-deny ignores LICENSE files when a crate declares its license.
  - ERFA ports carry a per-file Apache-2.0 AND BSD-3-Clause tag plus the BSD-3 disclaimer.
  - A session that has read GPL source must not implement GR terms or integrators.
- **Check results:** 121 requirements, every YAML block parses, every link resolves, and the repo is REUSE 3.3-compliant.

### Coordination note
A separate session is implementing M0–M2 in its own worktree (`FerroAstro-m0`, branch `m0-foundation`), stacked on PR #2. This branch does not touch it.
