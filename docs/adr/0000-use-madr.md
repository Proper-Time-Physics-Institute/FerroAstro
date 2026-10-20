---
status: accepted
date: 2026-10-14
decision-makers: Proper-Time Physics Institute maintainers
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Use MADR for decisions

## Context and Problem Statement
A large, agent-assisted project needs decisions that both humans and agents can find, parse and trust.

## Decision Outcome
Chosen: MADR 4.0.0 markdown records in `docs/adr/`, each with a Confirmation section naming its enforcing check.

### Consequences
* Good: plain text, diffable, readable by agents.
* Bad: no tooling enforces the template yet. A future `tools/` lint may.

