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

