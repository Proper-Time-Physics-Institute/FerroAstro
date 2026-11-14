---
status: accepted
date: 2026-10-14
decision-makers: maintainers (decision recorded in Journal #6)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Contributions: DCO now, CLA later

## Context and Problem Statement
The Institute wants an organisation-level CLA, but CLA text (especially for AI-agent-authored contributions) needs legal review first.

## Decision Outcome
Until counsel approves a CLA, every commit carries a **Developer Certificate of Origin** `Signed-off-by:` line, checked in CI. Agent-authored commits are signed off by the human or organisation operating the agent, who takes responsibility for them. The CLA (Apache ICLA/CCLA-derived, using the CLA Assistant Action with signatures in a private org repo) becomes ORG-010 once approved.

### Confirmation
DCO check required on `main` (ORG-008).

## Revision 2026-11-14 (proposed): signed commits and delegation records
Decided by the maintainer on 2026-10-14 (#12) and recorded with ADR-0015. Red-team C4 (Journal #9) showed that a `Signed-off-by:` trailer, under evidence-gated merges with no review, is text an agent can type, not evidence of a person.

* Each agent has a registered signing key (SSH or OpenPGP) tied to its operator. Keys live in `.github/signing/` (`allowed_signers`, `keys/*.asc`), each with a **delegation record** in `delegations.toml`: operator, agent, validity window and where the signed delegation statement is kept. Humans register their own keys the same way.
* CI (`signed-commits`, `tools/verify_signatures.py`) verifies every non-merge commit's signature against the registry read from the base branch. It also checks that the delegation names the operator in the commit's `Signed-off-by` and is valid on the commit date. A trailer alone is not trusted.
* The check starts in `report` mode, so it can be required without blocking open PRs. A maintainer switches it to `enforce` once the agents' keys are registered.

### Confirmation (for this revision)
`signed-commits` is a required check on `main` with `delegations.toml` in `enforce` mode. `tools/tests/test_verify_signatures.py` covers the rules (ORG-008).

