---
status: accepted
date: 2026-10-14
decision-makers: maintainers (decision recorded in Journal #6)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Requirements traceability tooling

## Considered Options
* Home-grown `tools/trace.py` (~150 lines, ruff-clean, no new deps)
* StrictDoc
* OpenFastTrace

## Decision Outcome
Start with **`tools/trace.py`**: it parses the YAML requirement blocks in `docs/requirements/*.md`, finds `Verifies: <ID>` citations in Rust and Python tests, writes `TRACE.md`, and fails CI on gaps or unknown IDs. **Re-evaluate StrictDoc and OpenFastTrace at M2**, against requirement count, HTML export needs and agent usability. Keeping requirements in YAML blocks makes migration mechanical.

### Confirmation
CI `trace` job (ORG-004).

## Proposal 2026-10-20 (adopted with conditions 2026-10-20 by the merge judge; see the ruling below): CI-gate evidence in `trace.py`
Source: the WP6 requirement gap table in `docs/strategy/IMPLEMENTATION-NOTES.md` on `m3-newtonian-nbody`, repeated as an open question in every progress review since (WP6, WP10, WP15, WP18).

* **Problem.** Some requirements are enforced by a CI gate rather than by a test, so `trace.py` reports them as uncited: STD-001…011 (fmt, clippy, docs, MSRV, workflow lints), TST-001, DEP-001/002/003 (`cargo deny`) and ORG-008 (the `dco` job). DEP-006 is enforced by the `wasm32` job and is uncited too (progress review after WP18). Today the gap is harmless because every requirement is `draft`, but from `approved` onwards ORG-004 would fail them.
* **Proposal.** Let a requirement name its gate, e.g. a `Verified-by-gate: <workflow>/<job>[/<step>]` record that `trace.py` checks exists in `.github/workflows/`, and count it as evidence for those requirements only. TRACE.md would show gate evidence apart from test evidence.
* **Open points for the judge.** Whether gate evidence is acceptable for `crit:high` requirements (DEP-001…003 licence policy); whether the gate must run on the default branch ("required workflows"); and how a removed or renamed job is detected. Requirement text and trace policy are human-owned; no agent changed `trace.py` for this.

## Ruling 2026-10-20: adopted with conditions
Merge judge ruling on PR #19 (comment by ferroastro-58; the PR was merged as 8353efc). Gate evidence is accepted under these conditions:

1. **Scope.** Gate evidence counts only for requirements whose verdict comes from a tool: STD-001…011, TST-001, DEP-001/002/003/006 and ORG-008. It **never** counts for PHY, NUM, REF or API requirements. Those still need a citing test.
2. **Human-owned target.** The requirement carries a `verified_by_gate: <workflow>/<job>[/<step>]` field, owned by a maintainer (ORG-011 meta surface, #12). `trace.py --check` **fails** if the named workflow, job or step does not exist in `.github/workflows/`. A removed or renamed job therefore fails the check.
3. **crit:high needs a pinned gate.** A `crit:high` requirement counts as gate-verified only once its gate is a **required check pinned to the default branch** (#12). Until then, TRACE.md shows it as `gate (unpinned)`, and it **does not count** toward moving the requirement past `draft`.
4. **Separate reporting.** TRACE.md lists gate evidence apart from test evidence.

### Confirmation
`trace.py --check` enforces conditions 1, 2 and 4. Condition 3 is enforced by the ruleset work in #12. Until #12 lands, every crit:high gate entry shows as `gate (unpinned)`. Implementation is a follow-up on the build branch, because `tools/trace.py` is a meta file.
