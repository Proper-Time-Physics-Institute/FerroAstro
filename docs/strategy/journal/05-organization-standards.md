## 📓 Journal #5 — Organization & engineering standards

> **Lead review notes:**
> - **ID collision fixed.** This stream's test-discipline requirements were renumbered `NUM-00x` → **`TST-00x`**, because the `NUM-` prefix belongs to Journal #4 (numerical methods).
> - **Milestones are a first pass.** M0–M5 below compress the ladder. The consolidated roadmap will expand them to the agreed rungs: Newtonian 2-body → +planets → +Moon/asteroids → +solar J2, then SR (two formulations), Schwarzschild test particle, 1PN Sun-only and full EIH N-body. DE440 reproduction is a separate milestone.
> - **Answer to open question 6:** the target is **both**. Each rung reproduces published values *and* carries an independent error budget, with a numerical secular fit checked against the analytic formula.

**Date:** 2026-10-14 · **Branch:** `1-scientific-computing-strategy` · **Scope:** organizational requirements (mission, requirements, traceability, ADRs, roadmap) and local engineering standards (Rust, Python, CI, licensing, agent conventions) for FerroAstro.

> Convention in this entry: a claim with a link was checked against that source during this research pass. **UNVERIFIED** marks items I could not confirm from a primary source. They need a check before we rely on them. Items marked *(proposal)* are our own design choices, not external facts.

---

### 1. TL;DR: what to adopt

| Area | Recommendation |
|---|---|
| Mission | One-page `docs/strategy/MISSION.md`. The flagship is the **Mercury perihelion precession ladder**: Newtonian two-body, then planetary perturbations, then 1PN GR, then a full ephemeris comparison. Each rung is a milestone with a falsifiable acceptance test. |
| Requirements | Plain markdown files with YAML front-matter IDs (`ORG-`, `STD-`, `PHY-`, `NUM-`, `TST-`, `API-`, `AGT-`). Tests cite IDs (`// REQ: PHY-012`). A Python script builds `docs/requirements/TRACE.md`, and CI fails when a requirement has no test. This is a lightweight tailoring of NASA NPR 7150.2 Class D/E and the ECSS-E-ST-40C criticality-based tailoring idea. |
| Decisions | MADR 4.0.0 ADRs in `docs/adr/NNNN-title.md`. |
| Rust | Edition 2024, pinned `rust-toolchain.toml`, a declared MSRV, `[workspace.lints]` (`unsafe_code = "forbid"`, `missing_docs = "deny"`, clippy `pedantic` at warn plus float and cast lints), `cargo-deny`, `cargo-semver-checks`, `cargo-nextest`. |
| Python | `uv` with a `pyproject.toml`. `ruff check` and `ruff format` are hard CI gates, with mypy in strict mode for `tools/`. pre-commit runs the same hooks locally. |
| Legal | Apache-2.0 `LICENSE` and `NOTICE`, REUSE-compliant SPDX headers (`reuse lint` in CI), and a CLA via the CLA Assistant GitHub Action (signatures stored in an org repo), using Apache ICLA/CCLA-derived text. |
| Agents | Root `AGENTS.md` as the canonical source, `CLAUDE.md` importing or pointing to it, `llms.txt` for the docs site, typed units and frames, structured errors, and doctested examples. An MCP server comes later. |
| Test vectors | `tests/vectors/*.toml`. Each value carries `source`, `citation`, `retrieved`, `tolerance`, a `tolerance_rationale`, and a `requirement`. A schema check runs in CI. |
| CI | GitHub Actions: fmt, clippy, nextest, doc, deny, semver, ruff, reuse, trace, CLA, plus nightly long runs and benchmarks on a pinned runner. Conventional Commits with release-please. |

---

### 2. Requirements management for scientific OSS

#### 2.1 What the heavyweight standards give us, and what we keep

- **NASA NPR 7150.2D** sorts software into Classes A–F. Class D covers "basic science, engineering design, and research and technology software" and Class E covers "design concept, research, technology, and general-purpose software." The project manager assigns "the highest applicable class" per subsystem (SWE-020). See the [SWEHB SWE-020 page](https://swehb.nasa.gov/x/BoHeB) and the [NPR 7150.2D PDF](https://soma.larc.nasa.gov/STP/DYNAMIC/pdf_files/NPR_7150.2D%20SW%20Eng.pdf).
  - **Our position** *(proposal)*: FerroAstro is Class **D/E**-equivalent research software. We are not bound by NPR 7150.2, and we make no NASA compliance claim. We borrow three ideas: an explicit classification, requirements traceable to tests, and a record of verification evidence. The per-SWE applicability matrix (Appendix C) was **not** reviewed here (**UNVERIFIED** which SWEs apply at Class D/E).
- **ESA ECSS-E-ST-40C Rev.1** (30 April 2025) is the software engineering standard, paired with **ECSS-Q-ST-80C** for product assurance. Criticality categories A–D are assigned from the most critical function the software implements, and the requirements are tailored by category ([ECSS-E-ST-40C Rev.1 PDF](https://ecss.nl/wp-content/uploads/2025/05/ECSS-E-ST-40C-Rev.1(30April2025).pdf), [Ansys intro to E-40/Q-80](https://innovationspace.ansys.com/knowledge/forums/topic/an-introduction-to-space-software-standards-ecss-e-st-40-and-ecss-q-st-80c/)). The authoritative A–D table is ECSS-Q-ST-80C Table D-1, which we did **not** retrieve (**UNVERIFIED** exact definitions). Some sources reference a Q-ST-80C **Rev.2**, so confirm the current revision before citing it.
  - **Our position** *(proposal)*: we keep a short `docs/requirements/TAILORING.md` that states "we follow the *spirit* of criticality-based tailoring." Each crate gets a criticality tag. A crate tagged `crit:high` requires a reference-comparison test against an independent implementation (Orekit or REBOUND) plus a published-value test.
- **OpenSSF:** the OpenSSF Scorecard runs automated checks scored 0–10. They include `Branch-Protection`, `Pinned-Dependencies`, and `CII-Best-Practices`, the last of which checks for a passing, silver, or gold Best Practices badge ([ossf/scorecard](https://github.com/ossf/scorecard)). We adopt Scorecard as a CI job and aim for the OpenSSF Best Practices "passing" badge. Badge criteria were not reviewed in detail (**UNVERIFIED** effort).

#### 2.2 Lightweight requirement format *(proposal)*

One file per requirement area, with one fenced YAML block per requirement. The format is machine-readable for agents and still renders on GitHub:

```markdown
<!-- docs/requirements/PHY.md -->
### PHY-012 — 1PN Schwarzschild perihelion advance
```yaml
id: PHY-012
title: 1PN perihelion advance for a test body around a point mass
status: approved        # draft | approved | implemented | verified | retired
criticality: high       # high | medium | low  (our ECSS-inspired tailoring)
rationale: Rung 3 of the Mercury ladder (ORG-002).
verification: test      # test | analysis | inspection | demonstration
acceptance: >
  Integrated secular advance of ω over ≥100 orbits matches
  Δω = 6πGM / (c² a (1−e²)) per orbit to relative 1e-6.
evidence: [tests/vectors/mercury_1pn.toml]
parent: ORG-002
```
```

- Tests reference IDs with a doc attribute or comment: `/// Verifies: PHY-012`. Python reference scripts use `# Verifies: PHY-012`.
- `tools/trace.py`, run with `uv run`, greps the IDs, joins them against the requirement YAML, and writes `docs/requirements/TRACE.md`, a requirement → tests → vector files → last CI run matrix. CI fails when an `approved` requirement with `verification: test` has zero linked tests, or when a test cites an unknown ID.
- Off-the-shelf alternatives worth an ADR comparison are **StrictDoc** and **OpenFastTrace**. Neither was evaluated in this pass (**UNVERIFIED** fit). The home-grown script is roughly 150 lines and keeps the dependency count down.

#### 2.3 Decision records: MADR 4.0.0

MADR 4.0.0 has YAML front-matter fields (`status`, `date`, `decision-makers`, `consulted`, `informed`) and optional sections (Decision Drivers, Considered Options, Pros and Cons, **Confirmation**, More Information). Records are named `NNNN-title-with-dashes.md` ([MADR 4.0.0 tag](https://github.com/adr/madr/tree/4.0.0), [adr.github.io/madr](https://adr.github.io/madr/), an example of real adoption in [JabRef ADR-0000](https://devdocs.jabref.org/decisions/0000-use-markdown-architectural-decision-records.html)). The **Confirmation** section is useful here: it is where an ADR names the CI check or test that enforces the decision.

Seed ADRs *(proposal)*:
- `0000-use-madr.md`
- `0001-units-and-frames-in-the-type-system.md` (newtype quantities versus `uom` versus a home-grown crate)
- `0002-time-scales-tdb-tt-utc.md`
- `0003-reference-implementations-orekit-rebound.md`
- `0004-test-vector-provenance-format.md`
- `0005-cla-mechanism.md`
- `0006-msrv-policy.md`
- `0007-agents-md-canonical.md`

#### 2.4 GitHub planning structure *(proposal)*

- **Milestones** follow the flagship ladder. Each milestone description names its `ORG-`/`PHY-` requirement IDs and the acceptance test.
  | Milestone | Content | Exit criterion |
  |---|---|---|
  | `M0 Foundations` | Workspace, CI, standards, licensing, AGENTS.md | All STD-* checks green on `main` |
  | `M1 Kepler` | Two-body propagation, time scales, frames | Two-body vectors match Orekit within documented tolerance |
  | `M2 N-body Newtonian` | Planetary perturbations, integrators | Mercury Newtonian precession contribution vs published value |
  | `M3 1PN GR` | EIH / Schwarzschild 1PN terms | Mercury GR term vs analytic Δω |
  | `M4 Ephemeris ladder` | Comparison against JPL DE-series | Full ladder reproduced, report published |
  | `M5 0.x API stabilization` | semver-checks enforced, docs complete | `cargo semver-checks` gating |
- **GitHub Projects (v2)**: one org-level project with the fields `Status`, `Requirement` (text), `Milestone`, `Area`, `Effort`, and `Agent-eligible` (bool), plus a Roadmap view grouped by milestone.
- **Issue forms** (`.github/ISSUE_TEMPLATE/*.yml`): `bug.yml`, `requirement.yml` (asks for the ID, acceptance, verification method, and parent), `physics-discrepancy.yml` (expected value, source citation, observed value, tolerance), `adr-proposal.yml`, `agent-task.yml` (scope, files, done-definition, commands to run).

#### 2.5 Label taxonomy *(proposal)*

| Prefix | Labels |
|---|---|
| `type:` | `bug`, `feature`, `requirement`, `adr`, `docs`, `chore`, `perf`, `physics-discrepancy` |
| `area:` | `time`, `frames`, `ephemeris`, `integrators`, `gr`, `orbit-determination`, `units`, `python-tools`, `ci`, `docs` |
| `crit:` | `high`, `medium`, `low` |
| `status:` | `triage`, `needs-design`, `ready`, `blocked`, `needs-reference-data` |
| `agent:` | `eligible`, `in-progress`, `needs-human-review` |
| `priority:` | `p0`, `p1`, `p2` |
| `semver:` | `breaking`, `minor`, `patch` |
| misc | `good-first-issue`, `help-wanted`, `cla:signed` (bot-managed) |

Labels are kept as code in `.github/labels.yml` and synced by an Action. `EndBug/label-sync` is one option (**UNVERIFIED** current maintenance status).

---

### 3. Rust standards

#### 3.1 Toolchain and MSRV

- **`rust-toolchain.toml`** pins the *development* toolchain, so every human and agent gets an identical `rustfmt` and `clippy`:
  ```toml
  [toolchain]
  channel = "1.90.0"          # bump via ADR/PR; exact version UNVERIFIED as "latest" — pick current stable at bootstrap
  components = ["rustfmt", "clippy", "rust-docs"]
  profile = "minimal"
  ```
- **MSRV** is declared as `rust-version` in `[workspace.package]`. A separate CI job tests on exactly that version.
- **Edition 2024** implies `resolver = "3"`, which makes the MSRV-aware resolver default to `incompatible-rust-versions = "fallback"`. Opt-in support shipped in Rust 1.84 ([Rust 1.84.0 release](https://blog.rust-lang.org/2025/01/09/Rust-1.84.0/), [Edition guide: Rust-version aware resolver](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html)). Edition 2024 requires Rust ≥ 1.85.
- **Policy** *(proposal, ADR-0006)*: MSRV equals the latest stable minus 2 minor releases at most. Raising it is a `semver:minor` change, not a breaking one, and it is recorded in the CHANGELOG.

#### 3.2 `rustfmt.toml` *(proposal)*

Keep it near-default and stable-only, so formatting never depends on nightly:
```toml
# rustfmt.toml
edition = "2024"
max_width = 100
use_field_init_shorthand = true
use_try_shorthand = true
newline_style = "Unix"
```
`imports_granularity` and `group_imports` are nightly-only options (**UNVERIFIED** current stabilization status), so we avoid them.

#### 3.3 Workspace lints

`[lints]` and `[workspace.lints]` were stabilized in **Rust 1.74**. A member crate opts in with `[lints] workspace = true` and cannot override individual lints when inheriting ([RFC 3389](https://rust-lang.github.io/rfcs/3389-manifest-lint.html)). Per-item exceptions use `#[expect(clippy::lint, reason = "...")]`. `#[expect]` was stabilized in Rust 1.81 (**UNVERIFIED** exact version; it is stable on 1.85+).

Clippy lint groups: `cast_precision_loss` and `cast_possible_truncation` are **pedantic** (allow by default), and `approx_constant` is **correctness** (deny by default) ([Clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html)). `float_cmp` was **demoted from correctness to pedantic** in clippy PR #7692 ([issue #7666](https://github.com/rust-lang/rust-clippy/issues/7666)). Group membership of `float_cmp_const`, `lossy_float_literal`, `suboptimal_flops`, `imprecise_flops`, `unwrap_used`, and `indexing_slicing` is **UNVERIFIED** in this pass. My understanding is restriction, restriction, nursery, nursery, restriction, restriction; confirm against the lint list before merging.

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
edition = "2024"
rust-version = "1.85"            # MSRV — see ADR-0006
license = "Apache-2.0"
repository = "https://github.com/Proper-Time-Physics-Institute/FerroAstro"

[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "deny"
missing_debug_implementations = "warn"
unreachable_pub = "warn"
unused_qualifications = "warn"
rust_2018_idioms = { level = "warn", priority = -1 }

[workspace.lints.rustdoc]
broken_intra_doc_links = "deny"
private_intra_doc_links = "deny"
missing_crate_level_docs = "warn"

[workspace.lints.clippy]
# groups first, at lower priority so individual lints below win
all = { level = "deny", priority = -1 }
pedantic = { level = "warn", priority = -1 }

# --- numerical-correctness lints (the point of a physics library) ---
float_cmp = "deny"                 # pedantic; force tolerance-based compares
float_cmp_const = "deny"           # group UNVERIFIED
cast_precision_loss = "deny"       # pedantic; i64/u64 -> f64 silently lossy
cast_possible_truncation = "deny"  # pedantic
cast_sign_loss = "deny"
cast_lossless = "warn"
lossy_float_literal = "deny"       # literal not exactly representable
excessive_precision = "warn"       # NOTE: may fire on published constants; use #[expect] w/ citation
suboptimal_flops = "warn"          # suggests mul_add etc.; review — FMA changes bits
imprecise_flops = "warn"

# --- robustness in library code ---
unwrap_used = "deny"
expect_used = "warn"
panic = "warn"
indexing_slicing = "warn"
missing_errors_doc = "warn"
missing_panics_doc = "warn"

# --- pedantic noise acceptable in physics code ---
many_single_char_names = "allow"   # r, v, a, e, i, ω are domain notation
similar_names = "allow"
module_name_repetitions = "allow"
unreadable_literal = "warn"        # prefer 1.327_124_4e20
```
Each member crate's `Cargo.toml` contains `[lints] workspace = true`. In test modules, `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::float_cmp))]` is acceptable **only** when the comparison is an exact-by-construction check; otherwise use the `approx` helpers.

**`unsafe` policy** *(proposal)*: `forbid` workspace-wide. A future SIMD or FFI crate would need an ADR, would sit outside the default lint inheritance, and would carry `// SAFETY:` comments. Clippy's `undocumented_unsafe_blocks` is the relevant lint.

#### 3.4 Supply chain, API stability, tests

- **cargo-deny** (`deny.toml`): `licenses.allow = ["Apache-2.0", "MIT", "BSD-3-Clause", "Unicode-3.0", ...]`, advisories from RustSec, `bans.multiple-versions = "warn"`, `sources.unknown-registry = "deny"`. The action is `EmbarkStudios/cargo-deny-action` ([cargo-deny book](https://embarkstudios.github.io/cargo-deny/)).
- **cargo-semver-checks** lints public API changes against a baseline, which can be crates.io, a git revision, or a local path. It is available as `obi1kenobi/cargo-semver-checks-action@v2` ([repo](https://github.com/obi1kenobi/cargo-semver-checks)). The author suggests running it before `cargo publish`. Some projects find the premade action too opinionated for per-PR checks and install it with `taiki-e/install-action` instead ([tokio discussion](https://github.com/tokio-rs/tokio/pull/5437)). The job is advisory until `M5`, then gating.
- **cargo-nextest** runs tests in CI (process-per-test, JUnit output, retries for flaky tests) ([nexte.st](https://nexte.st/)). nextest does **not** run doctests, so CI also runs `cargo test --doc` (per the nextest docs; **UNVERIFIED** whether this has changed recently).
- **Docs:** `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features`. Every public item gets a doc comment with a runnable example where practical. Doctests are our primary agent-facing examples (§6).

---

### 4. Python standards (tooling only)

Python lives under `tools/` (reference comparison, report generation, Orekit and REBOUND drivers) with its own `pyproject.toml`, managed by **uv** (`uv sync`, `uv run`, committed `uv.lock`) ([uv docs](https://docs.astral.sh/uv/)).

```toml
# tools/pyproject.toml
[project]
name = "ferroastro-tools"
version = "0.0.0"
requires-python = ">=3.11"
license = "Apache-2.0"
dependencies = ["numpy", "rebound", "pyyaml", "tomli-w"]   # orekit via orekit-jpype — UNVERIFIED packaging choice

[dependency-groups]
dev = ["ruff", "mypy", "pytest", "pre-commit", "reuse"]

[tool.ruff]
line-length = 100
target-version = "py311"
extend-exclude = ["vendor/"]

[tool.ruff.lint]
select = [
  "E", "W",    # pycodestyle
  "F",         # pyflakes
  "I",         # isort
  "N",         # pep8-naming
  "UP",        # pyupgrade
  "B",         # bugbear
  "SIM",       # simplify
  "C4",        # comprehensions
  "PTH",       # use pathlib
  "NPY",       # NumPy-specific (e.g. legacy np.random)
  "PD",        # pandas-vet (harmless if pandas unused)
  "RUF",       # ruff-specific
  "D",         # pydocstyle — must be selected explicitly
  "ANN",       # annotations (pairs with mypy --strict)
  "PL",        # pylint subset
  "PT",        # pytest style
  "S",         # bandit subset (subprocess calls to Orekit/Java)
]
ignore = [
  "PLR2004",   # magic numbers — physics constants are cited in vectors instead
  "N803", "N806",  # allow physics names like GM, J2 in args/locals
]

[tool.ruff.lint.pydocstyle]
convention = "numpy"

[tool.ruff.lint.per-file-ignores]
"tests/**" = ["D", "S101", "ANN"]

[tool.ruff.format]
quote-style = "double"
docstring-code-format = true

[tool.mypy]
strict = true
python_version = "3.11"
```
Notes: Ruff's `D` rules are **off by default** and must be selected explicitly. Setting `convention` disables rules outside that convention ([ruff settings: pydocstyle](https://docs.astral.sh/ruff/settings/#lint_pydocstyle_convention), [ruff FAQ](https://docs.astral.sh/ruff/faq/)). Current ruff puts lint options under `[tool.ruff.lint]`; older examples use bare `[tool.ruff]`. The CI gates are `uv run ruff check --output-format=github` and `uv run ruff format --check`. Pyright is an acceptable substitute for mypy; pick one in an ADR, not both.

**pre-commit** (`.pre-commit-config.yaml`) *(proposal)*: `astral-sh/ruff-pre-commit` (`ruff-check --fix`, `ruff-format`), `fsfe/reuse-tool` (`reuse lint`), a local hook for `cargo fmt --check`, a local hook for `tools/trace.py --check`, plus `check-yaml`, `check-toml`, and `end-of-file-fixer`. Hook IDs and revisions are **UNVERIFIED**; pin them at bootstrap with `pre-commit autoupdate`. CI runs the same checks, so pre-commit is a convenience, not the gate.

---

### 5. Licensing, NOTICE, CLA

- **NOTICE:** Apache-2.0 §4.4 defines the NOTICE mechanism. ASF guidance puts `NOTICE` next to `LICENSE` at the top level and says it should list only what is actually distributed ([ASF: Assembling LICENSE and NOTICE](https://infra.apache.org/licensing-howto.html), [ASF: Applying the license](https://www.apache.org/legal/apply-license.html)). Proposed content:
  ```
  FerroAstro
  Copyright 2026 Proper-Time Physics Institute

  This product includes software developed at the
  Proper-Time Physics Institute (https://github.com/Proper-Time-Physics-Institute).
  ```
  Bundled data, such as JPL ephemeris excerpts or IERS tables, needs its own attribution and terms checked before inclusion. Do **not** vendor DE-series files without confirming their terms (**UNVERIFIED** redistribution terms per dataset).
- **REUSE / SPDX headers:** `reuse lint` checks that every file has copyright and license information, either in a header or in a sidecar `.license` file, and that every referenced license text exists under `LICENSES/` ([reuse lint docs](https://reuse.readthedocs.io/en/stable/man/reuse-lint.html), [REUSE spec 3.3](https://reuse.software/spec-3.3/)). Header:
  ```rust
  // SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
  // SPDX-License-Identifier: Apache-2.0
  ```
  Bulk-licensed paths, such as test-vector data and images, go in `REUSE.toml`. Note that `LICENSES/Apache-2.0.txt` sits alongside the root `LICENSE`.
- **CLA:** there are two mechanisms.
  - **cla-assistant.io** (hosted GitHub App, provided by SAP): the CLA text lives in a Gist, it has a UI and custom fields, and it asks contributors to re-sign when the CLA changes ([cla-assistant](https://github.com/cla-assistant/cla-assistant)).
  - **contributor-assistant/github-action** ("CLA Assistant Lite"): signatures are stored as JSON in this repo or another (possibly private) repo, with no external database. It triggers on `pull_request_target` and `issue_comment`. A PAT or app token may be needed when branch protection blocks the default token ([action repo](https://github.com/contributor-assistant/github-action)).
  - **Recommendation** *(proposal, ADR-0005)*: use the **Action** with signatures in a private `Proper-Time-Physics-Institute/cla-signatures` repo. That keeps signature records under org control with no third-party service. Base the text on the **Apache ICLA** (individual) and **CCLA** (corporate) templates ([ASF licenses page](https://www.apache.org/licenses/contributor-agreements.html)), with the ASF name replaced, after legal review. The CLA needs an explicit clause on **AI-generated contributions**. Proposed: the human or org operating the agent signs and is responsible. Whether and how CLAs cover agent-authored code is legally unsettled (**UNVERIFIED**; needs counsel). Bot accounts are allowlisted in the action config.

---

### 6. Agentic-first conventions

- **AGENTS.md** is a plain-markdown "README for agents," stewarded by the **Agentic AI Foundation under the Linux Foundation**. The nearest file in the directory tree wins, and explicit user prompts override it. Recommended sections: project overview, build/test commands, code style, testing instructions, security considerations, plus PR guidelines. Listed supporting tools include Codex, Jules, Cursor, Aider, Zed, Copilot coding agent, Gemini CLI, Devin, and Junie, among others ([agents.md](https://agents.md/)).
  - **Policy** *(proposal, ADR-0007)*: the root `AGENTS.md` is canonical. Crate-level `crates/*/AGENTS.md` holds local physics conventions such as frame and time-scale assumptions. `CLAUDE.md` contains `@AGENTS.md` (the Claude Code import syntax; **UNVERIFIED** that it is still the recommended pattern) plus Claude-specific notes only, so the instructions never drift between copies.
  - **Required AGENTS.md content:** exact commands (`cargo nextest run`, `cargo clippy --all-targets -- -D warnings`, `uv run ruff check`, `uv run tools/trace.py --check`), the "every numeric test cites a vector file" rule, the forbidden list (no `unsafe`, no new deps without `cargo deny check`, no editing golden values without a provenance update), and the PR checklist.
- **llms.txt:** a community *proposal* by Jeremy Howard (Answer.AI, Sept 2024) for a root `/llms.txt` markdown index, optionally with `.md` copies of pages. It is **not** an official standard, and Google says Search does not use it ([llmstxt.org](https://llmstxt.org/)). It costs little, so we generate `llms.txt` and `llms-full.txt` for the docs site, but it is a nice-to-have.
- **Machine-readable API metadata:** rustdoc JSON (`cargo +nightly rustdoc -- -Z unstable-options --output-format json`) is nightly-only, so it is a CI artifact and not a gate (**UNVERIFIED** stabilization status). `cargo-semver-checks` already consumes rustdoc JSON internally.
- **MCP server** (later milestone, `ferroastro-mcp`): tools such as `propagate`, `convert_frame`, and `convert_time`, each declaring an `outputSchema` and returning `structuredContent`, with a TextContent copy of the same JSON for backward compatibility, per the MCP spec. Rev 2025-06-18 introduced structured output, and 2025-11-25 keeps it ([MCP tools spec 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)). Tool errors use the `isError` result flag (**UNVERIFIED** in this pass; check the spec's error-handling section).
- **Library ergonomics for agents** *(proposal)*:
  1. **Typed quantities and frames.** `Length<Km>`, `Epoch<Tdb>`, and `State<Icrf, Barycentric>` turn frame and time-scale mix-ups into compile errors. A type error is the cheapest feedback an agent can get. The `uom` crate versus newtypes is decided in ADR-0001.
  2. **Structured errors.** Use `thiserror` enums with stable variant names and an error code (`FA-E0xx`). Each message names the offending quantity, its unit, the valid range, and a remediation hint, e.g. `FA-E012: eccentricity 1.02 ≥ 1 for elliptic propagator; use HyperbolicPropagator`. No `panic!` in library paths.
  3. **Doctests on every public fn**, using real values and citing the vector ID. Plus an `examples/` directory with one runnable example per ladder rung (`cargo run --example mercury_1pn`).
  4. **Deterministic outputs.** Fixed seeds, no wall-clock time in results, and documented float reproducibility (no implicit FMA; see the `suboptimal_flops` note).
  5. **Small, explicit modules** with `//!` crate docs stating the conventions: units, frames, time scales, and sign conventions.

---

### 7. Test-vector provenance

No external standard for this was found, so it is entirely *(proposal)*, recorded in ADR-0004. Each vector file is TOML, so Rust can read it with `serde` + `toml` and Python with `tomllib`. A JSON Schema at `tests/vectors/schema.json` is validated in CI (via `tomli` → `jsonschema` in `tools/`).

```toml
# tests/vectors/constants_iau2015.toml
# SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
# SPDX-License-Identifier: Apache-2.0
schema_version = 1

[[vector]]
id = "CONST-GM-SUN-N"
requirement = ["PHY-001"]
quantity = "nominal solar mass parameter GM_sun^N"
value = 1.3271244e20
unit = "m^3 s^-2"
kind = "defined"                 # defined | measured | computed-reference | analytic
source = "IAU 2015 Resolution B3"
citation = "Prša et al. 2016, AJ 152, 41, doi:10.3847/0004-6256/152/2/41"
url = "https://www.iau.org/static/resolutions/IAU2015_English.pdf"
retrieved = "2026-10-14"
tolerance = { abs = 0.0 }        # exact by definition
tolerance_rationale = "Nominal conversion constant, exact by IAU definition."
verified_by = "human:tmorris"    # who opened the source
```

```toml
# tests/vectors/mercury_ladder.toml (illustrative — values to be filled from opened sources)
[[vector]]
id = "MERC-GR-1PN-ANALYTIC"
requirement = ["PHY-012", "ORG-002"]
quantity = "GR perihelion advance of Mercury, 1PN, per Julian century"
value = "TBD"                    # fill only from an opened source; commonly quoted ≈43″/cy — UNVERIFIED exact figure/source
unit = "arcsec / Julian century"
kind = "analytic"
source = "TBD"
tolerance = { rel = 1e-3 }
tolerance_rationale = "Analytic formula vs numerical secular fit; residual dominated by fit window (n orbits)."
generated_by = "tools/ref/mercury_analytic.py@<git-sha>"   # for computed-reference kinds
```

Rules:
1. A test may not contain a bare physical literal. It loads a vector by `id`. Clippy's `PLR2004`-style magic-number policing is not available in Rust, so a `tools/lint_literals.py` grep check fills the gap *(proposal)*.
2. `kind = "computed-reference"` requires a `generated_by` script path with a commit SHA, plus the reference engine and its version (Orekit x.y, REBOUND x.y, integrator settings).
3. Changing `value` or `tolerance` requires the `golden-change` label and a reviewer from CODEOWNERS for `tests/vectors/`.
4. Tolerances state an absolute and/or relative bound **and** a rationale: physical uncertainty, integrator error budget, or float rounding.
5. Long reference runs (century-scale integrations) are `#[ignore]` tests run by the nightly workflow. Results are uploaded as artifacts with a JSON summary that `tools/report.py` turns into the ladder report.

---

### 8. GitHub Actions and repository policy

**Workflows** *(proposal)*. Pin every third-party action by commit SHA, which is what Scorecard's `Pinned-Dependencies` check rewards ([ossf/scorecard](https://github.com/ossf/scorecard)).

| Workflow | Trigger | Jobs |
|---|---|---|
| `ci.yml` | PR, push to `main` | `fmt` (`cargo fmt --all --check`), `clippy` (`--workspace --all-targets --all-features -- -D warnings`), `test` (nextest on Linux/macOS/Windows + `cargo test --doc`), `msrv` (build/test on `rust-version`), `docs` (`RUSTDOCFLAGS=-D warnings cargo doc`), `deny` (cargo-deny-action) |
| `python.yml` | PR touching `tools/**` | `uv sync --locked`, `ruff check`, `ruff format --check`, `mypy`, `pytest` |
| `governance.yml` | PR | `reuse lint`, `tools/trace.py --check`, vector schema validation, commit-message lint (Conventional Commits), PR-title lint |
| `cla.yml` | `pull_request_target`, `issue_comment` | contributor-assistant action |
| `semver.yml` | PR (advisory until M5) | cargo-semver-checks vs last release |
| `nightly.yml` | `schedule` (cron) | `cargo nextest run --run-ignored all` long physics runs, reference comparisons vs Orekit/REBOUND, report artifact, `cargo update` + test (catches upstream breakage) |
| `bench.yml` | push to `main`, manual | criterion/divan benches on a **self-hosted or larger pinned runner**. Shared GitHub runners are noisy, so trends are informational only, not gating (**UNVERIFIED** variance figures). |
| `release-please.yml` | push to `main` | `googleapis/release-please-action@v4`, `release-type: rust` with a manifest config for the workspace. Publishing runs on release creation. |
| `scorecard.yml` | weekly | `ossf/scorecard-action` |

- **release-please:** Conventional Commits map `fix:` → patch, `feat:` → minor, and `!` or a `BREAKING CHANGE:` footer → major. The action moved from `google-github-actions/` to `googleapis/`. It does **not** publish to crates.io itself. Release PRs created with `GITHUB_TOKEN` do not trigger other workflows, so a PAT or app token is needed for CI on the release PR ([release-please-action](https://github.com/googleapis/release-please-action), [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)). Cargo-workspace internal-dependency bumping was not confirmed (**UNVERIFIED**). Evaluate `release-plz`, a Rust-native alternative, in the ADR.
- **Branch protection / rulesets on `main`:** require a PR, 1 approving review (2 for `crit:high` paths via CODEOWNERS), status checks (`fmt`, `clippy`, `test`, `msrv`, `docs`, `deny`, `python`, `governance`, `cla`), linear history (squash merge, with the PR title as a Conventional Commit), no force-push, and signed commits *(optional; UNVERIFIED that it interacts cleanly with agent workflows)*. Agent-authored PRs always need human approval.
- **CODEOWNERS:** `tests/vectors/`, `docs/requirements/`, `docs/adr/`, `LICENSE`, `NOTICE`, and `.github/` are owned by maintainers.

---

### 9. Proposed repository documentation structure

```
FerroAstro/
├── AGENTS.md                  # canonical agent instructions
├── CLAUDE.md                  # @AGENTS.md + Claude-specific notes
├── README.md
├── LICENSE                    # Apache-2.0
├── NOTICE
├── LICENSES/Apache-2.0.txt    # REUSE
├── REUSE.toml
├── CONTRIBUTING.md            # CLA, commit style, PR checklist, provenance rules
├── CODE_OF_CONDUCT.md
├── SECURITY.md
├── CHANGELOG.md               # release-please managed
├── Cargo.toml  rust-toolchain.toml  rustfmt.toml  clippy.toml  deny.toml
├── .pre-commit-config.yaml
├── .github/
│   ├── workflows/{ci,python,governance,cla,semver,nightly,bench,release-please,scorecard}.yml
│   ├── ISSUE_TEMPLATE/{bug,requirement,physics-discrepancy,adr-proposal,agent-task}.yml
│   ├── PULL_REQUEST_TEMPLATE.md
│   ├── CODEOWNERS
│   └── labels.yml
├── docs/
│   ├── strategy/
│   │   ├── MISSION.md         # goal, flagship Mercury ladder, non-goals
│   │   ├── ROADMAP.md         # milestones M0–M5 ↔ requirement IDs
│   │   └── journal/           # these journal entries, archived
│   ├── requirements/
│   │   ├── README.md          # ID scheme, statuses, how to trace
│   │   ├── TAILORING.md       # NPR 7150.2 / ECSS-inspired classification
│   │   ├── ORG.md STD.md PHY.md NUM.md API.md AGT.md
│   │   └── TRACE.md           # generated — do not edit
│   ├── adr/
│   │   ├── adr-template.md    # MADR 4.0.0
│   │   └── 0000-use-madr.md …
│   ├── standards/
│   │   ├── rust.md  python.md  testing.md  provenance.md  errors.md
│   ├── physics/               # conventions: units, frames, time scales, constants
│   └── validation/            # generated ladder reports + evidence
├── crates/
├── tests/vectors/             # *.toml golden values + schema.json
├── tools/                     # Python (uv project): trace.py, report.py, ref/{orekit,rebound}/
└── llms.txt                   # generated from docs (nice-to-have)
```

---

### 10. Open questions for maintainers

1. CLA text and the AI-contribution clause need legal review. Who signs for agent PRs?
2. Units: `uom` versus a home-grown newtype crate (ADR-0001), which affects compile times and error-message readability for agents.
3. Requirement tooling: home-grown `trace.py` versus StrictDoc or OpenFastTrace.
4. Benchmark hardware: whether a self-hosted runner is available.
5. Reference data licensing: JPL DE-series and IERS EOP redistribution in test fixtures.
6. Whether the Mercury ladder target is a reproduction of published numbers or an independent re-derivation with error budgets. This affects how `PHY-` acceptance criteria are written.

---

### Requirements implied

| ID | Requirement | Verification |
|---|---|---|
| **ORG-001** | The project SHALL maintain `docs/strategy/MISSION.md` stating the goal, non-goals, and the flagship deliverable. | Inspection |
| **ORG-002** | The flagship SHALL be the Mercury perihelion precession ladder (Newtonian two-body → planetary perturbations → 1PN GR → ephemeris comparison). Each rung SHALL have an acceptance test with a cited reference value and stated tolerance. | Test (`PHY-*` children) |
| **ORG-003** | Requirements SHALL have stable IDs (`ORG/STD/PHY/NUM/API/AGT-NNN`) in YAML blocks under `docs/requirements/`, with status, criticality, verification method, and acceptance criteria. | CI: `trace.py --check` |
| **ORG-004** | Every `approved` requirement with `verification: test` SHALL be cited by ≥1 test, and every cited ID SHALL exist. `TRACE.md` SHALL be generated, never hand-edited. | CI |
| **ORG-005** | Significant decisions SHALL be recorded as MADR 4.0.0 ADRs in `docs/adr/`, and each ADR SHALL name its Confirmation check where one exists. | Inspection / PR template |
| **ORG-006** | Milestones SHALL map to roadmap rungs and list their requirement IDs. Issues SHALL use the label taxonomy defined in `.github/labels.yml`. | Inspection / label-sync |
| **ORG-007** | The project SHALL document a software classification in `TAILORING.md`, NPR 7150.2 Class D/E-equivalent and ECSS-criticality-inspired, without claiming compliance. | Inspection |
| **ORG-008** | Contributions SHALL require a signed CLA (individual or corporate), checked automatically on every PR. Agent-authored PRs SHALL be attributable to a signing human or org. | CI: `cla.yml` |
| **ORG-009** | Agent-authored PRs SHALL require approval from at least one human maintainer before merge. | Branch ruleset |
| **STD-001** | Rust code SHALL pass `cargo fmt --check` using the committed `rustfmt.toml` and pinned toolchain. | CI |
| **STD-002** | All crates SHALL inherit `[workspace.lints]`, and `cargo clippy --all-targets -- -D warnings` SHALL pass. Lint exceptions SHALL use `#[expect(..., reason = "...")]`. | CI |
| **STD-003** | `unsafe_code` SHALL be forbidden workspace-wide. Any exception requires an ADR. | CI (lint) |
| **STD-004** | All public items SHALL be documented (`missing_docs = "deny"`), and rustdoc SHALL build with `-D warnings`. | CI |
| **STD-005** | The workspace SHALL declare `rust-version` (MSRV) and SHALL be tested on it in CI. MSRV changes follow ADR-0006. | CI |
| **STD-006** | Dependencies SHALL pass `cargo deny check` (licenses, advisories, bans, sources). | CI |
| **STD-007** | Public API changes SHALL be checked with cargo-semver-checks: advisory before M5, gating from M5. | CI |
| **STD-008** | Tests SHALL run under cargo-nextest, with doctests run separately. Long runs SHALL be `#[ignore]` and run nightly. | CI |
| **STD-009** | Python under `tools/` SHALL pass `ruff check` (rule set in §4) and `ruff format --check`, be managed by uv with a committed lockfile, and pass mypy strict. | CI |
| **STD-010** | Every source file SHALL carry SPDX copyright and license information, and the repo SHALL pass `reuse lint`. `LICENSE` and `NOTICE` SHALL be present at the root. | CI |
| **STD-011** | Commits merged to `main` (squash PR titles) SHALL follow Conventional Commits 1.0.0. Releases and CHANGELOG SHALL be produced by release-please (or an ADR-chosen alternative). | CI |
| **STD-012** | Third-party GitHub Actions SHALL be pinned by commit SHA, and an OpenSSF Scorecard SHALL run weekly. | CI / Scorecard |
| **TST-001** | Floating-point comparisons in library and tests SHALL use explicit tolerances (`float_cmp` denied). Lossy casts SHALL be denied unless they carry a `#[expect]` with a reason. | CI (lint) |
| **TST-002** | Every numeric test SHALL load its expected value from `tests/vectors/*.toml` with `source`, `citation`, `retrieved`, `tolerance`, and `tolerance_rationale`. No bare physical literals in tests. | CI: schema + literal check |
| **TST-003** | Computed-reference vectors SHALL record the generating script, commit SHA, and reference engine version and settings. | CI: schema |
| **TST-004** | Changes to golden values or tolerances SHALL carry the `golden-change` label and a CODEOWNERS review. | Branch ruleset |
| **TST-005** | Numerical results SHALL be deterministic across runs on the same platform (fixed seeds, no wall-clock dependence), with documented cross-platform float caveats. | Test |
| **API-001** | Physical quantities, time scales, and reference frames SHALL be encoded in types such that frame or time-scale mismatches fail to compile (design in ADR-0001/0002). | Test (compile-fail tests, e.g. `trybuild`) |
| **API-002** | Library code SHALL NOT panic on invalid input. Errors SHALL be typed enums with stable codes (`FA-Exxx`) and messages naming the quantity, unit, valid range, and remediation. | Test + lint (`unwrap_used`) |
| **API-003** | Each public function SHALL have a doctest example using real cited values, and each ladder rung SHALL have a runnable `examples/` program. | CI (doctests) |
| **AGT-001** | The repo SHALL have a root `AGENTS.md` (canonical) with exact build, test, and lint commands, the provenance rules, and the forbidden actions. `CLAUDE.md` SHALL defer to it. | Inspection / CI link check |
| **AGT-002** | The docs site SHALL publish `llms.txt` (nice-to-have; non-gating). | Inspection |
| **AGT-003** | Issue forms SHALL include an `agent-task` template with scope, files, a done-definition, and commands to verify. | Inspection |
| **AGT-004** | A future MCP server SHALL declare `outputSchema` for each tool and return `structuredContent` conforming to it (MCP spec 2025-06-18 or later). | Test |

---

**Sources consulted:** [agents.md](https://agents.md/) · [llmstxt.org](https://llmstxt.org/) · [RFC 3389 manifest lints](https://rust-lang.github.io/rfcs/3389-manifest-lint.html) · [Clippy lint list](https://rust-lang.github.io/rust-clippy/master/index.html) · [clippy #7666 float_cmp](https://github.com/rust-lang/rust-clippy/issues/7666) · [Rust 1.84.0 release](https://blog.rust-lang.org/2025/01/09/Rust-1.84.0/) · [Edition guide: resolver 3](https://doc.rust-lang.org/edition-guide/rust-2024/cargo-resolver.html) · [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) · [cargo-deny](https://embarkstudios.github.io/cargo-deny/) · [nextest](https://nexte.st/) · [ruff settings](https://docs.astral.sh/ruff/settings/) · [uv](https://docs.astral.sh/uv/) · [MADR](https://adr.github.io/madr/) · [JabRef ADR-0000](https://devdocs.jabref.org/decisions/0000-use-markdown-architectural-decision-records.html) · [NASA SWEHB SWE-020](https://swehb.nasa.gov/x/BoHeB) · [NPR 7150.2D](https://soma.larc.nasa.gov/STP/DYNAMIC/pdf_files/NPR_7150.2D%20SW%20Eng.pdf) · [ECSS-E-ST-40C Rev.1](https://ecss.nl/wp-content/uploads/2025/05/ECSS-E-ST-40C-Rev.1(30April2025).pdf) · [OpenSSF Scorecard](https://github.com/ossf/scorecard) · [ASF licensing how-to](https://infra.apache.org/licensing-howto.html) · [ASF apply-license](https://www.apache.org/legal/apply-license.html) · [ASF CLAs](https://www.apache.org/licenses/contributor-agreements.html) · [REUSE lint](https://reuse.readthedocs.io/en/stable/man/reuse-lint.html) · [cla-assistant](https://github.com/cla-assistant/cla-assistant) · [contributor-assistant action](https://github.com/contributor-assistant/github-action) · [release-please-action](https://github.com/googleapis/release-please-action) · [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) · [MCP tools spec 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/server/tools)
