## 📓 Journal #6 — Maintainer decisions (2026-10-14)

Answers to the open questions raised in Journals #3 and #5:

| Question | Decision | Effect on requirements |
|---|---|---|
| CLA | **Defer the CLA; use DCO sign-off (`Signed-off-by:`) for now.** The CLA is drafted later and reviewed by counsel before CI enforces it. | **ORG-008 revised:** every commit SHALL carry a DCO sign-off, checked in CI (DCO GitHub App or Action). Agent-authored commits are signed off by the human or org operating the agent. CLA enforcement becomes a future requirement (ORG-010). |
| Benchmark / long-run hardware | **GitHub paid larger runners** | **CI-008 refined:** benchmarks run on one fixed larger-runner class and are compared only within the same run. Long jobs (nightly, Stage D, multi-millennia) SHALL checkpoint and resume across jobs because of the 6 h per-job limit (new **CI-010**). | *(Superseded 2026-10-15, Journal #10: long runs and benchmarks run on the maintainer's local machine for now; checkpoint/resume is kept. See ADR-0013, CI-008, CI-010.)* |
| Units / frames / time scales in types | **Home-grown newtypes** (e.g. `Epoch<Tdb>`, `State<Icrf, Barycentric>`, `Length`), generic over the crate's `Real` scalar | **API-001** design fixed. The `uom` crate is not used (ADR-0001). Compile-fail tests via `trybuild`. |
| Requirement traceability tooling | **Start with `tools/trace.py`; write an ADR evaluating StrictDoc and OpenFastTrace**, revisited at M2 | ORG-003/004 unchanged. New ADR `000x-requirements-tooling.md`. |
