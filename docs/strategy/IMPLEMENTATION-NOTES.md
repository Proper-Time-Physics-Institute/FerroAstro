<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Implementation notes

Interpretations, deviations and open questions recorded while implementing the roadmap. Each work package (WP)
has its own heading. Items flagged **NEEDS HUMAN REVIEW** must be resolved by a maintainer before merge.

## WP1 — M0 workspace, CI and gates

Branch `m0-foundation`, stacked on PR #2 (`1-scientific-computing-strategy`). Agent-authored.

### Toolchain and MSRV
- `rust-toolchain.toml` stays at **1.95.0**. ADR-0006 says M0 bumps it to the then-current stable; newer toolchains
  (1.98/1.99) exist on the build machine, but the WP brief fixes 1.95.0 and the pin is on the human-owned meta
  surface (red-team §2 control 1). **NEEDS HUMAN REVIEW:** decide whether to bump the pin in a separate PR.
- `rust-version = "1.85"`: edition 2024 needs 1.85, and every adopted dependency (thiserror 2, serde 1, toml 0.8,
  sha2 0.10) builds on it. Cargo's resolver 3 picks versions compatible with 1.85 (it held trybuild at 1.0.119).
  ADR-0006 is treated as authoritative over STD-005's note (red-team I2): MSRV ≤ toolchain − 2 = 1.93. ✔
- The MSRV CI job skips the `trybuild` compile-fail suite, whose `.stderr` snapshots are rustc-version specific.

### Crate naming
- The WP brief names `ferroastro-core`; ARCHITECTURE.md lists `ferroastro-units` and `ferroastro-real`.
  `ferroastro-core` holds the M0 subset of `ferroastro-units` (quantities, frame/centre markers, time-scale
  markers, `Epoch`) plus the shared `FA-Exxxx` error codes. It can be split when `Real` arrives in M1.
- **Deviation from ADR-0001:** the newtypes are `f64`-only in M0. ADR-0001/API-001 require genericity over `Real`,
  which does not exist until `ferroastro-real` (M1). API-001's "compiles with the DD backend" clause is therefore
  not yet met.
- `Epoch<S>` stores a two-part Julian date (ERFA convention). UTC is omitted on purpose (non-uniform; hifitime).

### Lints
- Workspace lints copied verbatim from docs/standards/rust.md. `clippy.toml` allows unwrap/expect/panic/indexing
  only inside `#[test]` / `#[cfg(test)]` (`allow-*-in-tests`), so library code keeps `unwrap_used = deny`.
- `float_cmp_const` is accepted by clippy 1.95 without an unknown-lint warning (OPEN-ITEMS note on group
  membership: it compiles; whether it is a separate restriction lint was not re-checked).

### Test-vector schema (ADR-0004)
- File layout: `schema_version = 1` plus `[[vector]]` tables. Field names follow ADR-0004 (`tolerance_rationale`,
  not `rationale`). Required: id, requirement, quantity, value, unit, kind, source, citation, retrieved, tolerance,
  tolerance_rationale, verified_by. `generated_by` (`path@sha`) and `engine {name, version, settings}` are
  required for `computed-reference` (TST-003). Optional: `notes`. Unknown fields are rejected (typo guard).
- `value` may be a float, an integer (|i| ≤ 2^53) or a **decimal string**, kept verbatim so that M5's
  double-double path can parse more digits than f64 holds (e.g. GM☉ = 132712440041.279419 km³/s²).
- Tolerance semantics: pass iff `|x − v| ≤ max(abs, rel·|v|)`. `abs` therefore doubles as the absolute floor
  that the red-team review asks for on NUM-015's tiny effects (LT, J2).
- Machine guard for PHY-018 / rule 8: a vector whose `source` or `citation` contains `UNVERIFIED` or `[S]` is
  rejected. This catches honest labelling only; it is not a proof of verification (red-team H5).
- The same schema exists twice: Rust (`ferroastro-vectors`, used by tests) and JSON Schema
  (`tests/vectors/schema.json`, used by `tools/trace.py --check`). Both are exercised in CI.
- First vectors: `CONST-CLIGHT-SI` (SI 2019) and `CONST-AU-IAU2012` (IAU 2012 B2), kind `defined`. They are
  ephemeris-independent oracles for the header.440 `CLIGHT`/`AU` entries (REF-002). OI-05 qualifies the au.
