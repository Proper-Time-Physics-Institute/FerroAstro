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

## Proposal 2026-10-20 (pending judge review; not adopted): CI-gate evidence in `trace.py`
Source: the WP6 requirement gap table in `docs/strategy/IMPLEMENTATION-NOTES.md` on `m3-newtonian-nbody`, repeated as an open question in every progress review since (WP6, WP10, WP15, WP18).

* **Problem.** Some requirements are enforced by a CI gate rather than by a test, so `trace.py` reports them as uncited: STD-001…011 (fmt, clippy, docs, MSRV, workflow lints), TST-001, DEP-001/002/003 (`cargo deny`) and ORG-008 (the `dco` job). DEP-006 is enforced by the `wasm32` job and is uncited too (progress review after WP18). Today the gap is harmless because every requirement is `draft`, but from `approved` onwards ORG-004 would fail them.
* **Proposal.** Let a requirement name its gate, e.g. a `Verified-by-gate: <workflow>/<job>[/<step>]` record that `trace.py` checks exists in `.github/workflows/`, and count it as evidence for those requirements only. TRACE.md would show gate evidence apart from test evidence.
* **Open points for the judge.** Whether gate evidence is acceptable for `crit:high` requirements (DEP-001…003 licence policy); whether the gate must run on the default branch ("required workflows"); and how a removed or renamed job is detected. Requirement text and trace policy are human-owned; no agent changed `trace.py` for this.

