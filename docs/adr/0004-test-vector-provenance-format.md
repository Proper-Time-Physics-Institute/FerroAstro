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


## Revision 2026-10-21: the `provisional` field
Requested by the merge judge as a precondition for #13. The schema change is on the build branch `m3-newtonian-nbody` (#13), in a separate NEEDS-HUMAN-REVIEW commit to `tests/vectors/schema.json`. The wording below is the contract that schema and parser must match.

* **Meaning.** A vector MAY carry `provisional = "OI-NN"`. The row **asserts** under a protocol or decision that is provisional until that open item closes. Its first use is the PHY-012 / NUM-013(b) Park 2017 Table 3 rows under ADR-0014 (P-remove), pending OI-38. This differs from `blocked_by`, which is **non-gating**.
* **It gates.** A provisional row is loaded, compared and **fails** when it is outside its tolerance, exactly like an ordinary vector. Being provisional never excuses a failure.
* **Well-formed reference.** The value must match `^OI-\d{2,}$`, and the item must exist in `docs/requirements/OPEN-ITEMS.md`, whether open or closed. The schema check and the parser reject anything else.
* **Mutually exclusive with `blocked_by`.** A row may carry at most one of `provisional` and `blocked_by`. The schema check rejects a row that carries both.
* **Reporting.** Every report of a provisional row (test output, nightly summary, CLI ladder report, TRACE evidence) labels the result **PROVISIONAL**. It names the open item and states the revert condition, taken from the ADR that governs it. For the first use, that is ADR-0014: "if the Park 2017 text says P-add, revert".
* **Resolution.** When the open item closes:
  - **Confirmed:** the closing PR removes the `provisional` field from the affected rows. The rows then gate as ordinary vectors, and their values do not change.
  - **Refuted (revert):** the closing PR switches the asserted protocol or decision, as specified in the governing ADR, and updates the affected vectors under TST-004 (golden-change label and new evidence). Any result that the old provisional assertion hid becomes a `type:physics-discrepancy` issue.
  - **Either way:** the tick in OPEN-ITEMS.md and the vector change land in the **same PR**, so `main` never holds a closed item next to a stale `provisional` field.
* **Not evidence of closure.** A passing provisional row shows consistency under the provisional choice only. It is never cited as evidence for closing its own open item.

### Confirmation (for this revision)
These are schema and parser tests on the build branch:
1. a provisional row outside tolerance fails;
2. a malformed or unknown OI id is rejected;
3. `provisional` together with `blocked_by` is rejected;
4. a provisional row cannot drop its OI reference (ferroastro-ca's schema test);
5. report output contains `PROVISIONAL`, the OI id and the revert text.
