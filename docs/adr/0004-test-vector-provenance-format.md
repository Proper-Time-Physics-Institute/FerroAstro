---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Test-vector provenance format

## Decision Outcome
Expected values live in `tests/vectors/*.toml`, validated against `tests/vectors/schema.json`. Each vector has `id`, `requirement`, `quantity`, `value`, `unit`, `kind` (defined | measured | computed-reference | analytic), `source`, `citation`, `retrieved`, `tolerance` (abs/rel), `tolerance_rationale` and `verified_by`. `computed-reference` vectors also record `generated_by` (script@sha) and the engine version and settings. Tests load values by `id`, so bare physical literals in tests are rejected. Values from secondary sources ([S]) or UNVERIFIED values may not be used until closed in OPEN-ITEMS.md.

### Confirmation
CI schema check plus a literal-lint script (TST-002/003). Changes to golden values need the `golden-change` label and a CODEOWNERS review (TST-004).

## Revision 2026-10-20 (proposed): the `blocked_by` field
Recorded from the build branch `m3-newtonian-nbody` (WP14, `docs/strategy/IMPLEMENTATION-NOTES.md`); the schema change was made there by an agent and is still marked NEEDS HUMAN REVIEW (WP14 deviation 4). It becomes part of this decision once a maintainer confirms it.

* A vector MAY carry an optional `blocked_by = "OI-NN"`. It is in `tests/vectors/schema.json` and in the `ferroastro-vectors` parser (`Vector::blocking_open_item`).
* A blocked vector is still loaded, printed and compared. Its test reports the comparison and does not fail while the open item is unticked in `docs/requirements/OPEN-ITEMS.md` (or absent from it).
* **Ticking the open item enables the gate automatically.** No code or vector edit is needed for a blocked comparison to start failing.
* Consequences: (1) a blocked test cannot fail, so it is not evidence for its requirement while blocked; (2) ticking an item can turn a nightly red by itself, so the tick and any protocol or vector change it implies land in the same PR (the WP15 progress review's F13: ticking OI-38 alone would arm the P-add rows of `park2017-table3.toml`, 4 of which fail; see ADR-0014).
* In use on the build branch: Park 2017 Table 3 rows and N-AST (`OI-38`), the full-set SSB offset (`OI-24`), the NUM-009 cost ratio (`OI-42`).

