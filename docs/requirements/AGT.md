<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# AGT — Agent-facing conventions

**Scope.** FerroAstro is built to be used and maintained by AI coding agents as well as people. These requirements cover the canonical `AGENTS.md`, `llms.txt` for the docs site, an `agent-task` issue form, and the structured-output contract of the future MCP server. Human oversight of agent PRs is ORG-009. Sign-off for agent commits is ORG-008. Source: Journal #5 §6 (`docs/strategy/journal/05-organization-standards.md`).

---

### AGT-001 — Canonical AGENTS.md

```yaml
id: AGT-001
title: "Root AGENTS.md is canonical with exact commands, provenance rules and forbidden actions"
status: draft
criticality: medium
verification: inspection
statement: >-
  The repository SHALL have a root AGENTS.md (canonical) containing the exact
  build, test and lint commands, the provenance rules and the forbidden
  actions. CLAUDE.md SHALL defer to it.
acceptance: >-
  AGENTS.md lists cargo nextest run, cargo clippy --all-targets -- -D warnings,
  uv run ruff check and uv run tools/trace.py --check; states the
  "every numeric test cites a vector file" rule; and lists the forbidden
  actions (no unsafe, no new deps without cargo deny check, no editing golden
  values without a provenance update). CLAUDE.md imports or points to it. A CI
  link check passes.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 and AGT-001;
  https://agents.md/.
parent: null
related: [ORG-004, ORG-009, TST-002]
notes: >-
  Whether `@AGENTS.md` is still the recommended Claude Code import pattern is
  UNVERIFIED. Crate-level crates/*/AGENTS.md hold local physics conventions.
```

### AGT-002 — llms.txt

```yaml
id: AGT-002
title: "Docs site publishes llms.txt (nice-to-have, non-gating)"
status: draft
criticality: low
verification: inspection
statement: >-
  The docs site SHALL publish llms.txt (and llms-full.txt). This is a
  nice-to-have and SHALL NOT gate CI.
acceptance: >-
  The published docs site serves /llms.txt.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 and AGT-002;
  https://llmstxt.org/.
parent: null
related: []
notes: >-
  llms.txt is a community proposal, not an official standard.
```

### AGT-003 — agent-task issue form

```yaml
id: AGT-003
title: "Issue forms include an agent-task template with scope, files, done-definition and commands"
status: draft
criticality: low
verification: inspection
statement: >-
  Issue forms SHALL include an agent-task template asking for scope, files, a
  done-definition and the commands to verify.
acceptance: >-
  .github/ISSUE_TEMPLATE/agent-task.yml exists with those four fields
  required.
source: >-
  docs/strategy/journal/05-organization-standards.md §2.4 and AGT-003.
parent: null
related: [ORG-006]
notes: null
```

### AGT-004 — MCP structured output

```yaml
id: AGT-004
title: "MCP server tools declare outputSchema and return conforming structuredContent"
status: draft
criticality: medium
verification: test
statement: >-
  The future MCP server SHALL declare an outputSchema for each tool and return
  structuredContent conforming to it, with a TextContent copy of the same JSON
  for backward compatibility (MCP spec 2025-06-18 or later).
acceptance: >-
  A test calls each tool (propagate, convert_frame, convert_time, …) and
  validates structuredContent against the declared outputSchema; the
  TextContent copy parses to the same JSON.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 and AGT-004; MCP tools
  spec 2025-11-25.
parent: null
related: [DEP-013]
notes: >-
  DEP-013 pins the targeted spec revision (2026-07-28 via rmcp 3.x). Tool
  errors using the isError result flag are UNVERIFIED in the spec's
  error-handling section.
```
