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

