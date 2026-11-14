<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Testing standards

1. **Real values, with provenance.** Expected values come from `tests/vectors/*.toml` (ADR-0004).
   Each has a `source`, `citation`, `retrieved`, `tolerance` and `tolerance_rationale`.
   - `kind = "computed-reference"` vectors also record the generating script@sha, the engine version and its settings.
2. **Traceability.** Each test cites the requirement(s) it verifies. `tools/trace.py` builds
   `docs/requirements/TRACE.md` and fails on uncovered or unknown IDs (ADR-0010).
3. **Tolerance-based float comparison only.** Use `approx`; `float_cmp` is denied (TST-001).
4. **Independent oracles, in priority order:**
   1. analytic formula
   2. published value
   3. independent code in a container
   4. self-consistency (conservation laws, Brouwer-law slopes, forward–backward reversibility)
5. **Tiers:**
   - PR-fast: under 10 min, cached goldens only.
   - Nightly long runs: `#[ignore]`, on larger runners.
   - Weekly DE440 reproduction: checkpointed (CI-010).
6. **Golden changes** need the maintainer-applied `golden-change` label, new evidence in the same PR and a
   CODEOWNERS review through the ORG-011 meta surface (TST-004). The `ratchet` check also fails widened
   tolerances, downgraded oracles, deleted tests, pytest configuration changes, and Rust `#[ignore]`/`cfg`
   gates or Python skip/xfail/collection gates off the allowlist that are new on the test or item they stand
   on (moving a gate onto another test counts), unless a maintainer approves. Only the forms TST-006 lists
   are detected; its notes list the known undetected ones (`known_gap_*`, e.g. a skip built at run time,
   an emptied test body, an allowlisted feature `cfg` that hides a test), and pytest has no test-count
   backstop. Do not rely on CI to catch those: disclose them in the PR (TST-006, ADR-0015).
7. **UNVERIFIED and secondary-source [S] values** may not be oracles until they are closed in
   [OPEN-ITEMS.md](../requirements/OPEN-ITEMS.md) (PHY-018).
8. **Compile-fail tests** (`trybuild`) prove that type-level physics guards work (API-001).
