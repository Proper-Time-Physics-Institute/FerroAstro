## 📓 Journal #8 — Maintainer decisions, round 2 (2026-10-14)

Applied in commit 216c58e on PR #2:

| Question | Decision | Change |
|---|---|---|
| N-body differential tolerance | Split by rung type | NUM-013: 1e-4″/cy for isolated single-body terms against analytic formulas; ±0.005″/cy for N-body contributions, with cross terms reported separately |
| DE430 J2 bias (OI-50) | Correct to "biased low", then verify | Journal #4 and NUM-019 corrected; OI-50 stays open until the Park 2021 sentence is read |
| `anise` (MPL, depends on `sofars`) | Test oracle only until cleared | DEP-002, `deny.toml` comment |
| Toolchain | Current stable at M0 | ADR-0006, `rust-toolchain.toml` placeholder note |
| Legal items (CLA, JPL/NAIF data, SOFA via `sofars`) | Defer until before the first release | Note in OPEN-ITEMS. Conservative defaults stay in force |
| M0 setup | Create milestones, labels and issues | Milestones M0–M6, 25 labels, issues #3–#10 |
| Merge policy | "Agent-led PRs; data, not humans, decides what merges to main" | **Not yet applied.** The edit was blocked by the session's permission system as removing human oversight. Proposal: ORG-009 evidence-gated merges, ORG-011 human sign-off only where data can't decide, TST-004 golden changes need new evidence. See Journal #9 for the red-team review of this proposal. |
