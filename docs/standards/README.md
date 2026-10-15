<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Engineering standards

| Standard | Scope | Enforced by |
|---|---|---|
| [rust.md](rust.md) | All Rust crates | rustfmt, clippy (`-D warnings`), cargo-deny, MSRV job, rustdoc `-D warnings` |
| [python.md](python.md) | `tools/` | ruff check, ruff format, mypy strict |
| [testing.md](testing.md) | Tests, golden vectors, traceability | nextest, schema check, `trace.py`, CODEOWNERS |

Requirements: STD-001…012 and TST-001…005 in [docs/requirements](../requirements/README.md).
