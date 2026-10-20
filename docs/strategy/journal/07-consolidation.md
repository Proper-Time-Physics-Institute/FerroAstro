## 📓 Journal #7 — Consolidation into the plan (draft PR #2)

All five research streams are folded into **draft PR #2** on `1-scientific-computing-strategy`:
- **Strategy:** mission, roadmap M0–M6, architecture.
- **Requirements:** 119 draft requirements, plus 71 tracked open items.
- **Decisions:** 14 ADRs.
- **Standards:** Rust, Python (ruff) and testing.
- **Governance:** AGENTS.md, DCO, NOTICE, REUSE.

The repo is REUSE 3.3-compliant, and the ruff config validates on ruff 0.17.0.

### Implementation order (≠ presentation order)
1. **M0** Foundations: workspace, CI gates, test-vector schema, `trace.py`, DE440 constants loaded from `header.440`.
2. **M1** `Real` trait + newtypes + time scales + IAS15 → rung 1 (2-body = 0).
3. **M2** Single-body relativistic rungs: SR-a 7.1634, SR-b gauge-labelled, GR 42.98, J2 0.0286, LT −0.0020 → first CLI report.
4. **M3** Newtonian N-body ladder with the Park-2017 estimator and oracle containers → first speed table.
5. **M4** Full EIH → **575.31″/cy**. MCP server alpha.
6. **M5** DE440 reproduction (ABM13 + mixed double-double, checkpointed on larger runners).
7. **M6** WASM demo, report and 0.x release.

### Cross-stream contradictions resolved provisionally (see OPEN-ITEMS.md)
- **Estimator:** Park 2017 (NUM-011) is canonical. The LRL-vector fit is secondary (CI-009 updated).
- **575.31 tolerance:** ±0.005 (PHY-014, physics) vs ±0.01 (NUM-011, estimator-level). Both are kept, each for its own scope.
- **NUM-013 "10⁻⁴″/cy differential":** reachable only for single-body isolated terms. In N-body, planet-interaction residuals reach about 10⁻². *Needs maintainer confirmation.*
- **DE430 J2 bias direction (OI-50).** Lead analysis: the Lense–Thirring precession is *negative* (−0.0020″/cy) and J2's is positive. A J2 fitted without LT therefore absorbs the missing negative term and comes out **low**. INPOP agrees (2.01 without LT → 2.218 with LT), and so does DE430's 2.11 < DE440's 2.196. Journal #4's "too high" phrasing looks inverted. To confirm against Park 2021's text.

### Next actions
- Maintainer review of PR #2, especially OI-68–71.
- On merge, open M0 issues: workspace skeleton, CI workflows, test-vector schema, `trace.py`, constants loader, issue forms and labels.
