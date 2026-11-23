<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Software classification and tailoring

This file is required by ORG-007. It states how FerroAstro classifies itself, what it borrows from the
NASA and ECSS software standards, and what the repository does with a requirement's `criticality`.
It describes the repository as it is, and it marks each rule as checked by a tool, checked by a person,
or not checked.

## Classification

FerroAstro is research software. It classifies itself as:

- **equivalent to NASA NPR 7150.2 Class D/E**: Class D is "basic science, engineering design, and research
  and technology software", and Class E is "design concept, research, technology, and general-purpose
  software" (wording as quoted in
  [Journal #5, section 2.1](../strategy/journal/05-organization-standards.md));
- **inspired by the criticality-based tailoring of ECSS-E-ST-40C and ECSS-Q-ST-80C**: more is demanded of
  the parts whose failure would matter most.

## No compliance claim

**FerroAstro does not claim compliance with NPR 7150.2, with ECSS-E-ST-40C, with ECSS-Q-ST-80C, or with
any other software-engineering or product-assurance standard.** The project is not bound by them. It has
not been assessed against them, and nothing in this repository is evidence of conformance to them.

The standards were not read in full. Two points are open items in [OPEN-ITEMS.md](OPEN-ITEMS.md):

- OI-51: which NPR 7150.2D requirements (SWEs) apply at Class D/E. Appendix C was not reviewed.
- OI-52: the criticality definitions of ECSS-Q-ST-80C Table D-1, and whether that standard is now at Rev.2.

FerroAstro's levels `high`, `medium` and `low` are therefore its own. They are not the ECSS categories A–D.

## What is borrowed

Journal #5 names three ideas taken from the standards. Each has a place in the repository.

| Idea | Where it lives |
|---|---|
| An explicit classification | This file, and the `criticality` key that every requirement carries ([README](README.md), "Requirement format") |
| Requirements traceable to tests | `Verifies: <ID>` citations in tests and `requirement = [...]` lists in `tests/vectors/*.toml`, joined by `tools/trace.py` into the generated [TRACE.md](TRACE.md) |
| A record of verification evidence | The `status` lifecycle ([README](README.md), "Status lifecycle"), the evidence written in each requirement's `notes`, the provenance fields of every test vector (ADR-0004), and the `ci` workflow run of each commit |

## Criticality levels

The levels are defined in the [README](README.md), "Criticality tailoring":

| Level | Applied to |
|---|---|
| `high` | Physics correctness (the ladder rungs, equations of motion, estimator, constants, time scales, frames) and licence or legal obligations |
| `medium` | Infrastructure whose failure degrades evidence or reproducibility |
| `low` | Conveniences and documentation hygiene |

Criticality is a property of a **requirement**. No crate carries a criticality tag: no `Cargo.toml` in the
workspace has one. ORG-007 speaks of "crates tagged crit:high"; in this repository that is read as "code
that implements a `high` requirement". The crate-to-requirement map is the table in
[ARCHITECTURE.md](../strategy/ARCHITECTURE.md), and the test-to-requirement map is TRACE.md.

The GitHub labels `crit:high`, `crit:medium` and `crit:low` mark issues with the same three levels.

## What each level demands, and how it is checked

| Rule | Applies to | Checked by |
|---|---|---|
| `criticality` is one of `high`, `medium`, `low`, and `verification` is one of `test`, `analysis`, `inspection`, `demonstration` | every requirement | Tool: `tools/trace.py --check`, run in the `python` job of `.github/workflows/ci.yml` |
| A requirement at `approved` or later with `verification: test` has at least one citing test, or counted gate evidence | every level | Tool: `tools/trace.py --check` |
| Gate evidence (`verified_by_gate`) counts only when the gate is listed in `PINNED_GATES` in `tools/trace.py`; otherwise TRACE.md shows `gate (unpinned)` | `high` only; `medium` and `low` gates count without it | Tool: `tools/trace.py` |
| A change of `status` or `verified_by_gate`, and a change of `statement` or `acceptance` in `PHY.md` or `NUM.md`, needs a maintainer's approval of the PR head | every level | Tool: the `ratchet` check (`tools/ratchet.py`, TST-006) |
| Code implementing the requirement has a published-value test and, where an oracle exists, a reference-comparison test against an independent implementation | `high` | Person: the maintainer who approves the status change. No tool checks it |
| At least one automated test or a recorded inspection | `medium` | Person, as above. For `verification: test` the citing-test rule above also applies from `approved` |
| Inspection is enough | `low` | Person, as above |

The data a reviewer uses for the `high` rule is in the repository: each vector in `tests/vectors/*.toml`
declares its `kind` (`defined`, `measured`, `computed-reference` or `analytic`, per
`tests/vectors/schema.json`) with its source and citation, and the comparisons against independent
implementations are the tests of `crates/ferroastro-oracle` and the oracle definitions under `ci/`.

Review does not depend on criticality. A maintainer's review is required for the files listed in
`.github/CODEOWNERS` (the meta surface, ORG-011) and for the findings of the `ratchet` check; everything
else merges on the required checks (ORG-009, ADR-0015). `.github/CODEOWNERS` has no entry that selects
paths by criticality.

## Known gaps on 2026-11-23

- The `high` rule (published-value test plus reference-comparison test) is not checked by any tool.
- `PINNED_GATES` in `tools/trace.py` is empty, so every `high` requirement that names a gate shows as
  `gate (unpinned)` in TRACE.md.
- The README's criticality table says "Two approving reviews on `crit:high` paths via CODEOWNERS". The
  repository does not do this: see "Review does not depend on criticality" above.
- OI-51 and OI-52 are open.
