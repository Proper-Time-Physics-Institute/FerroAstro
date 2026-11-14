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
- [ ] I did not disable or hollow out a test in a way the `ratchet` does not detect. It detects Rust `ignore`/`cfg`/`cfg_attr` attributes (any spacing) off the allowlist, and Python `@pytest.mark.skip`/`skipif`/`xfail`, `pytest.skip()`/`xfail()`/`importorskip()`, unittest skips and `pytestmark`/`__test__`/`collect_ignore` assignments. It does NOT detect (TST-006 notes, `known_gap_*`): skips built at run time, calls to an existing skipping helper, early `return`, emptied test bodies or assertions under Rust `if false`, Rust `#[should_panic]` on a failing test, pytest outcome exceptions raised directly (`_pytest.outcomes.Skipped`/`XFailed`/`Exit`), an empty `@pytest.mark.parametrize(..., [])`, `os._exit`/`sys.exit` in a test file, `__init__`/`__new__` on a `Test*` class, `@pytest.fixture` on a `test*` function, pytest hooks bound by assignment (`pytest_* = ...`), an allowlisted `cfg(feature = ...)` or `ignore = "data..."` that hides a test, test files moved out of the build or dropped `mod` lines, gates moved between same-named fns, or anything outside `crates/` and `tools/`; and pytest has no test-count backstop. Disclose any such change here
