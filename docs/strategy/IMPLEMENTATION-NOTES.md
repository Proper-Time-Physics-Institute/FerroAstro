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

### WP1 gate results (2026-10-14, local, toolchain 1.95.0)
All eight AGENTS.md commands passed locally, with cargo-nextest 0.9.148 and reuse 6.2.0 installed user-level
(no fallback to `cargo test` was needed). Extras that also passed: `uv run pytest` (13 tests), the MSRV run on
1.85.0 (16 tests, trybuild excluded), and the `data` profile (1 test, real download). GitHub Actions run
38675095723 on PR #11 was green for rust, msrv, python, reuse and dco. The `data` job was skipped as designed.
It cannot be dispatched by hand until `ci.yml` exists on the default branch, because `workflow_dispatch` reads
the workflow from there.

### WP1 adversarial review (2026-10-14)
- Rule 8 / OI-05: `CONST-AU-IAU2012` cited IAU 2012 Resolution B2, whose text OPEN-ITEMS lists as UNVERIFIED, and
  its only cross-check was header.440 — the very file the `data` test checks, which made the oracle circular.
  The reviewer opened the Resolution text (SYRTE mirror `Res_IAU2012_B2.pdf`, sha256 `3489ebb1…7afec984`;
  the iau.org `static/resolutions` URL now returns 404): "recommends 1. that the astronomical unit be re-defined
  to be a conventional unit of length equal to 149 597 870 700 m exactly". The vector's `verified_by` now records
  this. Value and tolerance are unchanged. OI-05 can be closed in the requirements PR (#2).
- Mutation testing: replacing `abs.max(rel)` with `abs.min(rel)` in `Tolerance::allowed` survived the whole suite
  (the fixture had `rel = 0`). Added `tolerance_is_the_wider_of_abs_and_rel`, which kills that mutant and also
  checks that NaN never passes. A perturbed `CONST-CLIGHT-SI` (+1e-4 km/s, via `FA_VECTORS_DIR`) made the
  `data` test fail as expected.

## WP2 — DE440 constants from the real header (PHY-002, REF-002, PHY-003)

Crate `crates/ferroastro-constants` (new; ARCHITECTURE.md name). Error codes `FA-E0300` (header malformed),
`FA-E0301` (constant missing), `FA-E0302` (table malformed) added to `ferroastro-core`.

### Parser and loading mechanism (REF-002)
- `header440::Header::parse` reads GROUP 1010 (title), 1040 (count + names) and 1041 (count + values). Each
  value is kept as its verbatim Fortran `D` token and as the nearest `f64`. Counts must agree with each other
  and with the tokens present; duplicate names and non-finite tokens are errors.
- REF-002's run-time path ("load ICs and constants directly from header.440") is this parser applied to the
  downloaded file: all 645 values are available by name. The **states** (`X1` … `ZDC`) are not copied into the
  committed table and have no typed accessor, because their frame (CENTER) and units are OI-23. REF-002 stays
  blocked on OI-23 for the states (red-team I5); only the scalar part of its acceptance is tested.
- The WP1 smoke reader in `ferroastro-testdata/tests/data_de440_header.rs` is kept on purpose as a second,
  independent reader of the same groups (it agrees: 645 names, CLIGHT and AU exact).

### Versioned constants table (PHY-002)
- `crates/ferroastro-constants/data/de440-constants.toml` is **generated** by
  `cargo run -p ferroastro-constants --example gen_de440_table`, which fetches the pinned product, parses it
  and calls `table::render`. It carries `table_version = 1`, the header's URL, SHA-256, size, retrieval date and
  Last-Modified, the GROUP 1010 title and the counts (645 constants, 409 `MA*`). It is embedded with
  `include_str!` and parsed at run time by `Table::de440()` / `De440::load()`.
- Only a subset is committed (23 entries: DENUM, JDEPOC, CENTER, CLIGHT, BETA, GAMMA, AU, EMRAT, GM1, GM2, GMB,
  GM4–GM9, GMS, GMSDOT, ASUN, J2SUN, MOISUN, OMGSUN). This is a small derived table, not the JPL file, in line
  with the "do not commit JPL data" rule while redistribution terms are open. **NEEDS HUMAN REVIEW:** whether
  committing 23 verbatim tokens of header.440 is acceptable under JPL's terms (ROADMAP legal track).
- The header states no units. Units and their basis are metadata in `table::DE440_SELECTION` (strings only):
  GMs au³/d² (because converting with AU and 86400 s/d reproduces Park21 Table 2 in km³/s², which the tests
  check), CLIGHT km/s and AU km (equal the SI / IAU 2012 defined values), ASUN km and MOISUN from Park21 §3.4,
  OMGSUN rad/d (Park21 §3.4 quotes 14.1844 deg/day; the header value converts to 14.1843972 deg/day; a test
  checks it), JDEPOC as a Julian date on TDB.
- Citation keys are `DE440hdr` (header URL + SHA-256) and `Park21` (AJ 161, 105). Every entry has ≥ 1.
- Staleness: an offline unit test asserts that each entry's unit, quantity, unit basis and citations equal the
  generator selection, so a hand edit of metadata fails PR CI. Exactness of the **tokens** is enforced by the
  `data` test (entry-by-entry token and `f64` equality, plus a byte-for-byte re-render). Offline, GMS, EMRAT,
  J2SUN, MOISUN, ASUN, JDEPOC, BETA, GAMMA, CLIGHT and AU are pinned exactly, but the planetary GMs only to the
  6 decimals Park21 prints (about 2e-11 relative for Mercury). Mutation check: a GM1 token edited in the 11th
  significant digit passes the offline tests and fails `data_de440_table_matches_header`, as expected.
- `De440` refuses (FA-E0302) to expose any entry with an `open_item`, and checks the unit string of every entry it
  reads. `Body::Earth`/`Body::Moon` are derived as GMB·EMRAT/(1+EMRAT) and GMB/(1+EMRAT).
- `AngularRate` (rad/s) was added to `ferroastro-core` for OMGSUN, and `time::SECONDS_PER_DAY` was made public.

### Oracles and tolerances (tests/vectors/de440-constants.toml)
- Independent oracle: Park et al. 2021, read in the published PDF (sha256 `96c3945d…1634f68ee0`): Table 2
  (GMSun and the planetary/system GMs, Earth and Moon), §3.4 after Eq. 39 (696,000 km, C/MR² = 0.06884,
  14.1844 deg/day). Header-only values (GMS in au³/d², EMRAT, J2SUN, JDEPOC, BETA, GAMMA, counts) are pinned
  from header.440 / the requirement text. These are regression pins, not independent checks; their vectors say so.
- `DE440-GMS-KM3S2` tolerance `rel = 1e-15`: the 18-digit au³/d² header value converted exactly gives
  132712440041.2794478 km³/s², 2.2e-16 relative from Park21's 132712440041.279419; f64 conversion adds a few
  roundings. Measured |diff| = 0 in f64.
- Planetary GMs tolerance `abs = 5e-7 km³/s²` = half a unit in Park21's 6th printed decimal. Measured diffs:
  Moon 4.58e-7, Mercury 4.00e-7, Neptune 3.99e-7, Mars 2.44e-7, Earth 2.3e-8, Jupiter 1.5e-8, Uranus 1.9e-9,
  Venus/Saturn/Pluto 0. These are the rounding of Park21's printed values, not FerroAstro error. The Moon sits at
  92 % of the tolerance; that is the arithmetic of rounding, not a fit.
- `DE440-OMGSUN-RADD` `abs = 5e-13 rad/d` (PHY-002 quotes 12 decimals; measured 2.9e-15).
  `PARK21-OMGSUN-DEGD` `abs = 5e-5 deg/d` (Park21 prints 4 decimals; measured 2.8e-6).
- ADR-0004 has no kind for "a fact read from a data product" (the 645 and 409 counts); they use `measured`
  with a note. BETA/GAMMA use `defined` (DE440 adopts GR).
- Rule 8: GMSDOT (OI-04) and CENTER (OI-23) appear in the table with `open_item` and `unit = "unverified"`
  (GMSDOT), have no vector and are not asserted. OI-06 (GM☉ uncertainty / J2 rationale in Park21) was not
  needed: no tolerance relies on it. Red-team I5 still holds: PHY-002 is satisfied for the listed constants, but
  REF-002's state-vector part remains blocked on OI-23.
- Interpretation: PHY-002 lists the planetary GMs without asserted values; WP2 asserts them anyway against
  Park21 Table 2 (as quoted in PHY-002's notes, re-read in the PDF).

### PHY-003 nominal namespace
- `ferroastro_constants::nominal`: (GM)☉ᴺ, R☉ᴺ, (GM)ᴇᴺ, (GM)ᴊᴺ from Prša et al. 2016 Table 1 (read in
  arXiv:1605.09788v1, sha256 `2b7bcd88…a6f4a99`), stored in km units (decimal-exact) and returned as
  `Nominal<T>`, which has no `Deref`, `From` or arithmetic. Vectors: `tests/vectors/iau2015-nominal.toml`.
  These four values are hand-typed in `nominal.rs`; REF-002's "never hand-transcribed" applies to header
  constants, and these are defined conversion constants checked against the cited table.
- "No ladder-rung code path references `nominal`" is enforced by `tests/nominal_quarantine.rs`: it scans all
  Rust files under `crates/` (except `nominal.rs`, the crate's `lib.rs` and the test itself), strips comments and
  strings, and fails on any identifier `nominal`/`Nominal`. No ladder crate exists yet, so today the test guards
  future code. It is a lexical scan: it cannot see a macro that builds the path from tokens, and it flags a
  local variable named exactly `nominal` (conservative). A self-test shows it catches `use …::nominal`,
  grouped imports, spaced paths and the wrapper type, and ignores comments, strings and `nominal_value`.

### Fixes to WP1 infrastructure
- `ferroastro-testdata`: two nextest processes downloading the same product into an empty cache raced on
  `<file>.part` (ENOENT on rename, seen as a flaky first try). Temp files now carry the process id.

### WP2 real-data results (run locally 2026-10-14, UTC date 2026-10-15)
`FA_DATA_DIR` pointed at an empty scratch directory, so all three `data` tests downloaded header.440 (cache
miss): sha256 `0f4636b663e6f00af9efa2b605ef23635489f56b23dd1a5e3821d3acc6ac3359`, 22802 bytes, Last-Modified
`Tue, 22 Dec 2020 00:51:11 GMT`, URL https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/header.440.
- `data_de440_table_matches_header`: 23/23 entries equal (token and f64), re-render byte-identical. PASS.
- `data_de440_header_ref002_acceptance`: JDEPOC 2440400.5, CLIGHT 299792.458, BETA 1, GAMMA 1, AU 149597870.7,
  J2SUN 2.1961391516529825e-7, ASUN 696000, GMS 2.95912208284119561e-4, EMRAT 81.3005682214972154 all |diff| = 0;
  645 constants, 409 MA entries. PASS.
- `data_de440_header_defined_constants` (WP1): PASS.

## WP3 — IAS15 integrator, Newtonian two-body, analytic GR (PHY-001, PHY-004, NUM-001 subset, NUM-023)

### Plan and pre-registered checks (written before any run)
- New crates (ARCHITECTURE.md names): `ferroastro-integrators` (IAS15-class Gauss–Radau, clean-room from
  Everhart 1985 and Rein & Spiegel 2015 — no REBOUND/REBOUNDx/ASSIST/Swiftest source was opened),
  `ferroastro-forces` (Newtonian point masses), `ferroastro-precession` (Kepler elements, LRL-vector
  perihelion longitude, secular slope fit, analytic Schwarzschild advance).
- Oracles fixed before running anything, so no tolerance is chosen after seeing a result:
  - PHY-001: |fitted ϖ̇| < 1e-6 ″/cy over ≥ 1000 yr (the requirement's own bound). If IAS15 does not reach
    it, the achieved value is reported and the test is **not** loosened without a NEEDS HUMAN REVIEW note.
  - PHY-004: 42.9805 ± 0.0001 ″/cy, 5.01866e-7 rad/orbit (± half a unit in the 6th significant digit) and
    415.2009 orbits/cy (± half a unit in the 4th decimal), from DE440 GM☉, c, au and SW92 a, e.
  - NUM-023: Brouwer slopes ½ (energy) and 3/2 (phase / along-track), each ± 0.1 (requirement).
  - Real data (Horizons, DE441): (a) my state→elements conversion of the Horizons heliocentric state must
    reproduce Horizons' own osculating EC and A with the same GM (tolerance set by the 16 printed digits and
    f64 round-off, ≤ 1e-12 relative); (b) the analytic GR rate from **osculating** J2000 a, e is a diagnostic
    compared with Park et al. 2017 Table 3 (42.9799 ± 0.0009 ″/cy) at 3σ (± 0.0027): single-epoch
    osculating elements carry short-period perturbations, and red-team §1 labels the mean-vs-osculating
    explanation of the 0.0006 gap a hypothesis, so this is not a 1σ test; (c) PHY-001 repeated with the
    Horizons J2000 state as initial condition.
