---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Toolchain pin and MSRV policy

## Decision Outcome
`rust-toolchain.toml` pins the development toolchain, so every human and agent gets the same rustfmt and clippy. **At M0 it is bumped to the then-current stable release** (maintainer decision 2026-10-14). The placeholder is 1.95.0, the version verified locally, and later bumps go by PR. Edition 2024 / resolver 3. The MSRV (`rust-version`) is set in M0 to the highest minimum required by the adopted dependencies (e.g. `rmcp`, `hifitime`), and it is never more than the pinned toolchain minus two minors. Raising the MSRV is a minor-version change.

### Confirmation
CI `msrv` job builds and tests on `rust-version` (STD-005).

