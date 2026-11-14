<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

## Summary

## Requirements
Verifies / implements: <!-- e.g. PHY-004, NUM-015 -->

## Checklist
- [ ] All commands in AGENTS.md pass locally
- [ ] Commits are signed off (DCO) and signed with a registered key (ORG-008)
- [ ] New numeric expectations come from `tests/vectors/` with provenance; no golden values changed without `golden-change`
- [ ] No GPL/LGPL/AGPL dependencies; `cargo deny check` passes
- [ ] Clean-room: I did not consult REBOUND/REBOUNDx/ASSIST/Swiftest source for this change
- [ ] ADR added or updated if this is a significant decision
- [ ] If this touches the meta surface (`.github/CODEOWNERS`, ORG-011) or the `ratchet` check flags it (TST-006): a maintainer has approved the current head; otherwise it merges on the evidence checks (ORG-009)
- [ ] I did not skip, deselect, hide or neuter a test in a way the `ratchet` does not detect. The ratchet detects exactly these forms (TST-006 notes, `detects_*`): a deleted or renamed Rust `#[test]` fn under `crates/` or Python `test*` function/method in a `tools/` test file; a new Rust `ignore`/`cfg`/`cfg_attr` attribute under `crates/` off the allowlist; a fall in the nextest executed count; under `tools/`, a new `skip`/`skipif`/`xfail`/`importorskip`/unittest-skip name, pytest `exit`, an assignment to `pytestmark`/`__test__`/`collect_ignore`/`collect_ignore_glob`, a `def pytest_*` hook or an unparsable file; and pytest configuration changes in `pytest.toml`, `.pytest.toml`, `pytest.ini`, `.pytest.ini`, `tox.ini`, `setup.cfg` or `pyproject.toml` under `tools/`. Anything else is NOT detected, including but not limited to the non-exhaustive `known_gap_*` examples in TST-006 notes (a skip built at run time, an early `return`, an emptied body, `#[should_panic]`, an empty `parametrize`, `sys.exit` in a test file, a test file moved out of `testpaths`, a replacing decorator); pytest has no test-count backstop. Reviewers check for these; disclose any such change here
