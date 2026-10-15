---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# AGENTS.md is canonical agent guidance

## Decision Outcome
The root `AGENTS.md` (agents.md convention, stewarded by the Agentic AI Foundation / Linux Foundation) is the single source of agent instructions: commands, provenance rules and forbidden actions. `CLAUDE.md` imports it with `@AGENTS.md` and adds only Claude-specific notes. Crate-level `AGENTS.md` files hold local physics conventions. `llms.txt` is a non-gating nice-to-have.

### Confirmation
Inspection, plus a CI link check that the commands listed in AGENTS.md exist in the workflows (AGT-001).

