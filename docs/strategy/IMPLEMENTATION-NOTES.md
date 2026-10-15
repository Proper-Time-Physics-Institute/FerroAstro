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

### Traceability and lints (tools/)
- `tools/trace.py` parses the fenced YAML blocks in `docs/requirements/<PREFIX>.md` (README, OPEN-ITEMS and
  TRACE are not catalogue files), validates keys/enums/ID format/duplicates/file placement/dangling
  `parent`/`related`, maps `/// Verifies:` (Rust) and `# Verifies:` (Python) comments to the next `fn`/`def`,
  joins vector `requirement` lists, and writes TRACE.md. The "last CI run" column of the README is **not**
  written: it would make TRACE.md non-deterministic and break `--check`. CI evidence is the workflow run.
- Coverage is enforced from status `approved` onwards (README lifecycle). Every requirement is `draft` today,
  so the coverage gate is vacuous until a maintainer approves requirements (red-team H4: requirements can stay
  `draft` to escape coverage; that needs a human/ruleset control, not a script).
- `trace.py --check` also runs the vector JSON-schema validation, `lint_literals.py` and `lint_workflows.py`,
  because the AGENTS.md command list (which CI runs verbatim) has no separate entry for them.
- `lint_literals.py` heuristic: floats other than small integers (|v| ≤ 10) and 0.5/0.25, and integers with
  |v| > 1000, are flagged in test code; strings and comments are ignored; per-line escape
  `lint: allow-literal(<reason>)` with a mandatory reason. Rust unit tests are found from the first
  `#[cfg(test)]` to end of file (convention: test module last). It is a heuristic, not a proof.
- `trace.py` shadows the stdlib `trace` module; the pytest suite loads it by path for that reason.
- `tools/.python-version` pins Python 3.12 (uv otherwise picked 3.14 on this machine).
- `pytest` (STD-009) is not in the AGENTS.md command list; CI runs it as an extra step next to ruff/mypy.

### Real-data tier (`data` tests)
- `crates/ferroastro-testdata` (publish = false) downloads products listed in `tests/data/MANIFEST.toml` with the
  system `curl`, so no TLS crate enters the graph (`ureq`'s `webpki-roots` is CDLA-Permissive-2.0, which the
  allow-list rejects; red-team L2). Files go to `$FA_DATA_DIR` or `<target>/fa-data`, are never committed, and
  get a `.provenance.toml` sidecar (URL, UTC retrieval date, SHA-256, bytes, `Last-Modified`).
- Pinned products are re-verified on every run (`FA-E0201` on mismatch). Un-pinned fetches (`fetch_url(…, None)`)
  are for service answers such as Horizons, whose provenance is the service's `signature.version`; the WP that
  adds Horizons tests must print it.
- Data tests are `#[ignore]`d and named `data_*`; `.config/nextest.toml` profile `data` selects them and shows
  their output. Command: `cargo nextest run --workspace --profile data --run-ignored only`. CI runs this in a
  scheduled (and manually dispatchable) `data` job, not on PRs, so PR CI stays offline-safe.
- First data test, run locally 2026-10-14 (UTC date 2026-10-15): `data_de440_header_defined_constants`.
  header.440 sha256 `0f4636b6…6ac3359`, 22802 bytes, Last-Modified 2020-12-22. GROUP 1040/1041 declares
  645 constants (645 distinct names, matching REF-002's note). `CLIGHT` = 299792.458 km/s and `AU` =
  149597870.7 km equal the SI / IAU 2012 B2 defined values exactly (|diff| = 0).
- The GROUP 1040/1041 reader in that test is a smoke-test reader only. The full REF-002 loader (JDEPOC, states,
  409 MA entries, J2SUN, …) belongs to `ferroastro-ephem`/`ferroastro-constants` in a later WP and should replace
  it. REF-002/PHY-002 remain blocked on OI-23/OI-04 for the state units and GMSDOT (red-team I5).

### CI (`.github/workflows/ci.yml`)
- Jobs `rust`, `python`, `reuse` run the AGENTS.md commands verbatim (split across jobs for parallelism);
  `python` also runs `uv sync --locked` and `pytest`. `msrv` runs nextest on 1.85.0 without the trybuild suite.
  `dco` checks a `Signed-off-by` trailer on every PR commit (ORG-008; text check only — red-team C4: it cannot
  prove the human signed). `data` runs only on the weekly cron and `workflow_dispatch`.
- Top-level `permissions: contents: read`, no `pull_request_target`, checkout with `persist-credentials: false`,
  and every action pinned by SHA (resolved 2026-10-14 from each repository's latest release tag:
  checkout v7.0.1, setup-uv v10.3.0, install-action v2.87.28, rust-cache v2.9.2). `lint_workflows.py` enforces
  the pinning and the trigger/permission rules.
- **Not done here (NEEDS HUMAN REVIEW):** red-team §2 control 2 asks for required checks to run from the default
  branch (ruleset "required workflows") so that a PR cannot weaken its own gates (C2). That is a repository
  setting; this WP may not change settings. Likewise CODEOWNERS for the meta surface (control 1), the TST-004
  golden-change label check, and the red-team L1 `deny-release.toml` (anise dev-only enforcement) are left for a
  human-owned PR. OpenSSF Scorecard (STD-012 second half) is not added.
- `cargo deny check` passes with only `license-not-encountered` warnings for allow-list entries unused so far.
- REUSE: lockfiles, trybuild `.stderr` snapshots, `schema.json` and `.python-version` are annotated in
  `REUSE.toml`; `trace.py` wraps the SPDX lines it emits in `REUSE-IgnoreStart/End`.
