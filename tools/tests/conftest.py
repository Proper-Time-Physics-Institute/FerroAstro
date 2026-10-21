# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
"""Shared fixtures. ``trace.py`` shadows the stdlib ``trace`` module, so load it by path."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path
from types import ModuleType

import pytest

TOOLS = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="session")
def fa_trace() -> ModuleType:
    spec = importlib.util.spec_from_file_location("fa_trace", TOOLS / "trace.py")
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["fa_trace"] = module
    spec.loader.exec_module(module)
    return module
