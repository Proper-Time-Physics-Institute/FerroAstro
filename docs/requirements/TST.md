<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# TST — Test discipline and test-vector provenance

**Scope.** These requirements set how FerroAstro's tests handle numbers. Floating-point comparisons always carry explicit tolerances. Every expected value is loaded from a provenance-tracked vector file under `tests/vectors/`, never written as a bare literal. Computed reference values record how they were generated. Golden values change only through review, and results are deterministic. Source: Journal #5 §7 (`docs/strategy/journal/05-organization-standards.md`). The Journal #5 lead review renumbered these requirements from `NUM-00x` to `TST-00x`, because `NUM-` belongs to Journal #4.

---

### TST-001 — Explicit float tolerances; lossy casts denied

```yaml
id: TST-001
title: "Float comparisons use explicit tolerances; lossy casts denied unless #[expect] with reason"
status: draft
criticality: high
verification: test
statement: >-
  Floating-point comparisons in library and test code SHALL use explicit
  tolerances (clippy float_cmp denied). Lossy casts (cast_precision_loss,
  cast_possible_truncation, cast_sign_loss) SHALL be denied unless they carry
  an #[expect] with a reason.
acceptance: >-
  The clippy CI job passes with float_cmp, float_cmp_const,
  cast_precision_loss, cast_possible_truncation and cast_sign_loss at deny.
  A canary `==` on f64 fails.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.3 and TST-001
  (renumbered from NUM-).
parent: null
related: [STD-002]
verified_by_gate: "ci/rust/lint config canary"
notes: >-
  Test modules may allow float_cmp only for exact-by-construction checks.
  float_cmp_const group membership is UNVERIFIED.
```

### TST-002 — Expected values come from provenance-tracked vectors

```yaml
id: TST-002
title: "Every numeric test loads its expected value from tests/vectors/*.toml; no bare literals"
status: draft
criticality: high
verification: test
statement: >-
  Every numeric test SHALL load its expected value by id from
  tests/vectors/*.toml, where each vector carries source, citation, retrieved,
  tolerance, tolerance_rationale and requirement. Tests SHALL NOT contain bare
  physical literals.
acceptance: >-
  CI validates every vector file against tests/vectors/schema.json, and
  tools/lint_literals.py finds no bare physical literal in test code.
source: >-
  docs/strategy/journal/05-organization-standards.md §7 and TST-002.
parent: null
related: [PHY-002, PHY-018, REF-002, TST-003]
notes: >-
  Journal #5's illustrative vectors mis-tag requirements: CONST-GM-SUN-N cites
  PHY-001 (should be PHY-003) and MERC-GR-1PN-ANALYTIC cites PHY-012 (should be
  PHY-004/PHY-005). Use the PHY.md numbering. Values marked [S] or UNVERIFIED
  cannot enter a vector until PHY-018 clears them.
  blocked_by FIELD (sync 2026-10-20; WP14 on m3-newtonian-nbody; ADR-0004
  revision pending maintainer confirmation): a vector may carry an optional
  blocked_by = "OI-NN" (added to tests/vectors/schema.json and the
  ferroastro-vectors parser, Vector::blocking_open_item). A blocked vector is
  still loaded, printed and compared, but its test reports instead of
  failing while the item is unticked in OPEN-ITEMS.md (or missing from it);
  it becomes gating automatically once a maintainer ticks the item. In use:
  every Park17 Table 3 row and N-AST (OI-38), the full-set SSB offset
  (OI-24) and the NUM-009 cost ratio (OI-42). Consequence: ticking an open
  item can turn a nightly red without any code change (the WP15 review's
  F13: ticking OI-38 alone would arm the P-add rows, 4 of which fail), so a
  tick and the matching protocol or vector change belong in the same PR.
  A blocked test cannot fail, so it is not evidence for its requirement
  until unblocked (progress review after WP15).
```

### TST-003 — Computed-reference vectors record their generator

```yaml
id: TST-003
title: "Computed-reference vectors record generating script, commit SHA, engine version and settings"
status: draft
criticality: high
verification: test
statement: >-
  Vectors with kind = "computed-reference" SHALL record the generating script
  path with commit SHA, and the reference engine with its version and
  settings (e.g. Orekit x.y, REBOUND x.y, integrator settings).
acceptance: >-
  The schema check rejects a computed-reference vector without generated_by
  (path@sha) and engine version/settings.
source: >-
  docs/strategy/journal/05-organization-standards.md §7 rule 2 and TST-003.
parent: TST-002
related: [CI-003]
notes: null
```

### TST-004 — Golden changes need label and CODEOWNERS review

```yaml
id: TST-004
title: "Changes to golden values or tolerances carry golden-change label and CODEOWNERS review"
status: draft
criticality: high
verification: inspection
statement: >-
  Changes to golden values or tolerances SHALL carry the golden-change label
  and a review from a CODEOWNERS owner of tests/vectors/.
acceptance: >-
  The branch ruleset requires CODEOWNERS review for tests/vectors/; a CI check
  fails a PR that modifies a value or tolerance without the golden-change
  label.
source: >-
  docs/strategy/journal/05-organization-standards.md §7 rule 3 and TST-004.
parent: null
related: [CI-004]
notes: null
```

### TST-005 — Deterministic results

```yaml
id: TST-005
title: "Results are deterministic on a platform; cross-platform float caveats documented"
status: draft
criticality: medium
verification: test
statement: >-
  Numerical results SHALL be deterministic across runs on the same platform
  (fixed seeds, no wall-clock dependence), and cross-platform float caveats
  SHALL be documented.
acceptance: >-
  A test runs the same scenario twice and asserts bitwise-identical output;
  the docs contain a float-reproducibility section.
source: >-
  docs/strategy/journal/05-organization-standards.md §6 item 4 and TST-005.
parent: null
related: [NUM-021, CI-010]
notes: >-
  FMA policy matters: NUM-021 requires an explicit mul_add policy, while
  Journal #5 §6 says "no implicit FMA". These are consistent: FMA only where
  written explicitly.
```
