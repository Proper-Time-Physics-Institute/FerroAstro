<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Testing standards

1. **Real values, with provenance.** Expected values come from `tests/vectors/*.toml` (ADR-0004).
   Each has a `source`, `citation`, `retrieved`, `tolerance` and `tolerance_rationale`.
   - `kind = "computed-reference"` vectors also record the generating script@sha, the engine version and its settings.
2. **Traceability.** Each test cites the requirement(s) it verifies. `tools/trace.py` builds
   `docs/requirements/TRACE.md` and fails on uncovered or unknown IDs (ADR-0010).
3. **Tolerance-based float comparison only.** Use `approx`; `float_cmp` is denied (TST-001).
4. **Independent oracles, in priority order:**
   1. analytic formula
   2. published value
   3. independent code in a container
   4. self-consistency (conservation laws, Brouwer-law slopes, forward–backward reversibility)
5. **Tiers:**
   - PR-fast: under 10 min, cached goldens only.
   - Nightly long runs: `#[ignore]`, on larger runners.
   - Weekly DE440 reproduction: checkpointed (CI-010).
6. **Golden changes** need the maintainer-applied `golden-change` label, new evidence in the same PR and a
   CODEOWNERS review through the ORG-011 meta surface (TST-004). The `ratchet` check also fails widened
   tolerances, downgraded oracles, deleted tests, pytest configuration changes, and Rust `#[ignore]`/`cfg`
   gates or Python skip/xfail/collection gates off the allowlist that are new on the test or item they stand
   on (moving a gate onto another test counts), unless a maintainer approves. The ratchet is a heuristic,
   non-adversarial scanner: it detects these forms of test weakening when they are written in ordinary syntax
   (plain Rust attribute paths without `r#` written directly on the item, standard comments and literals,
   statement-level Python definitions, assignments and imports; TST-006 notes define each `detects_*` form and
   "ordinary"), and claims nothing else: a deleted or renamed `#[test]` fn under `crates/` or `test*`
   function/method in a `tools/` test file; a new Rust `ignore`/`cfg`/`cfg_attr` attribute under `crates/` off
   the allowlist; a fall in the nextest executed count; under `tools/`, a new
   `skip`/`skipif`/`xfail`/`importorskip`/unittest-skip name, pytest `exit`, an assignment to
   `pytestmark`/`__test__`/`collect_ignore`/`collect_ignore_glob`, a `def pytest_*` hook or an unparsable file;
   and a pytest configuration change in `pytest.toml`, `.pytest.toml`, `pytest.ini`, `.pytest.ini`, `tox.ini`,
   `setup.cfg` or `pyproject.toml` under `tools/`. Deliberately obfuscated or adversarially encoded code may
   evade it. Any other way of skipping, deselecting, hiding or neutering a test is NOT detected, and pytest has
   no test-count backstop. TST-006 notes give a non-exhaustive list of examples (`known_gap_*`, e.g. a skip
   built at run time, an emptied test body, Rust `#[should_panic]` on a failing test, an empty `parametrize`, a
   test file moved out of `testpaths`). Do not rely on CI for those: reviewers check for them, and authors
   disclose them in the PR (TST-006, ADR-0015).
7. **UNVERIFIED and secondary-source [S] values** may not be oracles until they are closed in
   [OPEN-ITEMS.md](../requirements/OPEN-ITEMS.md) (PHY-018).
8. **Compile-fail tests** (`trybuild`) prove that type-level physics guards work (API-001).
9. **Cached JPL data and local runs** (maintainer decision 2026-11-14). Tests that need the pinned
   `header.440` read it from the data cache (`$FA_DATA_DIR`, default `target/fa-data`; fill it with
   `(cd tools && uv run python fetch_jpl.py de440-header)`, or plain `fetch_jpl.py` for every
   PR-fast product). They go through `De440::load_cached_or_skip` (or
   `ferroastro_testdata::skip_if_absent`):
   - **header absent, `FA_REQUIRE_DATA` unset:** the test prints
     `SKIP <test>: skipped: header.440 not cached at <path> (set FA_DATA_DIR or run tools/fetch_jpl.py de440-header); FA_REQUIRE_DATA=1 makes this fail`
     and returns, so `cargo test` passes on a machine without the data. The line is test output:
     see it with `cargo test -- --nocapture` or `cargo nextest run --success-output immediate`.
   - **`FA_REQUIRE_DATA=1` (or `CI=true`):** a missing header fails the test loudly. CI sets it
     in every job that fetches the data and runs tests, and the guard
     `required_data_means_the_header_loads` fails if the header does not load, so CI can never
     pass by skipping.
   - **header present:** every test runs, with or without `FA_REQUIRE_DATA`. A file that does not
     match its pinned SHA-256 is always an error, never a skip.
   Only the header is skipped this way; a test that also reads another product (`de440.bsp`,
   `testpo.440`) still fails when that product is missing and the header is present. The orbit
   viewer's System-mode and Rung-lab tests skip separately when built without `embed-header`.
