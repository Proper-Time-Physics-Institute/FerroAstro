# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Unit tests of the WP28 nightly summary (review W3): labels from synthetic nextest logs.

# Verifies: CI-006, TST-002
"""

from __future__ import annotations

from pathlib import Path

import nightly_summary as ns

KNOWN = """
schema_version = 1

[[failure]]
test = "nightly_alpha"
vector = "VEC-A"
review = "notes A"
last_value = 2.0e-5
unit = "km"
measured_on = "today"
value_regex = 'FAIL VEC-A: FA-E0105: VEC-A .*?measured ([0-9.eE+-]+) km'
rel_change = 1e-9

[[failure]]
test = "nightly_beta"
vector = "VEC-B"
review = "notes B"
last_value = 3.0
unit = "1"
measured_on = "today"
value_regex = 'id: "VEC-B".*?measured: ([0-9.eE+-]+)'
rel_change = 1e-9
"""


def _log(alpha: str, beta: str, extra: str = "") -> str:
    return (
        "        FAIL [   1.0s] (1/3) crate::bin nightly_alpha\n"
        "  stdout ───\n"
        f"      FAIL VEC-A: FA-E0105: VEC-A (q): measured {alpha} km, expected 0 km\n"
        f"{extra}"
        "        FAIL [   0.1s] (2/3) crate::bin nightly_beta\n"
        "    called `Result::unwrap()` on an `Err` value: "
        f'ToleranceExceeded(Deviation {{ id: "VEC-B", measured: {beta}, expected: 0.0 }})\n'
        "        PASS [   0.1s] (3/3) crate::bin nightly_gamma\n"
        "        FAIL [   1.0s] (1/3) crate::bin nightly_alpha\n"
    )


def _summary(tmp_path: Path, log: str) -> list[ns.Line]:
    known = tmp_path / "known.toml"
    known.write_text(KNOWN, encoding="utf-8")
    return ns.summarise(ns.parse_log(log), ns.load_known(known))


def test_unchanged_known_failures(tmp_path: Path) -> None:
    lines = _summary(tmp_path, _log("2e-05", "3.0"))
    assert [ln.label for ln in lines] == ["known (value unchanged)"] * 2


def test_a_moved_value_is_known_changed(tmp_path: Path) -> None:
    lines = _summary(tmp_path, _log("2.5e-05", "3.0"))
    assert lines[0].label == "known (value changed)"
    assert "rel. change 2.50e-01" in lines[0].detail


def test_an_unlisted_failing_test_is_new(tmp_path: Path) -> None:
    log = _log("2e-05", "3.0") + "        FAIL [   1.0s] (4/4) crate::bin nightly_delta\n"
    lines = _summary(tmp_path, log)
    assert ns.Line("NEW", "nightly_delta", "FAIL; not in the known-failures list") in lines


def test_an_unlisted_check_in_a_known_test_is_new(tmp_path: Path) -> None:
    extra = "      FAIL VEC-Z: FA-E0105: VEC-Z (q): measured 1 km\n"
    labels = {(ln.label, ln.detail[:30]) for ln in _summary(tmp_path, _log("2e-05", "3", extra))}
    assert ("NEW", "failing vector VEC-Z is not li") in labels
    assert ("NEW", "unlisted failing check: VEC-Z:") in labels


def test_a_known_test_that_passes_or_is_absent_is_reported(tmp_path: Path) -> None:
    log = "        PASS [   1.0s] (1/1) crate::bin nightly_alpha\n"
    lines = _summary(tmp_path, log)
    assert ns.Line("known failure now PASSES", "nightly_alpha", "listed; ask a human") in lines
    assert ns.Line("not run", "nightly_beta", "listed, but absent from this log") in lines


def test_missing_value_counts_as_changed(tmp_path: Path) -> None:
    log = "        FAIL [   1.0s] (1/1) crate::bin nightly_beta\n    panicked\n"
    lines = _summary(tmp_path, log)
    assert lines[0].label == "known (value changed)"


def test_the_committed_list_loads_and_every_regex_has_one_group() -> None:
    known = ns.load_known(ns.DEFAULT_KNOWN)
    assert len(known) >= 5
    assert all(k.value_regex.groups == 1 for k in known)


def test_exit_status(tmp_path: Path) -> None:
    known = tmp_path / "known.toml"
    known.write_text(KNOWN, encoding="utf-8")
    good, bad = tmp_path / "good.log", tmp_path / "bad.log"
    good.write_text(_log("2e-05", "3.0"), encoding="utf-8")
    bad.write_text(_log("2e-05", "4.0"), encoding="utf-8")
    assert ns.main([str(good), "--known", str(known)]) == 0
    assert ns.main([str(bad), "--known", str(known)]) == 1
    assert ns.main([str(tmp_path / "missing.log"), "--known", str(known)]) == 2
