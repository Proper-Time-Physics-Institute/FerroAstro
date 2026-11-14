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

### TST-004 — Golden changes need maintainer label, new evidence and meta-surface review (revised)

```yaml
id: TST-004
title: "Golden value or tolerance changes carry the maintainer-applied golden-change label, new evidence and an ORG-011 review"
status: draft
criticality: high
verification: test
statement: >-
  A change to a golden value or tolerance SHALL carry the golden-change label
  applied by a maintainer, SHALL bring new evidence for the changed vector in
  the same PR, and SHALL have a maintainer review, which ORG-011 requires
  because tests/vectors/ is on the meta surface. A computed-reference vector
  whose generated_by SHA is a commit of the same PR SHALL be rejected.
acceptance: >-
  The ratchet check (tools/ratchet.py) fails a PR that changes a vector value
  or tolerance unless the golden-change label is on it and its most recent
  application was by a maintainer, and unless one of source, citation,
  retrieved, verified_by or generated_by of that vector also changed. It fails
  without override a computed-reference vector whose generated_by SHA is in
  the PR's commits. The ruleset requires Code Owner review for tests/vectors/.
source: >-
  docs/strategy/journal/05-organization-standards.md §7 rule 3 and TST-004;
  GitHub issue #12 §3; ADR-0015.
parent: null
related: [CI-004, ORG-011, TST-006]
notes: >-
  REVISED by ADR-0015 (#12). The CODEOWNERS review of goldens is kept, now
  through the ORG-011 meta surface. "New evidence" is checked mechanically as
  a change to an evidence field of the same vector; whether that evidence is
  adequate is the reviewer's call. Labels set by non-maintainers have no
  effect (tools/governance.py).
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

### TST-006 — The evidence ratchet

```yaml
id: TST-006
title: "A base-branch ratchet fails a PR that weakens the evidence in a detected way, unless a maintainer approves"
status: draft
criticality: high
verification: test
statement: >-
  A check executed from the base branch SHALL fail a PR, unless a maintainer
  approves the PR's current head, when: a vector tolerance widens; a vector's
  oracle kind is downgraded (analytic or defined, then measured, then
  computed-reference); |Δvalue| exceeds k times the old effective tolerance
  (k in ci/ratchet.toml); a vector or a test is deleted; a vector gains
  blocked_by; the number of tests the PR-fast tier executes falls (cargo
  nextest list JSON); pytest configuration under tools/ changes (pytest.ini,
  .pytest.ini, the [pytest] section of tox.ini, [tool:pytest] of setup.cfg,
  [tool.pytest] of a pyproject.toml); a gate appears on an item that did not
  carry it, and it is not on the allowlist in ci/ratchet.toml or names a
  Cargo feature the base tree does not declare; or a requirement field owned
  by ORG-011 changes. A Rust test is a fn whose outer attributes include
  test or <path>::test, and a Python test is what pytest's default naming
  collects from the parsed module (module-level test* functions and test*
  methods of Test* classes; one under if, try or with, inside a string, or
  del-ed or rebound later is deleted). A gate is keyed by its file, the item
  it stands on and its text, so moving an existing gate onto another test is
  a new gate. In Rust under crates/, a gate is an ignore, cfg or cfg_attr
  attribute anywhere outside comments and string literals (whitespace inside
  it ignored, also across lines), keyed by the item it decorates (inline mod
  path, kind and name). In Python under tools/, found by parsing whatever the
  module alias and keyed by the function or class it decorates or sits in, a
  gate is: a call or reference to skip, skipif, xfail, importorskip, skipIf,
  skipUnless, skipTest, expectedFailure or SkipTest (so pytest.mark
  decorators, pytest.skip/xfail/importorskip calls in a test body and
  unittest skips); such a name, or pytest's exit, imported under any alias;
  pytest.exit; an assignment to pytestmark, __test__, collect_ignore or
  collect_ignore_glob, or append/extend/insert on the last two; a def
  pytest_* hook; or a file that does not parse. From
  milestone M2, mutation testing (cargo-mutants) SHALL run on changed physics
  functions, with a ratchet on surviving mutants.
acceptance: >-
  The `ratchet` and `ratchet (test count)` checks of
  .github/workflows/ratchet.yml are required on main and run tools/ratchet.py
  and ci/ratchet.toml from a base-SHA checkout, comparing the PR head with
  git merge-base(base, head), in jobs that never build or run PR code.
  pytest shows each listed weakening is detected, that each known gap
  named in the notes is not (one test_known_gap_* per gap, asserting no
  finding, and a test that the notes, the ratchet.py docstring and those
  tests name the same gaps), that a maintainer's
  approval of the head SHA lets an approval finding pass, that the repository
  compared with itself has no finding, and that a branch behind main gets no
  finding for what main gained after the branch point. From M2: a cargo-mutants job is required and fails when the number
  of surviving mutants in changed physics functions rises.
source: >-
  GitHub issue #12 §3 (maintainer decisions 2026-10-14, including mutation
  testing from M2); red-team review, Journal #9; ADR-0015.
parent: TST-002
related: [TST-004, ORG-009, ORG-011, TST-005]
notes: >-
  NEW (ADR-0015). Approval means a review in state APPROVED by a login in
  .github/governance/maintainers.txt on the current head SHA; a later push
  needs a new approval. Rust tests are matched by crate and function name,
  Python tests by file name and Class::function, so a rename counts as a
  deletion and needs approval. The test-count job lists
  each tree with its own .config/nextest.toml, so filtering tests out through
  a profile edit also lowers the count. The head listing is produced by PR
  code, so it is the PR's own claim; deleted tests are caught by the source
  scan independently. Known gaps, NOT detected (each pinned by a test
  test_<name> in tools/tests/test_ratchet.py that asserts no finding, so
  this list and the code cannot drift; follow-up issue "ratchet: close the
  detection gaps listed in TST-006", #55): known_gap_py_runtime_gate, a Python
  skip or __test__ built at run time (getattr(pytest, "sk" + "ip"),
  setattr(f, "__test__", False), globals()["__test__"] = False);
  known_gap_py_existing_helper, a new call to a helper that already existed
  and skips; known_gap_hollowed_test, a Python or Rust test that returns
  early, whose body is emptied, or whose assertions are put under dead code
  (Rust if false { ... }); known_gap_py_nondefault_name, deleting a
  test pytest collects outside its default naming (e.g. a method of a
  unittest.TestCase subclass not named Test*); known_gap_py_no_count, there
  is no executed-test count for pytest, so nothing backstops a Python gap;
  known_gap_rs_allowlisted_gate, an allowlisted gate hiding a Rust test
  from the PR tier: cfg(feature = ...) or cfg(not(feature = ...)) on a
  feature the base declares (e.g. cfg(not(feature = "std")) with std on by
  default), or ignore = "data..."/"network..." on a test that needs neither
  (the reason is not checked), caught only by the test count, whose head
  listing is the PR's own (cfg(not(target_arch = "wasm32")) is allowlisted
  and hides nothing from the x86_64 nextest run); known_gap_rs_uncompiled_file, a
  test file moved to a path cargo does not compile, or a dropped mod line,
  caught only by the test count; known_gap_rs_same_name_items, a gate moved
  between two same-named Rust fns of one module (e.g. in two impl blocks);
  known_gap_scan_scope, only Rust under crates/ and Python under tools/ are
  scanned; known_gap_py_outcome_exception, raising pytest's outcome
  exceptions directly (from _pytest.outcomes import Skipped, XFailed, Exit or
  OutcomeException, then raise Skipped("x")), reported as skipped;
  known_gap_py_empty_parametrize, @pytest.mark.parametrize("x", []) on a
  test, which an empty parameter set skips; known_gap_py_process_exit,
  os._exit(0) or sys.exit(0) in a test file (e.g. a new
  tools/tests/test_0.py), which stops pytest with exit 0 and no summary so
  later tests never run; known_gap_py_uncollectable_class, __init__ or
  __new__ added to a Test* class, which pytest then does not collect (a
  warning only); known_gap_py_fixture_on_test, @pytest.fixture on a test*
  function, no longer collected but still counted as a test by the deletion
  check; known_gap_py_assigned_hook, a pytest hook bound by assignment
  (pytest_collection_modifyitems = lambda ...), e.g. in a subdirectory
  conftest.py off the CODEOWNERS surface (only def pytest_* is detected);
  known_gap_rs_verdict_attribute, #[should_panic] added to a failing Rust
  test, which turns the failure into a pass; the test still runs, so the
  test count does not see it (only ignore, cfg and cfg_attr are gate
  attributes). The Python gate names are matched on any expression, so args.skip
  in non-test code also needs approval (over-detection). tools/pyproject.toml
  (pytest options) and
  tools/tests/conftest.py are on the ORG-011 meta surface. A new Cargo
  feature that gates code needs approval until it is in the base tree. The
  mutation-testing part is not implemented before M2.
```
