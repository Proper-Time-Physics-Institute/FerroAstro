<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# STD — Engineering standards (Rust, Python, licensing hygiene, CI hygiene)

**Scope.** These requirements are the local engineering standards every change to FerroAstro must pass. They cover Rust formatting, lints, the `unsafe` policy, docs, MSRV, dependency checks, semver checks, the test runner, Python tooling under `tools/` (ruff, uv, mypy), SPDX/REUSE licensing hygiene, Conventional Commits and release automation, and pinning of GitHub Actions. Source: Journal #5 (`docs/strategy/journal/05-organization-standards.md`). Test-discipline rules (tolerances, test-vector provenance) are in [TST.md](TST.md). Dependency licence policy is in [DEP.md](DEP.md).

---

### STD-001 — rustfmt

```yaml
id: STD-001
title: "Rust passes cargo fmt --check with committed rustfmt.toml and pinned toolchain"
status: draft
criticality: low
verification: test
statement: >-
  Rust code SHALL pass `cargo fmt --all --check` using the committed
  rustfmt.toml and the toolchain pinned in rust-toolchain.toml.
acceptance: >-
  The CI fmt job passes on main and fails on an unformatted canary change.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.1–3.2 and STD-001.
parent: null
related: [STD-005]
notes: >-
  rustfmt.toml uses stable-only options (edition 2024, max_width 100). The
  pinned channel 1.95.0 is a placeholder (ADR-0006), bumped to the then-current
  stable at M0. Stabilisation status of imports_granularity / group_imports is
  UNVERIFIED, so they are avoided.
```

### STD-002 — Workspace lints and clippy

```yaml
id: STD-002
title: "All crates inherit [workspace.lints]; clippy -D warnings passes; exceptions use #[expect]"
status: draft
criticality: medium
verification: test
statement: >-
  All crates SHALL inherit [workspace.lints] ([lints] workspace = true), and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings` SHALL
  pass. Lint exceptions SHALL use #[expect(..., reason = "...")].
acceptance: >-
  The CI clippy job passes; a check confirms every member Cargo.toml has
  [lints] workspace = true and that no #[allow(clippy::…)] lacks a reason
  outside the permitted test-module exception.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.3 and STD-002.
parent: null
related: [TST-001, STD-003]
notes: >-
  UNVERIFIED: #[expect] stabilised in Rust 1.81 (stable on 1.85+ either way);
  group membership of float_cmp_const, lossy_float_literal, suboptimal_flops,
  imprecise_flops, unwrap_used, indexing_slicing. Confirm against the clippy
  lint list before merging.
```

### STD-003 — unsafe forbidden

```yaml
id: STD-003
title: "unsafe_code forbidden workspace-wide; any exception needs an ADR"
status: draft
criticality: medium
verification: test
statement: >-
  unsafe_code SHALL be forbidden workspace-wide. Any exception SHALL require an
  ADR, SHALL sit outside the default lint inheritance and SHALL carry
  // SAFETY: comments.
acceptance: >-
  [workspace.lints.rust] has unsafe_code = "forbid"; CI fails on a canary
  unsafe block in an inheriting crate.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.3 and STD-003.
parent: null
related: [NUM-021, ORG-005]
notes: >-
  A future SIMD crate (NUM-021) is the likely first exception.
```

### STD-004 — Documentation completeness

```yaml
id: STD-004
title: "All public items documented; rustdoc builds with -D warnings"
status: draft
criticality: low
verification: test
statement: >-
  All public items SHALL be documented (missing_docs = "deny"), and rustdoc
  SHALL build with `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
  --all-features`.
acceptance: >-
  The CI docs job passes.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.4 and STD-004.
parent: null
related: [API-003]
notes: null
```

### STD-005 — MSRV declared and tested

```yaml
id: STD-005
title: "Workspace declares rust-version (MSRV) and is tested on it"
status: draft
criticality: low
verification: test
statement: >-
  The workspace SHALL declare rust-version (MSRV) in [workspace.package] and
  SHALL be built and tested on exactly that version in CI. MSRV changes SHALL
  follow ADR-0006.
acceptance: >-
  The CI msrv job builds and tests on the declared rust-version.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.1 and STD-005.
parent: null
related: [STD-001, DEP-006]
notes: >-
  Policy per ADR-0006 (AUTHORITATIVE): rust-version is set at M0 to the highest
  minimum required by the adopted dependencies (e.g. rmcp, hifitime) and is
  never more than the pinned toolchain minus two minors (MSRV ≤ toolchain − 2);
  raising it is semver:minor. Edition 2024 sets a floor of Rust ≥ 1.85.
  Journal #5's "MSRV ≥ latest stable minus 2" wording is superseded.
```

### STD-006 — cargo deny gate

```yaml
id: STD-006
title: "Dependencies pass cargo deny check (licenses, advisories, bans, sources)"
status: draft
criticality: high
verification: test
statement: >-
  Dependencies SHALL pass `cargo deny check` (licenses, advisories, bans,
  sources) in CI.
acceptance: >-
  The CI deny job (EmbarkStudios/cargo-deny-action, SHA-pinned) passes on main.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.4 and STD-006.
parent: null
related: [DEP-001, DEP-002, DEP-003, DEP-015, DEP-016]
notes: >-
  This is the CI gate. The policy content (allow-list, bans) is AUTHORITATIVE
  in DEP-001..DEP-003.
```

### STD-007 — cargo-semver-checks

```yaml
id: STD-007
title: "Public API changes checked with cargo-semver-checks: advisory before M6, gating from M6"
status: draft
criticality: medium
verification: test
statement: >-
  Public API changes SHALL be checked with cargo-semver-checks against the last
  release: advisory before milestone M6, gating from M6.
acceptance: >-
  semver.yml runs on every PR; from M6 it is a required status check.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.4 and STD-007.
parent: null
related: [STD-011]
notes: >-
  Journal #5 said "from M5" using its own milestone numbering; that milestone
  (Showcase & 0.x release) is M6 in ROADMAP.md.
```

### STD-008 — nextest, doctests and long runs

```yaml
id: STD-008
title: "Tests run under cargo-nextest, doctests separately; long runs #[ignore] and nightly"
status: draft
criticality: medium
verification: test
statement: >-
  Tests SHALL run under cargo-nextest, with doctests run separately via
  `cargo test --doc`. Long runs SHALL be marked #[ignore] and run nightly.
acceptance: >-
  The ci.yml test job runs nextest on Linux/macOS/Windows plus
  `cargo test --doc`; nightly.yml runs `cargo nextest run --run-ignored all`.
source: >-
  docs/strategy/journal/05-organization-standards.md §3.4, §7 rule 5 and
  STD-008.
parent: null
related: [DEP-014, CI-005, CI-006, CI-010]
notes: >-
  That nextest does not run doctests is per its docs; whether that has changed
  is UNVERIFIED.
```

### STD-009 — Python tooling standards

```yaml
id: STD-009
title: "Python under tools/ passes ruff check and ruff format, uses uv with lockfile, passes mypy strict"
status: draft
criticality: low
verification: test
statement: >-
  Python under tools/ SHALL pass `ruff check` (rule set in Journal #5 §4) and
  `ruff format --check`, SHALL be managed by uv with a committed uv.lock, and
  SHALL pass mypy in strict mode.
acceptance: >-
  python.yml runs `uv sync --locked`, `uv run ruff check --output-format=github`,
  `uv run ruff format --check`, mypy --strict and pytest, all passing.
source: >-
  docs/strategy/journal/00-kickoff.md (ruff for all Python tooling);
  docs/strategy/journal/05-organization-standards.md §4 and STD-009.
parent: null
related: [ORG-004]
notes: >-
  Pyright is an acceptable substitute for mypy, chosen by ADR (one, not both).
  The orekit-jpype packaging choice and the pre-commit hook IDs/revisions are
  UNVERIFIED. Licence guard (red-team L12, Journal #9): python.yml runs
  pip-licenses over the locked tools/ environment and fails on any GPL/AGPL/LGPL
  runtime dependency (e.g. rebound), so GPL oracles stay in ci/oracles/
  containers (LIC-001, LIC-005). `reuse`, partly GPL-3, is allowed only as a
  CLI tool, not imported.
```

### STD-010 — SPDX headers and REUSE

```yaml
id: STD-010
title: "Every file carries SPDX information; repo passes reuse lint; LICENSE and NOTICE at root"
status: draft
criticality: high
verification: test
statement: >-
  Every source file SHALL carry SPDX copyright and license information (header
  or .license sidecar, or REUSE.toml for bulk paths), and the repository SHALL
  pass `reuse lint`. LICENSE (Apache-2.0) and NOTICE SHALL be present at the
  root.
acceptance: >-
  The governance.yml reuse job passes; LICENSE, NOTICE and
  LICENSES/Apache-2.0.txt exist.
source: >-
  docs/strategy/journal/05-organization-standards.md §5 and STD-010.
parent: null
related: [DEP-005, DEP-009, LIC-004]
notes: >-
  Header text: the standard two-line SPDX header (copyright "2026 Proper-Time
  Physics Institute", license identifier Apache-2.0); see docs/standards/.
  Bundled data needs its own attribution and checked terms.
```

### STD-011 — Conventional Commits and release automation

```yaml
id: STD-011
title: "Squash PR titles follow Conventional Commits 1.0.0; releases via release-please"
status: draft
criticality: low
verification: test
statement: >-
  Commits merged to main (squash PR titles) SHALL follow Conventional Commits
  1.0.0. Releases and the CHANGELOG SHALL be produced by release-please or an
  ADR-chosen alternative.
acceptance: >-
  The governance PR-title lint passes; release-please opens release PRs from
  main.
source: >-
  docs/strategy/journal/05-organization-standards.md §8 and STD-011.
parent: null
related: [ORG-008, STD-007]
notes: >-
  The DCO Signed-off-by trailer (ORG-008) must survive squash merges. Cargo
  workspace internal-dependency bumping by release-please is UNVERIFIED;
  evaluate release-plz in the ADR. Release PRs created with GITHUB_TOKEN do not
  trigger CI, so a PAT or app token is needed.
```

### STD-012 — Pinned Actions and OpenSSF Scorecard

```yaml
id: STD-012
title: "Third-party GitHub Actions pinned by commit SHA; OpenSSF Scorecard runs weekly"
status: draft
criticality: medium
verification: test
statement: >-
  Third-party GitHub Actions SHALL be pinned by commit SHA, and an OpenSSF
  Scorecard SHALL run weekly.
acceptance: >-
  A workflow lint finds no third-party `uses:` reference without a 40-hex SHA;
  scorecard.yml runs on a weekly schedule.
source: >-
  docs/strategy/journal/05-organization-standards.md §2.1, §8 and STD-012;
  ossf/scorecard.
parent: null
related: [CI-001]
notes: >-
  Goal: OpenSSF Best Practices "passing" badge; the effort is UNVERIFIED.
```
