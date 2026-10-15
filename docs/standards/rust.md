<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Rust standards

- **Toolchain:** pinned in `rust-toolchain.toml` (ADR-0006). Edition 2024, `resolver = "3"`, MSRV declared in `[workspace.package]`.
- **Formatting:** `cargo fmt --all --check` with the committed `rustfmt.toml` (STD-001).
- **Lints:** all crates set `[lints] workspace = true`. Exceptions use `#[expect(lint, reason = "...")]` (STD-002).
- **Safety:** `unsafe_code = "forbid"` (STD-003). **Docs:** `missing_docs = "deny"` (STD-004).
- **Dependencies:** `cargo deny check` with the root `deny.toml` (STD-006, DEP-001…003).

## Workspace lints (to be placed in the root `Cargo.toml` at M0)

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "deny"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }
# numerical correctness (NUM-001 / TST-001)
float_cmp = "deny"
float_cmp_const = "deny"
cast_precision_loss = "deny"
cast_possible_truncation = "deny"
cast_sign_loss = "deny"
lossy_float_literal = "deny"
excessive_precision = "warn"   # published constants: use #[expect] with a citation
suboptimal_flops = "warn"      # FMA changes bits; follow the FMA policy (NUM-021)
imprecise_flops = "warn"
# robustness
unwrap_used = "deny"
expect_used = "warn"
panic = "warn"
indexing_slicing = "warn"
missing_errors_doc = "warn"
missing_panics_doc = "warn"
# domain notation
many_single_char_names = "allow"
similar_names = "allow"
module_name_repetitions = "allow"
unreadable_literal = "warn"
```

The lint-group membership of some restriction and nursery lints is UNVERIFIED (see OPEN-ITEMS). Confirm it against
the Clippy lint list when the workspace is created.

## Physics-code conventions
- Frame, centre, time scale and unit live in types (ADR-0001). Conversions cite their source routine.
- Generic over `Real` throughout the dynamics path (ADR-0002).
- Deterministic: fixed reduction order and an explicit `mul_add` policy (NUM-021).
- Errors: `thiserror` enums, stable `FA-Exxxx` codes, and messages naming the quantity, unit, valid range and fix (API-002).
- Every public item has a doctest that uses cited values (API-003).
