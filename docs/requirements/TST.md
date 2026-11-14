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
title: "A base-branch ratchet fails a PR that weakens the evidence in one of a closed list of detected forms, unless a maintainer approves"
status: draft
criticality: high
verification: test
statement: >-
  A check executed from the base branch SHALL fail a PR, unless a maintainer
  approves the PR's current head, when: a vector tolerance widens; a vector's
  oracle kind is downgraded (analytic or defined, then measured, then
  computed-reference); |Δvalue| exceeds k times the old effective tolerance
  (k in ci/ratchet.toml); a vector is deleted; a vector gains blocked_by; a
  requirement field owned by ORG-011 changes; or the PR weakens a test in one
  of the forms the ratchet detects. The ratchet detects exactly these forms
  of test weakening, and no others: detects_rs_test_deleted,
  detects_rs_gate_attr, detects_rs_executed_count, detects_py_test_deleted,
  detects_py_gate_name, detects_py_exit, detects_py_collection_target,
  detects_py_hook_def, detects_py_unparsable and detects_pytest_config, as
  defined in the notes. Any other way of skipping, deselecting, hiding or
  neutering a test is NOT detected, and this requirement claims nothing
  about it. From milestone M2, mutation testing (cargo-mutants) SHALL run on
  changed physics functions, with a ratchet on surviving mutants.
acceptance: >-
  The `ratchet` and `ratchet (test count)` checks of
  .github/workflows/ratchet.yml are required on main and run tools/ratchet.py
  and ci/ratchet.toml from a base-SHA checkout, comparing the PR head with
  git merge-base(base, head), in jobs that never build or run PR code.
  pytest shows that each listed vector and requirement weakening and each
  detects_* form is detected (DETECTED_FORMS in tools/tests/test_ratchet.py
  maps each form to its tests); that each example of an undetected form
  named in the notes is not detected (one test_known_gap_* per example,
  asserting no finding); that the notes, the ratchet.py docstring and those
  tests name the same detects_* forms and the same examples, and that every
  document stating the guarantee calls the detected list exact and the
  examples non-exhaustive; that a maintainer's approval of the head SHA lets
  an approval finding pass; that the repository compared with itself has no
  finding; and that a branch behind main gets no finding for what main
  gained after the branch point. From M2: a cargo-mutants job is required
  and fails when the number of surviving mutants in changed physics
  functions rises.
source: >-
  GitHub issue #12 §3 (maintainer decisions 2026-10-14, including mutation
  testing from M2); red-team review, Journal #9; ADR-0015.
parent: TST-002
related: [TST-004, ORG-009, ORG-011, TST-005]
notes: >-
  NEW (ADR-0015). Approval means a review in state APPROVED by a login in
  .github/governance/maintainers.txt on the current head SHA; a later push
  needs a new approval. The ratchet detects exactly these forms of test
  weakening (each pinned by tests, DETECTED_FORMS in
  tools/tests/test_ratchet.py): detects_rs_test_deleted, a Rust test removed
  or renamed, where a test is a fn in a .rs file under crates/ whose run of
  outer attributes holds test or <path>::test, outside comments and string
  literals, counted by crate directory and function name;
  detects_rs_gate_attr, a new ignore, cfg or cfg_attr attribute (outer or
  inner) in a .rs file under crates/, outside comments and string literals,
  whitespace normalised (also across lines), new when its file, item (inline
  mod path, kind and name) and text were not in the merge base, unless it
  matches gate_allowlist in ci/ratchet.toml and names no Cargo feature
  undeclared in the merge base; detects_rs_executed_count, fewer tests that
  cargo nextest list --workspace (default profile, each tree with its own
  .config/nextest.toml) reports as run, not ignored and not filtered out,
  for the head than for the merge base (the head listing is produced by PR
  code); detects_py_test_deleted, a Python test removed or renamed, where a
  test is, in a file under tools/ named test_*.py or *_test.py, a
  module-level def test* or a def test* in a (nested) class Test*, as a
  direct statement of the parsed module still bound at the end of its scope
  (so one put in a string, under if, try or with, del-ed or rebound is
  removed), counted by file name and Class::function; detects_py_gate_name,
  in any .py file under tools/, a new call or reference <expr>.<name> with
  <name> one of skip, skipif, xfail, importorskip, skipIf, skipUnless,
  skipTest, expectedFailure, SkipTest (so pytest.mark decorators,
  pytest.skip/xfail/importorskip calls and unittest skips), or a bare name
  imported as one of those from pytest, _pytest or unittest (or their
  submodules) under any alias; detects_py_exit, exit on a name bound by
  import pytest or import _pytest[.sub] (any alias) or by from
  pytest|_pytest[.sub] import ..., or a bare exit imported from those under
  any alias; detects_py_collection_target, an assignment (plain, annotated
  or augmented) whose target contains the name or attribute pytestmark,
  __test__, collect_ignore or collect_ignore_glob, or append, extend, insert
  or __setitem__ called on one of those names; detects_py_hook_def, a def
  or async def named pytest_* at any scope; detects_py_unparsable, a .py
  file under tools/ that does not parse; detects_pytest_config, pytest
  configuration added, changed or removed in a file at any depth under
  tools/: all of pytest.toml, .pytest.toml, pytest.ini and .pytest.ini, the
  [pytest] section of tox.ini, [tool:pytest] of setup.cfg and [tool.pytest]
  of pyproject.toml (unparsable: compared as text). Python gates are keyed
  by file, enclosing Class::function and unparsed text, so moving a gate
  onto another test is new. Directories target, .venv, .git, node_modules,
  __pycache__ and .mypy_cache are not scanned. A rename counts as a
  deletion and needs approval. Any other way of skipping, deselecting,
  hiding or neutering a test is NOT detected; reviewers must check for it.
  Non-exhaustive examples, each pinned by test_<id> in
  tools/tests/test_ratchet.py asserting no finding (issue #55 collects
  them): known_gap_py_runtime_gate, a skip or __test__ built at run time
  (getattr(pytest, "sk" + "ip"), setattr(f, "__test__", False),
  globals()["__test__"] = False); known_gap_py_existing_helper, a new call
  to a helper that already existed and skips; known_gap_hollowed_test, a
  Python or Rust test that returns early, whose body is emptied, or whose
  assertions are under dead code (Rust if false { ... });
  known_gap_py_nondefault_name, deleting a test pytest collects outside its
  default naming (a unittest.TestCase method in a class not named Test*);
  known_gap_py_no_count, there is no executed-test count for pytest, so
  nothing backstops a Python gap; known_gap_rs_allowlisted_gate, an
  allowlisted gate hiding a Rust test from the PR tier (cfg(feature = ...)
  or cfg(not(feature = ...)) on a declared feature, e.g.
  cfg(not(feature = "std")) with std on by default, or ignore =
  "data..."/"network..." on a test that needs neither), visible only to the
  test count, whose head listing is the PR's own;
  known_gap_rs_uncompiled_file, a test file moved to a path cargo does not
  compile, or a dropped mod line, visible only to the test count;
  known_gap_rs_same_name_items, a gate moved between two same-named Rust
  fns of one module (two impl blocks); known_gap_scan_scope, Rust outside
  crates/ and Python outside tools/; known_gap_py_outcome_exception, raising
  pytest's outcome exceptions directly (from _pytest.outcomes import
  Skipped, XFailed, Exit or OutcomeException, then raise Skipped("x"));
  known_gap_py_empty_parametrize, @pytest.mark.parametrize("x", []);
  known_gap_py_process_exit, os._exit(0) or sys.exit(0) in a test file,
  which stops pytest with exit 0 and no summary;
  known_gap_py_uncollectable_class, __init__ or __new__ on a Test* class
  (not collected, a warning only); known_gap_py_fixture_on_test,
  @pytest.fixture on a test* function (not collected, still counted);
  known_gap_py_assigned_hook, a pytest hook bound by assignment
  (pytest_collection_modifyitems = lambda ...);
  known_gap_rs_verdict_attribute, #[should_panic] on a failing Rust test,
  which turns the failure into a pass (the test still runs, so the count
  does not see it); known_gap_py_aliased_exit, exit through a rebound
  module object (p = pytest; p.exit()) or one imported at run time
  (importlib.import_module("pytest").exit()); known_gap_py_moved_out_of_testpaths,
  a test file moved out of pytest's testpaths (tests are keyed by file
  name); known_gap_py_replacing_decorator, a decorator that replaces the
  test with a no-op; known_gap_py_imported_marker, pytestmark bound by from
  m import pytestmark; known_gap_rs_macro_gate, a gate passed to a macro as
  tokens (#[$a] in a macro_rules!, invoked as t!(ignore)). Python gate
  names are matched on any expression, so args.skip in non-test code also
  needs approval (over-detection). tools/pyproject.toml,
  tools/tests/conftest.py and the pytest configuration files above are on
  the ORG-011 meta surface. A new Cargo feature that gates code needs
  approval until it is in the base tree. The mutation-testing part is not
  implemented before M2.
```
