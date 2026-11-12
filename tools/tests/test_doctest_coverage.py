# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Tests of the API-003 doctest-coverage scan."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("doctest_coverage", HERE / "doctest_coverage.py")
assert spec is not None
assert spec.loader is not None
dc = importlib.util.module_from_spec(spec)
sys.modules["doctest_coverage"] = dc
spec.loader.exec_module(dc)

SOURCE = """
/// Has a runnable example.
///
/// ```
/// assert!(true);
/// ```
#[must_use]
pub fn covered() {}

/// Only a text block.
///
/// ```text
/// not run
/// ```
pub const fn text_only() {}

pub(crate) fn internal() {}

#[cfg(test)]
mod tests {
    /// ```
    /// x
    /// ```
    pub fn in_tests() {}
}
"""


# Verifies: API-003
def test_scan_counts_runnable_examples_only() -> None:
    items = dc.items_in(SOURCE, "c", "c/src/lib.rs")
    names = {i.name: i.covered for i in items}
    assert names == {"covered": True, "text_only": False}


# Verifies: API-003
def test_workspace_scan_runs() -> None:
    items = dc.scan()
    assert items
    assert all(i.path.startswith("crates/") for i in items)
