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
6. **Golden changes** need the `golden-change` label and a CODEOWNERS review (TST-004).
7. **UNVERIFIED and secondary-source [S] values** may not be oracles until they are closed in
   [OPEN-ITEMS.md](../requirements/OPEN-ITEMS.md) (PHY-018).
8. **Compile-fail tests** (`trybuild`) prove that type-level physics guards work (API-001).
