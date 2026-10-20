<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Python standards (tooling only)

Python is used only for tooling: oracle drivers, reference comparison, report generation and `trace.py`. It lives in
`tools/` as a **uv** project with a committed `uv.lock`.

| Gate | Command |
|---|---|
| Lint | `uv run ruff check --output-format=github` |
| Format | `uv run ruff format --check` |
| Types | `uv run mypy .` (strict) |
| Tests | `uv run pytest` |

The rule set lives in [`tools/pyproject.toml`](../../tools/pyproject.toml): `E W F I N UP B SIM C4 PTH NPY RUF D ANN PL PT S`,
with the numpy docstring convention.
- `PLR2004` (magic numbers) is ignored because physical values belong in `tests/vectors/`, not in code.
- `N803`/`N806` are ignored so physics names such as `GM` and `J2` are allowed.

Oracle drivers communicate with GPL tools **only through files and process boundaries** (LIC-001).
`subprocess` calls take fixed argument lists, never `shell=True`.
