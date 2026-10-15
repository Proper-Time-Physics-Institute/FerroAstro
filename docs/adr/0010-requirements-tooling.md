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

