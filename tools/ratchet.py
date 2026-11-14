# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
r"""The evidence ratchet (TST-006, TST-004, ORG-011; ADR-0015).

Compares a PR head tree with the tree it branched from and fails when the evidence gets weaker.
The ``ratchet`` workflow runs this file from a checkout of the **base** SHA (the current tip of
``main``), with its configuration ``ci/ratchet.toml`` also read from there, so a PR cannot loosen
the check that judges it.

What is compared is the head against ``git merge-base(base, head)``, never against the base tip:
``main`` moves on after a PR branches, and everything it gained since (new tests, vectors,
requirement status changes) would otherwise read as removed by the PR. ``static`` exports the
merge-base tree from the head checkout's history (``git archive``) unless ``--against-root`` names
a tree explicitly (tests). ``test-count`` takes nextest listings that the workflow produced for the
merge base and for the head in separate jobs.

Usage (from the base checkout's ``tools/``)::

    uv run --locked python ratchet.py static --base .. --head ../../head \\
        --base-sha BASE --head-sha HEAD [--maintainer-approved] [--golden-change]
    uv run --locked python ratchet.py merge-base --repo ../../head --base-sha BASE --head-sha HEAD
    uv run --locked python ratchet.py test-count --base-list base.json --head-list head.json \\
        [--maintainer-approved]

Each finding has a class:

* ``approval``: passes only with ``--maintainer-approved`` (a maintainer's review approving the
  head SHA, computed by ``governance.py``). Tolerance widened; oracle ``kind`` downgraded;
  ``|Δvalue| > k·old_tolerance``; vector deleted; ``blocked_by`` added (it disarms a gate); test
  deleted (:func:`rust_scan`, :func:`python_test_names`); a gate not on the allowlist, or naming
  a Cargo feature the base does not declare, that is new for the item it stands on (Rust
  ``ignore``/``cfg``/``cfg_attr`` attributes, :func:`rust_scan`; Python skip/xfail/collection
  controls, :func:`python_gates`; gates are keyed by file, item and text, so moving an existing
  gate onto another test is new); pytest configuration under ``tools/`` changed
  (:func:`collect_pytest_configs`); executed nextest count fell; requirement ``status`` or
  ``verified_by_gate`` changed, or a requirement removed or added past ``draft``;
  ``statement``/``acceptance`` (where values and tolerances live) changed in PHY.md or NUM.md
  (ORG-011).
* ``golden``: a vector ``value`` or ``tolerance`` changed. Passes only with ``--golden-change``
  (the label, applied by a maintainer) **and** when the vector's evidence also changed in the same
  PR (one of source, citation, retrieved, verified_by, generated_by) (TST-004). Values and
  tolerances are compared as numbers, so re-spelling ``"0.1"`` as ``0.1`` is no change.
* ``reject``: never passes. A computed-reference vector whose ``generated_by`` SHA is a commit of
  this PR (TST-004): the generator must land, reviewed, before the golden it produces.

What the ratchet guarantees about tests (TST-006). It detects exactly these forms of test
weakening; each ``detects_*`` id is pinned by the tests listed for it in ``DETECTED_FORMS`` in
tools/tests/test_ratchet.py, and nothing else is detected:

* ``detects_rs_test_deleted``: a Rust test removed or renamed. A test is a ``fn`` in a ``.rs``
  file under ``crates/`` whose run of outer attributes holds ``#[test]`` or ``#[<path>::test]``,
  outside comments and string literals; tests are counted by crate directory and function name
  (:func:`rust_scan`).
* ``detects_rs_gate_attr``: a new ``ignore``, ``cfg`` or ``cfg_attr`` attribute, outer or inner,
  in a ``.rs`` file under ``crates/``, outside comments and string literals, with whitespace
  normalised (``# [ignore]``, ``#[ ignore ]``, an attribute split over lines). New means its
  (file, inline ``mod`` path + item kind + item name, normalised text) was not in the merge base,
  so a gate moved onto another item is new. It passes only if it matches ``gate_allowlist`` in
  ``ci/ratchet.toml`` and names no Cargo feature that is undeclared in the merge base.
* ``detects_rs_executed_count``: the number of tests ``cargo nextest list --workspace`` (default
  profile, each tree with its own ``.config/nextest.toml``) reports as run, neither ignored nor
  filtered out, is lower for the head than for the merge base. The head listing is produced by
  PR code.
* ``detects_py_test_deleted``: a Python test removed or renamed. A test is, in a file under
  ``tools/`` named ``test_*.py`` or ``*_test.py``, a module-level ``def test*`` or a ``def
  test*`` in a (nested) ``class Test*``, as a direct statement of the parsed module and still
  bound at the end of its scope (so one put in a string, under ``if``/``try``/``with``,
  ``del``-ed or rebound is removed); tests are counted by file name, not directory, and
  ``Class::function`` (:func:`python_test_names`).
* ``detects_py_gate_name``: in any ``.py`` file under ``tools/``, a new call or reference
  ``<expr>.<name>`` where ``<name>`` is ``skip``, ``skipif``, ``xfail``, ``importorskip``,
  ``skipIf``, ``skipUnless``, ``skipTest``, ``expectedFailure`` or ``SkipTest`` (whatever
  ``<expr>`` is: ``@pytest.mark.skip``, ``@mark.xfail``, ``pytest.skip()``, ``raise
  unittest.SkipTest``), or a bare name bound by ``from pytest|_pytest|unittest[.sub] import
  <one of those> [as alias]`` (:func:`python_gates`).
* ``detects_py_exit``: ``<m>[.<attr>...].exit`` where ``<m>`` is a name bound by ``import
  pytest``/``import _pytest[.sub]`` (any alias) or by ``from pytest|_pytest[.sub] import ...``,
  and a bare name bound by ``from pytest|_pytest[.sub] import exit [as alias]``.
* ``detects_py_collection_target``: an assignment (plain, annotated or augmented) whose target
  contains the name or attribute ``pytestmark``, ``__test__``, ``collect_ignore`` or
  ``collect_ignore_glob`` (``f.__test__ = False`` too), or ``append``/``extend``/``insert``/
  ``__setitem__`` called on one of those bare names.
* ``detects_py_hook_def``: a ``def``/``async def`` named ``pytest_*``, at any scope.
* ``detects_py_unparsable``: a ``.py`` file under ``tools/`` that does not parse.
* ``detects_pytest_config``: pytest configuration added, changed or removed in a file at any
  depth under ``tools/``: all of ``pytest.toml``, ``.pytest.toml``, ``pytest.ini``,
  ``.pytest.ini``; the ``[pytest]`` section of ``tox.ini``; ``[tool:pytest]`` of ``setup.cfg``;
  ``[tool.pytest]`` (with ``ini_options``) of ``pyproject.toml``; such a file that does not
  parse is compared as text (:func:`collect_pytest_configs`).

Python gates are keyed by (file path, enclosing ``Class::function`` or ``<module>``, unparsed
text), so a gate moved onto another test is new. Directories named ``target``, ``.venv``,
``.git``, ``node_modules``, ``__pycache__`` or ``.mypy_cache`` are not scanned.

Any other way of skipping, deselecting, hiding or neutering a test is NOT detected; reviewers
must check for it. The non-exhaustive examples below are each pinned by ``test_<id>`` in
tools/tests/test_ratchet.py, asserting no finding (issue #55 collects them):

* ``known_gap_py_runtime_gate``: a skip or ``__test__`` built at run time (``getattr(pytest,
  "sk" + "ip")``, ``setattr(f, "__test__", False)``, ``globals()["__test__"] = False``).
* ``known_gap_py_existing_helper``: a new call to a helper that already existed and skips.
* ``known_gap_hollowed_test``: a Python or Rust test that returns early, whose body is emptied,
  or whose assertions are put under dead code (Rust ``if false { ... }``).
* ``known_gap_py_nondefault_name``: deleting a test pytest collects outside its default naming,
  e.g. a method of a ``unittest.TestCase`` subclass not named ``Test*``.
* ``known_gap_py_no_count``: there is no executed-test count for pytest, so nothing backstops a
  Python gap; ``test-count`` counts ``cargo nextest list`` only.
* ``known_gap_rs_allowlisted_gate``: an allowlisted gate hiding a test from the PR tier: a
  ``cfg(feature = ...)`` / ``cfg(not(feature = ...))`` on a feature the base declares (e.g.
  ``cfg(not(feature = "std"))`` while ``std`` is a default feature), or ``ignore = "data..."``
  / ``"network..."`` on a test that needs neither (the reason is not checked); only
  ``test-count`` can see it, and its head listing is produced by PR code.
* ``known_gap_rs_uncompiled_file``: a Rust test file moved to a path cargo does not compile, or a
  dropped ``mod`` line; the source scan still sees the names; only ``test-count`` can see it.
* ``known_gap_rs_same_name_items``: a gate moved between two same-named fns of one module (two
  ``impl`` blocks), which are one item key.
* ``known_gap_scan_scope``: Rust outside ``crates/`` and Python outside ``tools/``.
* ``known_gap_py_outcome_exception``: raising pytest's outcome exceptions directly (``from
  _pytest.outcomes import Skipped`` / ``XFailed`` / ``Exit`` / ``OutcomeException``, then ``raise
  Skipped("x")``).
* ``known_gap_py_empty_parametrize``: ``@pytest.mark.parametrize("x", [])``, which skips the test.
* ``known_gap_py_process_exit``: ``os._exit(0)`` or ``sys.exit(0)`` in a test file (e.g. a new
  ``tools/tests/test_0.py``); pytest stops with exit 0 and no summary.
* ``known_gap_py_uncollectable_class``: ``__init__`` or ``__new__`` added to a ``Test*`` class,
  which pytest then does not collect (a warning only).
* ``known_gap_py_fixture_on_test``: ``@pytest.fixture`` on a ``test*`` function; no longer
  collected, still counted as a test.
* ``known_gap_py_assigned_hook``: a pytest hook bound by assignment
  (``pytest_collection_modifyitems = lambda ...``), e.g. in a subdirectory ``conftest.py``.
* ``known_gap_rs_verdict_attribute``: ``#[should_panic]`` on a failing Rust test, which turns the
  failure into a pass; the test still runs, so ``test-count`` does not see it either.
* ``known_gap_py_aliased_exit``: ``exit`` through a module object rebound (``p = pytest;
  p.exit()``) or imported at run time (``importlib.import_module("pytest").exit()``).
* ``known_gap_py_moved_out_of_testpaths``: a test file moved out of pytest's ``testpaths`` (e.g.
  ``tools/tests/test_a.py`` to ``tools/attic/test_a.py``); tests are keyed by file name.
* ``known_gap_py_replacing_decorator``: a decorator that replaces the test with a no-op.
* ``known_gap_py_imported_marker``: ``pytestmark`` bound by ``from m import pytestmark``.
* ``known_gap_rs_macro_gate``: a gate passed to a macro as tokens (``#[$a]`` in a
  ``macro_rules!``, invoked as ``t!(ignore)``).

``tools/pyproject.toml``, ``tools/tests/conftest.py`` and the pytest configuration files above,
at any depth under ``tools/``, are also on the ORG-011 meta surface (CODEOWNERS).
"""

from __future__ import annotations

import argparse
import ast
import configparser
import json
import re
import subprocess
import sys
import tempfile
import tomllib
from collections import Counter
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from pathlib import Path
from typing import Any

import yaml

EVIDENCE_KEYS = ("source", "citation", "retrieved", "verified_by", "generated_by")
SKIP_DIRS = {"target", ".venv", ".git", "node_modules", "__pycache__", ".mypy_cache"}
YAML_BLOCK_RE = re.compile(r"^```yaml\n(.*?)^```", re.DOTALL | re.MULTILINE)
# Rust lexing (see `rust_mask`): a char literal, and the opening of a raw (byte/C) string.
RUST_CHAR_RE = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F_]{1,8}\}|.)|[^'\\\n])'")
RUST_RAW_STR_RE = re.compile(r'[bc]?r(#*)"')
# An outer or inner attribute anywhere in code (comments and literals already masked out);
# `#`, `!` and `[` may be separated by whitespace (`# [ignore]` is valid Rust).
RUST_ATTR_RE = re.compile(r"#\s*(!)?\s*\[")
RUST_TEST_ATTR_RE = re.compile(r"^(?:[\w:]+::)?test$")
RUST_GATE_HEAD_RE = re.compile(r"^(?:ignore|cfg|cfg_attr)\b")
# The item an outer attribute run stands on (in masked code), and a struct field.
RUST_ITEM_RE = re.compile(
    r"(?:pub(?:\s*\([^)]*\))?\s+)?"
    r'(?:(?:default|const|async|unsafe|safe|extern(?:\s*"[^"]*")?)\s+)*'
    r"(?P<kind>fn|mod|struct|enum|union|trait|type|const|static|impl|use|macro_rules!|let)(?!\w)"
    r"\s*(?P<name>\w+)?"
)
RUST_FIELD_RE = re.compile(r"(?:pub(?:\s*\([^)]*\))?\s+)?(?P<name>\w+)\s*:(?!:)")
RUST_MOD_OPEN_RE = re.compile(r"\bmod\s+(\w+)\s*\{")
RUST_WORD_RE = re.compile(r"\w+")
FEATURE_REF_RE = re.compile(r'\bfeature = "([^"]*)"')
# Python names that skip, xfail or conditionally drop a test (pytest and unittest).
PY_GATE_NAMES = frozenset(
    {
        "skip",
        "skipif",
        "xfail",
        "importorskip",
        "skipIf",
        "skipUnless",
        "skipTest",
        "expectedFailure",
        "SkipTest",
    }
)
PY_GATE_MODULES = frozenset({"pytest", "_pytest", "unittest"})
# Modules whose `exit` stops the whole pytest session.
PY_EXIT_MODULES = frozenset({"pytest", "_pytest"})
# Module-level or class-level names that make pytest skip or not collect tests.
PY_GATE_TARGETS = frozenset({"pytestmark", "__test__", "collect_ignore", "collect_ignore_glob"})
# Files under tools/ that can hold pytest configuration (testpaths, addopts such as `-k`, ...).
PY_CONFIG_FILES = frozenset(
    {
        "pytest.toml",
        ".pytest.toml",
        "pytest.ini",
        ".pytest.ini",
        "tox.ini",
        "setup.cfg",
        "pyproject.toml",
    }
)


@dataclass(frozen=True)
class Finding:
    """One ratchet finding."""

    cls: str  # approval | golden | reject
    where: str
    what: str

    def __str__(self) -> str:
        """``[class] where: what``."""
        return f"[{self.cls}] {self.where}: {self.what}"


@dataclass(frozen=True)
class Config:
    """``ci/ratchet.toml`` (read from the base tree)."""

    k: Decimal
    kind_rank: Mapping[str, int]
    gate_allow: tuple[re.Pattern[str], ...]

    @classmethod
    def load(cls, path: Path) -> Config:
        """Parse the configuration file."""
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        return cls(
            k=Decimal(str(data["value_change_k"])),
            kind_rank={str(k): int(v) for k, v in data["kind_rank"].items()},
            gate_allow=tuple(re.compile(p) for p in data["gate_allowlist"]),
        )


# ---------------------------------------------------------------------------------------------
# Vectors (TST-002/003/004/006)


def _dec(x: Any) -> Decimal:  # noqa: ANN401 (TOML scalar)
    try:
        return Decimal(str(x).strip())
    except InvalidOperation as exc:
        raise ValueError(f"not a number: {x!r}") from exc


def effective_tolerance(vec: Mapping[str, Any]) -> Decimal:
    """``max(abs, rel·|value|)``, the pass band of ``ferroastro-vectors``."""
    tol = vec.get("tolerance") or {}
    abs_tol = _dec(tol.get("abs", 0))
    rel_tol = _dec(tol.get("rel", 0))
    return max(abs_tol, rel_tol * abs(_dec(vec["value"])))


def _tol_numbers(tol: Mapping[str, Any]) -> dict[str, Decimal | str]:
    """Tolerance table with numeric entries as Decimals (non-numeric kept as text)."""
    out: dict[str, Decimal | str] = {}
    for key, val in tol.items():
        try:
            out[str(key)] = _dec(val)
        except ValueError:
            out[str(key)] = str(val)
    return out


def load_vectors(root: Path) -> dict[str, tuple[str, dict[str, Any]]]:
    """Map vector id to (file, vector) for every ``tests/vectors/*.toml`` under ``root``."""
    out: dict[str, tuple[str, dict[str, Any]]] = {}
    for path in sorted((root / "tests" / "vectors").glob("*.toml")):
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        for vec in data.get("vector", []):
            out[str(vec["id"])] = (path.name, vec)
    return out


def vector_findings(
    base: Mapping[str, tuple[str, Mapping[str, Any]]],
    head: Mapping[str, tuple[str, Mapping[str, Any]]],
    cfg: Config,
    pr_commits: Iterable[str],
) -> list[Finding]:
    """Compare two vector sets."""
    commits = [c.lower() for c in pr_commits]
    findings: list[Finding] = []
    for vid in sorted(set(base) - set(head)):
        findings.append(Finding("approval", f"{base[vid][0]}:{vid}", "vector deleted"))
    for vid in sorted(head):
        file, new = head[vid]
        where = f"{file}:{vid}"
        gen = str(new.get("generated_by", ""))
        if "@" in gen and new.get("kind") == "computed-reference":
            sha = gen.rsplit("@", 1)[1].lower()
            old_gen = str(base[vid][1].get("generated_by", "")) if vid in base else ""
            if gen != old_gen and any(c.startswith(sha) for c in commits):
                findings.append(
                    Finding("reject", where, f"generated_by {gen} names a commit of this PR")
                )
        if vid not in base:
            continue
        old = base[vid][1]
        old_rank = cfg.kind_rank.get(str(old.get("kind")), 0)
        new_rank = cfg.kind_rank.get(str(new.get("kind")), 0)
        if new_rank < old_rank:
            findings.append(
                Finding(
                    "approval", where, f"kind downgraded {old.get('kind')} -> {new.get('kind')}"
                )
            )
        if "blocked_by" in new and "blocked_by" not in old:
            findings.append(Finding("approval", where, f"blocked_by {new['blocked_by']} added"))
        old_tol, new_tol = old.get("tolerance") or {}, new.get("tolerance") or {}
        widened = [
            key for key in ("abs", "rel") if _dec(new_tol.get(key, 0)) > _dec(old_tol.get(key, 0))
        ]
        if widened or effective_tolerance(new) > effective_tolerance(old):
            findings.append(
                Finding("approval", where, f"tolerance widened {dict(old_tol)} -> {dict(new_tol)}")
            )
        delta = abs(_dec(new["value"]) - _dec(old["value"]))
        if delta > cfg.k * effective_tolerance(old):
            findings.append(
                Finding(
                    "approval",
                    where,
                    f"|Δvalue| = {delta} exceeds {cfg.k}·old tolerance {effective_tolerance(old)}",
                )
            )
        # Numeric comparison: `abs = "0.1"` rewritten as `abs = 0.1` is not a change.
        if _dec(new["value"]) != _dec(old["value"]) or _tol_numbers(new_tol) != _tol_numbers(
            old_tol
        ):
            evidence = any(new.get(key) != old.get(key) for key in EVIDENCE_KEYS)
            findings.append(
                Finding(
                    "golden",
                    where,
                    "value or tolerance changed"
                    + ("" if evidence else "; NO new evidence (source/citation/retrieved/…)"),
                )
            )
    return findings


# ---------------------------------------------------------------------------------------------
# Tests and gates (TST-006)
#
# Every gate is keyed by (file, the item it stands on, normalised gate text): the Rust item an
# attribute decorates (`mod::path::fn name`), or the Python function or class a decorator or call
# belongs to. Moving an existing gate from one test to another is therefore a new gate.


def _sources(root: Path, suffix: str, top: str) -> Iterable[Path]:
    base = root / top
    if not base.is_dir():
        return []
    return sorted(
        p for p in base.rglob(f"*{suffix}") if not SKIP_DIRS.intersection(p.relative_to(root).parts)
    )


def _blank(buf: list[str], start: int, end: int) -> None:
    for k in range(start, end):
        if buf[k] != "\n":
            buf[k] = " "


def _rust_comment_end(text: str, i: int) -> int | None:
    """End of the comment starting at ``i`` (``None``: no comment there); block comments nest."""
    if text.startswith("//", i):
        j = text.find("\n", i)
        return len(text) if j < 0 else j
    if not text.startswith("/*", i):
        return None
    depth, j = 1, i + 2
    while j < len(text) and depth:
        if text.startswith("/*", j):
            depth, j = depth + 1, j + 2
        elif text.startswith("*/", j):
            depth, j = depth - 1, j + 2
        else:
            j += 1
    return j


def _rust_literal(text: str, i: int) -> tuple[int, int, int] | None:
    """``(start, end, resume)``: what to blank of a literal at ``i``, and where it ends."""
    c = text[i]
    if c == '"':
        j = i + 1
        while j < len(text) and text[j] != '"':
            j += 2 if text[j] == "\\" else 1
        return i + 1, min(j, len(text)), j + 1
    if c == "'":
        m = RUST_CHAR_RE.match(text, i)
        return None if m is None else (i + 1, m.end() - 1, m.end())  # None: a lifetime
    if c in "bcr" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
        raw = RUST_RAW_STR_RE.match(text, i)
        if raw:
            j = text.find('"' + raw.group(1), raw.end())
            end = len(text) if j < 0 else j + 1 + len(raw.group(1))
            return i, end, end
    return None


def rust_mask(text: str) -> tuple[str, str]:
    """Return ``(code, masked)`` for Rust source ``text``, both the same length as ``text``.

    ``code`` has every comment (line, block, nested block, doc) blanked; ``masked`` additionally
    blanks the contents of string, raw string and char literals. Positions are found in
    ``masked`` (so ``#[ignore]`` in a comment or a string is not an attribute, and a test inside
    ``/* ... */`` does not exist) and attribute text is read from ``code``.
    """
    code, masked = list(text), list(text)
    i = 0
    while i < len(text):
        end = _rust_comment_end(text, i)
        if end is not None:
            _blank(code, i, end)
            _blank(masked, i, end)
            i = end
            continue
        lit = _rust_literal(text, i)
        if lit is None:
            i += 1
            continue
        _blank(masked, lit[0], lit[1])
        i = lit[2]
    return "".join(code), "".join(masked)


def _normalise_attr(body: str) -> str:
    """Return the canonical spelling of an attribute body.

    Whitespace outside strings is dropped, then ``a = b`` and ``a, b`` are spaced as rustfmt
    writes them (``ignore="x"`` -> ``ignore = "x"``).
    """
    out: list[str] = []
    i, in_str = 0, False
    while i < len(body):
        c = body[i]
        if in_str:
            out.append(c)
            if c == "\\" and i + 1 < len(body):
                out.append(body[i + 1])
                i += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            out.append(c)
        elif c == "=":
            out.append(" = ")
        elif c == ",":
            out.append(", ")
        elif not c.isspace():
            out.append(c)
        i += 1
    return "".join(out)


@dataclass(frozen=True)
class _Attr:
    start: int
    end: int
    inner: bool
    body: str


def _rust_attrs(code: str, masked: str) -> list[_Attr]:
    attrs: list[_Attr] = []
    last = 0
    for m in RUST_ATTR_RE.finditer(masked):
        if m.start() < last:
            continue  # nested inside the previous attribute
        depth, j = 1, m.end()
        while j < len(masked) and depth:
            depth += {"[": 1, "]": -1}.get(masked[j], 0)
            j += 1
        if depth:
            continue
        attrs.append(_Attr(m.start(), j, bool(m.group(1)), _normalise_attr(code[m.end() : j - 1])))
        last = j
    return attrs


def _mod_paths(masked: str, positions: list[int]) -> dict[int, str]:
    """Map each position to the ``a::b`` path of the inline ``mod`` blocks enclosing it."""
    opens = {m.end() - 1: m.group(1) for m in RUST_MOD_OPEN_RE.finditer(masked)}
    wanted = sorted(set(positions))
    out: dict[int, str] = {}
    stack: list[str | None] = []
    k = 0
    for i, c in enumerate(masked):
        while k < len(wanted) and wanted[k] == i:
            out[i] = "::".join(s for s in stack if s)
            k += 1
        if c == "{":
            stack.append(opens.get(i))
        elif c == "}" and stack:
            stack.pop()
    for pos in wanted[k:]:
        out[pos] = "::".join(s for s in stack if s)
    return out


def _rust_item(masked: str, i: int) -> tuple[str, str]:
    """Return ``(kind, name)`` of the item at or after ``i`` (``?``, first word: not an item)."""
    while i < len(masked) and masked[i].isspace():
        i += 1
    item = RUST_ITEM_RE.match(masked, i)
    if item:
        return item.group("kind"), item.group("name") or ""
    field = RUST_FIELD_RE.match(masked, i)
    if field:
        return "field", field.group("name")
    word = RUST_WORD_RE.match(masked, i)  # enum variant, match arm, expression
    return "?", word.group() if word else ""


def rust_scan(text: str) -> tuple[list[str], list[tuple[str, str]]]:
    """Return the ``#[test]`` function names and the ``(item, gate)`` pairs of Rust ``text``.

    A run of outer attributes (only whitespace between them, on one line or many) belongs to the
    item that follows: a test is a ``fn`` whose run holds ``#[test]`` or ``#[<path>::test]``, and
    each ``ignore``/``cfg``/``cfg_attr`` attribute in the run is a gate keyed by that item
    (``m::fn name``, ``m::struct S``, ``field x``; ``? word`` for an enum variant, a match arm or an
    expression, keyed by its first word). An inner
    attribute (``#![...]``) is keyed by its module path and ``!``. Comments and string literals are
    masked first, so attributes there do not count and a commented-out test is gone.
    """
    code, masked = rust_mask(text)
    attrs = _rust_attrs(code, masked)
    paths = _mod_paths(masked, [a.start for a in attrs])
    tests: list[str] = []
    gates: list[tuple[str, str]] = []
    run: list[_Attr] = []
    for idx, attr in enumerate(attrs):
        if attr.inner:
            if RUST_GATE_HEAD_RE.match(attr.body):
                gates.append((f"{paths[attr.start]}!", attr.body))
            continue
        run.append(attr)
        nxt = attrs[idx + 1] if idx + 1 < len(attrs) else None
        if nxt and not nxt.inner and not masked[attr.end : nxt.start].strip():
            continue
        kind, name = _rust_item(masked, attr.end)
        path = paths[run[0].start]
        item = f"{path}::" if path else ""
        item += f"{kind} {name}".strip()
        if kind == "fn" and any(RUST_TEST_ATTR_RE.match(a.body) for a in run):
            tests.append(name)
        gates += [(item, a.body) for a in run if RUST_GATE_HEAD_RE.match(a.body)]
        run = []
    return tests, gates


def rust_test_fns(text: str) -> list[str]:
    """Names of the functions marked ``#[test]`` in Rust source ``text`` (see :func:`rust_scan`)."""
    return rust_scan(text)[0]


def rust_gates(text: str) -> list[tuple[str, str]]:
    """``(item, normalised gate)`` for each ``ignore``/``cfg``/``cfg_attr`` in Rust ``text``."""
    return rust_scan(text)[1]


def _py_is_test_file(path: Path) -> bool:
    return path.name.startswith("test_") or path.name.endswith("_test.py")


def _py_bindings(body: list[ast.stmt], prefix: str) -> dict[str, list[str]]:
    """Map each name bound in ``body`` (one scope, in order) to the test ids it finally holds.

    Only direct statements count: a test under ``if``/``try``/``with`` or inside a string is not
    collected here, and a later ``del``, assignment, import or other definition of the same name
    removes it.
    """
    live: dict[str, list[str]] = {}
    for stmt in body:
        if isinstance(stmt, (ast.FunctionDef, ast.AsyncFunctionDef)):
            live.pop(stmt.name, None)
            if stmt.name.startswith("test"):
                live[stmt.name] = [prefix + stmt.name]
        elif isinstance(stmt, ast.ClassDef):
            live.pop(stmt.name, None)
            if stmt.name.startswith("Test"):
                inner = _py_bindings(stmt.body, f"{prefix}{stmt.name}::")
                live[stmt.name] = [t for ids in inner.values() for t in ids]
        else:
            for node in ast.walk(stmt):
                if isinstance(node, ast.Name) and isinstance(node.ctx, (ast.Store, ast.Del)):
                    live.pop(node.id, None)
                elif isinstance(node, ast.alias):
                    live.pop((node.asname or node.name).split(".")[0], None)
                elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                    live.pop(node.name, None)
    return live


def python_test_names(text: str) -> list[str]:
    """Return the tests pytest would collect from Python ``text`` (default naming rules).

    Module-level functions named ``test*`` and ``test*`` methods of module-level classes named
    ``Test*`` (nested ``Test*`` classes too), taken from the parsed module, not from the text. A
    test that is wrapped in a string, placed under ``if``/``try``/``with``, or ``del``-ed or
    rebound later is not a test; a file that does not parse has none.
    """
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return []
    return [t for ids in _py_bindings(tree.body, "").values() for t in ids]


class _PyGateFinder(ast.NodeVisitor):
    """Collect ``(scope, gate)`` pairs; decorators belong to the function or class they decorate."""

    def __init__(self, aliases: set[str], pytest_mods: set[str]) -> None:
        self.aliases = aliases
        self.pytest_mods = pytest_mods
        self.scope: list[str] = []
        self.gates: list[tuple[str, str]] = []

    def add(self, text: str) -> None:
        self.gates.append(("::".join(self.scope) or "<module>", text))

    def is_gate(self, node: ast.expr) -> bool:
        if isinstance(node, ast.Attribute):
            if node.attr in PY_GATE_NAMES:
                return True
            root = node.value
            while isinstance(root, ast.Attribute):
                root = root.value
            return (
                node.attr == "exit" and isinstance(root, ast.Name) and root.id in self.pytest_mods
            )
        return isinstance(node, ast.Name) and node.id in self.aliases

    def _scoped(self, node: ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef) -> None:
        self.scope.append(node.name)
        if not isinstance(node, ast.ClassDef) and node.name.startswith("pytest_"):
            self.add(f"def {node.name}")
        self.generic_visit(node)
        self.scope.pop()

    visit_FunctionDef = _scoped  # noqa: N815 (ast.NodeVisitor API)
    visit_AsyncFunctionDef = _scoped  # noqa: N815
    visit_ClassDef = _scoped  # noqa: N815

    def visit_Call(self, node: ast.Call) -> None:
        func = node.func
        if self.is_gate(func) or (
            isinstance(func, ast.Attribute)
            and func.attr in ("append", "extend", "insert", "__setitem__")
            and isinstance(func.value, ast.Name)
            and func.value.id in PY_GATE_TARGETS
        ):
            self.add(ast.unparse(node))
        else:
            self.visit(func)
        for arg in [*node.args, *(k.value for k in node.keywords)]:
            self.visit(arg)

    def _assign(self, node: ast.Assign | ast.AnnAssign | ast.AugAssign) -> None:
        targets = node.targets if isinstance(node, ast.Assign) else [node.target]
        names = {
            n.id if isinstance(n, ast.Name) else n.attr
            for t in targets
            for n in ast.walk(t)
            if isinstance(n, (ast.Name, ast.Attribute))
        }
        if names & PY_GATE_TARGETS:
            self.add(ast.unparse(node))
        else:
            self.generic_visit(node)

    visit_Assign = _assign  # noqa: N815
    visit_AnnAssign = _assign  # noqa: N815
    visit_AugAssign = _assign  # noqa: N815

    def _ref(self, node: ast.Attribute | ast.Name) -> None:
        if isinstance(node.ctx, ast.Load) and self.is_gate(node):
            self.add(ast.unparse(node))
        else:
            self.generic_visit(node)

    visit_Attribute = _ref  # noqa: N815
    visit_Name = _ref  # noqa: N815


def python_gates(text: str) -> list[tuple[str, str]]:
    """``(scope, gate)`` for each skip/xfail/collection gate in Python ``text``, found by parsing.

    ``scope`` is the ``Class::function`` the gate is in or decorates (``<module>`` at top level).
    Detected: any call or reference ``<anything>.skip``/``skipif``/``xfail``/``importorskip``
    (and unittest's ``skipIf``/``skipUnless``/``skipTest``/``expectedFailure``/``SkipTest``),
    so ``@pytest.mark.skip``, ``@mark.xfail``, ``pytest.skip(...)`` in a test body,
    ``pytest.importorskip(...)`` and ``raise unittest.SkipTest`` all count, whatever the module
    alias; the same names imported from pytest/_pytest/unittest under any alias; ``exit`` on a
    name bound by importing pytest or _pytest (``import pytest as p``: ``p.exit``; ``import
    _pytest.outcomes as o``: ``o.exit``; ``from pytest import exit``); any assignment to
    ``pytestmark``, ``__test__``, ``collect_ignore`` or ``collect_ignore_glob`` (also as an
    attribute, ``f.__test__ = False``) and ``append``/``extend``/``insert``/``__setitem__`` on
    one of those names; any
    ``def pytest_*`` hook. A file that does not parse yields one gate. Any ``<expr>.skip`` counts,
    also in code that is not a test (``args.skip``): it over-detects, which only costs an approval.
    Anything else is not detected; see the module docstring for the closed list and examples.
    """
    try:
        tree = ast.parse(text)
    except SyntaxError:
        return [("<module>", "<python file does not parse>")]
    aliases: set[str] = set()
    pytest_mods: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.ImportFrom) and (node.module or "").split(".")[0] in (
            PY_GATE_MODULES
        ):
            from_pytest = node.module.split(".")[0] in PY_EXIT_MODULES  # type: ignore[union-attr]
            gate_names = PY_GATE_NAMES | ({"exit"} if from_pytest else set())
            aliases |= {a.asname or a.name for a in node.names if a.name in gate_names}
            if from_pytest:  # `from _pytest import outcomes as o`: `o.exit` is pytest's exit
                pytest_mods |= {a.asname or a.name for a in node.names}
        elif isinstance(node, ast.Import):
            pytest_mods |= {
                a.asname or a.name.split(".")[0]
                for a in node.names
                if a.name.split(".")[0] in PY_EXIT_MODULES
            }
    finder = _PyGateFinder(aliases, pytest_mods)
    finder.visit(tree)
    return finder.gates


def _pytest_config(path: Path) -> str | None:
    """Return the pytest part of a configuration file under tools/ (``None``: it has none)."""
    text = path.read_text(encoding="utf-8")
    if path.name == "pyproject.toml":
        try:
            pytest_cfg = tomllib.loads(text).get("tool", {}).get("pytest")
        except tomllib.TOMLDecodeError:
            return text
        return None if pytest_cfg is None else json.dumps(pytest_cfg, sort_keys=True)
    if path.name in ("tox.ini", "setup.cfg"):
        parser = configparser.ConfigParser(interpolation=None)
        try:
            parser.read_string(text)
        except configparser.Error:
            return text
        section = "pytest" if path.name == "tox.ini" else "tool:pytest"
        return json.dumps(dict(parser[section])) if parser.has_section(section) else None
    return text  # pytest.ini / .pytest.ini are pytest's whatever they contain


def collect_pytest_configs(root: Path) -> dict[str, str]:
    """Map each file under ``root/tools`` holding pytest configuration to that configuration."""
    out: dict[str, str] = {}
    base = root / "tools"
    if not base.is_dir():
        return out
    for path in sorted(base.rglob("*")):
        rel = path.relative_to(root)
        if path.name in PY_CONFIG_FILES and path.is_file() and not SKIP_DIRS & set(rel.parts):
            cfg = _pytest_config(path)
            if cfg is not None:
                out[rel.as_posix()] = cfg
    return out


def test_names(root: Path) -> Counter[str]:
    """Multiset of ``crate::name`` (Rust) and ``tools/<file>::name`` (Python) tests under ``root``.

    See :func:`rust_scan` and :func:`python_test_names` for what is a test.
    """
    names: Counter[str] = Counter()
    for path in _sources(root, ".rs", "crates"):
        crate = path.relative_to(root).parts[1]
        for name in rust_test_fns(path.read_text(encoding="utf-8")):
            names[f"{crate}::{name}"] += 1
    for path in _sources(root, ".py", "tools"):
        if _py_is_test_file(path):
            for name in python_test_names(path.read_text(encoding="utf-8")):
                names[f"tools/{path.name}::{name}"] += 1
    return names


def declared_features(root: Path) -> set[str]:
    """Cargo features declared under ``root/crates`` (``[features]`` keys and optional deps).

    The union over all crates: a ``cfg(feature = "std")`` in a crate that does not declare
    ``std`` is not caught here, but by cargo's check-cfg (``unexpected_cfgs``) under clippy
    ``-D warnings``.
    """
    features: set[str] = set()
    for path in sorted((root / "crates").glob("*/Cargo.toml")):
        data = tomllib.loads(path.read_text(encoding="utf-8"))
        features |= set(data.get("features", {}))
        for table in ("dependencies", "dev-dependencies"):
            for name, spec in data.get(table, {}).items():
                if isinstance(spec, dict) and spec.get("optional"):
                    features.add(name)
    return features


def gate_attributes(root: Path) -> Counter[tuple[str, str, str]]:
    """Multiset of (file, item or scope, gate) for Rust attribute gates and Python gates.

    See :func:`rust_scan` and :func:`python_gates` for exactly what counts.
    """
    gates: Counter[tuple[str, str, str]] = Counter()
    for path in _sources(root, ".rs", "crates"):
        rel = path.relative_to(root).as_posix()
        for item, gate in rust_gates(path.read_text(encoding="utf-8")):
            gates[(rel, item, gate)] += 1
    for path in _sources(root, ".py", "tools"):
        rel = path.relative_to(root).as_posix()
        for scope, gate in python_gates(path.read_text(encoding="utf-8")):
            gates[(rel, scope, gate)] += 1
    return gates


def source_findings(base_root: Path, head_root: Path, cfg: Config) -> list[Finding]:
    """Return deleted tests, new gates that are not allowlisted and pytest configuration changes.

    A gate is new when its (file, item, text) is new, so moving a gate onto another test is new.
    An allowlisted gate that names a Cargo feature the base tree does not declare is not
    allowlisted (``#[cfg(feature = "nope")]`` would hide a test from every CI run).
    """
    findings: list[Finding] = []
    lost = test_names(base_root) - test_names(head_root)
    findings += [Finding("approval", name, "test deleted or renamed") for name in sorted(lost)]
    added = gate_attributes(head_root) - gate_attributes(base_root)
    features = declared_features(base_root)
    for (file, item, attr), _ in sorted(added.items()):
        where = f"{file} ({item})"
        unknown = sorted(set(FEATURE_REF_RE.findall(attr)) - features)
        if unknown:
            findings.append(
                Finding("approval", where, f"new gate names undeclared feature {unknown}: {attr}")
            )
        elif not any(p.search(attr) for p in cfg.gate_allow):
            findings.append(Finding("approval", where, f"new gate not on the allowlist: {attr}"))
    old_cfg, new_cfg = collect_pytest_configs(base_root), collect_pytest_configs(head_root)
    for file in sorted(set(old_cfg) | set(new_cfg)):
        if old_cfg.get(file) != new_cfg.get(file):
            findings.append(Finding("approval", file, "pytest configuration added/changed/removed"))
    return findings


# ---------------------------------------------------------------------------------------------
# Requirement meta fields (ORG-011)


def load_requirements(root: Path) -> dict[str, tuple[str, dict[str, Any]]]:
    """Map requirement id to (file, block) for docs/requirements/*.md under ``root``."""
    out: dict[str, tuple[str, dict[str, Any]]] = {}
    for path in sorted((root / "docs" / "requirements").glob("*.md")):
        for m in YAML_BLOCK_RE.finditer(path.read_text(encoding="utf-8")):
            try:
                data = yaml.safe_load(m.group(1))
            except yaml.YAMLError:
                continue
            if isinstance(data, dict) and "id" in data:
                out[str(data["id"])] = (path.name, data)
    return out


def requirement_findings(
    base: Mapping[str, tuple[str, Mapping[str, Any]]],
    head: Mapping[str, tuple[str, Mapping[str, Any]]],
) -> list[Finding]:
    """Status, gate and PHY/NUM value changes that need a maintainer (ORG-011)."""
    findings: list[Finding] = []
    for rid in sorted(set(base) - set(head)):
        findings.append(Finding("approval", f"{base[rid][0]}:{rid}", "requirement removed"))
    for rid in sorted(head):
        file, new = head[rid]
        where = f"{file}:{rid}"
        if rid not in base:
            if new.get("status") != "draft":
                findings.append(Finding("approval", where, f"added as {new.get('status')}"))
            if new.get("verified_by_gate"):
                findings.append(Finding("approval", where, "added with verified_by_gate"))
            continue
        old = base[rid][1]
        for key in ("status", "verified_by_gate"):
            if new.get(key) != old.get(key):
                findings.append(
                    Finding("approval", where, f"{key} {old.get(key)!r} -> {new.get(key)!r}")
                )
        if file in ("PHY.md", "NUM.md"):
            for key in ("statement", "acceptance"):
                if new.get(key) != old.get(key):
                    findings.append(Finding("approval", where, f"{key} changed (value/tolerance)"))
    return findings


# ---------------------------------------------------------------------------------------------
# Executed test count (nextest JSON)


def executed_count(listing: Mapping[str, Any]) -> int:
    """Count the tests ``cargo nextest list`` would run (not ignored, filter matched)."""
    total = 0
    for suite in (listing.get("rust-suites") or {}).values():
        for case in (suite.get("testcases") or {}).values():
            status = (case.get("filter-match") or {}).get("status", "matches")
            if not case.get("ignored", False) and status == "matches":
                total += 1
    return total


# ---------------------------------------------------------------------------------------------


def merge_base(repo: Path, base_sha: str, head_sha: str) -> str:
    """Return ``git merge-base base_sha head_sha`` in ``repo`` (the tree the PR branched from)."""
    out = subprocess.run(  # noqa: S603
        ["git", "merge-base", base_sha, head_sha],  # noqa: S607
        cwd=repo,
        check=True,
        capture_output=True,
        text=True,
    )
    return out.stdout.strip()


def export_tree(repo: Path, sha: str, dest: Path) -> Path:
    """Write the tree of commit ``sha`` of ``repo`` into ``dest`` (``git archive``; no checkout)."""
    dest.mkdir(parents=True, exist_ok=True)
    archive = subprocess.run(  # noqa: S603
        ["git", "archive", "--format=tar", sha],  # noqa: S607
        cwd=repo,
        check=True,
        capture_output=True,
    )
    subprocess.run(  # noqa: S603
        ["tar", "-x", "-C", str(dest)],  # noqa: S607
        input=archive.stdout,
        check=True,
        capture_output=True,
    )
    return dest


def pr_commits(head: Path, base_sha: str, head_sha: str) -> list[str]:
    """Return the commits in ``base_sha..head_sha`` of the head checkout."""
    out = subprocess.run(  # noqa: S603
        ["git", "rev-list", f"{base_sha}..{head_sha}"],  # noqa: S607
        cwd=head,
        check=True,
        capture_output=True,
        text=True,
    )
    return out.stdout.split()


def verdict(findings: list[Finding], *, approved: bool, golden_change: bool) -> int:
    """Print the findings and return the exit code."""
    failed = False
    for f in findings:
        if f.cls == "reject":
            ok = False
        elif f.cls == "golden":
            ok = golden_change and "NO new evidence" not in f.what
        else:
            ok = approved
        failed |= not ok
        tag = "notice" if ok else "error"
        print(f"::{tag}::ratchet {f}{' (allowed)' if ok else ''}")
    if not findings:
        print("ratchet: no finding")
    elif failed:
        print(
            "ratchet: FAILED. `approval` findings need a maintainer's review approving the head "
            "SHA; `golden` findings need the maintainer-applied golden-change label and new "
            "evidence; `reject` findings cannot be overridden (TST-004, TST-006, ADR-0015)."
        )
    return 1 if failed else 0


def main(argv: list[str] | None = None) -> int:
    """Entry point; returns the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    st = sub.add_parser("static")
    st.add_argument("--base", type=Path, required=True)
    st.add_argument("--head", type=Path, required=True)
    st.add_argument("--base-sha", required=True)
    st.add_argument("--head-sha", required=True)
    st.add_argument("--config", type=Path, help="default: <base>/ci/ratchet.toml")
    st.add_argument(
        "--against-root",
        type=Path,
        help="tree to compare the head with (default: merge-base(base, head) exported from --head)",
    )
    mb = sub.add_parser("merge-base")
    mb.add_argument("--repo", type=Path, required=True)
    mb.add_argument("--base-sha", required=True)
    mb.add_argument("--head-sha", required=True)
    tc = sub.add_parser("test-count")
    tc.add_argument("--base-list", type=Path, required=True)
    tc.add_argument("--head-list", type=Path, required=True)
    for p in (st, tc):
        p.add_argument("--maintainer-approved", action="store_true")
        p.add_argument("--golden-change", action="store_true")
    args = parser.parse_args(argv)

    if args.command == "test-count":
        before = executed_count(json.loads(args.base_list.read_text(encoding="utf-8")))
        after = executed_count(json.loads(args.head_list.read_text(encoding="utf-8")))
        print(f"ratchet: executed tests base={before} head={after}")
        findings = (
            [Finding("approval", "nextest", f"executed tests fell {before} -> {after}")]
            if after < before
            else []
        )
        return verdict(findings, approved=args.maintainer_approved, golden_change=False)

    if args.command == "merge-base":
        print(merge_base(args.repo, args.base_sha, args.head_sha))
        return 0

    cfg = Config.load(args.config or args.base / "ci" / "ratchet.toml")
    commits = pr_commits(args.head, args.base_sha, args.head_sha)
    with tempfile.TemporaryDirectory() as tmp:
        against = args.against_root
        if against is None:
            mb_sha = merge_base(args.head, args.base_sha, args.head_sha)
            print(f"ratchet: comparing head {args.head_sha} with merge-base {mb_sha}")
            against = export_tree(args.head, mb_sha, Path(tmp) / "merge-base")
        return static_verdict(against, args.head, cfg, commits, args)


def static_verdict(
    against: Path, head: Path, cfg: Config, commits: list[str], args: argparse.Namespace
) -> int:
    """Run every static comparison of ``head`` against ``against`` and return the exit code."""
    findings = vector_findings(load_vectors(against), load_vectors(head), cfg, commits)
    findings += source_findings(against, head, cfg)
    findings += requirement_findings(load_requirements(against), load_requirements(head))
    return verdict(findings, approved=args.maintainer_approved, golden_change=args.golden_change)


if __name__ == "__main__":
    sys.exit(main())
