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

### Clean-room record (DEP-008)
- Sources used: Everhart 1985 (as described in Rein & Spiegel 2015 §2), and Rein & Spiegel 2015 itself
  (arXiv:1409.4779 PDF, sha256 `8097c8e5…7e69033d`, read for §2–3: PC loop, δb₆ < 1e-16 and oscillation exit,
  12-iteration cap, Eq. 11 step control, compensated summation, extended-precision constants). No REBOUND,
  REBOUNDx, ASSIST or Swiftest source was opened. The Gauss–Radau spacings are **computed** (roots of P₇+P₈ in
  double-double, rounded once; they equal a 50-digit mpmath evaluation to the last bit), not transcribed.

### Integrator interpretations (`ferroastro-integrators`)
- **IAS15 shipped, not DOP853.** The full IAS15-class scheme fit in this WP, so no DOP853 fallback was written.
- Notation: `B₁…B₇` = IAS15's `b₀…b₆`; the step criterion uses `max|B₇|/max|a|` over all coordinates and all
  node accelerations of the last iteration (R&S "global" estimate, Eq. 8/11), ε = 1e-9, safety 0.25 (reject if
  the proposed step is < 0.25·Δt, cap growth at 4×). The growth cap and the 0.25 threshold are this WP's reading
  of the paper ("If dt_trial > dt_required the step is rejected"); R&S reject whenever dt_required < dt_trial.
  **NEEDS HUMAN REVIEW:** whether to adopt R&S's stricter rejection rule; with ours, steps are rarely rejected.
- PC loop: Gauss–Seidel order (gₙ updated as soon as aₙ is known); stops when `max|ΔB₇|/max|a| ≤ 1e-16`
  (at least 2 iterations, because with a zero predictor B₇ can look converged after one pass), when the
  correction stops decreasing after the 2nd iteration (R&S "begins to oscillate"), or at 12 iterations
  (counted in `Ias15Stats::unconverged`; 0 in every run reported here). Stalls at the B₇ round-off floor
  (~1e-12·a, from the divided differences) happen on ~0.06 % of steps and are counted.
- Predictor for the next step: plain polynomial extrapolation `B'ₖ = qᵏ Σ C(m,k) Bₘ`; Everhart's additional
  correction term (eₖ) is **not** implemented. Result: ~3 PC iterations/step (R&S report ~2). Rejected steps
  rescale B by `qᵐ`.
- **Bug found and fixed by NUM-023:** after a step shortened to land on a requested time, extrapolating that
  tiny step's polynomial by a huge factor produced garbage predictors and rare energy jumps (up to 3.5e-10 in
  5 of 32 runs). Fix: restart the predictor from zero when |q| > 1/safety (`Ias15Stats::cold_starts`).
- `Ias15` is `f64`-only (no `Real` trait yet, as in WP1). It integrates raw slices; typed quantities live at the
  force-model boundary (`ferroastro-forces` packs/unpacks `State<F, C>`; internal units km, km/s, TDB s).
- The Kozai–Lidov test of NUM-001's acceptance is **not** implemented (needs a 3-body set-up); only the
  Kepler test (e = 0.5, 0.9, 100 orbits) plus velocity-dependent damped-oscillator and gyration tests are.

### PHY-001 (rung N-2B)
- Set-up: barycentric Sun + Mercury, DE440 GM☉ and GM☿, SW92 J2000 mean elements turned into an osculating
  heliocentric state with μ = GM☉ + GM☿ (caveat C7: documented in the test output), frame "mean ecliptic and
  equinox of J2000" (SW92). Estimator: LRL eccentricity-vector longitude in the **initial osculating orbit
  plane** (zero at the initial periapsis), sampled after every accepted step, straight-line OLS fit. This is not
  the Park-2017 estimator (ADR-0011), which is not needed for an exactly-zero rung.
- Result (PR tier, 2026-10-15): fitted ϖ̇ = **1.35e-10 ″/cy** over 1000 Julian yr (317 336 steps,
  6.99e6 force evaluations, 0 rejected, 0 unconverged, 1.96 s at opt-level 1) — 7400× below the 1e-6 bound,
  which IAS15 therefore reaches; red-team §1's concern applies to DOP853/WH, not to this integrator. The output
  labels the residual as numerical error. From the Horizons DE441 state (data tier): 1.09e-9 ″/cy.
- The formal slope error (4.5e-13 ″/cy) understates the uncertainty: round-off residuals are correlated.

### PHY-004 (rung GR-SCHW analytic)
- `6πGM/(c²a(1−e²))` with GM = GM☉ (statement), period from μ = GM☉ + GM☿ (acceptance), a = 0.38709927 au ×
  DE440 AU, e = 0.20563593, Julian century 36 525 d × 86 400 s.
- Result: **42.980478977 ″/cy** (oracle 42.9805 ± 0.0001, |diff| 2.1e-5), 5.0186604e-7 rad/orbit (|diff| 4.4e-14),
  415.200918 orbits/cy (|diff| 1.8e-5). The C7 alternative (GM☉ + GM☿ in the advance too) gives 42.980486
  (shift 7.1e-6) and also passes. Minus Park17 Table 3 (42.9799): +0.0006 ″/cy, as journal 01 says.
- Park17 Table 3 was read in the published PDF (sha256 `24fb2153…2433380b`): GE 42.9799 ± 0.0009.

### NUM-023 (Brouwer-law diagnostics) — NEEDS HUMAN REVIEW
- Method: ensemble of Sun + Mercury runs differing only in initial mean anomaly; RMS over the ensemble of
  relative energy error, relative |L| error and mean-longitude error (vs the exact Kepler motion of each run's
  own initial osculating orbit) at 24 log-spaced checkpoints from 10 orbits; log-log OLS slope. Default size
  (fixed in code before the first run): 64 runs × 2000 orbits. Nightly tier (`nightly_*`, `#[ignore]`).
- Default result (release, 9.78e6 steps, 4.5 s on 32 threads): energy slope **0.526**, phase **1.513**,
  angular momentum 0.479 — both asserted slopes within ± 0.1. PASS.
- **But longer runs show a bias.** Exploratory 32 runs × 20 000 orbits (4.9e7 steps): energy slope **0.63**
  (fails ± 0.1), phase 1.59 (passes, barely), and the ensemble *mean* energy error equals its RMS
  (−4.6e-14 at 2·10⁴ orbits, i.e. a systematic drift of ≈ 3e-20 relative per step), so the late-time growth is
  linear, not Brouwer. Diagnosis so far: not the clamped-step bug (fixed), not the constants (now correctly
  rounded; no change), not PC convergence (pc_tolerance = 0 changes nothing: steps reach an exact fixed
  point). It depends on ε non-monotonically: mean drift at 2·10⁴ orbits is +5.8e-14 (ε 1e-7), −4.6e-14
  (1e-9), −6.9e-14 (1e-10), −9.4e-15 (1e-11, where both slopes pass: 0.456 / 1.414). NUM-001 asks for Brouwer
  behaviour over ≥ 1e9 steps; this WP demonstrates it only up to ~1e7 steps per ensemble and **does not meet the
  1e9-step clause**. The default nightly size was not changed after seeing these results, and no tolerance was
  touched. Candidate causes to investigate: the missing Everhart predictor correction, the B₇ round-off floor at
  stalls, and the force evaluated at the rounded `x` instead of `x − compensation`.
- Forward–backward reversibility (reported only; NUM-023 gives no number): 1000 orbits forward and back,
  |Δr|/|r₀| = 4.5e-12 (|Δr| = 3.1e-4 km).
- Exploration knobs (env): `FA_NUM023_RUNS`, `FA_NUM023_ORBITS`, `FA_NUM023_EPSILON`, `FA_NUM023_PC_TOL`.

### Real-data results (data tier, run locally 2026-10-15 UTC with an empty `FA_DATA_DIR`)
| test | measured | oracle / tolerance | result | provenance |
|---|---|---|---|---|
| `data_sw92_table1_mercury` | Table 1 Mercury row a, e, I, L, ϖ, Ω | vectors, exact | PASS (all 6 equal) | approx_pos.html sha256 `9f7b30ca…f9c4d742`, Last-Modified 2026-10-06 |
| `data_horizons_osculating_elements_match` | e rel. diff 1.08e-15, a 1.29e-16; angles ≤ 3e-14° | `HORIZONS-ELEMENTS-AGREEMENT` ≤ 1e-12 | PASS | Horizons API v1.2, DE441, ELEMENTS sha256 `f21a8758…4897a31f91`, VECTORS sha256 `a4557352…b19efe4` |
| `data_horizons_gr_rate_from_osculating_elements_vs_park17` | 42.980669 ″/cy (osculating a = 57 909 068.29 km, e = 0.20563029) | Park17 42.9799, ± 0.0027 (3σ, pre-registered) | PASS (+0.00077, 0.85σ) | as above |
| `data_horizons_n2b_from_de441_state` | 1.09e-9 ″/cy over 1000 yr | `PHY001-NEWTON-RATE` < 1e-6 | PASS | VECTORS sha256 `e04b750d…ebb6357e` |
- Horizons answers embed a request timestamp, so two downloads of the same query differ in SHA-256 (seen above);
  the numeric rows are identical. The Keplerian GM Horizons uses is 132 712 462 073.148 km³/s² = GM☉ + GM☿.
- The +0.00077 ″/cy of the osculating-element rate vs Park17 is larger than the +0.0006 of the mean SW92 elements:
  single-epoch osculating a, e are not Park's estimator. This neither confirms nor refutes the "mean vs osculating"
  hypothesis that red-team §1 flags as unverified.

### Benchmark (Criterion, `cargo bench -p ferroastro-precession --bench two_body`, release, 2026-10-15)
| benchmark | median |
|---|---|
| IAS15 Sun + Mercury, 1 Julian yr (~317 steps) | 1.97 ms |
| IAS15 Sun + Mercury, 100 Julian yr | 196 ms |
| one Newtonian two-body force evaluation | 11.3 ns |
- ≈ 6 µs per step for 6 coordinates, ~22 force evaluations per step: the cost is the scheme's bookkeeping, not
  the force. Not optimised. Machine: 32 logical cores, shared with other work (numbers are indicative).
- `criterion` is 0.5.1 with default features off: 0.8.x needs Rust 1.86, above the 1.85 MSRV.

### CI and tiers
- New nextest profile `nightly` (filter `nightly_*`) and a scheduled/dispatch-only CI job `long` running it in
  release mode. AGENTS.md's command list is unchanged (it is a human-owned meta surface).
- New vectors: `mercury-sw92.toml`, `phy004-gr-analytic.toml`, `integrator-checks.toml`. New defined constants in
  `ferroastro-core::time`: `DAYS_PER_JULIAN_YEAR`, `DAYS_PER_JULIAN_CENTURY`, `J2000_JD`, `Epoch::j2000()`.

### WP3 gate results (2026-10-15, local, toolchain 1.95.0)
All eight AGENTS.md commands passed: fmt, clippy `-D warnings`, `cargo nextest run --workspace` (53 passed,
9 skipped = the `data_*`/`nightly_*` tiers) and the doctests, `cargo deny check`, rustdoc `-D warnings`, ruff
check/format + mypy, `reuse lint` (138/138 files), `trace.py --check` (119 requirements, 91 citing tests).
Extras: `uv run pytest` (13 passed), MSRV 1.85.0 nextest (52 passed), `data` profile (7 passed, fresh downloads),
`nightly` profile in release (2 passed).

## WP4 — Relativistic single-body rungs (PHY-005…011, PHY-015, NUM-015, NUM-017)

### Plan and pre-registered checks (written 2026-10-15, before any WP4 run)
- Set-up for every single-body rung: a **heliocentric test particle** in the field of a fixed Sun, one
  gravitational parameter `μ = GM☉` (DE440) for the Newtonian term **and** every correction, initial state from the
  SW92 J2000 mean elements of Mercury converted with that `μ`. This is the test-particle limit the analytic
  formulas assume; the O(m☿/M☉) ≈ 1.7e-7 relative two-body corrections are not modelled (they are 400× below the
  tightest ″/cy tolerance used here). Caveat C7 (which μ) is therefore resolved as "GM☉ everywhere" for these rungs.
- Estimator (new, `ferroastro-precession::passages`): **perihelion passages**, i.e. the roots of `r·v = 0` with
  `r·v` rising, refined by Newton iterations on short re-integrations from the last accepted step; the
  perihelion longitude is the angle of `r` at the passage, measured in the initial osculating orbit plane from the
  initial periapsis direction. The per-orbit advance is the OLS slope of longitude against passage index; the
  anomalistic period is the OLS slope of passage time against index; ″/cy = per-orbit × (Julian century /
  anomalistic period). This is the geometric part of the Park-2017 observable without the mean-orbit frame and
  periodic terms (not needed for one body). The WP3 LRL estimator is kept and reported as a cross-check.
- Span: 1000 anomalistic orbits (≈ 241 yr) per run (NUM-015 "averaged over 10³ orbits").
- Oracles and tolerances, fixed now:
  | check | oracle | tolerance | source |
  |---|---|---|---|
  | PHY-005 numerical GR (β = γ = 1) | 42.980 ″/cy | ± 0.001; and ≤ 0.001 from PHY-004's analytic value | PHY-005 |
  | PHY-006 ratio rate(β,γ)/rate(1,1) at (β,γ) ∈ {(1±1e-3, 1), (1, 1±1e-3), (1+1e-3, 1−1e-3)} | (2+2γ−β)/3 | 1e-6 relative | PHY-006 as corrected by red-team §1 (differential, identical ICs) |
  | PHY-006 example shifts | −0.00056 ″/cy at β−1 = 3.9e-5; +0.00066 at γ−1 = 2.3e-5 | ± 5e-6 (half a unit in the printed digit) | PHY-006 statement and notes |
  | PHY-007 SR-a numerical | 7.1634 ″/cy | ± 0.001; analytic πGM/(c²p) with PHY-004 inputs within the same ± 0.001 | PHY-007 |
  | NUM-015 R2 exact | 2π[(1−μ²/c²h²)^(−1/2) − 1], h = γ‖r×v‖ (conserved) | per-orbit, 1e-6 relative | NUM-015 |
  | PHY-008 SR-b | 50.144 (harmonic), 35.817 (areal) ″/cy | ± 0.002 each; output must carry a gauge label | PHY-008 |
  | PHY-009 per coefficient | 8A, 4B, 2C, 2D × πμ/(c²p) per orbit (A = 1 → 8; A = 1/8 → 1) | 1e-3 relative | PHY-009 as corrected by red-team §1 |
  | PHY-010 analytic | 0.02860 (J2 = 2.25e-7), 0.02792 (DE440 J2) ″/cy | ± 5e-6 | PHY-010 |
  | PHY-010 numerical, **equatorial** orbit (pole = orbit normal) | analytic 3πJ₂R☉²/p² per orbit | ± 0.0005 ″/cy (PHY-010) and NUM-015 (below) | PHY-010, red-team §1 |
  | PHY-010 inclined (pole tilted 3.4°) | Park17 Eq. 3: ϖ̇ = (3/2) n J₂ (R/a)² (1−e²)⁻² (1 − (3/2) sin² i) | NUM-015 (below) | Park17 Eq. 3 (read in the PDF), red-team §1 |
  | PHY-011 LT, equatorial | −0.0020 ″/cy numerical; analytic −0.00202 | ± 0.0002 numerical; ± 5e-6 analytic | PHY-011; Park17 Eq. 2 |
  | NUM-015 R1, R2, R3, R3b | analytic per-orbit advance | pass iff ‖Δ‖ ≤ 1e-6 relative **or** ‖Δ‖ ≤ 1.17e-15 rad/orbit (= 1e-7 ″/cy at 415.2 orbits/cy) | NUM-015 with the red-team §1 absolute floor |
- Real data (data tier, pre-registered):
  - D1: GR rung from the Horizons DE441 J2000 heliocentric state of Mercury; diagnostic vs Park17 Table 3 GE
    42.9799 at 3σ (± 0.0027), as WP3 pre-registered for osculating elements; and the numerical rung must agree with
    the analytic rate from the same osculating elements within PHY-005's 0.001.
  - D2: J2 rung (Park17 J2 = 2.25e-7) from the Horizons DE441 Mercury state in the **IAU_SUN body-equator frame**
    (real solar pole, real inclination); numerical in-plane ϖ̇ vs Park17 Table 3 "Solar Oblateness" 0.0286 ±
    0.0011 (Park's 1σ) and vs Park17 Eq. 3 at the real inclination (NUM-015 tolerance).
  - D3: LT rung with GS from a freshly downloaded header.440 (GMS·MOISUN·ASUN²·OMGSUN) in the same real geometry;
    numerical vs Park17 Table 3 "Lense–Thirring" −0.0020 ± 0.0002 (Park's 1σ) and vs Park17 Eq. 2 (NUM-015).
- If any of these fails, the measured value is reported and nothing is loosened without a NEEDS HUMAN REVIEW note.

### What was built
- `ferroastro-forces`: `TestParticle<F>` (fixed central body) with composable `CentralTerm`s, each declaring its
  gauge: `SrKepler` (R2), `SchwarzschildPpn` (R3), `PnLagrangian` (PHY-009 general form; constructors `ppn`,
  `sr_b_harmonic`, `sr_b_areal`, `schwarzschild_areal`, `generic`), `SolarJ2` (R1, configurable `Pole`),
  `LenseThirring` (R3b). Combining two different relativistic gauges is `FA-E0502`.
- `tools/pn_lagrangian.py` (sympy): derives A, B, C, D from `−c² dτ/dt` of the PPN (isotropic) and areal
  Schwarzschild metrics (1/8, γ+½, ½−β, 0 and 1/8, ½, ½, 1), checks the reduced Euler–Lagrange closed form
  `δa = (εμ/r²)[((4A−B−2D)v² + 2(B−C+D)U + 3Dṙ²) n + (8A+2B) ṙ v]` used in Rust, and that its PPN case equals
  Journal #4 R3. A negative control (B + ½) is detected. A Rust unit test checks the two independent 1PN
  implementations (`PnLagrangian::ppn` vs `SchwarzschildPpn`) agree to ≤ 64 ulp at 36 states × 3 (β, γ).
- `ferroastro-precession`: `passages` (r·v = 0 events, Newton-refined; per-orbit OLS fit), analytic oracles
  (`ppn_scaling`, `pn_unit`, `sr_exact_advance`, `j2_planar_advance`, `j2_inclined_rate` = Park17 Eq. 3,
  `lense_thirring_rate` = Park17 Eq. 2, `keplerian_period`) and `report::RateReport`.
- `ferroastro-ladder` (new): cited scenario `data/mercury-single-body.toml`, rung catalogue, `SingleBody` runner.
- `ferroastro-cli` (new, binary `ferroastro`): `ferroastro ladder --rungs single-body [--orbits N] [--details]
  [--no-check]`. Hand-written argument parsing (no `clap`, to keep the dependency graph unchanged).
- `ferroastro-core`: `SpinGm` (G·S, km⁵/s³), frame marker `SunEquatorIau` (Horizons `IAU_SUN`), codes
  FA-E0502, FA-E0602, FA-E0700, FA-E0701.

### Interpretations
- **C7 / which μ.** All single-body rungs use `μ = GM☉` for the Newtonian term and every correction, and build the
  initial state with that μ (test-particle limit, matching the analytic formulas). PHY-005's comparison with the
  PHY-004 analytic value uses PHY-004's own convention (GM☉ advance, GM☉+GM☿ period). The numerical rate
  uses the measured anomalistic period; the "analytic" column of the CLI uses the Keplerian period of the
  initial osculating orbit with μ = GM☉ (42.9804754 vs PHY-004's 42.9804790; the 8.3e-8 relative difference
  is the period's √(1 + m☿/M☉)).
- **Estimator.** Perihelion passages, not the WP3 LRL vector, because the Newtonian LRL vector carries 1PN
  periodic terms and step-time sampling is non-uniform. The LRL rate is still reported as a cross-check: for
  GR-SCHW it is 42.980487 vs 42.980469 from passages (4e-7 relative); for J2/LT they agree to ≤ 6e-7 relative.
- **PHY-006** asserts the ratio rate(β,γ)/rate(1,1) against (2+2γ−β)/3 (red-team fix), not
  `42.98 × (2+2γ−β)/3`. The γ = 1 + 2.3e-5 test point comes from PHY-006's notes ("Cassini σ_γ"); the Cassini
  source was **not** re-read in WP4, so it is used only as an input, never as an oracle.
- **PHY-008 / NUM-017 exemption.** NUM-017 forbids standard-Schwarzschild-coordinate EOMs except the labelled
  exact-geodesic oracle; PHY-008 requires an areal-gauge SR-b rung. Resolved as an explicitly labelled
  pedagogical gauge (`Gauge::SchwarzschildAreal`, label "… [pedagogical only, NUM-017 exemption]") that cannot be
  combined with harmonic-gauge terms. **NEEDS HUMAN REVIEW:** amend NUM-017's text to name this exemption.
- **PHY-009** tests 8, 4, 2, 2 per unit of A, B, C, D and 1 at A = 1/8 (red-team correction); the requirement
  text still says "1, 4, 2, 2". **NEEDS HUMAN REVIEW:** fix the PHY-009 / journal 01 §5 table wording.
- **PHY-010 geometry.** The planar oracle is tested with the pole equal to the initial orbit normal (strictly
  equatorial; inclination < 1e-12 rad asserted). The inclined check uses **Park17 Eq. 3** (read in the PDF), the
  in-plane rate `(3/2) n J₂ (R/a)² (1−e²)⁻² (1 − (3/2) sin² i)`, at Park17's 3.4° (offline) and at the real
  IAU solar pole from Horizons (data tier, i = 3.381087°). This gives a verified citation for the combination
  ω̇ + Ω̇ cos i that OI-37 asks about; the separate ω̇ and Ω̇ formulas remain uncited and are **not** asserted.
- **PHY-011 / OI-35.** The R3b (IERS-style) acceleration is implemented. PHY-011 says the rung "SHALL use the
  INPOP21a Eq. 1 acceleration"; INPOP21a (IMCCE note S110) could not be retrieved in this session (the IMCCE URL
  returned 404 and arXiv has no INPOP21a paper), so equivalence with Eq. 1 is **not** demonstrated. Evidence
  instead: the numerical R3b rate reproduces Park17 Eq. 2 (sign, (1+γ) factor and cos i) to 1.25e-6 relative
  (equatorial) and 7.7e-8 (real geometry). **NEEDS HUMAN REVIEW:** OI-35 stays open.
- **NUM-015 absolute floor (1.17e-15 rad/orbit = 1e-7 ″/cy).** It is a red-team recommendation, not yet
  requirement text. **NEEDS HUMAN REVIEW.** It decides one check: LT (equatorial) has a relative per-orbit
  deviation of **1.25e-6** (> 1e-6) and passes only through the floor (absolute 2.9e-17 rad/orbit). The measured
  noise floor of the estimator on a Newtonian test particle is 4.5e-17 rad/orbit (formal σ 2.8e-17), so the LT
  deviation is round-off, and the floor sits 26× above the measured noise. In the real geometry LT passes the
  relative bound directly (7.7e-8).
- **NUM-017 acceptance** (the R5 → R3 EIH limit) needs EIH, which does not exist yet; it is **not** met by this WP.
  Delivered for NUM-017: every 1PN EOM is in PPN isotropic/harmonic gauge (asserted per rung), the symbolic
  derivation, and the two-implementation cross-check.
- **NUM-016** (inspection): the Hamiltonian and the model-dependence statement are in the `SrKepler` rustdoc,
  the `ferroastro-ladder` crate docs and the CLI footer; a test checks the rung's dynamics string names
  `H = gamma m c^2 - mu m/r`.
- **PHY-015** is enforced by construction: `RateReportBuilder::build` returns FA-E0602 for a missing or blank
  frame, epoch, fit span, fit model, constants version, gauge, estimator, dynamics or rung name. The constants
  string always contains the header.440 SHA-256.
- **Park17 facts read in the PDF** (sha256 `24fb2153…2433380b`): Eq. 2 (LT, with (1+γ) and cos i), Eq. 3 (J2,
  inclined), S = 190 × 10³⁹ kg m² s⁻¹, Table 1 total J2 = 2.25 ± 0.09 × 10⁻⁷ and (β−1) = −2.6 ± 3.9 × 10⁻⁵,
  while the **abstract says (β−1) = −2.7 ± 3.9 × 10⁻⁵** (an inconsistency inside Park17; only σ = 3.9e-5 is used
  here). OI-11: INPOP21a's "2.28 ± 0.06" is Park17's §3 hypothetical case (γ = β = 1 fixed, no a-priori on J2
  and the solar moment of inertia), not the Table 1 total.

### WP4 results (offline PR tier, 1000 orbits per run, 2026-10-15)
| check | measured | oracle ± tolerance | result |
|---|---|---|---|
| PHY-005 GR-SCHW numerical | 42.9804688 ″/cy | 42.980 ± 0.001 | PASS |
| PHY-005 numerical − PHY-004 analytic | −1.02e-5 ″/cy | 0 ± 0.001 | PASS |
| PHY-006 ratio deviations (5 points) | ≤ 4.0e-10 | 0 ± 1e-6 | PASS |
| PHY-006 β shift (β−1 = 3.9e-5) / γ shift (γ−1 = 2.3e-5) | −0.00055875 / +0.00065904 ″/cy | −0.00056 / +0.00066 ± 5e-6 | PASS |
| PHY-007 SR-a numerical / analytic πGM/(c²p) | 7.1634124 / 7.1634126 ″/cy | 7.1634 ± 0.001 | PASS |
| NUM-015 R2 exact formula | 5.8e-10 relative per orbit | 1e-6 | PASS |
| PHY-008 SR-b harmonic / areal | 50.1438857 / 35.8170613 ″/cy | 50.144 / 35.817 ± 0.002 | PASS |
| PHY-009 A, B, C, D (= 1), A = 1/8 | 8.0000029, 4.0000001, 2.0000001, 2.0000001, 1.0000000 | 8, 4, 2, 2, 1 (1e-3 rel) | PASS |
| PHY-010 analytic (Park17 J2 / DE440 J2) | 0.0286015 / 0.0279168 ″/cy | 0.02860 / 0.02792 ± 5e-6 | PASS |
| PHY-010 numerical − analytic, equatorial | < 1e-8 ″/cy | 0 ± 0.0005 | PASS |
| NUM-015 R1 J2 equatorial (Park17 / DE440 J2) / inclined 3.4° | 2.3e-7 / 8.6e-8 / 2.1e-7 relative | 1e-6 | PASS |
| PHY-011 LT numerical / analytic | −0.0020184 / −0.0020183 ″/cy | −0.0020 ± 0.0002 / −0.00202 ± 5e-6 | PASS |
| NUM-015 R3b LT equatorial | 1.25e-6 relative, 2.9e-17 rad/orbit | 1e-6 rel **or** 1.17e-15 rad | PASS (floor only) |
| NUM-015 R3 GR / PPN points | ≤ 5.1e-9 relative | 1e-6 | PASS |
| Estimator noise (Newtonian test particle) | 4.5e-17 rad/orbit; 3.8e-9 ″/cy | floor 1.17e-15; PHY-001 1e-6 ″/cy | PASS |
- Cost: 76 468 IAS15 steps per 1000-orbit run, 0 rejected, 0 unconverged; ≈ 0.6 s per rung at opt-level 1;
  the whole CLI report (8 rungs) takes 4.8 s in release mode. Passage refinement leaves |r·v|/(|r||v|) ≤ 1e-13.
- Interesting but expected: the anomalistic period under 1PN is 87.969479 d vs the Keplerian 87.969466 d
  (+1.5e-7 relative, O(GM/c²a)).

### WP4 real-data results (data tier, run locally 2026-10-15, empty `FA_DATA_DIR`, all downloads fresh)
| test | measured | oracle / tolerance | result | provenance |
|---|---|---|---|---|
| `data_gr_rung_from_horizons_de441_state` | 42.980644 ″/cy (osculating a = 57 909 074.636 km, e = 0.2056301618); − Park17 = +0.00074 (0.83σ) | Park17 GE 42.9799 ± 0.0027 (3σ, pre-registered); − osculating analytic −1.0e-5 (± 0.001); NUM-015 4.7e-9 | PASS | Horizons API v1.2, DE441, `Ecliptic of J2000.0`, sha256 `01d74c36…49463f4` |
| `data_j2_and_lt_rungs_in_real_solar_equator_geometry` | i = 3.381087°; J2 0.028452 ″/cy; LT −0.0020148 ″/cy | Park17 Table 3 0.0286 ± 0.0011 (−0.13σ) and −0.0020 ± 0.0002 (−0.07σ); NUM-015 vs Park17 Eqs. 3/2: 1.3e-7 and 7.7e-8 relative | PASS | Horizons API v1.2, DE441, `IAU_SUN body equator and node of date`, sha256 `e6a416ec…6c3ce1d` |
| `data_solar_spin_from_fresh_header_440` | G·S = 1.2680765843106158e16 km⁵/s³ | committed table, ≤ 8 ulp | PASS (identical) | header.440 sha256 `0f4636b6…6ac3359`, Last-Modified 2020-12-22 |
- Horizons answers embed a request timestamp, so their SHA-256 changes with every download (as in WP3); the
  numbers above are the ones printed by the run.
- The J2 rung in the real geometry gives 0.028452, i.e. Park17's 0.0286 is matched to 0.13σ only because the
  real 3.38° inclination is included (the planar value 0.028601 would be +0.0000 vs Park17; both are inside 1σ,
  which is the J2 uncertainty). With DE440's own J2 the real-geometry value would be ≈ 0.02777 (not asserted).

### WP4 gate results (2026-10-15, local, toolchain 1.95.0)
All eight AGENTS.md commands passed: fmt; clippy `-D warnings`; `cargo nextest run --workspace` (79 passed,
12 skipped = the `data_*`/`nightly_*` tiers) and the doctests; `cargo deny check` (only the existing
`license-not-encountered` warnings); rustdoc `-D warnings`; ruff check/format + mypy strict (8 files);
`reuse lint` (169/169 files); `trace.py --check` (119 requirements, 152 citing tests). Extras: `uv run pytest`
(15 passed, including the sympy derivation), MSRV 1.85.0 nextest (78 passed, trybuild excluded), and the whole
workspace `data` profile on an empty cache (10 passed: 3 new WP4 tests plus the 7 from WP1–WP3). The `nightly`
profile was not re-run: no WP4 change touches the integrator or the NUM-023 test.

### WP4 adversarial review (2026-10-15, reviewer agent)
- Re-ran all eight AGENTS.md commands and the whole `data` profile on an empty `FA_DATA_DIR`: all pass, and
  the data-tier numbers match the builder's report (GR from Horizons 42.980644, J2 real geometry 0.028452,
  LT real geometry −0.0020148 ″/cy).
- Sign mutations of the R3 `(1+γ)` term, the LT `v × ŝ` term, the J2 axial term, the SR `1 − 1/γ` factor,
  the Lagrangian `−2D v²` term and the Park17 Eq. 3 geometry factor are each caught by at least one test.
  A sign flip of the Lagrangian's `3D ṙ² n` term was **not** caught: that term's orbit average
  `∝ ∮ sin²f cos f df` vanishes, so it has no first-order secular effect on the perihelion and no rung can
  see it. Added `relativity::tests::areal_lagrangian_matches_closed_form_areal_eom`, which checks the areal
  coefficients against the closed form `μ/(c²r²)[(2U − 2v² + 3ṙ²) n + 2ṙ v]` at the acceleration level. The
  reviewer re-derived that closed form independently with sympy (exact Euler–Lagrange solve, first order in
  `1/c²`) before using it. The new test kills the mutant.

## WP5 — Real observational data: Horizons DE441 + Park2017 estimator (NUM-011 subset, PHY-016 groundwork, PHY-015)

### Plan and pre-registered checks (written 2026-10-15, before any Rust code)
- **Transparency first.** Before fixing the oracles below, a throw-away Python prototype (scratchpad, not
  committed) was run on real Horizons answers to choose the sampling step and to check that the procedure is
  feasible. It printed: Park-frame ϖ̇ = 575.30916 ″/cy (quadratic + 14 Park17 Table 2 frequencies, J2000 ± 1000 yr,
  20-block jack-knife σ 0.0015), and J2000-ecliptic dog-leg ϖ̇ = 572.03 ″/cy (linear, 1800–2050). The tolerances
  below are therefore **not** blind. Each one is derived from a published uncertainty or a physical argument that
  does not use these numbers, and the derivations are written out so a reviewer can check them independently.
- **Fetcher** (`ferroastro-testdata::horizons`, test infrastructure): JPL Horizons API, `EPHEM_TYPE=VECTORS`,
  `COMMAND='199'`, `CENTER='500@10'` (Sun body centre), TDB, km and km/s, CSV. The span is split into chunks below
  Horizons' 90 024-line output limit (seen in an error answer: "Projected output length (~91313) exceeds 90024").
  Requests are serial, with a 1 s pause before each network request (Horizons does not document a rate limit). Each
  chunk is cached in `$FA_DATA_DIR` (never committed) with a provenance sidecar, and the test prints URL, retrieval
  date, SHA-256, `signature.version`, the ephemeris source string (`{source: DE441}`) and the reference-frame line
  for every chunk. A cached answer that does not parse (e.g. a cached error) is deleted and fetched again once.
- **Span and step.** The full product is J2000 ± 1000 Julian years (JD 2 086 295.0 – 2 816 795.0 TDB), i.e. Park17's
  "1000 January 1 to 3000 January 1" span as a symmetric Julian-year interval, at a 2-day step (5 chunks of 400 yr,
  73 051 rows each). The prototype measured the passage-direction error against a 6-hour-step reference: rms
  3e-6″ per passage at 2 d (8-point Lagrange), 3e-4″ at 4 d, 0.015″ at 8 d. The 2-day step keeps that error ~10⁶ times
  below the 0.4″ rms residual of the full fit.
- **Estimator** (`ferroastro-precession::park2017`, NUM-011), following Park17 §4 as read in the PDF:
  1. Perihelion passages: roots of `r·v` rising, found by bisection on an 8-point Lagrange interpolant of the sampled
     `r·v`. The position at the root comes from the same interpolant. A 6-point interpolant gives an error estimate,
     which is reported.
  2. Mean-orbit frame: z = the pole (default RA 280.9876°, Dec 61.4481° ICRF), y = z × x_ICRF normalised, x = y × z
     (Park17 §4 wording). The perihelion direction is the passage position projected onto the frame's xy-plane. The
     longitude is its angle, unwrapped, relative to the first passage.
  3. Fit B + ϖ̇t + Qt² + Σ(Sᵢ sin νᵢt + Cᵢ cos νᵢt), t in Julian centuries from J2000.0 TDB (Park17 says "the epoch of
     2000 January 1"; the half-day difference shifts ϖ̇ by 2Q·Δt ≈ 1e-6 ″/cy). The solver is Householder QR. The
     frequencies are either supplied or "FFT-derived" (periodogram peaks found by iterative prewhitening).
  4. The formal σ of ϖ̇ comes from the residual variance and (RᵀR)⁻¹. The jack-knife σ is a delete-one-block jack-knife
     over G = 20 contiguous blocks (100 yr each), because the residuals are serially correlated.
  5. A second longitude definition, `DogLeg` (ϖ = Ω + ω of the osculating orbit at the passage, with Ω on the
     reference plane), is needed for SW92. SW92's "long.peri." is Ω + ω, not a projected angle. For Mercury in the
     J2000 ecliptic, the two definitions differ in rate by ≈ 2 tan²(i/2) cos 2ω · ω̇ ≈ 4 ″/cy, so mixing them up would be
     a gross error.
- **Supplied frequencies** = Park17 Table 2's 14 combinations k·n_M + j·n_P. The mean motions are SW92 Table 2a
  (3000 BC – 3000 AD) mean-longitude rates of Mercury, Venus, EM Bary, Jupiter and Saturn, which cover the fit span.
  They are transcribed into vectors and re-read from the JPL page by the data test.
- **(a) SW92, fixed J2000 ecliptic, 1800–2050.** Horizons `REF_PLANE='ECLIPTIC'` ("Ecliptic of J2000.0"), one chunk
  JD 2 378 496.5 – 2 470 172.5, DogLeg longitude, **linear** fit (SW92's model is linear in time). The oracle is SW92
  Table 1 ϖ̇ = 0.16047689 °/cy = 577.716804 ″/cy. **Tolerance ±20 ″/cy**, derived from SW92's own fit quality and
  independent of the result. SW92 states nominal heliocentric-longitude errors of 15″ for Mercury over 1800–2050.
  They also say the elements "are not intended to represent any sort of mean; they are simply the result of being
  adjusted for a best fit". At fixed mean longitude, ∂λ/∂ϖ = 1 − df/dM ranges over [1 − (1+e)²/(1−e²)^{3/2},
  1 − (1−e)²/(1−e²)^{3/2}] = [−0.551, +0.327]. A rate error δ, pivoting at the window centre (half-width 1.25 cy),
  therefore produces a longitude error up to 0.551 · 1.25 · δ. Requiring this ≤ 15″ gives |δ| ≤ 21.8 ″/cy, rounded
  to 20. The bound is weak by construction: SW92's two fits of the same ephemeris (Table 1, 1800–2050: 577.72;
  Table 2a, 3000 BC – 3000 AD: 573.84 ″/cy) already differ by 3.9 ″/cy. The result is reported against both. This
  check is about frames and definitions (ecliptic vs mean-orbit, dog-leg vs projected), not about precision.
- **(b) Park17, mean-orbit frame, J2000 ± 1000 yr.** ICRF (`REF_PLANE='FRAME'`) vectors, default pole, Projected
  longitude, quadratic + 14 Table 2 frequencies. Oracle: Park17 total 575.3100 ± 0.0015 ″/cy. Two assertions:
  - **NUM-011 acceptance**, |ϖ̇ − 575.31| ≤ 0.01 ″/cy, for "a DE440-class model over 1000–3000 CE" (DE441 is one).
  - **Consistency**, |ϖ̇ − 575.3100| ≤ 3·√(σ_Park² + σ_jk²), with σ_Park = 0.0015 (Park17) and σ_jk measured. The
    argument from first principles: DE441 and Park17's ephemeris are both fitted to MESSENGER ranging. Park17 §4
    argues that any parameter combination that fits those ranges "must give the same precession rate". So the two
    rates should agree within Park's σ plus our estimator's σ, and no model-difference term is added.
  - Known model differences are **reported, not subtracted**: J2☉ (DE440 2.196e-7 vs Park17 2.25e-7 gives
    0.0286 × (2.196 − 2.25)/2.25 = −0.00075 ″/cy if nothing else compensates), and Park17's fitted (β − 1) = −2.6e-5
    (+0.00037 ″/cy if their integration used it; DE441 has β = 1). Both are inside the consistency band.
  - Sensitivity runs (reported only): FFT-derived frequencies, SW92 Table 1 vs Table 2a mean motions, a 4-day step
    (decimated), G = 10 and 40 jack-knife blocks, and linear and quadratic fits without periodic terms.
- PHY-015: every reported rate goes through `RateReportBuilder` (frame, epoch, fit span, fit model, constants
  version = "JPL DE441 via Horizons API v<signature.version>", gauge = "DE441 (PPN, isotropic/harmonic, as
  integrated by JPL)", estimator).
- If a check fails, the number is reported and nothing is loosened without a NEEDS HUMAN REVIEW note.

### What was built
- `ferroastro-testdata::horizons`: `VectorQuery`, `fetch_vectors` (chunking, serial requests with a 1 s pause,
  query-derived cache names, one automatic re-fetch of an unparsable cached answer), `parse_vector_answer`
  (checks TDB, `KM-S`, geometric states and the frame line, and extracts `signature.version` and both
  `{source: …}` strings), and a check that consecutive chunks agree **bit for bit** at their shared epoch.
  New code `FA-E0203`.
- `ferroastro-precession::park2017` (NUM-011): `SampledOrbit<F>` (typed by frame; built from raw arrays or
  `Epoch<Tdb>` + `State<F, Heliocentric>`), `ReferencePlane<F>` (`park2017_mean_orbit()` for `Icrf` only, so the
  ICRF pole cannot be applied to ecliptic data; `xy_plane()` for any frame), `find_passages`,
  `perihelion_longitudes` (`Projected` / `DogLeg`), `fit_longitudes` (formal and block jack-knife σ),
  `fft_frequencies` and `Park2017Result::report` (PHY-015 metadata). Also `lsq` (Householder QR with column
  equilibration and a rank check) and `spectral` (radix-2 FFT). New code `FA-E0603`.
- Vectors `tests/vectors/park2017-estimator.toml`: Park17 pole, total rate (± 0.01 NUM-011), its σ and Q; SW92
  Table 1 and 2a ϖ̇ and mean motions, all re-read digit for digit from the JPL page by `data_sw92_table1_mercury`.
  SW92 ϖ̇ in ″/cy carries the ± 20 bound derived in the plan.
- Tests: `num011_park2017_synthetic.rs` (offline: exact recovery of a driven precession, supplied and FFT-found
  periodic terms, dog-leg vs projected spherical trigonometry, input refusal, frame triad vs the vectors) and
  `data_park2017_horizons.rs` (two `data_*` tests, below).

### Interpretations and deviations from the plan
- **SW92 window.** The plan's end date JD 2 470 172.5 was a slip: that is 2051-Jan-01. The test uses 1800-Jan-01 0h to
  2050-Jan-01 0h TDB (JD 2 378 496.5 – 2 469 807.5). That span is not a whole number of 2-day steps, so it uses a
  **1-day** step (2 chunks). The tolerance is unchanged.
- **FFT refinement.** The plan said "periodogram peaks". The unit test showed that the peak of the DFT power of a
  real sinusoid with few cycles is pulled by its negative-frequency image (1e-4 relative at 7 cycles). Peaks are
  therefore refined by minimising the residual sum of squares of `[1, t, t², sin νt, cos νt]` (golden section within
  ± 1 FFT bin). This is a least-squares periodogram.
- **Longitude origin.** Park17 measures the angle "with respect to the initial position vector". Here it is measured
  from the frame's x-axis, which shifts only B.
- **Epoch.** t = 0 at J2000.0 TDB, for Park's "2000 January 1" (effect 2Q·0.5 d ≈ 6e-7 ″/cy).
- **Centre.** Heliocentric (Horizons `500@10`, the Sun's body centre), as PHY-016 specifies. Park17 does not say
  whether its passages are heliocentric or barycentric. **Open question.**
- **Park17 Table 2 amplitudes are not reproduced.** Sampled once per orbit, `k·n_M + j·n_P` aliases to
  `≈ j·n_P + k·ϖ̇`, so `1n_M − 2n_J` and `2n_J` (and similar pairs) are nearly collinear. Our fit gives 14.80″ and
  6.70″ for them, against Park's 0.83″ and 7.24″. The rate is orthogonal to these terms and is not affected: 11
  FFT-derived terms give the same ϖ̇ to 1e-5 ″/cy. The amplitudes are printed, not asserted. How Park separated
  the aliased pairs is unknown. **Open question.**
- **NUM-011 scope delivered:** the estimator, configurable pole, both frequency sources, both σ, and the
  acceptance on a DE440-class model (DE441, 1000–3000 CE). **Not delivered:** making it "the canonical
  observable for every absolute rate". The single-body rungs (WP4) still use the initial-plane passage estimator,
  and moving them over needs FerroAstro's own long N-body runs (EIH, M1+).
- **PHY-016 groundwork only.** The fetcher (≥ 500 yr: 2000 yr delivered), `signature.version` and the source string
  are recorded per chunk. The DE441 side of the comparison exists (575.30916 ″/cy, Park frame, Park fit).
  FerroAstro's EIH-FULL side does not exist yet, so the PHY-016 acceptance itself is **not** met.

### WP5 real-data results (data tier, run locally 2026-10-15 UTC on an empty `FA_DATA_DIR`, all downloads fresh)
| test | measured | oracle / tolerance | result | provenance |
|---|---|---|---|---|
| `data_sw92_table1_mercury` (extended) | 12 new SW92 rate/mean-motion tokens | vectors, exact | PASS (all equal) | approx_pos.html sha256 `9f7b30ca…f9c4d742`, Last-Modified 2026-10-06 |
| `data_sw92_rate_from_horizons_de441_ecliptic` | dog-leg ϖ̇ = **571.968 ″/cy** (formal σ 0.332, jack-knife σ 0.147, 10 blocks); 1038 passages; rms residual 7.71″ | SW92 Table 1 577.716804 ± 20 → **−5.749** | PASS | Horizons API v1.2, DE441, "Ecliptic of J2000.0", 2 chunks (1 d step), sha256 (first run) `1f79316e…d04323da5c`, `4d1901a5…fcf7966f2` |
| `data_park17_rate_from_horizons_de441_mean_orbit` | ϖ̇ = **575.30916 ″/cy**, formal σ 0.00083, **jack-knife σ 0.00153** (20 blocks); Q = −0.0447605 ″/cy²; 8304 passages; rms residual 0.437″ | Park17 575.3100: NUM-011 ± 0.01 and 3·√(0.0015² + σ_jk²) = ± 0.00643 → **−0.00084 (−0.39 σ)** | PASS (both) | Horizons API v1.2, DE441, ICRF, 5 chunks of 73 051 rows (2 d step), sha256 (first run) `27ff0431…92383853`, `015ac2fd…85358dfe`, `7d2a118a…e9cd6fed`, `09033cf7…a136e67e13`, `db6db6a5…76cb31cda9` |

Sensitivity of (b), reported only (Δ vs 575.30916):
| variant | ϖ̇ (″/cy) | Δ | jack-knife σ |
|---|---|---|---|
| linear only, no periodic terms | 575.30972 | +0.00056 | 0.07444 |
| quadratic only | 575.30973 | +0.00056 | 0.00260 |
| SW92 Table 1 mean motions | 575.30918 | +0.00001 | 0.00149 |
| 11 FFT-derived frequencies (≥ 0.5″) | 575.30917 | +0.00001 | 0.00152 |
| 4-day sampling (decimated) | 575.30916 | 0.00000 | — (max passage error 2.7e-3″ vs 1.1e-4″ at 2 d) |
| 10 / 40 jack-knife blocks | 575.30916 | 0 | 0.00251 / 0.00109 |
- The jack-knife σ depends on the block count (0.0011–0.0025): the residuals have structure on 50–200-yr scales.
  The 3σ band evaluated at the largest of these (G = 10) would be ± 0.0088, and the result passes for every G.
- Q = −0.0447605 vs Park17 −0.04478132 ″/cy² (−0.05 %, reported only).
- Known model offsets, reported only: DE440/441 J2☉ 2.196e-7 vs Park17 2.25e-7 → −0.00068 ″/cy (printed by the test).
  Park17's fitted β − 1 = −2.6e-5, if their integration used it → +0.00037 (= 42.98 × 2.6e-5 / 3). The measured
  −0.00084 is consistent with the J2 offset dominating. With σ = 0.0015, that is not evidence for it.
- (a) is inside the SW92 bound, but **it is not a precision agreement**. The gap of −5.75 ″/cy is 39 times our
  jack-knife σ. Several things fit this picture. Adding the 14 periodic terms leaves −5.84. The projected angle in the
  same ecliptic gives 568.980, which is 3.0 ″/cy below the dog-leg value: a definitional effect of the predicted
  order. SW92's own Table 2a value (573.84) is closer (−1.87). Candidate causes, **unverified**: SW92 fitted
  positions along the whole orbit, not perihelion passages, and their ϖ̇ absorbs whatever the six-element
  linear model cannot represent; DE200 vs DE441; long-period terms in a 250-yr window. **Open question, flagged for
  human review:** whether a tighter, independently justified SW92 oracle exists. No tolerance was changed.
- Horizons answers embed a request timestamp, so every fresh download has a new SHA-256. The second empty-cache run
  (whole `data` profile) printed different digests, but the same numbers to every printed digit.
- Cost: the (b) test takes 20 s on a fresh cache (≈ 72 MB in 5 requests) and < 2 s cached. The (a) test takes 6 s
  (≈ 18 MB). The weekly CI `data` job picks both up through the `data_*` filter.

### WP5 gate results (2026-10-15, local, toolchain 1.95.0)
All eight AGENTS.md commands passed:
- fmt;
- clippy `-D warnings`;
- `cargo nextest run --workspace` (91 passed, 14 skipped = `data_*`/`nightly_*`), and the doctests;
- `cargo deny check` (only `license-not-encountered`/`license-exception-not-encountered` warnings);
- rustdoc `-D warnings`;
- ruff check/format and mypy strict (8 files);
- `reuse lint` (176/176);
- `trace.py --check` (119 requirements, 174 citing tests).

Extras: `uv run pytest` (15 passed), MSRV 1.85.0 nextest (90 passed, trybuild excluded), and the whole-workspace
`data` profile on an empty cache (12 passed: the 2 new WP5 tests and the 10 from WP1–WP4). The `nightly` profile was
not re-run: WP5 does not touch the integrator.

### WP5 adversarial review (2026-10-15, reviewer agent)

- Re-ran every AGENTS.md command (all pass) and the data tier on an empty scratch `FA_DATA_DIR` with
  fresh downloads: 12/12 pass. The builder's numbers reproduce to every printed digit: (b) 575.309163
  ″/cy, formal σ 0.000832, jack-knife σ 0.001531, 8304 passages, Q −0.04476052; (a) 571.968047 ″/cy,
  1038 passages. Horizons answered `signature.version` 1.2 with ephemeris source DE441 on every chunk.
- Mutation checks (local only, reverted): flipping the projected-angle sign, the dog-leg `ω` sign or
  the pole vector each fail an offline test. Flipping the y-axis convention is a 180° in-plane
  rotation that only shifts B, so it correctly survives. **Changing the jack-knife normalisation
  `(g−1)/g` to `1/g` survived every test (offline and data tier).** The reviewer added
  `jackknife_sigma_matches_formal_sigma_for_white_noise`, which kills it (ratio 0.220 against an
  accepted band of 0.6–1.6; unmutated 0.959).
- Not covered offline: `fetch_vectors`' check that chunks agree at their shared epoch (disabling it
  survives); it can only fire against real answers. Non-blocking.
- The ±20 ″/cy SW92 band is weak by design (it tests frame and definition only). The −5.75 ″/cy gap
  stays an open question, as the builder recorded.

## 📈 Progress review: M0-M2 after WP5

I am the independent progress reviewer (an agent, not the builder). I audited `m0-foundation` at `9ec1f86` (WP1–WP5), stacked on `1-scientific-computing-strategy` (merge base `216c58e`). I ran everything below myself on 2026-10-15 UTC, on the local machine with toolchain 1.95.0. Nothing was loosened, and no mutation was committed.

### Overall health: 🟡 YELLOW
- **Green:**
  - All eight AGENTS.md gates pass.
  - All 12 real-data tests pass on an empty cache.
  - Three key results reproduce in independent Python, including the 575.309 ″/cy Park17 rate, which I rebuilt with my own estimator from the raw Horizons rows.
  - Nobody has moved any golden value or tolerance.
- **Not green:**
  - A longer NUM-023 ensemble still fails the Brouwer energy slope, which is a known issue. NUM-001's 1e9-step clause is unmet.
  - One integrator mutant survives.
  - The crate layering differs from CRATE-PLAN: estimators run propagation, and the analytic oracles live in `precession`.
  - Six M1 requirements (the debt crates) and two M2 requirements have no citing test.

### 1. Commands
| command | result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo nextest run --workspace` + `cargo test --doc --workspace` | 92 passed, 14 skipped (12 `data_*` + 2 `nightly_*`, the only `#[ignore]`s); doctests pass |
| `cargo deny check` | pass (only `license-not-encountered` warnings) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` | pass |
| `ruff check`, `ruff format --check`, `mypy` (strict) | pass (8 files) |
| `reuse lint` | pass |
| `trace.py --check` | pass: 119 requirements, 175 citing tests, 28/92 `test` requirements cited |
| `data` profile, empty `FA_DATA_DIR`, all downloads fresh | **12/12 pass** (20.2 s) |
| `nightly` profile (release) | **2/2 pass** (5.2 s) |
| exploratory nightly, `FA_NUM023_RUNS=32 FA_NUM023_ORBITS=20000` (4.9e7 steps) | **FAIL**: energy slope 0.629 ± 0.018 (needs ±0.1 of 0.5). This reproduces WP3's NEEDS-HUMAN-REVIEW finding exactly. |

GitHub PR checks on `9ec1f86`: dco, msrv, python, reuse and rust pass. `data` and `long` are skipped, because they are schedule-only by design.

### 2. Real-data and benchmark results
Provenance:
- **header.440:** https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/header.440, sha256 `0f4636b6…6ac3359`, 22802 B, Last-Modified 2020-12-22, retrieved 2026-10-15.
- **SW92:** approx_pos.html, sha256 `9f7b30ca…f9c4d742`.
- **Horizons:** API `signature.version` 1.2, `{source: DE441}` on every chunk. Answers carry a timestamp, so each download has a new sha256. For example, chunk 1 of (b) this run had sha256 `13bc0b0e…abb0b86`.

Margin = |Δ| / tolerance.

| test | measured | oracle (source) | tolerance | margin |
|---|---|---|---|---|
| `data_de440_table_matches_header` | 23/23 entries equal | header.440 (fresh) | exact | 0 |
| `data_de440_header_ref002_acceptance` | JDEPOC, CLIGHT, BETA, GAMMA, AU, J2SUN, ASUN, GMS, EMRAT: \|Δ\| = 0 | header.440 | exact | 0 |
| `data_de440_header_defined_constants` | CLIGHT 299792.458, AU 149597870.7 | SI / IAU 2012 B2 | exact | 0 |
| `data_sw92_table1_mercury` | 6 elements + 12 rates equal | SW92 Tables 1 and 2a (JPL page) | exact | 0 |
| `data_horizons_osculating_elements_match` | e rel 1.08e-15, a rel 1.29e-16, angles ≤ 2.8e-14° | Horizons ELEMENTS | 1e-12 rel | 0.001 |
| `data_horizons_n2b_from_de441_state` (PHY-001) | 1.087e-9 ″/cy over 1000 yr | exactly 0 | < 1e-6 | 0.001 |
| `data_horizons_gr_rate_from_osculating_elements_vs_park17` | 42.980669 ″/cy | Park17 Table 3 GE 42.9799 | ±0.0027 (3σ, pre-registered) | 0.28 |
| `data_gr_rung_from_horizons_de441_state` (PHY-005) | 42.9806442 ″/cy | Park17 GE 42.9799 / osculating analytic 42.980654 | ±0.0027 / ±0.001 | 0.28 / 0.01 |
| `data_j2_and_lt_rungs_in_real_solar_equator_geometry`: J2 | 0.0284524 ″/cy (i = 3.381087°) | Park17 Table 3, 0.0286 | ±0.0011 (Park 1σ) | 0.13 |
| same test: LT | −0.0020148 ″/cy | Park17 Table 3, −0.0020 | ±0.0002 | 0.07 |
| same test: NUM-015 vs Park17 Eqs. 3 and 2 | 1.3e-7, 7.7e-8 rel | analytic | 1e-6 rel | 0.13, 0.08 |
| `data_solar_spin_from_fresh_header_440` | G·S = 1.2680765843106158e16 km⁵/s³ | recomputed from header tokens | 8 ulp | 0 |
| `data_sw92_rate_from_horizons_de441_ecliptic` | dog-leg 571.968047 ″/cy (jk σ 0.147) | SW92 Table 1 577.716804 | ±20 (derived from SW92 15″) | 0.29 ⚠ weak oracle; −5.75 gap is unexplained (open) |
| `data_park17_rate_from_horizons_de441_mean_orbit` | **575.309163 ″/cy**, jk σ 0.00153, Q −0.0447605 | Park17 total 575.3100 ± 0.0015 | NUM-011 ±0.01; 3·√(σP²+σjk²) = ±0.00643 | 0.08; 0.13 (−0.39σ) |
| CLI `ferroastro ladder --rungs single-body` (PR tier, release) | SR-a 7.1634124; SR-b 50.1438857 / 35.8170613; GR 42.9804688; J2 0.0286015 / 0.0279168; LT −0.0020184 | PHY-007/008/005/010/011 | ±0.001 / ±0.002 / ±0.001 / ±0.0005 / ±0.0002 | ≤ 0.10 for every rung; LT NUM-015 rel.dev 1.25e-6 passes only via the 1e-7 ″/cy floor (already flagged NEEDS HUMAN REVIEW) |
| `nightly_num023_brouwer_slopes_ias15_f64` (64×2000 orbits) | energy 0.526, phase 1.513 | Brouwer ½, 3/2 | ±0.1 | 0.26, 0.13 |
| same, 32×20000 orbits (exploratory) | energy **0.629**, phase 1.590 | ½, 3/2 | ±0.1 | **1.29 FAIL**, 0.90 |

**Independent recomputation** (Python and numpy, no FerroAstro code; inputs are the freshly downloaded header.440 and Horizons rows):
1. **PHY-004 GR rate**, from GMS·AU³/86400², CLIGHT, and SW92 a, e:
   - 5.0186604e-7 rad/orbit and 415.200918 orbits/cy give **42.980478977 ″/cy**, identical to FerroAstro to 9 decimals.
   - From the Horizons osculating a, e it gives 42.980669, identical too.
2. **Park17 contributions**:
   - **J2:** 0.0286015 at Park J2, planar. 0.0284523 at i = 3.381087° (FerroAstro measures 0.0284524). With the DE440 J2 it is 0.0279168.
   - **G·S:** 1.2680765843106158e16 km⁵/s³ from header tokens, so S = 1.8999e41 kg m²/s.
   - **LT:** −0.0020183 (planar) and −0.0020148 (real geometry).
   - **Table 3 sum:** 532.3035 + 42.9799 + 0.0286 − 0.0020 = 575.3100.
   - **PHY-014 oracle shift:** 0.0286·(2.19614/2.25 − 1) = −0.000685, which gives 575.309315. This agrees with the maintainer's 575.3093.
3. **Reproduced benchmark: the Park17 rate from DE441.** I wrote my own estimator and ran it on the 5 cached Horizons ICRF chunks (365 251 rows):
   - Passage finding is different from FerroAstro's: a degree-7 Hermite interpolant through 4 samples using both position and velocity, against FerroAstro's 8-point Lagrange on r·v.
   - Frame: Park17 pole. Fit: quadratic + 14 Table-2 frequencies.
   - Result: **575.309174 ″/cy** (formal σ 0.00083, Q −0.0447574, rms 0.4376″, 8304 passages), against FerroAstro's 575.309163. Δ = 1.1e-5 ″/cy, which is 0.007 of the jack-knife σ.
   - Quadratic only: 575.309736 (FerroAstro 575.30973).

### 3. Drift audit
- **`git log -p -- tests/vectors` since `216c58e`:** 9 commits, 1335 lines added. The only changed lines are 3 metadata edits:
  - `af7cb63` and `b4d5cad`: `verified_by`/`notes` of CLIGHT and AU.
  - `c901f86`: the `verified_by` of the Park17 J2 vector.
  
  **No value or tolerance was ever moved or widened.**
- **Agent-closed open item:** `CONST-AU-IAU2012` notes say OI-05 is "resolved" by an agent re-reading, but OI-05 is still unticked in OPEN-ITEMS.md (red-team H5 pattern). I re-downloaded https://syrte.obspm.fr/IAU_resolutions/Res_IAU2012_B2.pdf. Its sha256 is `3489ebb1…afec984` (matches the vector), and the text reads "149 597 870 700 m exactly". The claim holds. **NEEDS HUMAN REVIEW:** a human should tick OI-05 in PR #2.
- **`#[ignore]`:** 14 in total, all named `data_*` (12) or `nightly_*` (2), and all 14 ran in this review. I found no `cfg` gating or early returns. One exception: `num023_brouwer.rs` reads `FA_NUM023_EPSILON`/`_PC_TOL`/`_RUNS`/`_ORBITS`, so an environment change in CI could alter what the nightly test asserts (red-team H2 route). Low severity.
- **UNVERIFIED values as oracles:** none found. GMSDOT (OI-04) and CENTER (OI-23) are excluded from the vectors. The Cassini γ is used only as input. The INPOP21a Eq. 1 equivalence (OI-35) is not claimed.
- **Circular oracles:**
  - PHY-005, PHY-010 and NUM-015 compare numerical results with analytic formulas in the same crate tree. That would be circular on its own. It is mitigated because the formulas are re-derived independently (red-team §1 and item 2 above) and the data tier checks against Park17 Table 3.
  - `data_solar_spin_from_fresh_header_440` recomputes G·S from header tokens, independently of `de440_solar_spin`. The mutant below confirms this.
  - Nothing is circular in a way that matters.
- **Mutation probes** (local, each one reverted, tree clean afterwards):

| crate | mutant | result |
|---|---|---|
| vectors | `allowed = max(abs, rel)` ×10 | killed (2 tests) |
| core | arcsec/rad 3600 → 3600.1 | killed (PHY-004, NUM-011 synthetic, …) |
| constants | GM au³/d² → km³/s² scale ×(1+1e-9) | killed (`asserted_de440_constants`, Park21 GMs, PHY-010) |
| **integrators** | **IAS15 step-control exponent 1/7 → 1/6** | **SURVIVED every PR-tier test** |
| forces | R3 along-track term ×1.001 | killed (PPN cross-check, PHY-005/006, CLI) |
| forces | J2 coefficient −1.5 → −1.503 | killed (unit, PHY-010 Eq. 3, CLI) |
| precession | analytic J2 oracle 3π → 3.003π | killed |
| precession | Park2017 Lagrange order 8 → 4 | PR tier survived; killed by the data tier (the passage-error budget assertion; ϖ̇ unchanged to 1e-6) |
| ladder | DE440 solar G·S ×1.01 | killed (PHY-011 offline + `data_solar_spin`) |
- **Layering** (`cargo metadata`): no cycles and no upward edges. Deviations from CRATE-PLAN:
  - `ferroastro-precession` has a runtime dependency on `ferroastro-integrators`. `passages.rs` runs IAS15 re-integrations, which breaks "estimators contain no propagation".
  - Analytic oracles live in `precession::analytic`, not `ferroastro-analytic`.
  - `ferroastro-forces` depends on `ferroastro-integrators` (the `SecondOrderSystem` trait), an edge CRATE-PLAN does not list.
  - `tools/check_layers.py` does not exist yet. All four items are already WP6 scope.

### 4. Requirement coverage (trace.py; "cited" = at least one `Verifies:` test)
- **M0, uncited:**
  - STD-001…011
  - TST-001
  - DEP-001, DEP-002, DEP-003, DEP-014
  - ORG-008
  
  Most of these are enforced by CI jobs (fmt, clippy, deny, MSRV, DCO) rather than by citing tests. **NEEDS HUMAN REVIEW:** should trace.py accept "verified by gate X"? TST-004 and AGT-001 are inspection requirements.
- **M1, uncited:** DEP-007, DEP-009, DEP-010, NUM-006, NUM-010, REF-009. These are the M1-debt crates `real`, `erfa` and `time`, planned for WP7/WP8.
- **M1, cited but acceptance unmet:**
  - NUM-001: the 1e9-step Brouwer clause, and Kozai–Lidov.
  - NUM-023: fails at 4.9e7 steps.
- **M2, uncited:**
  - NUM-019: the single-body J2/LT vs Park17 test exists but does not cite it; the momentum part belongs to M4.
  - API-003: no doc example per PHY rung.
- **M2, cited but acceptance partial:**
  - NUM-017: R5→R3 needs EIH.
  - PHY-011: INPOP21a Eq. 1 is not demonstrated (OI-35).
  - NUM-012: there is no documented cross-estimator spread table beyond LRL vs passages.
- **Trace inflation:**
  - `data_sw92_table1_mercury` and the pure Kepler-utility unit tests carry `Verifies: PHY-001`. These are supporting tests, not N-2B evidence.

### 5. Corrections for the next builder (WP6)
1. **Kill the IAS15 step-control mutant.** Add a test that pins Eq. 11. For example, assert that the accepted-step ratio between ε and ε·10⁻⁷ on the same smooth orbit is 10 ± 5 %. Alternatively, pin the step count for Kepler e = 0.5 at ε = 1e-9 to a pre-registered value.
2. **NUM-023.** Add a pre-registered long-ensemble nightly check (32×20000, 25 s) that is reported as a known failure, linked to the WP3 NEEDS-HUMAN-REVIEW note. Then a green nightly no longer implies that NUM-001's 1e9-step clause holds. Do not widen ±0.1. Investigate:
   - Everhart's predictor correction
   - forcing at `x − compensation`
   - the B₇ stall floor
3. **Make the NUM-023 env knobs report-only.** If any `FA_NUM023_*` override is set, print the numbers and skip the assertion, so an environment change cannot neuter the test.
4. **Do the WP6 layering work:**
   - Move the passage runner (IAS15 use) out of `ferroastro-precession` into ladder/diffrun.
   - Move the analytic oracles to `ferroastro-analytic`.
   - Add `tools/check_layers.py` with a test.
   - Either record the forces→integrators edge in CRATE-PLAN, or move `SecondOrderSystem` down to core.
5. **Trace hygiene:**
   - Drop `Verifies: PHY-001` from the table-transcription and Kepler-utility tests.
   - Add `Verifies: NUM-019` only to the Park17 J2/LT data test (single-body part).
   - Add API-003 doc examples per rung.
6. **PHY-001:** name IAS15 and its settings (ε = 1e-9, safety 0.25, PC tolerance 1e-16) in the test output and the vector rationale (red-team §1).
7. Keep everything in the open-question queue for humans; none of it may be resolved by an agent:
   - OI-05 tick
   - NUM-015 floor
   - NUM-017 areal exemption
   - PHY-009 "8A" wording
   - IAS15 rejection rule
   - SW92 −5.75 ″/cy gap
   - OI-35 / INPOP21a Eq. 1
   - Park17 heliocentric vs barycentric passages

Reviewer artefacts are in the local scratchpad, not committed: the gate, data and nightly logs, `recompute.py` and `mutate.py`.

## WP6 — M2 close-out: crate alignment, layer check and red-team test corrections

Builder agent, 2026-10-15. Applies the "Corrections for the next builder" of the progress review above. No golden
value or tolerance of an existing vector was changed. The `PHY001-NEWTON-RATE` vector has a new
`tolerance_rationale` and `verified_by` text, with the same value and tolerance.

### Plan and pre-registration
- Every new oracle has a vector. Four new vectors were written **before** the first run of their tests:
  `NUM013-ISOLATION-DIFF` (NUM-013(a)'s own 1e-4 ″/cy), `NUM012-ESTIMATOR-SPREAD` (1e-4 ″/cy, the NUM-013(a)
  bound, so the isolation result cannot depend on the estimator), `IAS15-EQ11-EPS-EXPONENT` (1/7 to 1e-12) and
  the 32 × 20 000 NUM-023 ensemble size.
- **Not blind:** the tolerance of `IAS15-EQ11-DEADBEAT` (0 ± 0.05) was chosen after a probe measured +0.0112 for the
  correct code and −0.150 for the mutant. Its rationale derives the band without these numbers: below half the
  distance to the nearest plausible wrong exponents (|1 − 7k| = 0.125 for k = 1/8, 0.167 for k = 1/6), and above the
  O(ω dt) ≈ 1e-2 contamination of b₇ ∝ dt⁷.

### Crate alignment (CRATE-PLAN §1)
- **New `ferroastro-analytic`** (L4): `kepler` (elements ↔ states, Kepler's equation), `advance` (GR, PPN scaling,
  1PN unit, exact SR, J2 planar/inclined, LT, Keplerian period, ″/cy conversion) and `vector` (cross, dot, norm,
  eccentricity vector). All moved from `ferroastro-precession` (git history kept). `AnalyticError::Domain` keeps the
  stable code FA-E0600, and `PrecessionError` and `LadderError` convert from it.
- **`ferroastro-precession` contains no propagation.** It no longer depends on `ferroastro-integrators`. It keeps
  only estimators on sampled data: Park2017, passages fit, LRL tracker and line fits, plus reports. It has its own
  compensated OLS `fit::fit_line`, about 40 lines that duplicate `integrators::diagnostics::fit_line` on purpose,
  since the alternative is an estimator → integrator edge.
- **The IAS15-driven passage run moved to `ferroastro-ladder::passage_run`.** It contains `find_perihelia` and the
  new `sample_uniform`. `ferroastro-diffrun` does not exist yet: it needs `ferroastro-propagate`, which comes in WP13/14.
  The propagating tests and the benchmark moved with it: PHY-001, NUM-023, the Horizons N-2B/elements tests and the
  Criterion bench (`cargo bench -p ferroastro-ladder --bench two_body`). PHY-004 moved to `ferroastro-analytic`.
- **forces → integrators edge:** `ferroastro-forces` (L6) implements `SecondOrderSystem` from
  `ferroastro-integrators` (L4). This edge points down, so the layering allows it. It is recorded here and in
  `tools/check_layers.py` instead of moving the trait to core. CRATE-PLAN is a scratchpad document, not committed,
  and its "depends on" column omits this edge.
- **`tools/check_layers.py`** runs `cargo metadata --no-deps` and fails on any of the following:
  - an unplaced crate;
  - an upward or same-layer normal or build edge;
  - an upward dev edge (test-support crates `vectors` and `testdata` are exempt);
  - a runtime use of a test-support crate;
  - any cycle, dev edges included.
  
  It runs inside `trace.py --check`, so CI runs it from the python job. GitHub run on `cd8a71a` succeeded, which
  shows cargo is available there. It has 6 pytest cases, one of which checks the real workspace. Current result:
  10 crates, 29 edges, **0 errors**.
- **NEEDS HUMAN REVIEW (layering):**
  1. `ferroastro-cli → ferroastro-vectors` is a runtime edge. The CLI checks rungs against the cited vectors. It is the
     only listed exception in `check_layers.py`. CRATE-PLAN calls `vectors` dev-only.
  2. ARCHITECTURE.md (PR #2) still lists "Event location (r·v = 0) … differential harness" under
     `ferroastro-precession`, and draws forces and integrators as one layer. CRATE-PLAN, and now the code, say
     otherwise. The document needs an update in the docs PR.
  3. No requirement covers crate layering, so the layer tests cite none. Consider a requirement, for example in STD.

### Red-team and review corrections applied
| item | change | result |
|---|---|---|
| IAS15 step-control mutant (1/7 → 1/6) | `ias15_step_control.rs`: (a) from an identical first step the proposal scales as ε^k; (b) the proposal does not depend on the step just taken (1 − 7k = 0) | k = 0.14285714285714285 (exact); dead-beat slope +0.0112. Mutant: k = 0.1667 and −0.150, **both tests fail it** (killed). |
| Review's suggested test (step-count ratio ε vs ε·1e-7 = 10 ± 5 %) | **not used: it cannot kill this mutant.** Every k has the same fixed point (ε/C)^(1/7) when b₇ ∝ dt⁷ | measured ratios 10.66 (k = 1/7) vs 10.56 (k = 1/6), e = 0.5, ε 1e-3 → 1e-10, 20 orbits |
| NUM-023 env knobs | any `FA_NUM023_*` set ⇒ numbers printed, **no assertion** (all three nightly tests) | checked with `FA_NUM023_RUNS=4 FA_NUM023_ORBITS=50`: "REPORT ONLY" |
| NUM-023 long ensemble | `nightly_num023_long_ensemble_known_failure`, 32 × 20 000 orbits, strict known failure (passes while the failure reproduces, fails once both slopes pass); cites no requirement | energy **0.629 ± 0.018** (FAIL vs 0.5 ± 0.1), phase 1.590, 4.89e7 steps, 29–41 s release |
| PHY-001 names IAS15 | test output, rung `dynamics` strings and the `PHY001-NEWTON-RATE` rationale: ε 1e-9, safety 0.25, PC tolerance 1e-16, max 12 iterations | value/tolerance unchanged |
| PHY-006 identical ICs | `SingleBody::ic_hash` (FNV-1a over the IEEE bits of r₀, v₀); every PHY-006 run asserts the β = γ = 1 hash | hash `6596cdfc5a9ca58d` (SW92 IC) |
| NUM-013(a) | `num013_isolation`: GR, J2 (Park J2, equatorial), LT (equatorial) minus a Newtonian test particle from the same IC hash, vs analytic | −6.6e-6, +2.7e-9, −6.3e-9 ″/cy (≤ 1e-4) |
| NUM-012 spread | `num012_estimator_spread`: passages, LRL, Park2017 linear/quadratic (2-d samples, initial-orbit plane) | table below; max spread 1.8e-5 ″/cy (≤ 1e-4) |
| Trace hygiene | dropped `PHY-001` from the SW92 transcription test, the Kepler-utility tests, the Newtonian force-kernel test and the example runners; added `NUM-019` (single-body part only, comment says so) to the Park17 J2/LT data test | trace: 188 citing tests |
| API-003 | `crates/ferroastro-ladder/examples/phy{001,004,005,006,007,008,009,010,011}_*.rs` (100-orbit demonstrations citing vector ids); `api003_examples.rs` runs every `main()` in the PR tier and checks that each implemented PHY rung has an example | 10/10 pass |
| NUM-015 floor, equatorial geometry, PHY-009 per 8A | already done in WP4; unchanged | — |

### NUM-012 cross-estimator spread (single-body rungs, offline, 1000 orbits, ″/cy)
| rung | passages | LRL (= osculating ϖ in-plane) | Park2017 linear (jk σ) | Park2017 quadratic | analytic | spread |
|---|---|---|---|---|---|---|
| SR-a | 7.163412437 | 7.163416199 | 7.163412433 (5.3e-9) | 7.163412464 | 7.163412591 | 3.8e-6 |
| SR-b/harmonic | 50.143885684 | 50.143898126 | 50.143885682 (6.3e-9) | 50.143885671 | 50.143887978 | 1.2e-5 |
| GR-SCHW | 42.980468784 | 42.980486530 | 42.980468786 (3.2e-9) | 42.980468768 | 42.980475410 | 1.8e-5 |
| J2 (Park J2) | 0.028601483 | 0.028601467 | 0.028601479 (4.4e-9) | 0.028601468 | 0.028601476 | 1.6e-8 |
| LT | −0.002018351 | −0.002018349 | −0.002018345 (4.2e-9) | −0.002018353 | −0.002018349 | 7.9e-9 |

- Passages and Park2017 agree to ≤ 2e-8 ″/cy. **The LRL estimator is the outlier for the 1PN rungs** (+1.8e-5 for GR,
  4e-7 relative). The Newtonian eccentricity vector carries 1PN periodic terms, and they do not average out under
  step-time (non-uniform) sampling. This matches NUM-012's note that a 1PN-corrected LRL form is UNVERIFIED.
- The −6.6e-6 ″/cy of GR passages vs "analytic" is the anomalistic vs Keplerian period (1.5e-7 relative, WP4). It
  is not an estimator error.
- **Not delivered:** the "per-orbit mean ellipse" estimator of NUM-012. Osculating Ω + ω equals the LRL longitude in
  the initial orbit plane, so it is covered by the LRL column (the dog-leg variant is in Park2017, WP5).

### WP6 real-data results (data tier, run locally 2026-10-15 UTC, empty `FA_DATA_DIR`, all downloads fresh)
The whole `data` profile ran: **12/12 pass** (21.4 s; the numbers equal WP4/WP5 to every printed digit), then the
new D4 test on a second empty cache:

| test | measured | oracle / tolerance | result | provenance |
|---|---|---|---|---|
| `data_isolation_and_estimator_spread_from_horizons_de441_state`: NUM-013(a) | GR −6.6e-6, J2 +4.8e-9, LT +1.3e-9 ″/cy (differential − analytic, inclined Park17 Eqs. 3/2 at i = 3.381087°); IC hash `1e014210d39b5326` | `NUM013-ISOLATION-DIFF` 0 ± 1e-4 | PASS | Horizons API `signature.version` 1.2, DE441, `IAU_SUN body equator and node of date`, sha256 `f8662aaf…06fdeb6` (timestamped answer: sha changes per download) |
| same test: NUM-012 spread | GR 1.76e-5 (LRL outlier again), J2 3.4e-8, LT 5.2e-8 ″/cy | `NUM012-ESTIMATOR-SPREAD` ≤ 1e-4 | PASS | as above |
| all 12 earlier data tests (moved files included) | e.g. Park17 rate 575.30916, GR from DE441 42.9806442, J2/LT real 0.0284524/−0.0020148 ″/cy | unchanged vectors | PASS | header.440 sha256 `0f4636b6…6ac3359`; approx_pos.html `9f7b30ca…f9c4d742`; Horizons v1.2/DE441 |

### Offline acceptance of the WP6 brief (unchanged physics, re-run)
- PHY-004: 42.980478977 ″/cy (42.9805 ± 1e-4).
- PHY-005: 42.9804688 (42.980 ± 0.001).
- PHY-007: 7.1634124 (± 0.001).
- PHY-008: 50.1438857 harmonic and 35.8170613 areal (± 0.002), each with its gauge label.
- PHY-010: numerical − analytic < 1e-8 for both analytic values, 0.0286015 (J2 = 2.25e-7) and 0.0279168 (DE440 J2).
  The tolerance is ± 0.0005.
- PHY-011: −0.0020184 (± 0.0002).
- NUM-013(a): within 1e-4 (above).
- Layer check: green.

### NUM-023 investigation (time-boxed; nothing committed to the integrator)
Each experiment used a scratch patch that was reverted. The long ensemble is 32 × 20 000 orbits, release build.
| experiment | energy slope | phase slope | ensemble-mean dE/E at 2·10⁴ orbits |
|---|---|---|---|
| baseline (as committed) | 0.629 ± 0.018 | 1.590 | −4.6e-14 (WP3; mean ≈ RMS) |
| start-of-step force `a₀` at the compensated state `x − x_comp, v − v_comp` (node forces already are) | 0.622 ± 0.030 | 1.583 | — |
| **predictor cold-started (B = 0) on every step** | **0.543 ± 0.019** | **1.489** | **+2.3e-14** (RMS 2.7e-14) |
| same, default 64 × 2000 ensemble | 0.483 | 1.473 | — |
- **Finding:** the predictor changes the result, so the claim in WP3 that "steps reach an exact fixed point" is
  wrong. The PC loop stops at the B₇ round-off floor (by tolerance or by stall), and that floor state keeps a
  predictor-dependent bias. With a cold start the slopes pass. The drift is still there, with its sign reversed:
  the mean is 84 % of the RMS. So a cold start is **not** a fix. It only shows that the bias comes from the PC
  termination, not from the force kernel or the state compensation.
- Not done: Everhart's predictor correction. Per the above it would change the bias, not necessarily remove it. A
  candidate real fix is compensated arithmetic in the `g`/`B` updates, or iterating to bitwise B stability. It has
  not been tried. **NEEDS HUMAN REVIEW** (this extends the WP3 item): NUM-001's 1e9-step clause and NUM-023 at ≥ 5e7
  steps remain unmet. The known-failure test guards the status.

### Requirement gap table, M0–M2 (trace.py after WP6; tests / vectors citing)
M0 (ROADMAP row):
- STD-001…011: 0/0.
- STD-012: 1/0.
- TST-001: 0/0.
- TST-002: 12/0.
- TST-003: 3/0.
- TST-004: inspection.
- TST-005: 2/0.
- DEP-001/002/003/014: 0/0.
- ORG-003: 3/0.
- ORG-004: 3/0.
- ORG-008: 0/0.
- PHY-002: 9/20.
- PHY-003: 4/4.
- REF-002: 10/11.
- AGT-001: inspection.

M1:
- API-001: 8/0.
- API-002: 10/0.
- DEP-007/009/010: 0/0.
- NUM-001: 13/2.
- NUM-006: 0/0.
- NUM-010: 0/0.
- NUM-023: 5/2.
- REF-009: 0/0.
- PHY-001: 3/7.

M2:
- PHY-004: 5/7.
- PHY-005: 3/2.
- PHY-006: 2/4.
- PHY-007: 2/2.
- PHY-008: 8/2.
- PHY-009: 6/5.
- PHY-010: 7/6.
- PHY-011: 4/3.
- NUM-011: 12/18.
- NUM-012: 9/1.
- NUM-013: 2/1.
- NUM-015: 13/2.
- NUM-016: inspection, 2/0.
- NUM-017: 4/0.
- NUM-019: 1/0.
- PHY-015: 6/2.
- API-003: 10/0.

The gaps, grouped. The uncited list is 22 items, above the 15 at which CRATE-PLAN says to split. Most of the items are
not WP6 code work, so they are listed rather than split:
| gap | requirements | status / owner |
|---|---|---|
| enforced by a CI gate, no citing test | STD-001…011, TST-001, DEP-001…003, DEP-014, ORG-008 | **NEEDS HUMAN REVIEW:** should `trace.py` accept "verified by CI gate <job/step>" as evidence? Requirement text and trace policy are human-owned, so the agent changed nothing. |
| M1 debt crates not built | DEP-007, DEP-009, DEP-010, NUM-006, NUM-010, REF-009 | WP7 (`ferroastro-real`), WP8 (`erfa`, `time`) |
| cited, acceptance unmet | NUM-001 (1e9-step Brouwer clause; Kozai–Lidov), NUM-023 (≥ 5e7 steps) | known failure above; NEEDS HUMAN REVIEW |
| cited, acceptance partial | NUM-017 (R5 → R3 needs EIH, WP19); PHY-011 (INPOP21a Eq. 1, OI-35); NUM-019 (momentum conservation with N-body J2, WP12/WP20); NUM-012 (no per-orbit mean-ellipse estimator); API-003 (examples per rung done; "every public fn has a doctest" **not** done, and no check exists); NUM-013 (b) (N-body contributions, WP14) | listed WPs |

### Kept for humans (not resolved by any agent)
The following stay as recorded in WP1–WP5: the OI-05 tick, the NUM-015 absolute floor, the NUM-017 areal
exemption, the PHY-009 "8A" wording, the IAS15 rejection rule, the SW92 −5.75 ″/cy gap, OI-35 / INPOP21a Eq. 1,
Park17 heliocentric vs barycentric passages. New in WP6: the CI-gate evidence question, the three layering items
above, and the NUM-023 predictor finding.

### WP6 gate results (2026-10-15, local, toolchain 1.95.0)
All eight AGENTS.md commands passed (exit 0):
- fmt;
- clippy `-D warnings`;
- `cargo nextest run --workspace`: 110 passed, 16 skipped (13 `data_*` + 3 `nightly_*`, the only `#[ignore]`s), plus
  the doctests (including the new `ferroastro-analytic` crate example);
- `cargo deny check`: advisories, bans, licenses and sources ok;
- rustdoc `-D warnings`;
- ruff check/format and mypy strict: 10 files;
- `reuse lint`: compliant;
- `trace.py --check`: 119 requirements, 188 citing tests, 0 errors, layer check included.

Extras:
- `uv run pytest`: 21 passed, 6 of them new layer-check tests.
- MSRV 1.85.0 nextest: 109 passed, trybuild excluded.
- `data` profile on an empty cache: 12/12, then D4 1/1 on a second empty cache.
- `nightly` profile in release: 3/3. The default ensemble gives energy 0.526 and phase 1.513 (PASS). The long
  ensemble gives energy 0.629, a known failure as expected. Reversibility |Δr|/|r₀| = 4.5e-12.
- GitHub Actions on the pushed commits: `cd8a71a`, `df95a49` and `eeb09a4` were green (rust, msrv, python, reuse,
  dco). The intermediate runs were cancelled by newer pushes.

## 📈 Progress review: after WP6

I am the independent progress reviewer (an agent, not the builder). I audited `m0-foundation` at `7333514` (WP1–WP6), stacked on `1-scientific-computing-strategy` (merge base `216c58e`). I ran everything below myself on 2026-10-15 UTC, on the local machine with toolchain 1.95.0. I loosened nothing and committed no mutation. The working tree was clean after every probe.

### Overall health: 🟡 YELLOW
- **Green:**
  - All eight AGENTS.md gates pass.
  - All 13 `data_*` tests pass on an empty cache, and all 3 `nightly_*` tests pass.
  - Three key results reproduce in independent Python.
  - All five non-equivalent mutants (one per changed crate) are killed in the PR tier, including the IAS15 1/7 → 1/6 mutant that survived the last review.
  - Nobody moved or widened any golden value or tolerance.
  - The layer check is green and matches `cargo metadata`.
- **Not green:**
  - NUM-001's 1e9-step clause and NUM-023 at ≥ 5e7 steps are still unmet. The guard is a known-failure test (energy slope 0.629 ± 0.018).
  - Six M1 requirements (the debt crates `real`, `erfa` and `time`) have no citing test.
  - The scheduled `long` (and `data`) CI jobs run on GitHub-hosted runners. That conflicts with the maintainer decision that long tiers run on the local machine.

### 1. Commands (tip `7333514`)
| command | result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo nextest run --workspace` + `cargo test --doc --workspace` | 110 passed, 16 skipped (13 `data_*` + 3 `nightly_*`, the only `#[ignore]`s). 6 doctests pass. |
| `cargo deny check` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` | pass |
| `ruff check`, `ruff format --check`, `mypy` (strict) | pass |
| `reuse lint` | pass |
| `trace.py --check` (layer check included) | pass: 119 requirements, 188 citing tests, 0 errors; `check_layers`: 10 crates, 30 edges, 0 errors |
| `uv run pytest` (tools) | 21 passed |
| `data` profile, empty `FA_DATA_DIR`, every download fresh | **13/13 pass** (22.6 s), with no retry used |
| `nightly` profile (release) | **3/3 pass** (29.7 s). The long ensemble is a *known failure* that is reported as a pass. |

GitHub checks on `7333514` and `63d11a4` succeeded. `data` and `long` run on a schedule only, so they did not run.

### 2. Real-data and benchmark results
Provenance (all retrieved 2026-10-15):
- **header.440:** https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/header.440, sha256 `0f4636b6…6ac3359`, 22802 B, Last-Modified 2020-12-22.
- **SW92:** https://ssd.jpl.nasa.gov/planets/approx_pos.html, sha256 `9f7b30ca…f9c4d742`, 29585 B. Last-Modified is now 2026-10-06, but the sha256 is unchanged.
- **Horizons API:** `signature.version` 1.2, with the DE441 source on every answer. Answers carry a timestamp, so the sha256 differs per download. The Park17 chunks this run were `6cebceeb…`, `032afb05…`, `d5e8bc73…`, `fdafcec7…` and `f4e1becb…`.

Margin = |Δ| / tolerance.

| test | measured | oracle (source) | tolerance | margin |
|---|---|---|---|---|
| `data_de440_table_matches_header` | 23/23 entries equal | header.440 (fresh) | exact | 0 |
| `data_de440_header_ref002_acceptance` | 9 constants \|Δ\| = 0; 645 constants, 409 MA* | header.440 | exact | 0 |
| `data_de440_header_defined_constants` | CLIGHT, AU \|Δ\| = 0 | SI / IAU 2012 B2 | exact | 0 |
| `data_sw92_table1_mercury` | 6 elements + 12 rates equal | SW92 Tables 1 and 2a | exact | 0 |
| `data_horizons_osculating_elements_match` | e 1.08e-15 rel, a 1.29e-16 rel, angles ≤ 2.8e-14° | Horizons ELEMENTS | 1e-12 rel | 0.001 |
| `data_horizons_n2b_from_de441_state` (PHY-001) | 1.087e-9 ″/cy, IAS15 ε 1e-9 | exactly 0 | 1e-6 | 0.001 |
| `data_horizons_gr_rate_from_osculating_elements_vs_park17` | 42.980669 | Park17 Table 3 GE 42.9799 | ±0.0027 (3σ) | 0.28 |
| `data_gr_rung_from_horizons_de441_state` (PHY-005) | 42.9806442 | analytic 42.980651 / Park17 42.9799 | ±1e-3 / ±0.0027 | 0.01 / 0.28 |
| `data_j2_and_lt_rungs_in_real_solar_equator_geometry`: J2 | 0.0284524 (i = 3.381087°) | Park17 0.0286 | ±0.0011 | 0.13 |
| same test: LT | −0.0020148 | Park17 −0.0020 | ±0.0002 | 0.07 |
| same test: NUM-015 | J2 1.29e-7, LT 7.7e-8 rel | analytic Park17 Eqs. 3 and 2 | 1e-6 rel | 0.13 / 0.08 |
| `data_isolation_and_estimator_spread_…` NUM-013(a) | GR −6.6e-6, J2 +4.8e-9, LT +1.3e-9 | `NUM013-ISOLATION-DIFF` 0 | ±1e-4 | 0.07 |
| same test: NUM-012 spread | GR 1.76e-5 (LRL outlier), J2 3.4e-8, LT 5.2e-8 | `NUM012-ESTIMATOR-SPREAD` 0 | ≤1e-4 | 0.18 |
| `data_solar_spin_from_fresh_header_440` | G·S 1.2680765843106158e16 km⁵/s³ | recomputed from header tokens | 8 ulp | 0 |
| `data_sw92_rate_from_horizons_de441_ecliptic` | dog-leg 571.968047 (jk σ 0.147) | SW92 T1 577.716804 | ±20 | 0.29 ⚠ weak oracle; the −5.75 gap is still open |
| `data_park17_rate_from_horizons_de441_mean_orbit` | **575.309163** (jk σ 0.00153), Q −0.0447605 | Park17 575.3100 ± 0.0015 | NUM-011 ±0.01; 3σ ±0.00643 | 0.08; 0.13 (−0.39σ) |
| `nightly_num023_brouwer_slopes_ias15_f64` (64 × 2000) | energy 0.526, phase 1.513 | Brouwer ½, 3/2 | ±0.1 | 0.26 / 0.13 |
| `nightly_num023_long_ensemble_known_failure` (32 × 20 000, 4.89e7 steps) | energy **0.629** ± 0.018, phase 1.590 | ½, 3/2 | ±0.1 | **1.29 FAIL** (known), 0.90 |
| `nightly_num023_forward_backward_reversibility` | \|Δr\|/\|r₀\| 4.50e-12 | — (reported) | — | — |

All numbers equal those in WP4–WP6 to every printed digit.

**Independent recomputation** (Python/numpy, no FerroAstro code). The inputs are this run's fresh header.440 and Horizons answers.
1. **PHY-004/005 GR rate:**
   - From the DE441 Sun-centred state, with μ = GMS = 132712440041.27942 km³/s²: a = 57909074.6365 km, e = 0.2056301618, giving **42.980650809 ″/cy**. FerroAstro's analytic value is identical: 42.980650809.
   - From the Horizons ELEMENTS (a, e) it gives 42.980665 with μ = GMS everywhere. With n built from GMS + GM1 it gives 42.980669, which is FerroAstro's value. That convention (the Mercury mass in n but not in 6πGM/c²) is worth 4e-6 ″/cy. It is undocumented, though harmless.
2. **J2, LT and PHY-014, in the real IAU_SUN geometry:**
   - The in-plane rate is ω̇ + cos i Ω̇. For J2 the factor is (1 − 1.5 sin² i), and for LT it is cos i. I get i = 3.381087°.
   - J2 is **0.0284524** at Park J2 and 0.0277713 at DE440 J2. LT is **−0.0020148**, with G·S = 1.2680765843106158e16. All three are identical to FerroAstro.
   - PHY-014 shift: 0.0286·(J2SUN/2.25e-7 − 1) = −0.000685, which gives **575.309315**. That agrees with the maintainer's 575.3093.
3. **Reproduced benchmark: the SW92 dog-leg rate from DE441, 1800–2050.**
   - My own estimator: r·v sign change on the 1-day rows, a degree-7 polyfit over 8 rows, bisection on r·v, then osculating Ω + ω at the passage, then a linear OLS fit.
   - It gives **571.968049 ″/cy** (1038 passages, ϖ(J2000) 278841.932″), against FerroAstro's 571.968047. Δ = 2e-6 ″/cy.
   - The −5.75 ″/cy gap to SW92 Table 1 is therefore a property of the data, not of the estimator.

   (The last review reproduced the Park17 575.309 rate independently; I did not repeat it.)

### 3. Drift audit
- **`git log -p -- tests/vectors` since `216c58e`:** 11 commits. Since the last review (`9ec1f86`) there are 2 commits:
  - `PHY001-NEWTON-RATE`: rationale and `verified_by` text only. Value 0 and tolerance 1e-6 are unchanged.
  - 4 new vectors: `IAS15-EQ11-EPS-EXPONENT`, `IAS15-EQ11-DEADBEAT`, `NUM013-ISOLATION-DIFF` and `NUM012-ESTIMATOR-SPREAD`.

  **No value or tolerance was moved or widened.**
- **Provenance nits on the new vectors** (low severity):
  - (a) `IAS15-EQ11-DEADBEAT`'s band 0 ± 0.05 was chosen after seeing +0.0112 (correct code) and −0.150 (mutant). This is disclosed in the rationale and in WP6, with an independent derivation, but it carries no NEEDS HUMAN REVIEW tag.
  - (b) The `source` of `NUM012-ESTIMATOR-SPREAD` says the periodic terms "average out over 1000 orbits". WP6's own measurement shows that the 1PN terms of the LRL vector do **not** average out under step-time sampling (GR spread 1.76e-5). The text should say so. Only the provenance text changes; the value and tolerance stay.
- **`#[ignore]`:** 16, all named `data_*` (13) or `nightly_*` (3). I ran all 16. I found no `cfg` gating or early return.
  - The NUM-023 env knobs are now report-only (fixed).
  - **New low-severity route:** `FA_VECTORS_DIR` (`ferroastro-vectors`) redirects the whole golden directory, and the CLI honours it too. CI does not set it, but a single env change could substitute oracles (red-team H2 pattern).
  - The `data` profile has `retries = 1`. A flaky physics assertion would pass on retry and show only as FLAKY. Nothing was retried this run.
- **UNVERIFIED values as oracles:** none. The NUM-013(a) and NUM-012 vectors are bounds copied from requirement text. The 1PN-corrected LRL (UNVERIFIED in NUM-012) is not used.
- **Circular oracles:** none that matter. NUM013-ISOLATION compares a FerroAstro differential with FerroAstro analytic formulas. It is not circular in practice, because item 2 above re-derives those formulas independently, and the data tier checks Park17 Table 3.
- **Tests that cannot fail:** `nightly_num023_long_ensemble_known_failure` passes *while the physics fails*. That is acceptable as a documented guard: it cites no requirement and is labelled KNOWN FAILURE. Even so, a green nightly does not imply NUM-023.
- **Mutation probes** (local; each was reverted with `git checkout`, and `git status` was clean at the end):

| crate | mutant | PR-tier result |
|---|---|---|
| analytic (new) | inclined-J2 factor 1 − 1.5 sin² i → 1 − 1.4 sin² i | killed (`phy010_j2` inclined, CLI report) |
| analytic (new) | GR 6π → 6.006π | killed (PHY-004, PHY-005, API-003 example) |
| integrators | IAS15 step exponent 1/7 → 1/6 (survived last review) | **killed** (both `ias15_step_control` tests) |
| precession | `fit_line` slope × (1 + 1e-7) | killed (3 tests) |
| ladder (`passage_run`, moved) | no Newton refinement of passages | killed (21 tests) |
| ladder `sample_uniform` | sample times × (1 + 1e-6) | survived. This is an **equivalent mutant**: the integration target and the label move together, so it only changes the step size. The data tier gives the same rates. No finding. |

- **Layering** (`cargo metadata`, cross-checked by hand): no cycles and no upward edges.
  - `precession` no longer depends on `integrators` (fixed).
  - The analytic oracles are in `ferroastro-analytic` (fixed).
  - Documented deviations: `forces → integrators` is a downward edge. The `cli → vectors` runtime edge is the single listed exception, with NEEDS HUMAN REVIEW.
  - Minor: FA-E0601 ("fit degenerate", the 06xx precession range) is also raised by `integrators::diagnostics::fit_line` (04xx range), and FA-E0600 now lives in `ferroastro-analytic`. The core error-range table should say which crate owns 06xx.
- **Maintainer-decision conflict:** `.github/workflows/ci.yml` jobs `long` (nightly tier, `timeout-minutes: 90`) and `data` run weekly on `ubuntu-24.04` GitHub-hosted runners. The decision in force is that long/nightly tiers run on the local machine and that GitHub runners do PR-fast work. Today the tier takes 30 s, so the cost is nil. It will not stay that way once M3–M5 long runs land.

### 4. Requirement coverage (`trace.py`: 31/92 `test` requirements cited; 188 citing tests)
- **M0, uncited:**
  - STD-001…011
  - TST-001
  - DEP-001, DEP-002, DEP-003, DEP-014
  - ORG-008

  All of these are enforced by CI gates rather than citing tests. The question "may trace.py accept gate evidence?" is still open for humans (unchanged).
- **M1, uncited:** DEP-007, DEP-009, DEP-010, NUM-006, NUM-010, REF-009. These are WP7 (`real`) and WP8 (`erfa`, `time`).
- **M1, cited but acceptance unmet:** NUM-001 (the 1e9-step clause; Kozai–Lidov) and NUM-023 (≥ 5e7 steps).
- **M2:** every requirement is cited. NUM-019 and API-003 are newly cited.
  - Partial: NUM-017 (needs EIH), PHY-011 (OI-35), NUM-012 (no per-orbit mean-ellipse estimator), NUM-013(b) (N-body).
  - API-003: the "every public fn has a doctest" clause is not met. Only 6 doctests exist across 10 crates, and nothing checks the clause.
  - The M2 exit criterion holds: `ferroastro ladder --rungs single-body` runs in the PR tier, and every rung is within tolerance.

### 5. Corrections for the next builder (WP7 onwards)
1. **CI tier placement:** remove the `schedule` trigger from the `long` job, keeping only `workflow_dispatch` or removing the job. Add a local runner script, e.g. `tools/run_long_local.sh`, that runs the nightly and data tiers with release settings and writes a log that includes the commit, date, toolchain and provenance. This applies the decision "long tiers run locally". Ask the maintainer whether the weekly `data` job may stay on GitHub. Do not touch rulesets or CODEOWNERS.
2. **Provenance text, metadata only, with a NEEDS HUMAN REVIEW note in IMPLEMENTATION-NOTES:**
   - Add NEEDS HUMAN REVIEW to `IAS15-EQ11-DEADBEAT` (its band was not chosen blind).
   - Correct the "average out" sentence in `NUM012-ESTIMATOR-SPREAD`'s `source`: LRL 1PN periodic terms do not average out under step-time sampling (WP6 measurement).
   - Leave every value and tolerance unchanged.
3. **Oracle-substitution routes:**
   - Make `FA_VECTORS_DIR` print a loud "NON-CANONICAL VECTORS" banner, or refuse the override in `data_*`/`nightly_*` tests and the CLI report.
   - In the `data` profile, scope `retries` to network errors, or set it to 0 and let the testdata client retry downloads itself.
4. **GR convention:** document, in the analytic GR helper and the `data_horizons_gr_rate_…` output, which μ goes into n and which into 6πGM/c². The 4e-6 ″/cy difference is harmless now, but it will matter at NUM-013's 1e-4 once differences are taken.
5. **Error-code ranges:** update the core `error.rs` range table so that FA-E06xx names `ferroastro-analytic` as well as precession. Give `integrators::diagnostics` its own 04xx code, or document why it shares FA-E0601.
6. **WP7 (`ferroastro-real`) and WP8 (`erfa`/`time`) must close DEP-007/009/010, NUM-006, NUM-010 and REF-009 with citing tests and vectors.** The double-double backend is also the natural place to retry the NUM-023 fix: compensated `g`/`B` updates, or iterating to bitwise B stability. Keep the known-failure guard until both slopes pass at 32 × 20 000, and do not widen ±0.1.
7. **API-003:** add a doctest-coverage check (e.g. `missing_doc_code_examples` is unstable, so use a script over `cargo doc` JSON or a grep in `trace.py`), or record in NEEDS HUMAN REVIEW that the "every public fn" clause is deferred.
8. **Still for humans (unchanged; no agent may resolve these):**
   - OI-05 tick
   - NUM-015 floor (the GR data run prints "floor exceeded" and passes on the relative bound)
   - NUM-017 areal exemption
   - PHY-009 "8A" wording
   - IAS15 rejection rule
   - SW92 −5.75 ″/cy gap (now reproduced independently, so it is not an estimator error)
   - OI-35
   - Park17 heliocentric vs barycentric passages
   - CI-gate evidence in trace
   - the three WP6 layering items
   - NUM-023 predictor finding

Reviewer artefacts are in the local scratchpad, not committed: the gate, data, nightly and mutation logs, `recompute.py` and `mutate.sh`.

## WP7 — [M1 debt] `ferroastro-real`: `Real` trait, double-double backend, generic refactor

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13. The section has
three parts:
- the progress-review corrections after WP6;
- WP7 itself;
- a time-boxed NUM-023 retry.

No existing golden value or tolerance was changed. Two vectors had metadata-only edits. One new vector had its test
domain amended (below), all flagged **NEEDS HUMAN REVIEW**.

### Progress-review corrections (applied)
| correction | change |
|---|---|
| Long tiers run locally | `ci.yml` job `long` is `workflow_dispatch` only. New `tools/run_long_local.sh` runs the `nightly` (release) and `data` profiles and logs the commit, dirty flag, UTC date, host, `rustc -Vv`, cargo and nextest versions, `FA_DATA_DIR`, and the full test output with each product's provenance to `target/long-local/<stamp>-<sha>.log`. It refuses to run if `FA_VECTORS_DIR` is set. **Maintainer question (open):** may the weekly `data` job stay on GitHub-hosted runners? It is unchanged until answered. Rulesets and CODEOWNERS were not touched. |
| `IAS15-EQ11-DEADBEAT` | `notes` now says NEEDS HUMAN REVIEW (non-blind band). Value and tolerance unchanged. |
| `NUM012-ESTIMATOR-SPREAD` | The `source` no longer says that the LRL 1PN terms "average out". It cites the WP6 +1.8e-5 ″/cy LRL offset under step-time sampling. Value and tolerance unchanged. **NEEDS HUMAN REVIEW** (metadata edit of a golden). |
| Oracle substitution via `FA_VECTORS_DIR` | Every use prints a `NON-CANONICAL VECTORS` banner. The new `VectorSet::load_canonical()` refuses the override with the new code **FA-E0106**. The `data_*`/`nightly_*` tests, the ladder rung tests (`tests/common`), the examples and the CLI report use it. `cli_report.rs::report_refuses_a_vector_directory_override` runs the binary with the override and checks FA-E0106 and the banner. Mutation probes must now perturb `tests/vectors` in a scratch checkout. `load_workspace()` keeps the override, with the banner, for PR-tier probes. |
| Data-profile retries | `.config/nextest.toml` `data`: `retries = 0`. Network retries live in `ferroastro-testdata` (`curl --retry 3 --retry-connrefused`) and in the Horizons client (one re-fetch of an unparsable answer). |
| μ convention of the GR helper | Documented in `schwarzschild_advance`: `6πGM/c²` uses GM☉, while `n` and the period use GM☉ + GM☿, the Keplerian GM of Horizons' elements. `data_horizons_gr_rate_from_osculating_elements_vs_park17` also prints the GM☉-only rate and its shift (about 4e-6 ″/cy, reported only). |
| Error-code ranges | The core `error.rs` table names `ferroastro-analytic` (FA-E0600) and `ferroastro-precession` (FA-E0601–0603) for 06xx. `integrators::diagnostics` fits now raise **FA-E0404** (`DiagnosticFitDegenerate`) instead of sharing FA-E0601. |
| API-003 doctest clause | `tools/doctest_coverage.py` (2 pytest cases) runs as a CI step that only reports. Result: **0 of 258** public functions carry a runnable per-function doctest. The existing doctests are crate- or module-level. **NEEDS HUMAN REVIEW:** the "every public fn has a doctest" clause is deferred. A maintainer sets the `--min-ratio` gate. |
| WP7/WP8 M1 requirements | WP7 adds citing tests and vectors for DEP-007, NUM-006 and NUM-010 (and DEP-012, API-001). DEP-009, DEP-010 and REF-009 belong to WP8 (ERFA, time) and remain uncited here. |
| Human-owned questions | Left unresolved, as listed in the WP6 progress review. |

### What was built
- **`ferroastro-real` (L0, new).**
  - The `Real` trait covers:
    - arithmetic and its assignment forms;
    - `mul_add`, `scale` (× binary64), `sqrt`, `hypot`, `recip`, `abs`, `min`/`max`;
    - `is_finite`/`is_nan`, `total_cmp`, `floor`/`round`/`rem_euclid`, `powi`;
    - the elementary functions the oracles use: `sin`, `cos`, `sin_cos`, `atan2`, `asin`, `acos`, `exp`, `exp_m1`, `ln`, `ln_1p`, `powf`;
    - constants (`ZERO`, `ONE`, `TWO`, `HALF`, `PI`, `TAU`, `FRAC_PI_2`, `LN_2`, `EPSILON`) and conversions (`from_f64`, `from_parts`, `from_i32`, `to_f64`, `to_parts`).
  - Backends:
    - **f64**: inherent methods with `std`, or the pure-Rust `libm` in `no_std`.
    - **`DoubleDouble`**: own implementation of JMP 2017 Alg. 6, 9, 12 and 18 and Lefèvre et al. 2023 Alg. 8. Its own `exp`/`ln`/trigonometric functions are described in `dd_math.rs`.
    - **f128** behind feature `f128`. `build.rs` detects a nightly compiler and compiles the backend only there, so `--all-features` still builds on stable.
  - `Neumaier<R>` compensated accumulator.
  - The crate is `#![no_std]` (`std` default feature; `libm` alternative).
- **Generic refactor, `R: Real = f64` everywhere, M0 call sites unchanged.**
  - `ferroastro-core`: quantities, `Position`/`Displacement`/`State` and `Epoch` (still a two-part JD). The f64-only consts (`Length::ZERO`, `Epoch::j2000()`) sit in `impl X<f64>`. Generic `zero()` and `cast()` are new.
  - `ferroastro-integrators`: `Ias15`, `SecondOrderSystem`, `StepReport`, the Kahan helpers, and Radau constants built in `ferroastro-real`'s double-double. The local `dd` module is deleted.
  - `ferroastro-forces`: every term and model, plus `ForceModel`.
  - `ferroastro-analytic`: every oracle, `Elements` and the vector helpers.
  - `ferroastro-ladder`: `SingleBody::run_in::<R>` and `model_in::<R>`, and the generic `find_perihelia`.
  - The IAS15 step-size factor is computed in f64 for every backend; it chooses Δt and does not affect accuracy.
- **f64 results are bitwise identical.** Every rate, IC hash and step count printed by the PHY-001/005/010/011 and NUM-012/013 tests was diffed against a pre-refactor capture after each crate's refactor: identical except wall times.
- **NUM-010 (new).**
  - Opt-in dense output (`Ias15Config::dense_output`; off by default, so the hot path does not copy): `Ias15::dense_state`, `last_step_span`.
  - `Ias15::locate_event(g)`: Illinois regula falsi on the last step's collocation polynomial, solved in the step fraction τ.
- **CI:** new `wasm32` job (PR-fast). It builds real, core, integrators, forces and analytic for `wasm32-unknown-unknown`, plus `ferroastro-real` as `no_std` + `libm`. Both builds were verified locally on 1.95.0.
- **NUM-006 benchmark harness:** `tools/bench_num006.py` (1 pytest case).
  - It puts a baseline and a candidate into detached worktrees, gives both the baseline's bench source, and runs Criterion `ias15_sun_mercury/propagate_1_julian_year` alternately for 5 rounds.
  - It compares the medians of the medians against vector `NUM006-F64-BENCH-RATIO` (2 %).

### DD backend choice (ADR-0002 follow-up; recorded audit, scratch crate, not committed)
100 000 random normalised double-words (|exponent| ≤ 60), oracle astro-float at 384 bits. Max relative error is in u² = 2⁻¹⁰⁶; speed is ns/op, the median of 5 passes over 10⁶ operations, release build. Same machine, 2026-10-15.

| op | own: error | own: ns/op | `qd` 0.8.0 (MIT): error | `qd`: ns/op | `twofloat` 0.8.4 (BSD-3): error | `twofloat`: ns/op |
|---|---|---|---|---|---|---|
| add | 1.91 | 4.3 | **47.1** | 6.4 | 1.91 | 4.2 |
| mul | 2.99 | 8.0 | 3.11 | 7.8 | 2.99 | 8.2 |
| div | 4.44 | 14.7 | 1.70 | 31.2 | **9.0e15** (binary64-accurate only) | 13.0 |
| sqrt | 2.75 | 10.5 | 7.05 | 63.7 | 7.88 | 10.4 |

**Decision: our own double-double.**
- It is best or equal on accuracy everywhere except `qd`'s division, and it stays within the proved bounds.
- Its speed equals `twofloat`'s, and it is 2–6× faster than `qd` on div and sqrt.
- `twofloat`'s division is only binary64-accurate, and `qd`'s addition is the "sloppy" one.
- It adds no runtime dependency.

**NEEDS HUMAN REVIEW:** record this as the ADR-0002 follow-up. ADR text is human-owned and was not edited.

### DEP-012 results (`crates/ferroastro-real/tests/dep012_ulp_bounds.rs`, proptest, deterministic ChaCha, vs astro-float 384 bit)
All 26 bounds were pre-registered in `tests/vectors/real-backends.toml` before the first run. The proved bounds cite the theorem. The elementary-function bounds are engineering bounds and are labelled as such.

| op (cases) | measured max | bound | source |
|---|---|---|---|
| DD add / sub / sub same-exponent (4096 each) | 1.35 / 1.43 / 0.99 u² | 3 | JMP 2017 Thm 3.1 |
| DD mul / scale / mul_add | 3.06 / 1.23 / 2.59 u² | 5 / 2 / 8 | Thm 5.4 / Thm 4.3 / derived |
| DD div / recip | 3.99 / 2.69 u² | 9.8 | Thm 7.2 |
| DD sqrt / hypot | 2.41 / 2.96 u² | 3.125 / 8 | Lefèvre et al. 2023 Thm 3.6 / derived |
| DD exp (x ∈ [−600, 700]) / expm1 | 1.82 / 4.87 u² rel. | 128 | engineering |
| DD ln / ln near 1 / ln1p | 1.71 / 3.63 / 4.43 u² rel. | 128 | engineering |
| DD powf | 197.9 u² rel. | 256 | engineering (condition number \|y ln x\| ≤ 140) |
| DD sin / cos / asin / acos / atan2 | 1.18 / 0.94 / 2.10 / 3.07 / 4.46 u² abs. | 128 | engineering |
| f64 add, sub, mul, div, sqrt, recip, mul_add | ≤ 0.5000 ulp | 0.5 | IEEE 754-2019 §5.4.1 |

The f64 bounds also pass on the `no_std` `libm` path.

The first run found two real problems:
1. **`ln_1p` truncation bug, fixed:** 274 u² at x = −0.398. The atanh series was sized for |z| ≤ 0.1716 but was used up to |x| < ½, where |z| reaches ⅓. The series is now restricted to x ∈ (−0.29, 0.41).
2. **`exp` underflow of the trailing word:** 2.3e-17 relative at x = −688.2. Below x ≈ −634.8 the result is under 2⁻⁹¹⁶, so its trailing word is subnormal. This is a format limit ("barring underflow" in JMP 2017). **NEEDS HUMAN REVIEW:** the pre-registered test domain [−700, 700] was amended to [−600, 700] after this failure. The bound was unchanged, and the vector's `notes` say so. The new lower end comes from the format limit, not from the measurement.

`hypot` was added to `Real` during the core refactor. Its bound (8 u²) was pre-registered before its first run.

Sources (read for the bounds):
- JMP 2017: HAL hal-01351529v3, sha256 `2a820178…6cbc0`.
- Lefèvre et al. 2023: HAL hal-03482567v2, sha256 `a60eb7ed…8ac93`.

### NUM-006, DEP-007 and API-001 results
- **Monomorphisation** (`num006_backends.rs`): one generic Kepler solver.
  - Residual: 2.2e-16 in f64 and 1.9e-34 in DD.
  - On **nightly** (`rustc 1.101.0-nightly 2026-10-14`, installed user-level with `rustup toolchain install nightly --profile minimal` for this check only) the f128 backend matches DD to ≤ 8.2e-33 for sqrt, exp, sin and ln (`cargo +nightly test -p ferroastro-real --features f128`).
  - On stable the f128 test is compiled out, so CI does not exercise f128.
- **f64 benchmark unchanged (NUM-006):** `tools/bench_num006.py`, baseline `c5fd5a0` (non-generic integrator and forces), 5 alternated rounds, ratio = candidate/baseline:

  | candidate | what is generic | ratio | verdict (±2 %) |
  |---|---|---|---|
  | `a913cc4` | integrator | 0.9922 | PASS |
  | `6b39fcd` | integrator + forces + analytic + ladder | 0.9846 | PASS |
  | `42b4a99` | final WP7 code, including the opt-in `pc_b_from_g` branch | 1.0048 | PASS |

  - In the `42b4a99` run the candidate was slower in all 5 rounds (by 0.2–1.3 %), probably the extra `pc_b_from_g` branch per node update.
  - In the `6b39fcd` run the candidate was faster in all 5 rounds. Its round medians were 2.11–2.29 ms, against the baseline's 2.18–2.33 ms.
  - The machine is shared: load average about 13 on 32 cores during the runs. A single-round plumbing run on identical integrator code showed a 6.5 % spread, so one round is not evidence.
  - **NEEDS HUMAN REVIEW:** the 2 % band is PROVISIONAL (vector `NUM006-F64-BENCH-RATIO`).
- **API-001 with DD:**
  - The trybuild suite now has 9 compile-fail cases: the 4 M0 classes in f64, the same 4 in `DoubleDouble`, and the new class "backend mismatch" (`Length<f64> + Length<DoubleDouble>`).
  - One compile-pass case composes the newtypes in DD.
  - The WP1 deviation is closed. The f64 `unit_mismatch.stderr` gained a rustc `= note:` line (same error E0308).
- **DEP-007 coverage.** Each component compiles and passes its tests with f64 and DD.
  - **Integrator:** `num010_dense_events`.
  - **Force model:** `double_double_terms_match_f64_and_each_other` checks every term: DD rounds to the f64 acceleration within f64 round-off, and the R3 and PPN-Lagrangian forms agree to DD round-off.
  - **Analytic oracles:** PHY-004 in DD gives 42.98047897736962 + 1.3e-15 against f64 42.980478977369629, 1 ulp apart. The elements round trip closes to DD round-off.
  - **Not covered:** the "ephemeris evaluator" (WP10) and "time arithmetic" (WP8) clauses. Only the `Epoch` type is generic so far.

### PHY-005 in f64 and double-double (pre-registered `PHY005-F64-DD-AGREEMENT`, 1e-6 ″/cy)
| initial state | f64 ″/cy | double-double ″/cy | f64 − DD | steps; wall (opt-level 1) |
|---|---|---|---|---|
| SW92 J2000 mean elements (PR tier) | 42.980468783683 | 42.980468783558 | +1.25e-10 | 76 468; 0.59 s f64, 3.29 s DD |
| header.440 GROUP 1041: Mercury − Sun at JDEPOC 2440400.5 (data tier, D5) | 42.980085159575 | 42.980085160105 | −5.30e-10 | — |

- Both rows pass 42.980 ± 0.001 (PHY005-GR-NUM-RATE) and the agreement bound, with about 2000× margin.
- The header.440 state gives a = 0.38709935 au and e = 0.2056166109 at JDEPOC.
  - Centre, axes and units (SSB, ICRF, au and au/day) are inferred, per OI-23. They are used only as an initial condition, never as an oracle; a units error would move the rate by orders of magnitude.
  - The heliocentric state is X1 − XS. The barycentric Sun's motion is ignored in this test-particle rung.
- NUM-015 for D5: relative −4.4e-8 passes. The absolute 1.2e-15 rad floor is "exceeded", as in WP4: anomalistic vs Keplerian period. That item stays with humans.

### NUM-010 results (`num010_dense_events.rs`)
Keplerian Mercury-like orbit: DE440 GM☉, SW92 a and e, aphelion start, 100 orbits, 7660 steps.

| check | measured | bound | vector |
|---|---|---|---|
| f64 dense-output event location vs the integrated trajectory's own r·v root (every 10th event re-integrated) | 1.69e-12 s | 1e-6 s | `NUM010-EVENT-LOCATION` |
| double-double event times vs analytic P/2 + kP | 1.16e-15 s | 1e-6 s | `NUM010-KEPLER-PERIHELION-TIMES` |
| f64 event times vs analytic (reported only) | 2.18e-11 s | — | — |

Interpretation: NUM-010's "≤ 1e-6 s" is read as the location error. "Match the analytic values to round-off" is shown in DD, where round-off is negligible.
- The f64 drift is reported, not asserted. My pre-registration estimate of 1e-5 s for it was far too pessimistic: measured 2.2e-11 s.
- The ladder's passage refinement still re-integrates, to keep f64 results bitwise identical. Switching it to dense output is a later option.

### NUM-023 retry (time-boxed; WP6 item)
32 × 20 000-orbit ensemble (4.89e7 steps), release build, report-only env knobs. The baseline is the WP6 known failure.

| variant | energy slope | phase slope | ensemble mean dE/E at 2·10⁴ orbits (RMS) | wall |
|---|---|---|---|---|
| baseline: `B += c·Δg` increments (as committed) | 0.629 ± 0.018 | 1.590 | ≈ RMS, −4.6e-14 (WP3) | 27–41 s |
| PC iterated until all B are bitwise stable | 0.619 ± 0.021 | 1.576 | — | 27 s |
| **B recomputed from g** each node update, Neumaier sum | 0.474 ± 0.015 | 1.443 | −5.4e-15 (1.34e-14) | 41 s |
| **B recomputed from g**, plain FMA sum | 0.509 ± 0.012 | 1.471 | −6.9e-16 (1.52e-14) | 47 s |
| same, reversed sum order (as committed) | 0.466 ± 0.011 | 1.442 | +3.0e-15 (1.51e-14) | 58 s (2 tests in parallel) |
| same, default 64 × 2000 ensemble | 0.436 ± 0.011 | 1.411 | −7.6e-16 (3.7e-15) | 15 s |

**Finding:** the systematic energy drift comes from accumulating B by increments `c[node][m]·Δg`. It is not from the PC stopping rule; bitwise convergence does not help. It is not from missing compensation either; a plain recompute works.
- With B recomputed from the Newton coefficients, both slopes are within NUM-023's ±0.1 at ≥ 5e7 steps.
- The ensemble mean falls to 5–40 % of the RMS. It was ≈ 100 % before.

**Committed as opt-in:** `Ias15Config::pc_b_from_g` (default **off**) and the report-only knob `FA_NUM023_PC_B_FROM_G=1`. Nothing else changed:
- the default integrator;
- `nightly_num023_long_ensemble_known_failure`, which still guards the default configuration;
- the ±0.1 tolerance.

**NEEDS HUMAN REVIEW:** adopting it as the default would:
- change every f64 result at round-off level;
- add about 1.5× wall time in this experiment (a NUM-006 question);
- turn the known-failure guard green.

The NUM-001 1e9-step clause was not tested. These ensembles total about 5e7 steps.

### DEP-015 provenance checklist (new dependencies in Cargo.lock)
| crate | role | licence (file read) | maintenance | WASM |
|---|---|---|---|---|
| `libm` 0.2.16 | optional runtime dep of `ferroastro-real` (`no_std`) | MIT (`LICENSE.txt` read; rust-lang/compiler-builtins) | latest 0.2.16, 2026-03-17 | pure Rust; built for wasm32 |
| `astro-float` 0.9.6 (+ `-num` 0.3.7, `-macro` 0.4.6) | dev-dependency only (DEP-012 oracle) | MIT (`LICENSE` read: "Copyright (c) 2022 stencillogic"). **The `-num` and `-macro` packages ship no licence file; only their SPDX field says MIT.** NEEDS HUMAN REVIEW: accept the repository licence for them. | releases 2026-08-07 | not needed (dev only) |
| `proptest` 1.11.0 | dev-dependency | MIT OR Apache-2.0 (both files present) | 1.12.0 released 2026-10-10; resolver held 1.11.0 for MSRV 1.85 | dev only |
| transitive: `rand*` 0.9, `rand_xorshift`, `unarray`, `bitflags`, `lazy_static`, `getrandom` 0.3, `syn` 1, `wasip2`, `wit-bindgen`, `r-efi`, `ppv-lite86` | dev-only, via proptest and astro-float | all MIT and/or Apache-2.0. `r-efi` is "MIT OR Apache-2.0 OR LGPL-2.1-or-later", taken under MIT. `wasip2` and `wit-bindgen` add "Apache-2.0 WITH LLVM-exception" | `unarray` last release 2022-11-30 (> 24 months; dev-only transitive of proptest; justification: test-only) | — |

`cargo deny check` passes. `anise` is not a dependency (unsettled decision). `qd` and `twofloat` were used only in the scratch audit crate and are not in Cargo.lock.

### Interpretations and NEEDS HUMAN REVIEW (WP7)
1. **ADR-0002 follow-up:** the DD backend is our own (audit above).
2. **`NUM006-F64-BENCH-RATIO`:** the 2 % band is provisional. The measurements are on a shared machine.
3. **`REAL-DD-EXP-REL-ERR`:** test domain amended after the first run (format underflow). Bound unchanged.
4. **Engineering bounds:** the DD elementary-function bounds (128 and 256 u²) are FerroAstro's own and are not proved. DEP-012 asks for "documented ulp bounds", and these are documented and pre-registered.
5. **NUM-023:** whether to adopt `pc_b_from_g` (above).
6. **API-003:** doctest-coverage threshold (above).
7. **DEP-015:** licence files of `astro-float-num` and `-macro`.
8. **Maintainer question:** the weekly GitHub `data` job (above).
9. **DEP-007 "ephemeris evaluator" and "time arithmetic":** not covered until WP10 and WP8.
10. **Requirement-text suggestions** for PR #2 (not edited here):
    - NUM-010 could state that "≤ 1e-6 s" is the location error, and that "to round-off" is backend-dependent.
    - DEP-012 could name the u² unit for double-word bounds.
    - NUM-006 could carry the benchmark band.
11. **Nightly toolchain:** installed user-level (`~/.rustup`) only to exercise `--features f128`. `rust-toolchain.toml` is unchanged.

### WP7 gate results (2026-10-15, local, toolchain 1.95.0, tip before this notes commit `2ea1523`)
All eight AGENTS.md commands passed (exit 0):
- fmt;
- clippy `-D warnings`;
- `cargo nextest run --workspace`: 130 passed, 17 skipped (14 `data_*` + 3 `nightly_*`), plus 8 doctests;
- `cargo deny check`;
- rustdoc `-D warnings`;
- ruff check/format and mypy strict (14 files);
- `reuse lint`;
- `trace.py --check`: 119 requirements, 234 citing tests, 0 errors, layer check included.

Extras:
- `uv run pytest`: 24 passed.
- MSRV 1.85.0 nextest without trybuild: 129 passed.
- wasm32 builds: both pass.
- Nightly `+nightly --features f128` tests of `ferroastro-real`: pass.

### WP7 long tiers (`tools/run_long_local.sh`, empty `FA_DATA_DIR`, every download fresh; log `target/long-local/20261015T072402Z-2ea1523247dd.log`)
- **nightly:** 3/3 pass.
  - Default NUM-023 ensemble: energy 0.526, phase 1.513.
  - Long ensemble: 0.629 / 1.590, the known failure as before.
  - Reversibility \|Δr\|/\|r₀\| = 4.50e-12.
- **data:** 13/14 passed on the first run. `data_horizons_osculating_elements_match` failed with **FA-E0200**: Horizons answered HTTP 503 to four curl attempts. That is a network error, not an assertion. With test-level retries now 0 (above), it shows as a failure. Run again alone on the same cache, it passed:
  - e relative diff 1.08e-15;
  - a relative diff 1.29e-16;
  - angles ≤ 2.8e-14°.
- **New D5:** f64 42.980085159575, DD 42.980085160105, difference −5.30e-10 ″/cy.
  - header.440 sha256 `0f4636b6…6ac3359`, Last-Modified 2020-12-22, retrieved 2026-10-15.
- **Unchanged from WP6, to every printed digit:**
  - Park17 frame: 575.309163 (jack-knife σ 0.00153).
  - SW92 dog-leg: 571.968047.
  - GR from DE441: 42.9806442.
  - Real-geometry J2 and LT: 0.0284524 / −0.0020148.
  - NUM-013(a) and the NUM-012 spread.
  - Horizons provenance on every answer: `signature.version` 1.2, ephemeris DE441.
- **New output line:** the Horizons GR-rate test now prints the μ convention: GM☉ in both places gives 42.980665, a shift of −3.57e-6 ″/cy.

## WP8 — [M1 debt] `ferroastro-erfa` + `ferroastro-time`: ERFA port and time scales

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13.

### Plan and pre-registered checks (written 2026-10-15, before any golden, round-trip or hifitime run)
The t_erfa_c check (1) was run once while this section was being written; every other number
below was fixed before its first run.

**Sources.**
- ERFA 2.0.1 release tarball `erfa-2.0.1.tar.xz`, SHA-256 `45111f0a…8c20e4`, tag `v2.0.1`, commit `9915ba38…8dee3`.
  It is the version that pyerfa 2.0.1.5 bundles (`erfa.version.erfa_version` = 2.0.1, SOFA 20231011).
- Only ERFA files were opened: the 11 routines, `erfam.h`, `t_erfa_c.c` and `LICENSE`. No SOFA file was opened or downloaded (LIC-004).
- The 787 × 3 Fairhead & Bretagnon table is copied mechanically by `tools/erfa_dtdb_table.py`, keeping the literal text. The input `dtdb.c` has SHA-256 `228bd7c7…442105`.

**Oracles and tolerances.**
| # | check | oracle | tolerance (pre-registered) | reason |
|---|---|---|---|---|
| 1 | each ported routine, f64 and DD (DEP-009) | ERFA `t_erfa_c.c` values, 23 vectors `ERFA-T-*` | ERFA's own `dval`, verbatim | DEP-009: "reproducing ERFA's reference values" |
| 2a | TT→TAI, TT→TDB, TDB→TCB, TT→TCG and the inverses, ≥ 200 epochs 1550–2650, f64 and DD (REF-009, DEP-010) | pyerfa 2.0.1.5 goldens (`tools/goldens/erfa_time.py`) | 1 ns per date, **PROVISIONAL** | Same algorithm. The residual is two-part-JD round-off, about 1e-11 s per operation. The TDB→TT inverse is iterated to a fixed point (below), whereas pyerfa evaluates `dtdb` once at the TDB date; the two differ by \|d(dtdb)/dt\|·\|dtdb\| ≈ 3.3e-10 × 1.7e-3 s ≈ 6e-13 s. |
| 2b | `dtdb` geocentric and topocentric at the golden epochs | pyerfa `dtdb` | 1e-14 s | Same 787-term sum in the same order. Argument rounding is ulp(4e3 rad) × 1.7e-3 s ≈ 8e-16 s, and platform `sin` differences are below that. |
| 2c | `jd2cal` / `cal2jd` at the golden epochs | pyerfa | y/m/d exact; fraction 1e-15 d | Same operations in binary64. |
| 3 | round trip A→B→A for all 10 pairs of {TAI, TT, TDB, TCB, TCG} (DEP-010) | identity | f64: 2e-10 s; DD: 1e-20 s | f64: at most about 16 roundings of a day-scale fraction, each ½ ulp(1 d) = 9.6e-12 s. DD: day-fraction ulp is about 1e-27 s. The TDB→TT fixed point after 3 iterations is (3.3e-10)³ × 1.7e-3 s ≈ 6e-32 s. |
| 4a | TAI−UTC from hifitime at each Jan 1 and Jul 1, 1972–2026, and 1 s before each leap | pyerfa `dat` goldens; data tier: IERS `Leap_Second.dat` and IANA `leap-seconds.list` | exact (0 s) | Leap seconds are integers. |
| 4b | hifitime TDB − ERFA TDB over the golden TT epochs 1550–2650 | ERFA port (`dtdb`, u = v = 0) | 4.0e-4 s, a bound, with the measured value recorded | hifitime uses the one-term model 1.658 ms·sin(g + 0.0167 sin g) with g′ = 1.990910018065731e-7 rad/s (6282.834 rad/millennium; ERFA's main term uses 6283.076). Analytic bound written before running hifitime: the sum of \|amplitude\|·\|T\|ⁿ of all non-main Fairhead terms at \|T\| = 0.65 millennium is 134.1 µs. The main-term frequency and phase mismatch over ±0.65 millennium is 262.2 µs. The JPL-mass terms are 0.02 µs. Total 396 µs, rounded up to 400 µs. |

**Design decisions.**
- **TDB→TT** is the exact inverse of ERFA's TT→TDB map `tdb = tt + dtdb(tt)`. It is solved by 3 fixed-point iterations `tt ← tdb − dtdb(tt)`, so round trips close to the backend's precision. ERFA's own `tdbtt` takes a caller-supplied `dtr`. The 6e-13 s difference from pyerfa's one-shot use is inside tolerance 2a.
- **`dtdb` is geocentric in `ferroastro-time`** (u = v = 0): the TDB of an ephemeris. The ERFA port keeps the topocentric arguments.
- **UTC exists only through the optional `hifitime` feature** (MPL-2.0, deny.toml exception, unmodified, no_std build). It is never a `TimeScale` marker: `Epoch<Tai>` is the uniform anchor.

### What was built
- **`ferroastro-erfa` (L1, new; `no_std`, generic over `Real`).**
  - Ports of `cal2jd`, `jd2cal`, `taitt`, `tttai`, `dtdb` (the full 787-term series plus topocentric terms), `tttdb`, `tdbtt`, `tdbtcb`, `tcbtdb`, `tttcg`, `tcgtt` and the `erfam.h` constants they use.
  - The function names are ERFA's without the prefix. No `iau…` or `era…` public symbol exists.
  - The defined decimals (32.184 s, `L_B`, `L_G`, `TDB_0`) are exact to 106 bits in double-double. The f64 backend sees ERFA's binary64 constants bit for bit (tested).
  - ERFA's non-zero statuses map to `ErfaError` with **FA-E0800–0803**. `ferroastro-core`'s `ErrorCode` lists the same strings, and a test checks that they agree.
- **Attribution (red-team L8, LIC-004, DEP-009).**
  - Every ported file carries `Apache-2.0 AND BSD-3-Clause`, `SPDX-FileCopyrightText: 2013-2023 NumFOCUS Foundation`, the ERFA routine and file it came from, and **ends with the ERFA notice, conditions and disclaimer verbatim** (BSD-3 clause 1).
  - `NOTICE` reproduces them for binary distributions (clause 2). `LICENSES/BSD-3-Clause.txt` was added. `reuse lint` passes.
  - The crate's `license` is `Apache-2.0 AND BSD-3-Clause`.
- **`ferroastro-time` (L2, new; generic over `Real`).**
  - `Convert<T, R>` covers all 20 ordered pairs of {TAI, TT, TDB, TCB, TCG}. Conversions route along the tree TAI–TT–TCG, TT–TDB–TCB.
  - Helpers: `from_calendar`/`to_calendar`, `tdb_seconds_since_j2000` and its inverse (the propagation variable), `seconds_between`, and `tdb_minus_tt` (geocentric `dtdb`).
  - `ferroastro-core` gains the `Tai` marker and **FA-E0804** (UTC unavailable) and **FA-E0805** (conversion result).
- **UTC: optional feature `hifitime` (`=4.3.1`, `default-features = false`, i.e. `no_std` without serde/web-time; MPL-2.0 deny.toml exception, unmodified).**
  - Module `utc`: `tai_from_utc`, `utc_from_tai` (23:59:60 representable), `tai_minus_utc`, and exact ns conversions `to_hifitime`/`from_hifitime`.
  - The crate's tests enable the feature through a self dev-dependency. `tools/check_layers.py` now ignores a crate's self dev-edge (pytest added).
  - The CI wasm32 job builds erfa and time, erfa as `no_std` + `libm`, and time with `hifitime`. All three builds were verified locally.
- **Tools.**
  - `tools/erfa_dtdb_table.py` regenerates the Fairhead table and its licence block from ERFA's `dtdb.c`.
  - `tools/goldens/erfa_time.py` writes the pyerfa goldens. It refuses any pyerfa but 2.0.1.5 and records `generated_by` (path@sha), engine versions and UTC time in the header.
  - pyerfa lives in a new non-default uv group `goldens`, so PR CI does not install it. The goldens' pytest skips without it.
- **Goldens:**
  - `tests/goldens/erfa-time-pyerfa.csv`: 223 epochs, 1550–2650, generated at `6f0dd8c`, SHA-256 `1c6caace…bc10b92`;
  - `tests/goldens/erfa-dat-pyerfa.csv`: 219 instants, SHA-256 `3fb51aeb…07004`.
  - Each is pinned by SHA-256 in the `notes` of the vectors that cite it. A regenerated table fails the tests until the vector is updated (golden change).

### WP8 results (offline PR tier, 2026-10-15, toolchain 1.95.0)
**(1) ERFA t_erfa_c reference values (DEP-009), 23 vectors `ERFA-T-*`, ERFA's own `dval`.** All pass in f64 and DD.
- Exact (diff 0) for `cal2jd`, `jd2cal` y/m/d, `taitt`, `tttai`, `tttdb` and `tttcg`/`tcgtt`. `tdbtt`, `tdbtcb` and `tcbtdb` differ by 1.1e-16 d (1 ulp; dval 1e-12).
- `dtdb`: f64 −1.2803680059370025e-3 s, |diff| 3.5e-18 s. DD −1.2803680059369977e-3 s, |diff| 1.3e-18 s (dval 1e-15).
- `jd2cal` fraction: 2.5e-12 d in both backends (dval 1e-7). The input 50123.9999 is itself a binary64 approximation.

**(2) pyerfa 2.0.1.5 goldens (REF-009), 223 epochs 1550–2650, max |FerroAstro − pyerfa| (pre-registered bound):**
| vector | f64 | double-double | bound |
|---|---|---|---|
| `PYERFA-TT-TAI-MAX-DIFF` | 0 | 1.9e-13 s | 1 ns |
| `PYERFA-TT-TDB-MAX-DIFF` | 9.6e-12 s | 4.8e-12 s | 1 ns |
| `PYERFA-TDB-TCB-MAX-DIFF` | 0 | 8.9e-12 s | 1 ns |
| `PYERFA-TT-TCG-MAX-DIFF` | 0 | 4.8e-12 s | 1 ns |
| `PYERFA-DTDB-MAX-DIFF` (geocentric and topocentric) | **0 (bitwise)** | 1.1e-15 s | 1e-14 s |
| `PYERFA-JD2CAL-FD-MAX-DIFF` (y/m/d exact) | 0 | 0 | 1e-15 d |

- In f64 the port is bitwise identical to pyerfa everywhere except TDB→TT. The 9.6e-12 s there (1 ulp of the day fraction) comes from the exact-inverse definition.
- The DD residuals are DD-versus-binary64 rounding of the same formulas. In DD the decimal constants are exact; in pyerfa they are rounded.

**(3) Round trips A→B→A, all 10 pairs × 2 directions × 223 epochs (DEP-010):**
- f64 worst **9.6e-12 s** (bound 2e-10 s), on the routes through TCB.
- DD worst **1.8e-27 s** (bound 1e-20 s; TCG→TCB→TCG, year 2265).
- TDB-seconds round trip in DD: 1.4e-22 s against `TIME-TDB-SECONDS-DD-ROUNDTRIP` (1e-20 s; non-blind, see below).

**(4) hifitime 4.3.1 cross-checks (DEP-010):**
- **TAI−UTC through the FerroAstro adapter vs ERFA `dat`, 219 instants, 27 leap steps: max |diff| 0 s (exact).** The independent JD path differs by at most 7.7e-12 s (bound 1 µs).
- **Second-by-second sweep, 23:59:50 → 00:00:10, around all 27 IERS leap seconds:** UTC→TAI→UTC is the identity, including 23:59:60, and TAI advances exactly 1 s per UTC second.
- **Finding:** hifitime's *native* UTC handling disagrees with `dat` at **27 of 219** instants: 23:59:59 on the eve of every leap second, e.g. 1972-06-30, hifitime 11 s vs dat 10 s.
  - hifitime 4.3.1 applies each leap second (old TAI−UTC) seconds early. A direct probe shows it already says 11 s at 1972-06-30T23:59:50 UTC and 37 s at 2016-12-31T23:59:59.
  - Cause: it compares TAI seconds with the table's NTP-style UTC timestamps.
  - This is upstream nyx-space/hifitime **#417** (open, 2026-03-23) and **#255** (open, 2024-10-02). The adapter therefore uses only hifitime's table and leap-free calendar, and does the boundary lookup itself.
- **hifitime TDB − FerroAstro TDB** (seconds since J2000 TDB):
  - max **1.957e-4 s** (year 2600) over 1550–2650; **3.9e-5 s** over 1950–2050 (23 epochs). Pre-registered analytic bound 4.0e-4 s.
  - Rationale: hifitime's TDB is the one-term SPICE model, whose frequency is 6282.834 rad/millennium against ERFA's 6283.076. The other Fairhead terms are neglected. The difference grows with distance from J2000.
  - hifitime TT vs ours: max 5.0e-10 s (hifitime's 1 ns grid).

**Mutation probes (reverted):** each was killed by the PR tier.
- `TDB_TO_TT_ITERATIONS` 3→1: killed by the DD round trip.
- Fairhead row 2 phase +1e-9 rad: killed by t_erfa_c f64/DD and the goldens f64/DD.
- `ELG` last digit: killed by the goldens f64/DD; t_erfa_c's 1e-12 d tolerance does not see it.
- A planted `pub fn iauTaitt` and `pub const IAU_DJ00`: killed by the iau-prefix scan.

### WP8 real-data results (data tier, `tools/run_long_local.sh --data-only`, empty `FA_DATA_DIR`, every download fresh; log `target/long-local/20261015T080404Z-8100091a0241.log`, commit `8100091`, clean)
- **15/15 `data_*` pass.**
- **New `data_leap_seconds_match_iers_and_iana`:**
  - IERS `Leap_Second.dat`: SHA-256 `6cb6f5d4…e268d7`, 1352 bytes, Last-Modified 2026-07-06, "Updated through IERS Bulletin 72 issued in July 2026", "File expires on 28 June 2027".
  - IANA `leap-seconds.list`: SHA-256 `db5a895f…bc3433`, 5065 bytes, Last-Modified 2026-09-30, `#@4023129600` (expires 28 June 2027).
  - Both list 28 entries, the last at MJD 57754 (2017-01-01, 37 s). The two products agree entry for entry.
  - FerroAstro TAI−UTC at 0 h of each entry and at 23:59:59 before it: **55 instants, max |diff| 0 s** (`LEAPSEC-IERS-IANA-VS-FERROASTRO`, exact). hifitime's table is current.
  - The IANA file's embedded SHA-1 (`#h` line) is printed, not verified: no SHA-1 implementation in the dependency graph.
- **Every earlier data result is unchanged to every printed digit:**
  - Park17 frame 575.309163 ″/cy, jack-knife σ 0.00153;
  - SW92 dog-leg 571.968047;
  - GR from DE441 42.9806442;
  - D5 f64 42.980085159575 / DD 42.980085160105;
  - Horizons `signature.version` 1.2, DE441 on every answer;
  - header.440 SHA-256 `0f4636b6…6ac3359`.

### Progress-review corrections (status in WP8)
WP7 applied every correction in the list: CI tiers, metadata-only vector notes, oracle substitution, data-profile retries, μ convention, error-code ranges and the API-003 report. WP8 adds:
- **Citing tests and vectors** for DEP-009, DEP-010, REF-009 and LIC-004, plus TST-003 for the goldens. Together with WP7's DEP-007, NUM-006 and NUM-010, all six M1 items in correction 6 are now cited. `trace.py`: 38/92 `test` requirements cited, 271 citing tests.
- **FA-E08xx range** added to the core `error.rs` table.
- **NUM-023:** not retried in WP8. WP7's retry found the fix (`pc_b_from_g`, opt-in), and adopting it is a human decision. The known-failure guard and ±0.1 are unchanged.
- **Human-owned questions** are left unresolved. That covers the full list in correction 8, plus the weekly GitHub `data` job question.

### DEP-015 provenance checklist (new dependencies in Cargo.lock)
| crate | role | licence (file read) | maintenance | WASM |
|---|---|---|---|---|
| `hifitime` 4.3.1 | optional runtime dep of `ferroastro-time` (feature `hifitime`, off by default) | MPL-2.0 (`LICENSE.txt` read: "Mozilla Public License Version 2.0"); deny.toml exception; unmodified | released 2026-08-07 | wasm32-unknown-unknown build verified (no_std config) |
| `snafu`/`snafu-derive` 0.9.2 | transitive (hifitime) | MIT OR Apache-2.0 (both files) | 2026-07-21 | yes |
| `lexical-core`/`-parse-float`/`-parse-integer` 1.0.6, `lexical-util` 1.0.7 | transitive (hifitime parsing) | MIT/Apache-2.0 (`LICENSE.md` read). Its Dragonbox Boost/LLVM terms apply only to `write-floats`, which is not compiled here. | 2025-09-21 | yes |
| `num-traits` 0.2.19 | transitive | MIT OR Apache-2.0 | **2024-05-03 (> 24 months)**: justification below | yes |
| `heck` 0.5.0 | transitive, build-time only (proc macro of snafu-derive) | MIT OR Apache-2.0 | **2024-03-12 (> 24 months)**: justification below | n/a (host) |
| `js-sys`, `wasm-bindgen*`, `web-sys`, `bumpalo`, `futures-*`, `slab`, `pin-project-lite`, `rustversion` | in Cargo.lock only: hifitime's emscripten-target dependencies, never built for our targets | MIT and/or Apache-2.0 (`slab`: MIT `LICENSE`) | — | — |

- **Justification for the two > 24-month crates:** `num-traits` is rust-num's stable core trait crate. `heck` is a string-case helper used only at build time.
- **MSRV:** hifitime declares no `rust-version`. It builds and its tests pass on 1.85.0 (MSRV nextest run below), within ADR-0006 (≤ 1.93).
- **cargo deny:** passes. Its warnings: `syn` 1 and 2 duplicate (syn 1 via proptest dev, syn 2 via snafu-derive); the anise exception is unused (not a dependency, per the unsettled decision).

### Interpretations and NEEDS HUMAN REVIEW (WP8)
1. **PROVISIONAL tolerances (red-team C3/H1):** 1 ns for the four `PYERFA-*` time-scale vectors, as the WP8 brief proposes. Measured values are 100× below it. Every other WP8 bound is a round-off or analytic bound written before its run.
2. **`TIME-TDB-SECONDS-DD-ROUNDTRIP` is non-blind.** It was added after its first run had borrowed `TIME-ROUNDTRIP-DD-MAX` (whose quantity is the scale-pair round trip), with the same 1e-20 s value. Measured 1.4e-22 s.
3. **TDB→TT definition.** FerroAstro inverts its own TT→TDB exactly (3 fixed-point iterations). ERFA's `tdbtt` takes a caller-supplied TDB−TT, which pyerfa users usually evaluate at the TDB date. The two differ by ≈ 6e-13 s, well inside every tolerance. Choose and document one convention project-wide.
4. **UTC "only through hifitime".** The adapter takes hifitime's leap-second table, leap-free calendar and ns instant. It does **not** use hifitime's UTC↔TAI boundary logic, which is wrong near every leap second in 4.3.1 (upstream #417, #255). Decide:
   - whether this reading of ADR-0003/DEP-010 is acceptable;
   - whether to report the cross-check upstream. No agent comments on external repositories.

   When hifitime is fixed, the report-only native count in `dep010_hifitime` drops to 0.
5. **REF-009 acceptance is only partly met.**
   - Met: pyerfa 2.0.1.5 is pinned and produces the TT↔TDB goldens used by PR-fast CI.
   - Not met: the goldens come from a uv dependency group run locally, not from an "erfa oracle image". The image is WP16, and WP16 must regenerate these goldens identically (compare all non-header lines). The ICRS↔ecliptic and J2000 rotation goldens are WP9.
6. **DEP-010 "at both f64 and DD".** The ERFA-derived checks run in both backends. The hifitime cross-check is limited to hifitime's integer-nanosecond grid whichever backend is used.
7. **Topocentric TDB.** `ferroastro-time` converts geocentrically (`dtdb` with u = v = 0), the TDB of a barycentric ephemeris. The topocentric terms (≤ 2 µs) are available through `ferroastro_erfa::dtdb`.
8. **DEP-007 clause "time arithmetic"** is now covered: `ferroastro-time` and `ferroastro-erfa` are generic over `Real`, and their tests run in f64 and DD. The "ephemeris evaluator" clause remains WP10.
9. **Tooling change:** `check_layers.py` ignores self dev-dependencies, and a self runtime dependency is still a cycle (pytest). Confirm that this is acceptable layering policy.
10. **Requirement-text suggestions for PR #2 (not edited here):**
    - DEP-010: state the tolerances, i.e. 1 ns against the ERFA-derived reference, exact for TAI−UTC, and the hifitime TDB difference as a bound, not an agreement.
    - DEP-009/LIC-004: record the per-file licence block (red-team L8) and the `era` prefix choice.
    - REF-009: split the acceptance into WP8 (pinned goldens), WP9 (frames) and WP16 (image).
    - DEP-002 exception list: hifitime, now in the graph as an optional feature.
11. **The earlier REUSE failure is fixed.** Commit `1417463` (ERFA port) failed `reuse lint`: the attribution test contained SPDX strings as data. Fixed in `d4a71e6`. The MSRV break (let chains, Rust ≥ 1.88) in the same test was fixed in `8100091`.

### WP8 gate results (2026-10-15, local, toolchain 1.95.0, code tip `8100091`)
All eight AGENTS.md commands pass (exit 0):
- fmt;
- clippy `-D warnings`;
- `cargo nextest run --workspace`: **148 passed, 18 skipped** (15 `data_*` + 3 `nightly_*`), plus **10 doctests**;
- `cargo deny check`;
- rustdoc `-D warnings`, which also passes for `ferroastro-time --features hifitime`;
- ruff check/format and mypy strict (17 files);
- `reuse lint`;
- `trace.py --check`: 119 requirements, 271 citing tests, 0 errors.

Extras:
- `uv run pytest`: 27 passed. The goldens pytest ran because the `goldens` group was synced locally; CI skips it.
- `check_layers`: 13 crates, 42 edges, 0 errors.
- MSRV 1.85.0 nextest without trybuild: **147 passed**.
- wasm32 builds: all three pass.
- Data tier: 15/15 pass (above).
- `doctest_coverage.py` (report-only): 0 of 289 public functions carry a per-function doctest. `ferroastro-erfa` and `ferroastro-time` each have a crate-level doctest. The API-003 clause stays deferred (WP7 item 6).
- The nightly tier was not re-run: WP8 changes no integrator, force or ladder code.

### WP8 adversarial review (2026-10-15, reviewer agent)
Re-ran every AGENTS.md command at `2ee4d15` (all pass: nextest 148 passed / 18 skipped, doctests pass, deny/reuse/trace/ruff/mypy clean) and the data tier on an **empty** cache (`FA_DATA_DIR` fresh, 15/15 pass, every download a cache miss). The IERS and IANA files had the same SHA-256 as the builder's run; the Park17-frame (575.309163 ″/cy), SW92 dog-leg (571.968047) and D5 (42.980085159575 / 42.980085160105) numbers are unchanged. Regenerating the pyerfa goldens with `uv run --group goldens python goldens/erfa_time.py` reproduced every data row; only the `generated_by` and `generated_utc` header lines differ, so the pinned files were left unchanged. Ported formulas were checked line by line against ERFA 2.0.1 (`tdbtcb`, `tcbtdb`, `tttcg`, `tcgtt`, `dtdb` topocentric, Fairhead and JPL-mass terms, `jd2cal` Klein summation). The licence headers, NOTICE and `LICENSES/BSD-3-Clause.txt` are present.

What the review changed:
1. **Test gap: the `|d1| <= |d2|` branch of every two-part-date routine had no test.** Both `t_erfa_c.c` and the pyerfa goldens split dates as "(JD at 0 h, fraction)", so only the first branch ran. Three planted sign flips in the second branches passed all 18 WP8 tests: in `add_to_smaller` (used by taitt, tttai, tttdb and tdbtt), in `tdbtcb` and in `tttcg`. A user passing `(0, JD)` or `(fraction, JD)` would have received a result 64 s or more out, silently. The review adds `crates/ferroastro-erfa/tests/dep009_split_order.rs` and the vector `ERFA-SPLIT-ORDER-DD-MAX`, pre-registered before the first run with a bound of 1e-15 s in double-double. The test checks that every routine gives the same instant for `(d1, d2)` and `(d2, d1)`, and for `(JD, 0)` and `(0, JD)`. Measured: 4.5e-21 s over 1170 comparisons. All five planted mutants fail it (the three sign flips, a `tcbtdb` TDB0 sign flip, and `elgg` replaced by `elg` in `tttcg`).
2. **`tdb_seconds_round_trip` now checks its own vector.** It still checked `TIME-ROUNDTRIP-DD-MAX`, which left `TIME-TDB-SECONDS-DD-ROUNDTRIP` unused. Both have the same 1e-20 s value, so the verdict is unchanged (1.4e-22 s).
3. Mutation probes on the UTC adapter: changing `<=` to `<` at the leap entry, or `>=` to `>` at the leap-second interval, fails the second-by-second and `erfa.dat` tests. Those tests do discriminate.

Non-blocking observations, for a maintainer:
- `tdb_seconds_since_j2000` maps a `TimeSpan::from_seconds` error to `TimeSpan::zero()`. In practice this is unreachable, because the epoch is finite and within ±1e9 d. Still, J2000 is a silently wrong value; returning `Result` or documenting it as infallible would be clearer.
- The hifitime TT-vs-ours difference (4.96e-10 s) is printed but not asserted.
- hifitime 4.3.1 declares no `rust-version`. The MSRV evidence is the 1.85 build alone.

## WP9 — [M3] `ferroastro-frames`: ecliptic, mean-orbit, solar-equator and IAU 2006 precession

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13.

### Plan and pre-registered checks (written 2026-10-15, before any golden, NUM-014 or Horizons run)
The t_erfa_c check (1) was run once, with the ERFA port, before this section was written (as in WP8);
every other tolerance below was fixed before its first run. Two expected values were estimated
beforehand with pyerfa (marked "pre-run"), so they are **not blind**; the tolerances were set from
the stated rationale, not from those runs.

**Sources.** ERFA 2.0.1 (the WP8 tarball, SHA-256 `45111f0a…8c20e4`): `obl06.c`, `p06e.c`, `pfw06.c`,
`fw2m.c`, `pmat06.c`, `ecm06.c`, `eqec06.c`, `eceq06.c`, the helpers they call, and `t_erfa_c.c`. No SOFA
file. JPL Horizons manual (<https://ssd.jpl.nasa.gov/horizons/manual.html>, retrieved 2026-10-15, SHA-256
`5c5100b0…49ddcc`): "[the ecliptic of J2000] is now produced by rotating around the ICRF x-axis by a
standard fixed obliquity angle (epsilon) of 84381.448 arcseconds", and body frames take "the
intersection (node) of the body's equator plane with the fixed ICRF fundamental plane" as x. NAIF
`pck00011.tpc` (SHA-256 `3dff7b1d…e28bb1`): `BODY10_POLE_RA = 286.13`, `BODY10_POLE_DEC = 63.87`
(Archinal et al. 2018).

**Definitions (documented in the crate).**
- `EclipticJ2000` = JPL/NAIF `ECLIPJ2000`: `R_x(84381.448″)` applied to ICRF, **no frame bias**. This is
  the frame of Horizons' `ECLIPTIC` tables that WP5 already feeds into `EclipticJ2000`. The IAU 2006
  mean ecliptic and equinox of J2000 (`ecm06` at J2000, which includes frame bias and ε₀ = 84381.406″)
  differs from it by ~0.04″; that offset is reported, not hidden.
- `EclipticOfDate` = IAU 2006 mean ecliptic and equinox of date, `ecm06` (TT date). Used only for
  PHY-017 and NUM-014. A rotation into it is built for one epoch; velocities are rotated without the
  frame-rotation term (≈ 1e-5 relative for Mercury), documented.
- `MercuryMeanOrbit`: z = Park et al. 2017's pole (α 280.9876°, δ 61.4481°), y = z × x̂_ICRF
  (normalised), x = y × z (Park17 §4). Consolidates WP5's `ReferencePlane::park2017_mean_orbit`.
- `SolarEquator`: z = a configurable solar pole, x = node of the solar equator on the ICRF equator
  (x̂ = ẑ_ICRF × pole, normalised). The pole and its source travel with the rotation as metadata
  (PHY-010). The IAU/NAIF pole gives Horizons' `IAU_SUN` (`SunEquatorIau`).
- ICRF stays the default output frame (NUM-014); no API converts implicitly.

**Oracles and tolerances.**
| # | check | oracle | tolerance (pre-registered) | rationale |
|---|---|---|---|---|
| 1 | each newly ported routine, f64 and DD (DEP-009) | ERFA `t_erfa_c.c`, 110 vectors `ERFA-T-*` | ERFA's `dval`, verbatim | as WP8 |
| 2 | pyerfa 2.0.1.5 goldens at ≥ 50 epochs 1550–2650 (REF-009): `obl06`, `p06e` (all 16 angles, incl. p_A), `pmat06`, `ecm06`, `eqec06`/`eceq06`, f64 and DD | `tools/goldens/erfa_frames.py` | 1e-12 rad (and 1e-12 per matrix element), **PROVISIONAL** as the brief states | Same algorithm; expected residual is binary64 round-off, ~1e-16 rad. |
| 3 | NUM-014: header.440 Sun+Mercury+GR (Schwarzschild rung), 200 yr from JDEPOC. The same passages are expressed in ICRF (inertial) and in the IAU 2006 ecliptic of date. Both perihelion longitudes are measured in **one plane**, the IAU 2006 J2000 ecliptic: from its fixed x-axis (inertial) and from the projection of the equinox of date (of date). Linear fits are made in TT Julian centuries. | the linear-fit slope of p_A(t) from the ported `p06e` at the same epochs (computed-reference, verified against pyerfa by check 2; never OI-02's snippet value) | 1e-3 ″/cy (the brief's number) | Pre-run with pyerfa at uniform 88-d sampling: difference − ṗ_A = **+4.4e-4 ″/cy**. All of it is the second-order projection term of the moving ecliptic, ≈ (π_A²/4)·sin 2(Π_A + p_A): the same slope computed from `p06e`'s own π_A, Π_A and p_A differs from the `ecm06` result by 8.4e-7 ″/cy. So the 1e-3 tolerance has a margin of only ~2×, and this is stated rather than hidden. |
| 3b | NUM-014 geometry: the same rate difference vs the slope of the **exact** projected-equinox longitude predicted from `p06e`'s π_A, Π_A and p_A, λ = Π_A − atan2(sin(Π_A+p_A)·cos π_A, cos(Π_A+p_A)) | `p06e` (computed-reference) | 1e-5 ″/cy | ERFA documents that the P03 parameterisations agree at the 1 µas level. The pre-run measured 8.4e-7 ″/cy. 1e-5 leaves a 10× margin. |
| 3c | the same check offline (PR tier) from the scenario's committed SW92 elements, with the epoch set to J2000 | as 3 and 3b | as 3 and 3b | The geometry does not depend on the initial conditions. |
| 4 | Data tier: our ICRF → `EclipticJ2000` and ICRF → `SunEquatorIau` (NAIF pole) rotations applied to Horizons DE441 Mercury states in `REF_PLANE=FRAME`, compared with Horizons' own `ECLIPTIC` and `BODY EQUATOR` answers at the same epochs (1900–2100) | JPL Horizons | max \|Δr\|/\|r\| and \|Δv\|/\|v\| ≤ 1e-12 | Horizons prints 16 significant digits (≈ 5e-17 relative). Omitting the frame bias, or using ε = 84381.406″, would give ≥ 1e-7. A wrong x-axis convention would give O(1). |
| 5 | Data tier: the solar pole of `pck00011.tpc` equals the vectors `SUN-POLE-IAU-RA/DEC` | NAIF PCK | exact | transcription check |
| 6 | compile-fail: a `State<MercuryMeanOrbit, _>` cannot be passed where `State<Icrf, _>` is required | trybuild | — | API-001 / NUM-014 |

### What was built
- **`ferroastro-erfa` (extended).** Ports of `obl06`, `p06e`, `pfw06`, `fw2m`, `pmat06`, `ecm06`, `eqec06` and `eceq06`. They come with the helpers they call: `ir`, `rx`, `rz`, `rxr`, `tr`, `rxp`, `trxp`, `s2c`, `c2s`, `anp` and `anpm`. All are generic over `Real`.
  - The WP8 attribution rules apply: per-file SPDX, the ERFA copyright, and the verbatim licence block at the end of each file. `NOTICE` lists the new routines.
  - In f64, ERFA's operations run in ERFA's order. In DD, arcseconds→radians is π/648000 to 106 bits. The series coefficients stay binary64, as for `dtdb`.
- **`ferroastro-frames` (L3, new).** `Rotation<From, To, R>` gives:
  - checked construction (FA-E0900);
  - inverse and composition;
  - application to positions, displacements and states (FA-E0901 for non-finite results);
  - a definition string for output metadata.

  Constructors:
  - `icrf_to_ecliptic_j2000`;
  - `icrf_to_ecliptic_of_date(Epoch<Tt>)`, plus `ecliptic_precession` returning π_A, Π_A and p_A;
  - `icrf_to_mean_orbit(&Pole)`, with `Pole::park2017_mercury_mean_orbit`;
  - `icrf_to_solar_equator(&Pole)`, for any pole, with the pole and its source in the definition;
  - `icrf_to_sun_equator_iau`, from `Pole::sun_wgccre2015`.

  The crate-level doctest builds an `EclipticJ2000` position from an ICRF one. Inertial ICRF stays the default: nothing converts implicitly.
- **`ferroastro-core`.** New markers `EclipticOfDate`, `MercuryMeanOrbit` and `SolarEquator`, and a new range FA-E09xx with `FrameDefinition` (FA-E0900) and `FrameTransform` (FA-E0901).
- **Consolidation.** `ferroastro-precession`'s `ReferencePlane::from_pole` and `park2017_mean_orbit` now take their axes from `ferroastro_frames::icrf_to_mean_orbit`, and `PARK2017_POLE_*` are re-exported from there. The Park17 data result is unchanged to every printed digit (575.309163 ″/cy).
- **`ferroastro-ladder`.**
  - `SingleBody::with_epoch`, `epoch` and `perihelion_passages`.
  - The D5 data test now dates its run at JDEPOC. This is metadata only: the single-body dynamics do not depend on the epoch, and the D5 numbers are unchanged.
- **Tools.**
  - `tools/goldens/erfa_frames.py` writes `tests/goldens/erfa-frames-pyerfa.csv`: 113 epochs, generated at `b2fbca9`, SHA-256 `5a68408e…60ea13`, pinned in the `PYERFA-*` vectors.
  - Its pytest is skipped without the `goldens` group.
- **CI.** The wasm32 job also builds `ferroastro-frames`; the build passes locally.

### WP9 results (offline PR tier, 2026-10-15, toolchain 1.95.0)
**(1) t_erfa_c (DEP-009), 110 vectors.** All pass in f64 and DD. The worst deviation is 8.9e-16 (`ECEQ06-DR`, `RX-33`, `RXP-1`), 1–2 ulp. `P06E-PA`, `OBL06` and `EQEC06-DL` are exact.

**(2) pyerfa 2.0.1.5 goldens (REF-009), 113 epochs 1550–2650 plus J2000 and JDEPOC; PROVISIONAL bound 1e-12:**
| vector | f64 | DD |
|---|---|---|
| `PYERFA-OBL06-MAX-DIFF` | 0 | 5.6e-17 rad |
| `PYERFA-P06E-MAX-DIFF` (16 angles + p_A through the typed API) | 0 | 4.4e-16 rad |
| `PYERFA-PMAT06-MAX-DIFF` | 0 | 2.2e-16 |
| `PYERFA-ECM06-MAX-DIFF` (via `icrf_to_ecliptic_of_date`) | 0 | 2.2e-16 |
| `PYERFA-EQEC-ECEQ-MAX-DIFF` | 0 | 1.5e-15 rad |
| `PYERFA-ECLIPJ2000-MAX-DIFF` | 0 | 0 |

In f64 the port is bitwise identical to pyerfa at every epoch.

**(3c) NUM-014 offline** (scenario SW92 elements rotated ECLIPJ2000→ICRF by the typed inverse rotation, epoch J2000, 831 passages, T = +0.001…+2.000 TT centuries):
- inertial 42.810612161 ″/cy;
- ecliptic of date 5073.820048017 ″/cy;
- difference 5031.009435857 ″/cy;
- p_A slope 5031.008868363 → **difference − p_A = +5.68e-4 ″/cy** (bound 1e-3);
- exact-geometry slope 5031.009434993 → **difference − geometry = +8.6e-7 ″/cy** (bound 1e-5).

The residual against p_A is larger than the data-tier one because this span (2000–2200) reaches larger T, where the (π_A²/4) term grows.

**Frame comparison (report only).** The IAU 2006 J2000 ecliptic (`ecm06` at J2000) and ECLIPJ2000 have poles 0.0409″ apart and x-axes 0.0224″ apart.

**Mutation probes (reverted):**
- p_A linear coefficient changed by +1e-4 ″/cy (5028.796195 → 5028.796295):
  - killed by t_erfa_c, by both golden tests, and by NUM-014 offline through `NUM014-RATE-DIFF-VS-GEOMETRY` (−9.9e-5 against a 1e-5 bound);
  - `NUM014-RATE-DIFF-VS-PA` alone would **not** have caught it (+4.7e-4 < 1e-3). That is why check 3b exists.
- `Pole::iau_sun` was caught by the WP8 iau-prefix scan. It is renamed `sun_wgccre2015`.

### WP9 real-data results (data tier, `tools/run_long_local.sh --data-only`, empty `FA_DATA_DIR`, every download fresh; log `target/long-local/20261015T083449Z-924a5863242f.log`, commit `924a586`)
**18 data tests.**
- First pass: 13 passed, and 5 failed with **HTTP 503 from Horizons**. The failing five were:
  - `data_frame_rotations_reproduce…`;
  - `data_horizons_osculating_elements_match`;
  - three `data_single_body` Horizons tests.

  In each, curl gave up after four attempts (FA-E0200 / the WP4 helper's fetch panic). None of the five was an assertion failure.
- Re-run on the same cache, serially (`--test-threads 1`): **5/5 pass**.

  The data profile runs Horizons tests in parallel, and Horizons answers 503 under that load. **NEEDS HUMAN REVIEW:** run the data profile with `test-threads = 1` for Horizons tests, or add longer back-off in `ferroastro-testdata`. Test-level retries stay at 0, per the WP7 correction.

**New in WP9:**
- **NUM-014 (header.440 ICs, JDEPOC 2440400.5, 831 passages, T = −0.305…+1.694):**
  - inertial 42.809284353 ″/cy;
  - ecliptic of date 5073.142303005 ″/cy;
  - difference 5030.333018652 ″/cy;
  - p_A slope 5030.332575517 → **difference − p_A = +4.431e-4 ″/cy** (bound 1e-3; pre-run +4.43e-4);
  - geometry slope 5030.333017810 → **difference − geometry = +8.42e-7 ″/cy** (bound 1e-5; pre-run 8.4e-7);
  - header.440 SHA-256 `0f4636b6…6ac3359`, Last-Modified 2020-12-22.
- **Horizons rotation checks, 74 epochs 1900–2100, DE441, `signature.version` 1.2:**
  - `HORIZONS-ECLIPJ2000-ROTATION`: max \|Δr\|/\|r\| 2.42e-16, \|Δv\|/\|v\| 3.28e-16.
  - `HORIZONS-IAU-SUN-ROTATION`: 4.72e-16 and 4.06e-16.
  - Bound 1e-12.
  - Horizons' frame lines: "Ecliptic of J2000.0" and "IAU_SUN body equator and node of date".
  - Answers' SHA-256: FRAME `8b3f5153…ab0c06`, ECLIPTIC `92a30167…439841`, BODY EQUATOR `eb40f1a0…7aae08`.
  - This confirms both documented definitions:
    - ECLIPJ2000 is `R_x(84381.448″)` with no frame bias;
    - `IAU_SUN` is pole (286.13°, 63.87°) with x at the node on the ICRF equator.
- **NAIF `pck00011.tpc`:** SHA-256 `3dff7b1d…e28bb1`, Last-Modified 2022-12-27, `BODY10_POLE_RA` = 286.13 and `BODY10_POLE_DEC` = 63.87. Both equal the vectors.

**Unchanged to every printed digit:**
- Park17 frame 575.309163 ″/cy (jack-knife σ 0.00153), after the axis consolidation;
- SW92 dog-leg 571.968047;
- GR from DE441 42.9806442;
- D5 f64 42.980085159575 / DD 42.980085160105;
- real-geometry J2/LT 0.0284524 / −0.0020148;
- NUM-012 GR-REAL spread 1.76e-5;
- μ-convention line −3.57e-6.

### Progress-review corrections (status in WP9)
WP7 and WP8 applied every item in the list. WP9 re-checked each one:
- `ci.yml` `long` is `workflow_dispatch` only, and `tools/run_long_local.sh` was used for this WP's data run.
- The two vector metadata notes are present.
- `FA_VECTORS_DIR` is refused by `load_canonical`, which all new WP9 tests use.
- The data profile has `retries = 0`.
- The μ-convention note is printed.
- The FA-E06xx row names `ferroastro-analytic`; FA-E0404 exists.
- WP9 adds the FA-E09xx row.
- The DEP-007/009/010, NUM-006/010 and REF-009 citing tests exist.
- NUM-023 was not touched.
- API-003 stays deferred. `doctest_coverage.py` reports 0 of 334 public functions with a per-function doctest; `ferroastro-frames` has a crate-level doctest.
- Every human-owned question in correction 8, and the weekly GitHub `data` job question, is left unresolved.

### Interpretations and NEEDS HUMAN REVIEW (WP9)
1. **NUM-014 "same reference plane".** It is read as follows: both longitudes are measured in one fixed plane, the IAU 2006 J2000 ecliptic. The inertial one is measured from that plane's x-axis; the ecliptic-of-date one is measured from the projected equinox of date, after the state has been rotated into `EclipticOfDate`.
   - With this reading, the difference is a property of the precession geometry, not of Mercury's orbit. Perihelion longitudes measured *in the moving ecliptic itself* (dog-leg) would differ from p_A by ~tan(i/2)·π̇_A ≈ 2 ″/cy for Mercury, which no 1e-3 tolerance can hold.
   - The 1e-3 bound passes with a margin of only ~2× (+4.4e-4 / +5.7e-4), because of the second-order projection term. The sharper 1e-5 geometry check carries the discriminating power.
   - Confirm this reading and the two tolerances. Suggestion for PR #2: state the plane and the tolerance in NUM-014's acceptance.
2. **Non-blind expected values.** The two NUM-014 expectations (+4.4e-4 and 8.4e-7) were computed with pyerfa before the Rust run. The tolerances come from the brief (1e-3) and from ERFA's documented 1 µas agreement (1e-5), not from the runs.
3. **`EclipticJ2000` = ECLIPJ2000 without frame bias.** This matches Horizons, NAIF and WP5's use. It is 0.041″ (pole) away from the IAU 2006 J2000 ecliptic. PHY-017 (WP30) must use `EclipticOfDate`, which includes the bias, consistently, and must never mix the two.
4. **Solar pole source.** The vectors `SUN-POLE-IAU-*` were read from NAIF `pck00011.tpc`, which cites Archinal et al. 2018. The Archinal paper itself was not opened. The kind is `defined` (an adopted IAU model constant).
5. **`EclipticOfDate` velocities.** These are rotated without the Ṁ·r term, documented in `Rotation::state`. NUM-014 uses positions only.
6. **Data-profile 503s.** See above; a policy choice for a human.
7. **Requirement-text suggestions for PR #2 (not edited here):**
   - NUM-014: name the reference plane and give the tolerance. Record that p_A comes from the `p06e` port verified against pyerfa, which can close OI-02's use for NUM-014: p_A is no longer taken from the snippet.
   - REF-009: the ICRS↔ecliptic and J2000 goldens now exist; the oracle image is still WP16.
   - PHY-010: record the default solar pole (IAU WGCCRE 2015, 286.13°/63.87°) and its configurability.
   - DEP-009/LIC-004: list the WP9 routines.

### WP9 gate results (2026-10-15, local, toolchain 1.95.0, code tip `924a586`)
All eight AGENTS.md commands pass (exit 0):
- fmt;
- clippy `-D warnings`;
- `cargo nextest run --workspace`: **160 passed, 21 skipped** (18 `data_*` + 3 `nightly_*`), plus **11 doctests**;
- `cargo deny check`;
- rustdoc `-D warnings`;
- ruff check/format and mypy strict (19 files);
- `reuse lint`;
- `trace.py --check`: 119 requirements, 305 citing tests, 0 errors. **39/92** `test` requirements are cited, up from 38 with NUM-014.

Extras:
- `uv run --group goldens pytest`: 29 passed.
- `check_layers`: 14 crates, 49 edges, 0 errors.
- MSRV 1.85.0 nextest without trybuild: **158 passed**.
- wasm32 builds of frames and erfa (std and no_std/libm): pass.
- Data tier: 18/18 pass after the serial re-run of the five 503 failures (above).
- The nightly tier was not re-run: WP9 changes no integrator, force or rung code.

### WP9 adversarial review (2026-10-15, reviewer agent, code tip `d3252a0`)

- All AGENTS.md gates re-run at `d3252a0`: they pass (nextest 160 passed / 21 skipped, doctests,
  deny, doc, ruff/mypy, reuse, trace, layers, literals, workflows, goldens pytest 29 passed).
- Data tier re-run **serially** (`--test-threads 1`) from a new empty `FA_DATA_DIR`. Every product
  was fetched fresh. **18/18 pass, with no HTTP 503.** The numbers match the builder's to every
  printed digit: ECLIPJ2000 2.423e-16 / 3.281e-16, IAU_SUN 4.718e-16 / 4.060e-16, NUM-014 +4.431e-4
  vs p_A and +8.424e-7 vs geometry, Park17 575.309163.
- Mutations, each run in a scratch clone and then reverted:
  - **p_A coefficient +1e-4 ″/cy:** caught by 4 tests (t_erfa_c, pyerfa f64 and DD, and the
    offline NUM-014 check).
  - **Flipped node sign of the solar equator:** passes the PR tier, and is caught only by the data
    tier, at 2.0 relative.
  - **Flipped y-axis of the Park17 mean-orbit frame** (a 180° turn about the pole): passes **every**
    tier. Rates do not depend on it, but the claim "Park17 axes" is not pinned in sign.
  - Follow-up (non-blocking): add offline assertions on the sign of each axis.
- **Fix applied:** removed the unused `ferroastro-time` dependency of `ferroastro-frames`.
- **Note:** NUM-014's difference λ_d − λ_i at each passage depends on the orbit only through the
  passage epochs. The check therefore exercises the typed ecm06 rotation against the independent
  p06e parameterisation, not the integrator. That is the requirement as written; it is recorded
  here so that it is not read as a dynamics test.

## WP10 — [M3, moved from M5] `ferroastro-testdata` + `ferroastro-ephem`: SPK type 2/3 reader validated on de440.bsp

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13.

### Plan and pre-registered checks (written 2026-10-15, before any FerroAstro SPK evaluation)
Only the products' structure was inspected before this section was written: the de440.bsp segment
table (14 type-2 segments, read with jplephem 2.24), the testpo.440 layout, the `testeph.f` threshold
and the first block of `ascp01950.440`. No FerroAstro number exists yet; every bound below is fixed
before its first run.

**Sources read.** NAIF "DAF Required Reading" (`daf.html`, SHA-256 `6d501ec3…0e095fee`) and "SPK
Required Reading" (`spk.html`, SHA-256 `d6ef659c…0cdc7dc3`), both retrieved 2026-10-15 from
naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/req/: file record, summary/name records, segment
descriptor, data precedence ("the latest segment in a file supersedes"), type 2 and 3 layouts; JPL `testeph.f` (SHA-256 `18f32f07…adeb7120`; public JPL software,
not on the clean-room list) and jplephem 2.24 `spk.py`/`daf.py` (MIT, the REF-004 reference reader).
No REBOUND/REBOUNDx/ASSIST/Swiftest/Mercury6 source, and no ANISE code.

**Products pinned on first retrieval (2026-10-15).** They go into `tests/data/MANIFEST.toml`, which
is human-owned: **NEEDS HUMAN REVIEW** of every new pin.
| product | URL (https://ssd.jpl.nasa.gov/ftp/eph/…) | bytes | SHA-256 | Last-Modified |
|---|---|---|---|---|
| `de440-bsp` | `planets/bsp/de440.bsp` | 119 799 808 | `a4ce9bf9…fff7c4b5` | 2020-12-22 |
| `de440-testpo` | `planets/ascii/de440/testpo.440` | 858 238 | `45084a7f…6693429f` | 2020-12-22 |
| `sb441-n373s-bsp` | `small_bodies/asteroids_de441/sb441-n373s.bsp` | 982 106 112 | `21431132…ca8da10b` | 2021-09-02 |
| `de440-ascp01550`, `-01950`, `-02550` | `planets/ascii/de440/ascpNNNNN.440` | 29.2 M each | pinned on first fetch | 2020-12-22 |

`sb441-n373.bsp` itself is 14.13 GB (HEAD request, 2026-10-15). The plan's file name is kept for the
REF-003 work, but this WP pins the 936.6 M `sb441-n373s.bsp`, which is the one the jpl oracle
image in journal 03 lists. It holds 374 type-2 segments, all centred on the Sun (10), frame 1,
1549-12-31 to 2650-01-25. Which file REF-003 should use is OI-24 (human-owned).

**Checks.** Margins are reported as |Δ| / tolerance.
| # | check | tier | oracle | tolerance (pre-registered) | rationale |
|---|---|---|---|---|---|
| 1 | (REF-004, DEP-011) every one of the 14 de440 segments, raw (target wrt its own centre), at ≥ 50 TDB epochs spread over the segment's coverage plus both ends, f64 | PR-fast (cached de440.bsp) | `tools/goldens/jplephem_de440.py`, jplephem==2.24 | position ≤ **1 mm** (1e-6 km), velocity ≤ **1 µm/s** (1e-9 km/s), per component | REF-004's target, copied. jplephem uses the same Clenshaw recurrence and the same three-way `divmod` of the two-part date, so we expect agreement at a few ulp. **Risk stated up front:** 1 ulp of a Pluto or Neptune barycentric coordinate (4–7·10⁹ km) is 0.95 mm, so a 2-ulp difference there would fail the 1 mm target. If that happens it is reported, not widened. |
| 2 | (REF-001) every testpo.440 point whose target and centre are bodies (1–13); nutations (14) and librations (15) are not in de440.bsp and are skipped, with a count | PR-fast (cached) | testpo.440, threshold of JPL `testeph.f`: "agreement is considered okay if DEL is less than 1e-13" (au, au/day) | **< 1e-13** in au or au/day, per point | JPL's own acceptance for a reader. 1e-13 au = 1.5 cm. AU from header.440 via `ferroastro-constants`. |
| 3 | (DEP-007) double-double evaluation vs f64 at the check-1 epochs, every segment: ratio ρ = \|x_f64 − x_DD\| / (n·u·Σₖ\|cₖ\|) for positions, and the same with Σₖ k²\|cₖ\|·2/intlen for velocities (u = 2⁻⁵³, n = number of coefficients) | PR-fast (cached) | DD itself | **ρ ≤ 2** | The classical a-priori bound for Clenshaw in floating point is O(n·u·Σ\|cₖ\|); ρ ≤ 2 means "f64 round-off". The time argument is exact for these epochs (half-day + fraction split). |
| 4 | type 3 and big-endian: a synthetic SPK (deterministic dyadic coefficients, 2 segments, type 2 and type 3) written by Python in both byte orders, evaluated by jplephem; Rust builds the same bytes (SHA-256 must equal the one in the golden header) and evaluates them | PR-fast (no download) | jplephem 2.24 | as check 1 | tests the type-3 layout and BIG-IEEE reading against the reference reader. |
| 5 | (API-002) proptest: every truncation of a valid synthetic file, random byte corruption, and random garbage give `Err` with an `FA-E10xx` code or a valid result, never a panic; every truncation gives `Err` | PR-fast | — | no panic; truncations → `Err` | API-002 acceptance. |
| 6 | header/ASCII products: for the 10 SSB-centred bodies, every Chebyshev coefficient in the records covered by `ascp01550/01950/02550.440` printed in the ASCII `0.dddddddddddddddd D±ee` form (16 significant digits) equals the ASCII token | data | JPL DE440 ASCII coefficients + header.440 GROUP 1050 | **0 mismatches** | both products come from the same fit; the ASCII file is a 16-digit rendering. Any record-mapping or layout error gives mismatches. |
| 6b | geocentric Moon: SPK (301 wrt 3) − (399 wrt 3) vs the ASCII geocentric Moon, at 3 times per 4-day sub-interval | data | as 6 | position ≤ **1e-8 km**, velocity ≤ **1e-12 km/s** | ASCII quantisation (≤ 5e-16 relative per coefficient) on ~4e5 km gives ~2e-10 km, plus the EMRAT scaling round-off in the SPK; 50× margin. |
| 7 | (DEP-011, replaces ANISE, see below) Horizons DE441 barycentric states of 199, 399 and 301 vs our de440 chain, 21 TDB epochs 1600–2600 | data | JPL Horizons API (`signature.version` printed) | gross-error bound: **10 km**, **1e-3 km/s** | DE441 is not DE440, so the bound only catches convention errors: a UT/TDB mix-up gives ≥ 3000 km for Mercury, a wrong frame ~1e7 km. It does **not** catch TT vs TDB (≤ 80 m); check 2 covers the time argument. The measured DE440−DE441 difference is reported. |
| 8 | sb441-n373s.bsp: 3 asteroids (Ceres 2000001, Pallas 2000002, Vesta 2000004) at 50 epochs | data | jplephem 2.24 goldens | as check 1 | a second, independent SPK file (374 segments) through the same reader. |
| 9 | (REF-001, LIC-003, DEP-017) no `.bsp`, DE ASCII (`header.4xx`, `testpo.4xx`, `ascp*`/`ascm*`), SOFA or NOVAS file is tracked by git, and `ferroastro-testdata` is `publish = false` | PR-fast (pytest) | `git ls-files` | 0 files | inspection made executable. |

**ANISE (DEP-011).** The plan asks for an ANISE cross-check from `tools/`. The maintainer decision in
force says: do not add anise as any dependency (dev or otherwise). This WP therefore adds no
anise, not even the Python package, and validates against Horizons and the ASCII products
(checks 6 and 7). DEP-011's acceptance ("CI comparison against ANISE passes") stays **unmet**:
**NEEDS HUMAN REVIEW**.

**Amendment to check 5, made while writing the test (before any FerroAstro truncation run).** The
plan said "every truncation gives `Err`". That cannot hold: the synthetic file ends with zero padding
up to a whole record, and removing only padding leaves a valid file. The test asserts instead that
every truncation that cuts into the file record, the summary or name record, or a segment's array
gives FA-E1000/FA-E1001, and that a padding-only truncation parses and evaluates.

### What was built
- **`ferroastro-ephem` (L4, new; `std`, builds for wasm32).**
  - `Daf`: the file record (both byte orders, `NAIF/DAF` files by `ND` inference), the FTP string
    check, the summary-record chain with a loop guard, and the name records. Every read is checked.
  - `Spk`: `ND = 2`, `NI = 6`. Every segment's word addresses and, for types 2 and 3, its directory
    (`INIT`, `INTLEN > 0`, `RSIZE = 2 + k·(deg+1)`, `N·RSIZE + 4` = array length, coverage inside the
    record grid) are validated at open, so evaluation cannot read outside the file. Other SPK types
    open but return FA-E1002 when evaluated.
  - Chebyshev types 2 and 3 by Clenshaw, generic over `Real`, in jplephem's operation order (no FMA).
    The two-part TDB date is split with jplephem's three-step `divmod`. The final epoch of the last
    record belongs to that record. `Segment::locate` returns the record and `s`.
  - Segment chaining through the lowest common ancestor (Moon − Earth goes via the EMB, not the SSB),
    NAIF data precedence (last covering segment wins), a frame-consistency check, and
    `Ephemeris::state::<C, R>` returning `State<Icrf, C, R>` for `C = Barycentric | Heliocentric`
    (NAIF frame 1 is read as the ICRF-aligned DE frame; anything else is FA-E1004).
  - `parse_testpo` and the testeph code → NAIF id map.
  - New codes FA-E1000…FA-E1006 (range FA-E10xx added to the core table) and FA-E0204.
- **`ferroastro-testdata`:** `locate_product` (offline lookup plus SHA-256 re-verification; the
  PR tier uses it so it never touches the network).
- **`tools/fetch_jpl.py`** (5 pytest cases, ruff, mypy strict): downloads manifest products with
  retries on network errors only, verifies the pin, writes the same provenance sidecar as the Rust
  crate. `--verify-only` checks the cache offline. The pytest also scans `git ls-files` for `.bsp`,
  DE ASCII, SOFA and NOVAS files (LIC-003, REF-001, DEP-017): none.
- **`tools/goldens/jplephem_de440.py`** writes three goldens at `78f6045`:
  `jplephem-de440.csv` (784 rows), `jplephem-synthetic.csv` (64 rows), `jplephem-sb441.csv` (156 rows).
- **CI (CI-005):** the `rust` and `msrv` jobs restore `.fa-data` with `actions/cache` (v6.1.0,
  pinned by SHA, key = hash of `MANIFEST.toml`) and run `fetch_jpl.py` (a cache hit downloads
  nothing). The wasm32 job also builds `ferroastro-ephem`. The PR tier therefore needs the cache;
  locally, run `(cd tools && uv run python fetch_jpl.py)` once. A missing file fails loudly with
  FA-E0204 and that command.

### WP10 results (PR tier, 2026-10-15, toolchain 1.95.0)
| check | measured | tolerance | margin |
|---|---|---|---|
| 1 (REF-004) 14 segments × 56 epochs vs jplephem, f64 | position **0** km, velocity **0** km/s at all 784 epochs (bit-identical) | 1e-6 km, 1e-9 km/s | 0 |
| 2 (REF-001) testpo.440: 11 354 body points (1 847 nutation/libration points skipped, none else) | max DEL **7.105e-15** (line 2733, 1777-01-01, Neptune wrt Moon, x) | 1e-13 | 0.071 |
| 3 (DEP-007) f64 vs double-double, 784 golden epochs | ρ = **0.428** | 2 | 0.21 |
| 3, nightly: every record of every segment, 3 points each (1 205 382 evaluations, release, 2.6 s) | ρ = **0.650** | 2 | 0.33 |
| 4 synthetic type 2 + type 3, LTL and BIG (Rust bytes SHA-256-identical to the Python file) | **0** / **0** | 1e-6 km, 1e-9 km/s | 0 |
| 5 (API-002) every truncation (2 × 5120 lengths): 4304 per byte order cut into records/data → FA-E1000/E1001; 816 padding-only truncations parse; 2000 proptest corruptions (1672 still parse and evaluate, 328 rejected at parse); 500 garbage inputs | no panic | — | — |
| report only: double-double vs jplephem | 1.19e-7 km (Jupiter barycentre at 2650) | — | — |

The 1 mm risk stated before the run did not occur: our f64 evaluation is bit-identical to jplephem
at every golden epoch, so no ulp at Neptune/Pluto distance was lost.

### WP10 real-data results (data tier, `tools/run_long_local.sh`, log `target/long-local/20261015T092004Z-1cbfa25adcbb.log`, commit `1cbfa25`)
| check | measured | tolerance | margin |
|---|---|---|---|
| 6 DE440 ASCII: 3427 blocks of ascp01550/01950/02550, **1 727 208** coefficients | **0** mismatches (every record's MID and RADIUS also equal) | 0 | exact |
| 6b geocentric Moon, ASCII series vs SPK (301 wrt 3) − (399 wrt 3) | 1.746e-10 km, 8.88e-16 km/s | 1e-8 km, 1e-12 km/s | 0.017, 0.001 |
| 7 Horizons DE441 (`signature.version` 1.2, source DE441, frame ICRF; answers `4cbb27f7…`, `2f8b3e85…`, `3402a1aa…`) vs de440, barycentric, 21 epochs 1600–2600 | Mercury 1.49e-8 km; Earth 5.14e-3 km; Moon **0.418 km** (at 1600); velocity ≤ 9.8e-7 km/s | 10 km, 1e-3 km/s (gross) | 0.042, 0.001 |
| 8 sb441-n373s.bsp (374 segments, internal name `SPKMERGE`): Ceres, Pallas, Vesta × 52 epochs vs jplephem | **0** / **0** (bit-identical) | 1e-6 km, 1e-9 km/s | 0 |

Provenance (all retrieved 2026-10-15, SHA-256 equal to the pins): de440.bsp `a4ce9bf9…fff7c4b5`,
testpo.440 `45084a7f…6693429f`, header.440 `0f4636b6…6ac3359`, ascp01550 `ed95a8eb…edb6375`,
ascp01950 `e24f6356…3477286`, ascp02550 `66e1230a…e479c08`, sb441-n373s.bsp `21431132…ca8da10b`.

The DE440 − DE441 differences of check 7 (cm for Mercury, metres for the Earth, 0.4 km for the Moon
in 1600) are physical differences between the two ephemerides, reported here as information. The
gross bound was not tuned to them.

The whole data tier: 20 of 21 tests passed in the parallel run. `data_j2_and_lt_rungs_in_real_solar_equator_geometry`
failed with **FA-E0200, HTTP 503 from Horizons** (curl gave up after 4 attempts; a network error,
not an assertion). Run again alone, it passed with the WP6–WP9 numbers (J2 0.0284524, LT −0.0020148).
Every earlier data number is unchanged to the printed digit: Park17 frame 575.309163 (jack-knife
σ 0.001531), SW92 dog-leg 571.968047, GR from DE441 42.980669 / numerical 42.9800852, NUM-014
+4.431e-4 / +8.424e-7. The nightly tier: 4/4 pass (NUM-023 default 0.526/1.513; the long ensemble
is still the known failure at 0.629).

### Mutation probes (each reverted with `git checkout`; tree clean afterwards)
| mutant | result |
|---|---|
| two-part date summed before the split (`jd1 + jd2`) | killed (jplephem de440, synthetic, DD ratio) |
| type-2 velocity factor × (1 + 1e-9) | killed (jplephem velocity, testpo) |
| segment search first-to-last (precedence) | **survived** the first suite (de440 has no overlapping segments) → added `the_last_overlapping_segment_takes_precedence`, which kills it |
| FMA in the Clenshaw step | survives: it changes results only at the ulp level, far inside 1 mm. The bit-identity with jplephem is an observation, not an asserted property. |

### Progress-review corrections (status in WP10)
All were applied in WP7/WP8 and re-checked here: `long` is `workflow_dispatch` only and
`run_long_local.sh` produced this WP's long-tier log; `IAS15-EQ11-DEADBEAT` carries NEEDS HUMAN
REVIEW and `NUM012-ESTIMATOR-SPREAD`'s source no longer says "average out"; every new WP10 test loads
vectors with `load_canonical` (refuses `FA_VECTORS_DIR`); the data profile has `retries = 0` and
`fetch_jpl.py` retries network errors only; the μ-convention line is printed (−3.57e-6); the core
range table names `ferroastro-analytic` for 06xx and FA-E0404 exists, and now also lists FA-E10xx;
the M1 citing tests exist, and **DEP-007's "ephemeris evaluator" clause is now covered** (f64 and DD
evaluation of every de440 segment); API-003 stays deferred (`doctest_coverage.py`: 0 of 367 public
functions with a per-function doctest; `ferroastro-ephem` has a crate-level doctest). Every
human-owned question in correction 8, and the weekly GitHub `data` job question, is left unresolved.

### Deviations and NEEDS HUMAN REVIEW (WP10)
1. **ANISE (DEP-011).** Not done, by the maintainer decision in force (no anise dependency of any
   kind). Horizons (gross) and the ASCII products (exact) replace it. DEP-011's acceptance text
   still names ANISE: a requirement-text change for PR #2.
2. **Pins.** Seven new `MANIFEST.toml` pins were taken by the agent on first retrieval. The manifest
   is human-owned; confirm them.
3. **sb441 file.** `sb441-n373s.bsp` (936.6 MB) is pinned, not the 14.13 GB `sb441-n373.bsp` that
   the plan names. OI-24 (which file REF-003 uses, what n373 contains) stays open. Observed: 374
   Sun-centred type-2 segments.
4. **The PR tier now needs the JPL cache.** `cargo nextest run --workspace` on a fresh clone fails
   (FA-E0204, with the fetch command) until `fetch_jpl.py` has run once. The plan asks for exactly
   this ("fail loudly, do not skip"), but it changes the developer contract, and on a cache miss the
   CI `rust`/`msrv` jobs download ~120 MB from JPL. Decide whether this is acceptable.
5. **REF-004 bit-identity.** It holds today because the Clenshaw order and the date split copy
   jplephem's. The asserted tolerance is still REF-004's 1 mm / 1 µm/s.
6. **`EPHEM-F64-DD-ROUNDOFF-RATIO` is a FerroAstro engineering bound** (ρ ≤ 2 of n·u·Σ|cₖ|), not a
   published theorem; no literature was opened for it.
7. **testpo threshold.** testeph's check is strict (`DEL ≥ 1e-13` warns); the vector check is `≤`.
   They differ only at exactly 1e-13, and the measured maximum is 7.1e-15.
8. **Frame label.** NAIF frame 1 (`J2000`) in the DE files is mapped to the typed `Icrf`. This is how
   the DE4xx files are used (and Horizons' ICRF answers agree to < 0.42 km at 1600), but it is an
   interpretation that frame-bias-level work (≤ 0.02″) must keep in mind.
9. **Commit `7c47fb8` does not build on its own** (`Segment::locate` arrived in `ece7615`, pushed
   right after). History was not rewritten. Bisecting across it needs `ece7615`.
10. **Requirement-text suggestions for PR #2 (not edited here):** DEP-011: replace or qualify the
    ANISE clause (Horizons + ASCII products), and record the jplephem bit-identity; REF-001: name
    `fetch_jpl.py`, the manifest and the testeph 1e-13 threshold; CI-005: name the `.fa-data` cache
    and its cold-cache download; REF-003/OI-24: `n373s` vs `n373`.

### WP10 adversarial review (2026-10-15)

The reviewer re-ran every AGENTS.md command, the ephem data tier (Horizons answers fetched live again,
not from cache) and the nightly record sweep. The numbers match the builder's report: REF-004 is
bit-identical (0 km and 0 km/s at 784 epochs), testpo DEL is 7.105e-15, ρ is 0.428 at the golden
epochs and 0.650 in the nightly sweep, the ASCII products show 0 coefficient mismatches out of
1,727,208, sb441 is bit-identical, and the largest DE440−DE441 difference from Horizons is 0.418 km.

**Mutation probes** (applied locally and reverted, never committed):

| Mutation | Killed by |
|---|---|
| Clenshaw derivative `2·b` → `b` | cheb unit tests, synthetic, testpo, REF-004 |
| Type-2 velocity loses the `×2/INTLEN` factor | synthetic, testpo, REF-004 |
| Data precedence takes the first segment instead of the last | `the_last_overlapping_segment_takes_precedence` |
| End-point rule removed | synthetic, REF-004, DD |
| Big-endian words read as little-endian | synthetic (both tests), API-002 truncation |
| testpo code 3 mapped to the EMB | testpo unit test and REF-001 |
| Type-3 velocity scaled by 2 | synthetic |
| **`divmod` carry `i3` dropped from the record index** | **nothing**, so the mutant survived |

Every golden epoch has its day part at 0h and a fraction below 1, so the remainders never add up
past INTLEN. `Epoch::from_jd_parts` accepts any split, though, so a caller who passes the day part at
noon (for example `Epoch::j2000()` plus a fraction) reaches the carry branch. The reviewer added
`state_is_independent_of_the_two_part_date_split`, which runs at every segment, at three records per
segment and across dyadic fractions. It checks bit-identity between the 0h, noon and one-day-earlier
splits, and 464 of its evaluations use the carry. With this test in place the mutant is killed.

**Hardening:** `Daf::record` and `Daf::f64_word` now use checked byte-offset arithmetic. A corrupt
`NEXT` pointer can be as large as 2⁵³ records. That multiplication overflows a 32-bit `usize` on
wasm32, and in a debug build the overflow panics, which API-002 forbids. On 64-bit targets there was
no defect.

No blockers were found. The NEEDS HUMAN REVIEW items 1–10 above still stand.

## 📈 Progress review: after WP10

I am the independent progress reviewer (an agent, not the builder). I audited `m3-newtonian-nbody` at `ba357d8` (WP7–WP10), stacked on `m0-foundation` (merge base `e23c9ef`, the WP6 review). I ran everything below myself on 2026-10-15 UTC, on the local machine with toolchain 1.95.0. I loosened nothing and committed no mutation; mutants ran in a scratch clone, and the worktree was clean throughout.

### Overall health: 🟡 YELLOW
- **Green:**
  - All eight AGENTS.md gates pass locally, and GitHub CI on `ba357d8` is green.
  - The data tier passes (21/21) on an empty cache, and so does the nightly tier (4/4). Some tests needed a re-run for infrastructure reasons, not assertions (see §1).
  - Four key numbers reproduce in independent Python, including an SPK reader I wrote from the NAIF Required Reading, checked against testpo.440.
  - No golden value or tolerance was moved or widened without a NEEDS HUMAN REVIEW note.
  - The layer check is green, all M1 debt requirements are cited, and every data number from WP5–WP9 is unchanged to the printed digit.
- **Not green:**
  - **Two of five new mutants survive the PR tier** (§3). The Park17 mean-orbit axis-sign flip still survives every tier; the WP9 review flagged it and it was not followed up. A disabled SHA-256 re-check in `locate_product` also survives, which is a cache-substitution route.
  - Infrastructure flakiness is now systematic. For the third WP running, Horizons answered HTTP 503 to the parallel data tier (2 tests this time). `run_long_local.sh` also fails on a fresh cache, because it runs the nightly tier before anything has fetched de440.bsp.
  - Unchanged from earlier reviews: NUM-001 (1e9 steps) and NUM-023 (≥ 5e7 steps) are unmet in the default configuration. The known-failure guard still holds at energy slope 0.629, and the `pc_b_from_g` fix is waiting on a human decision.

### 1. Commands (tip `ba357d8`)
| command | result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo nextest run --workspace` + `cargo test --doc --workspace` | **176 passed, 25 skipped**: the skips are 21 `data_*` and 4 `nightly_*`, which are the only `#[ignore]`s. 12 doctests pass. |
| `cargo deny check` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` | pass |
| `ruff check`, `ruff format --check`, `mypy` (strict) | pass |
| `reuse lint` | pass |
| `trace.py --check` | pass: 119 requirements, 345 citing tests, 0 errors. `check_layers`: 15 crates, 55 edges, 0 errors. |
| `uv run pytest` (tools) | 34 passed |
| GitHub CI on `ba357d8` (run 38713104902) | rust, msrv, wasm32, python, reuse and dco pass. `data` and `long` were skipped, as expected. |
| `tools/run_long_local.sh`, empty `FA_DATA_DIR` | **nightly 3/4 on the first pass.** `nightly_de440_double_double_vs_f64_every_record` failed with FA-E0204 (de440.bsp missing) because the script runs the nightly tier before the data tier and never runs `fetch_jpl.py`. It passed once the cache was filled (ρ = 0.650). |
| same run, data tier | **19/21 on the first pass.** `data_horizons_n2b_from_de441_state` and `data_horizons_osculating_elements_match` failed: Horizons returned HTTP 503 four times. Both passed when re-run with `--test-threads 1`. Neither failure was an assertion. |
| nightly tier (release) | 4/4. NUM-023 default ensemble: 0.526 / 1.513. The long ensemble is the known failure at 0.629 ± 0.018 / 1.590. Reversibility is 4.50e-12. |

### 2. Real-data and benchmark results
Provenance (every file was fetched fresh, retrieved 2026-10-15, and its SHA-256 equals the manifest pin):
- header.440: `0f4636b6…6ac3359`
- de440.bsp: `a4ce9bf9…fff7c4b5`, 119 799 808 B
- testpo.440: `45084a7f…6693429f`
- ascp01550: `ed95a8eb…edb6375`
- ascp01950: `e24f6356…3477286`
- ascp02550: `66e1230a…e479c08`
- sb441-n373s.bsp: `21431132…ca8da10b`
- pck00011.tpc: `3dff7b1d…e28bb1`
- IERS Leap_Second.dat: `6cb6f5d4…e268d7`
- IANA leap-seconds.list: `db5a895f…bc3433`
- SW92: `9f7b30ca…f9c4d742`

Every Horizons answer carried `signature.version` 1.2 and source DE441.

Margin = |Δ| / tolerance.

| test (tier) | measured | oracle (source) | tolerance | margin |
|---|---|---|---|---|
| REF-004 `ref004_jplephem_de440` (PR) | 0 km, 0 km/s at 784 epochs (bit-identical) | jplephem 2.24 goldens | 1e-6 km, 1e-9 km/s | 0 |
| REF-001 testpo.440 (PR) | max DEL **7.105e-15** (11 354 points; 1 847 nutation/libration points skipped) | JPL testpo.440, testeph threshold | 1e-13 | 0.071 |
| ↳ independent Python reader (mine) | max DEL 1.42e-14, same 11 354 and 1 847 counts | same | 1e-13 | 0.14 |
| DEP-007 f64 vs DD Clenshaw ρ (PR / nightly) | 0.428 / 0.650 | FerroAstro bound n·u·Σ\|c\| (engineering) | 2 | 0.21 / 0.33 |
| synthetic type 2 + 3, LTL and BIG (PR) | 0 / 0 | jplephem 2.24 | 1e-6 km, 1e-9 km/s | 0 |
| `data_de440_ascii_coefficients_equal_the_spk` | 0 mismatches in 1 727 208 coefficients | DE440 ASCII ascp01550/01950/02550 | exact | 0 |
| same test: geocentric Moon | 1.746e-10 km, 8.88e-16 km/s | ASCII series | 1e-8 km, 1e-12 km/s | 0.017, 0.001 |
| `data_horizons_de441_barycentric_states_vs_de440_gross_bound` | Mercury 1.49e-8 km, Earth 5.14e-3 km, Moon **0.418 km** (1600) | Horizons DE441 (gross bound) | 10 km, 1e-3 km/s | 0.042 ⚠ weak oracle by design |
| `data_sb441_asteroids_match_jplephem` | 0 / 0 (Ceres, Pallas, Vesta × 52) | jplephem goldens | 1e-6 km, 1e-9 km/s | 0 |
| DEP-009 / REF-009 pyerfa time goldens (PR), f64 | TT-TDB 9.59e-12 s; all others 0 | pyerfa 2.0.1.5 | 1 ns (PROVISIONAL) | 0.0096 |
| same, double-double | worst 8.92e-12 s (TDB-TCB) | pyerfa | 1 ns | 0.0089 |
| DEP-010 round trips, f64 | worst 9.59e-12 s | identity | 2e-10 s | 0.048 |
| hifitime TDB − ERFA TDB (PR) | 1.957e-4 s (year 2600) | analytic bound | 4.0e-4 s | 0.49 |
| `data_leap_seconds_match_iers_and_iana` | 55 instants, \|Δ\| 0 s | IERS + IANA | exact | 0 |
| pyerfa frame goldens (PR) | f64 0 everywhere; DD ≤ 1.49e-15 rad | pyerfa 2.0.1.5 | 1e-12 (PROVISIONAL) | 0.0015 |
| `data_frame_rotations_reproduce_horizons…` | ECLIPJ2000 2.42e-16 / 3.28e-16; IAU_SUN 4.72e-16 / 4.06e-16 | Horizons ECLIPTIC, BODY EQUATOR | 1e-12 rel | ≤ 0.0005 |
| `data_naif_pck_solar_pole` | 286.13 / 63.87 | NAIF pck00011 | exact | 0 |
| NUM-014 `data_num014_…` | difference − p_A **+4.431e-4**; − geometry +8.424e-7 ″/cy | p06e port (computed reference) | 1e-3 / 1e-5 | **0.44** / 0.084 |
| D5 `data_gr_schw_f64_and_double_double…` | f64 42.980085159575, DD 42.980085160105 (Δ −5.30e-10) | PHY-005 42.980 ± 0.001; f64 = DD to 1e-6 | ±1e-3; 1e-6 | 0.085; 0.0005 |
| `data_horizons_n2b_from_de441_state` (PHY-001) | 1.087e-9 ″/cy | 0 | 1e-6 | 0.001 |
| `data_horizons_osculating_elements_match` | e 1.08e-15 rel, a 1.29e-16 rel, angles ≤ 2.84e-14° | Horizons ELEMENTS | 1e-12 rel | 0.001 |
| `data_horizons_gr_rate_…_vs_park17` | 42.980669 | Park17 GE 42.9799 | ±0.0027 | 0.28 |
| `data_gr_rung_from_horizons_de441_state` | 42.9806442 | analytic 42.980651 / Park17 42.9799 | ±1e-3 / ±0.0027 | 0.010 / 0.28 |
| real-geometry J2 / LT | 0.0284524 / −0.0020148 | Park17 0.0286 / −0.0020 | ±0.0011 / ±0.0002 | 0.13 / 0.074 |
| NUM-013(a) isolation | GR −6.6e-6, J2 4.8e-9, LT 1.3e-9 | 0 | 1e-4 | 0.066 |
| NUM-012 spread | GR 1.76e-5, J2 3.4e-8, LT 5.2e-8 | 0 | 1e-4 | 0.18 |
| `data_park17_rate_from_horizons_de441_mean_orbit` | **575.309163** (jk σ 0.00153), Q −0.0447605 | Park17 575.3100 ± 0.0015 | NUM-011 ±0.01; 3σ ±0.00643 | 0.084; 0.13 |
| `data_sw92_rate_from_horizons_de441_ecliptic` | 571.968047 | SW92 T1 577.716804 | ±20 | 0.29 ⚠ weak oracle; the −5.75 gap is open (human) |
| NUM-023 default / long ensemble (nightly) | 0.526 / 1.513; **0.629** / 1.590 | Brouwer ½, 3/2 | ±0.1 | 0.26 / 0.13; **1.29 FAIL (known)** |

Every number above equals the builders' WP7–WP10 reports, and the WP5/WP6 numbers, to every printed digit.

**Independent recomputation.** Python/numpy only, with no FerroAstro, jplephem or ERFA code; scripts are in the local scratchpad.
1. **REF-001 with my own SPK reader.** I wrote a type-2 DAF/SPK reader from the NAIF DAF/SPK Required Reading: summary chain, Chebyshev with derivative, and EMB/Earth/Moon chaining. I evaluated de440.bsp at every testpo.440 body point.
   - Point counts: 11 354 evaluated and 1 847 skipped, the same as FerroAstro.
   - Max DEL is **1.42e-14** (Neptune wrt Pluto, 1594). FerroAstro gets 7.1e-15.
   - My reader collapses the date into one binary64 TDB-seconds value, which costs about one ulp. Both readers are below 1e-13 by about 7–14×. The testpo threshold therefore does not depend on FerroAstro sharing code with jplephem.
2. **NUM-014 p_A slope.** I fitted the IAU 2006 p_A series (Capitaine et al. 2003; IERS Conventions 2010 eq. 5.39) by OLS at the 831 perihelion epochs. I computed those epochs from header.440's Mercury − Sun state at JDEPOC: Kepler's equation for the first passage, then the Keplerian period.
   - Result: **5030.332574 ″/cy**, against FerroAstro's `p06e` slope of 5030.332575517. Δ = 1.5e-6.
   - Caveat: the 5028.796195 linear coefficient I typed is the OI-02 value. This check therefore confirms the port's arithmetic and the passage sampling, not the coefficient. The coefficient is checked by `t_erfa_c` and pyerfa, which take it from ERFA's `p06e.c`.
3. **GR rate from the header.440 state.** Mercury − Sun at JDEPOC, with μ = GM☉ for the elements, gives a = 0.38709935 au and e = 0.2056166109. Both are identical to FerroAstro's.
   - 6πGM☉/(c²a(1−e²)), with n from GM☉ + GM☿, gives **42.980101000 ″/cy**. FerroAstro's D5 "analytic (same osculating orbit)" prints 42.980097432.
   - The −3.57e-6 difference is exactly the μ-in-n convention: the single-body `analytic_per_orbit` uses GM☉ in the period, while the Horizons GR helper uses GM☉ + GM☿. See finding F5.
   - The numerical D5 rate, 42.980085160, is 1.6e-5 below the analytic value. That is the anomalistic-vs-Keplerian period effect already reported, inside PHY-005's ±1e-3.
4. **PHY-014 shift.** 0.0286·(J2SUN/2.25e-7 − 1), with J2SUN = 2.196139151652982e-7 from the fresh header, is −0.000685. That gives **575.309315**, confirming the maintainer's oracle of 575.3093.

### 3. Drift audit
- **`git log -p -- tests/vectors`, `e23c9ef..ba357d8`:** 13 commits. Only two of them remove any line:
  - `eed1741`: the `NUM012-ESTIMATOR-SPREAD` `source` text. This was requested by the WP6 review; the value and tolerance are unchanged, and it carries NEEDS HUMAN REVIEW.
  - `37808e5`: the `REAL-DD-EXP-REL-ERR` test domain changed from [−700, 700] to [−600, 700] after its first run failed at x = −688.2. The bound is unchanged, and the change is justified by the subnormal trailing word, a format limit. The NEEDS HUMAN REVIEW note is in the vector and in WP7.

  **No value or tolerance was moved or widened.** Disclosed non-blind items carry NEEDS HUMAN REVIEW: `TIME-TDB-SECONDS-DD-ROUNDTRIP`, the NUM-014 pre-runs, the PROVISIONAL 1 ns / 1e-12 pyerfa bands, and `NUM006-F64-BENCH-RATIO` 2 %.
- **`#[ignore]`:** 25, up from 16. All are named `data_*` (21) or `nightly_*` (4), and I ran all of them. There is no `cfg` gating. The early `return`s in `num023_brouwer.rs` fire only when report-only env knobs are set, as before.
- **UNVERIFIED values as oracles:** none.
  - OI-02's p_A is not a vector value. NUM-014 uses the `p06e` port, which `t_erfa_c` and pyerfa check (WP9 proposes that this closes OI-02's use; that is a human decision).
  - GMSDOT (OI-04) is deliberately absent.
- **Circular oracles:** none that matter.
  - REF-004 and sb441 compare against jplephem, an independent MIT reader.
  - The bit-identity with jplephem holds because FerroAstro copies its Clenshaw order and `divmod`. That is shared *algorithm*, not shared code, and recomputation 1 shows the testpo pass does not depend on it.
  - NUM-014 compares two ERFA parameterisations: `ecm06`, which is Fukushima–Williams, against `p06e` angles. It exercises frames, not dynamics, as the WP9 review noted.
  - `EPHEM-F64-DD-ROUNDOFF-RATIO` is a FerroAstro engineering bound (disclosed).
- **Mutation probes.** I mutated one key formula per new crate in a scratch clone at `ba357d8`, ran the PR tier, and reverted each one. Earlier per-WP reviews already probed `erfa` (p_A coefficient, split order), `frames` (p_A, solar node) and `ephem` (Clenshaw, divmod carry, precedence), so I chose new targets:

| crate | mutant | PR-tier result |
|---|---|---|
| real | DD `mul` drops the `lo·hi` cross term (`cl3 = cl1 + tl1`) | killed by 10 tests (DEP-012, forces DD, analytic DD, time DD) |
| time | `tdb_minus_tt` topocentric, u = 6378 km (≤ 2 µs effect) | killed (pyerfa goldens f64 and DD) |
| frames | Park17 mean-orbit y = X × z instead of z × X (a 180° turn about the pole) | **survived the PR tier.** The WP9 review showed it also survives the data tier, since rates do not depend on it. This is the second review to flag it, and it is not fixed. |
| ephem | `Heliocentric` NAIF centre = SSB instead of the Sun | killed (`typed_states_chain_segments_to_the_barycentre_and_the_sun`) |
| testdata | `locate_product` SHA-256 re-check disabled | **survived.** No test feeds a tampered cache file. A swapped de440.bsp or testpo.440 in `FA_DATA_DIR` would be used silently by the PR tier, which is an oracle-substitution route (red-team H2 pattern). |

- **Layering** (`cargo metadata`, checked by hand): no cycles and no upward edges.
  - `ephem` depends on core and real only.
  - `frames` depends on core, erfa and real. The unused `time` edge was removed in WP9.
  - The only listed exception is still `cli → vectors`.

**Findings (new in this review):**
- **F1 (medium): the Park17 axis sign is unpinned.** See the frames mutant above.
- **F2 (medium): no negative test for `locate_product`'s SHA-256 check.** See the testdata mutant above.
- **F3 (low): `run_long_local.sh` fails on a fresh cache.** It never runs `fetch_jpl.py`, and it runs nightly before data, so the nightly ephem test hits FA-E0204.
- **F4 (low, recurring): parallel Horizons requests get HTTP 503.** This is the third WP in a row. It is a test-infrastructure problem, not a physics one, but it makes every long-tier log need a manual serial re-run.
- **F5 (low): the μ-in-n convention differs between helpers.** `SingleBody::analytic_per_orbit` uses μ = GM☉ in the period, while `schwarzschild_advance`'s documented convention uses GM☉ + GM☿. The difference is 3.6e-6 ″/cy, so it is harmless now, but NUM-013(b) differences will need one convention.
- **F6 (info): the weekly GitHub `data` job now downloads about 1.2 GB per run** (de440.bsp, three ascp files and the 982 MB sb441), and it runs the Horizons tests in parallel on a shared runner IP. That adds to the open maintainer question about the weekly job.
- **F7 (info): `7c47fb8` does not build on its own** (disclosed in WP10). Anyone bisecting needs to skip it.

### 4. Requirement coverage
`trace.py` reports **42/92** `test` requirements cited (31 at the WP6 review) and 345 citing tests.
- **M0, uncited:** STD-001…011, TST-001, DEP-001/002/003/014 and ORG-008. These are gate-enforced. Whether `trace.py` may count gate evidence is still a human question.
- **M1:** every requirement is now cited. Acceptance is still unmet for NUM-001 (the 1e9-step clause and Kozai) and NUM-023 (≥ 5e7 steps, default configuration).
- **M2:** every requirement is cited. The partial items are unchanged: NUM-017, PHY-011/OI-35, NUM-012 and NUM-013(b). The API-003 per-function doctest clause is deferred, with 0 of 367 public functions covered.
- **M3, uncited (WP11–WP18 still to come):** PHY-012, PHY-013, NUM-008, NUM-009, REF-005, REF-006, REF-007, REF-011, CI-002, CI-003, CI-005 and CI-006.
  - **CI-005 has a delivered mechanism (WP10's `.fa-data` cache and `fetch_jpl.py`) but no citing test.**
  - Cited: NUM-011, NUM-013(a), NUM-014, PHY-015, REF-004 and LIC-003.
- **M5 items started early:** DEP-011 (cited; the ANISE clause is unmet by maintainer decision) and REF-001 (cited). REF-003 is uncited (OI-24).

### 5. Corrections for the next builder (WP11 onwards)
1. **F1:** add offline assertions that pin the sign of each axis of every `ferroastro-frames` rotation.
   - Park17 mean orbit: assert that ŷ·(ẑ × X̂_ICRF) > 0 and that x̂ = ŷ × ẑ.
   - Solar equator: x̂ = node on the ICRF equator, with ẑ_ICRF × pole as the ascending direction.
   - Write each expected component into a vector derived from the definition, not from a run. Confirm that the y-flip mutant dies in the PR tier.
2. **F2:** add a `ferroastro-testdata` unit test that writes a wrong-content file under a temporary `FA_DATA_DIR` and asserts that `locate_product` returns the mismatch error code. Add a pytest for `fetch_jpl.py --verify-only` that does the same. Cite CI-005 and REF-002, which closes CI-005's missing citation.
3. **F3:** in `tools/run_long_local.sh`, run `(cd tools && uv run python fetch_jpl.py)` before the nightly tier, or run data before nightly. Log the fetch provenance.
4. **F4:** serialise the Horizons tests with a nextest test group (`[test-groups] horizons = { max-threads = 1 }` plus a `[[profile.data.overrides]]` filter on the Horizons tests). This changes scheduling only, not assertions or retries: test-level `retries` stays 0. Record it in IMPLEMENTATION-NOTES.
5. **F5:** choose one μ convention for the analytic rate (GM☉ in 6πGM/c², and GM☉ + GM☿ or GM☉ in n) and use it in `analytic_per_orbit` and in `schwarzschild_advance`. Document it in the rung metadata. This must happen before WP14's NUM-013(b) differential runs. It is a computed-reference change, so record the before and after values (shift 3.57e-6 ″/cy).
6. **WP14, PHY-012:** implement exactly the maintainer protocol: contribution = (Sun + Mercury + planet) − (Sun + Mercury), with identical initial conditions and the same estimator, and cross terms reported separately (provisional, OI-38).
   - Do not tune the protocol to hit ±0.005; report the misses.
   - Carry the red-team M3 note that Park's protocol is UNVERIFIED.
   - Add the N-AST differential assertion (±0.0003, red team) once WP15 lands. Do not use OI-09's Mercury self-row (0.0050) as an oracle.
7. **WP12/WP22, PHY-014:** use 575.3093 ″/cy (DE440 J2) as the oracle. This review reproduced 575.309315 independently.
   - The vector rationale must carry the derivation: 0.0286·(J2_DE440/2.25e-7 − 1) = −0.000685.
   - State that missing-LT and missing-asteroid errors are caught by the differential rungs, not by PHY-014.
8. **WP16/WP17 oracle containers:**
   - GPL images stay local or CI-cache only. FerroAstro is never built inside them.
   - Use the documented APIs only (clean-room).
   - WP16 must regenerate the WP8/WP9 pyerfa goldens bit-identically (all non-header lines), as WP8 item 5 requires.
9. **Bisectability:** run `cargo check --workspace` on each commit before pushing (F7).
10. **Still for humans (unchanged; no agent may resolve these):**
    - adopting `pc_b_from_g`, which would clear NUM-023 and the known-failure guard;
    - the weekly GitHub `data` job, now about 1.2 GB per run (F6);
    - the PR tier depending on the JPL cache;
    - the seven agent-taken MANIFEST pins;
    - ANISE / DEP-011;
    - OI-24 (n373s vs n373);
    - the PROVISIONAL pyerfa bands;
    - the NUM-014 reading and its 2× margin;
    - the API-003 doctest threshold;
    - the hifitime #417 workaround;
    - every item in the WP6 review's correction 8.

Reviewer artefacts are in the local scratchpad, not committed: `gates.log`, `long.log`, `rerun.log`, `mutate.log`, `mutate.sh`, `py/testpo_indep.py` and `py/recompute.py`.


## WP11 — [M3] `ferroastro-system`: body catalogue, tiers and initial-condition assembly

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13.

### Plan and pre-registered checks (written 2026-10-15, before the first FerroAstro run)
**Disclosure, before anything else.** To choose the header symbols to map, I explored header.440 and
de440.bsp in Python with jplephem 2.24 (scratchpad scripts, not committed) **before** writing these
bounds. What I saw: every header state (`X1…ZD9`, `XB…`, `XS…`) equals the de440.bsp barycentric
state at JDEPOC 2440400.5 to ≤ 6.7e-7 km and ≤ 8.4e-15 km/s when read as SSB-centred ICRF in au and
au/day; `XM…` equals the SPK Moon − Earth to 1.8e-8 km; the heliocentric reading misses by
6.8e5 km; `XC…ZDC` are all zero; the header barycentre (Newtonian, no asteroids) is 122.7 km from
the origin. The position bound below is the plan's (≤ 1 m, PROVISIONAL), fixed before this
exploration. The velocity bound (1e-9 km/s) is mine, chosen **after** seeing the numbers: it is
REF-004's velocity target, copied, and is 1e5× the observed value, so it was not tuned.

**Header symbols (GROUP 1040/1041).** `X1…ZD1`, `X2…`, `X4…X9` (body or system barycentre),
`XB…ZDB` (Earth–Moon barycentre), `XS…ZDS` (Sun), `XM…ZDM` (Moon relative to Earth, the DE
"geocentric Moon" variable), `XC…ZDC` (all zero in DE440; their meaning stays OI-23 and they are
not used), `CENTER` (0). The interpretation under test is **"CENTER = 0 means SSB-centred states in
the ICRF-aligned DE frame, positions in au and velocities in au/day (AU = header `AU` km, day =
86 400 s TDB)"**. It is encoded as a named constant in the crate and printed by the test.

**NAIF ids.** Mercury 1, Venus 2, Mars…Pluto 4…9 (the header's planet states are system
barycentres, which for Mercury and Venus coincide with the body; GM1…GM9 are system GMs), Sun 10,
Earth 399, Moon 301 (split from `XB` and `XM` with EMRAT). Header `MAnnnn` with nnnn < 8000 →
asteroid NAIF 2 000 000 + nnnn; the 66 entries `MA8001…MA8236` are **not identified** here (their
GMs sum to 8.5e-8 GMS, about 0.03 Earth masses, and no SPK in the manifest carries ids for them).
They are kept as "unidentified masses" and are not placed. Their identity is an OI-24 question for
humans.

**Checks** (margin = |Δ| / tolerance).
| # | check | tier | oracle | tolerance | rationale |
|---|---|---|---|---|---|
| 1 | (REF-002, OI-23) header.440 ICs assembled by `InitialConditions::from_header` vs de440.bsp evaluated at JDEPOC through `from_ephemeris`, every body (Sun, 1, 2, 4–9, Earth, Moon), barycentric; plus the geocentric Moon `XM` vs SPK 301 − 399 | PR-fast (cached header.440 + de440.bsp) | de440.bsp (independent product) | position **≤ 1e-3 km** (PROVISIONAL, plan), velocity **≤ 1e-9 km/s** | the SPK is a Chebyshev fit to the same integration; at the integration's initial epoch the fit reproduces the ICs to its interpolation error. The test also prints the residual under the heliocentric reading and under "km, km/s" units, and asserts each is > 1e3 × the tolerance, so a pass discriminates between the readings. |
| 2 | Earth + Moon GMs from EMRAT recombine to GMB | PR-fast (committed table) | GMB itself | **≤ 2 ulp** of GMB | each GM is GMB·frac rounded once and the sum is rounded once; 2 ulp covers the three roundings. |
| 3 | the header catalogue's GMs equal `De440::gm` for all 12 bodies | PR-fast (cached header) | the committed table | **exact** (bitwise) | same tokens, same conversion arithmetic. |
| 4 | barycentre of all header bodies, Newtonian μ and relativistic μ* (DE440 convention), without and with the 343 numbered asteroids of sb441-n373s.bsp at JDEPOC | PR-fast (without) and data (with) | — | **report only** | the plan defers the assertion to WP15. |
| 5 | typed state ↔ SoA container round trips | PR-fast | identity | **exact** | no arithmetic. |
| 6 | compile-fail: a heliocentric state where a barycentric one is required | PR-fast (trybuild) | rustc | does not compile | API-001 pattern. |
| 7 | Horizons DE441 vectors at JDEPOC (500@0, ICRF, TDB) assembled by `from_states`, vs header ICs | data | JPL Horizons (signature.version printed) | gross bound **10 km, 1e-3 km/s** (as WP10 check 7) | DE441 ≠ DE440, so this only catches convention errors (frame, centre, units, time scale). The DE440 − DE441 differences are reported. |
| 8 | μ* barycentring: after `rebarycentre`, Σ μ*·r and Σ μ*·v vanish to round-off; μ* − μ has the size of (v² − U)/(2c²) | PR-fast (header ICs) | definition | Σμ*r ≤ 1e-6 km·(GM units), see vector | algebraic property. |

**Relativistic barycentre.** μᵢ* = μᵢ [1 + vᵢ²/(2c²) − (1/(2c²)) Σⱼ≠ᵢ μⱼ/rᵢⱼ], SSB: Σ μᵢ* rᵢ = 0,
Σ μᵢ* vᵢ = 0 (the time derivative of μ* is neglected in the velocity condition). Transcribed from
memory of Folkner et al. 2014 (IPN PR 42-196) / Park et al. 2021 Eqs. 1–2 as cited by NUM-018; I did
**not** re-open the PDFs: **NEEDS HUMAN REVIEW** (same status as OI-33 for the EIH transcription).

**Review corrections in this WP.** F1–F4 were applied before the crate (commits `e47834c`,
`78116c2`, `d5ee274`, `011ff7c`); F5 is discussed below.

### What was built (WP11)
- **`ferroastro-system` (L5, new; builds for wasm32, added to the CI wasm32 build).** Dependencies:
  core, real, constants, ephem (no `ferroastro-frames` edge yet: nothing in this WP rotates; the
  CRATE-PLAN edge arrives with the first rotation).
  - `Catalogue` keyed by NAIF id: `de440_major` (11 major bodies from the committed table) and
    `from_header` (run-time header.440: the 11 majors, 343 numbered asteroids as `Tier::Minor`
    with NAIF 2 000 000 + n, and the 66 `MA8001…MA8236` entries as `UnidentifiedMass`, never
    placed). `Tier::{Major, Minor, Ring}` per body, `set_tier`, `ids_in`, `retain`. GMs use the
    same conversion arithmetic as `De440`, so both sets agree bit for bit.
  - `SystemState<F, C, R>`: SoA `Vec<R>` components, typed `push`/`set`/`state`/`states`,
    `components_mut` for integrators, `cast` between backends, `heliocentric()` on barycentric
    systems and `barycentric(sun)` on heliocentric ones.
  - `EarthMoon<F, R>`: EMB + geocentric Moon (`Geocentric` centre added to core, NAIF 399 in
    ephem), `split(emrat)` and `from_bodies`.
  - `barycentre` / `rebarycentre` / `mass_weights` with `MassModel::{Newtonian, RelativisticDe440}`
    (Neumaier-compensated sums).
  - `InitialConditions<R>`: `from_header` (refuses CENTER ≠ 0, FA-E1101), `from_ephemeris`,
    `from_states`; provenance string carries `HEADER_STATE_INTERPRETATION`.
  - Codes FA-E1100 (body), FA-E1101 (initial conditions), FA-E1102 (barycentring); the core
    range table lists FA-E11xx.
- **Review corrections:** F1 axis-sign test (frames), F2 cache-substitution tests (Rust + pytest,
  citing CI-005 and REF-002; `locate_product_in` added), F3 `run_long_local.sh` fetches first and
  logs provenance, F4 nextest `horizons` test group (max-threads 1) in the data profile.

### WP11 results (PR tier, 2026-10-15, toolchain 1.95.0)
| check | measured | tolerance | margin |
|---|---|---|---|
| 1 header.440 ICs vs de440.bsp at JDEPOC, 11 bodies + EMB + geocentric Moon, f64 | **4.77e-7 km** (Neptune system), **7.1e-15 km/s** (Mercury) | 1e-3 km, 1e-9 km/s | 4.8e-4, 7e-6 |
| 1, double-double | 5.66e-7 km (Neptune), 5.7e-15 km/s | same | 5.7e-4, 6e-6 |
| 1, geocentric Moon `XM` vs SPK 301 − 399 | 2.9e-11 km, 5.6e-17 km/s | same | — |
| 1, wrong readings | heliocentric ≥ **6.735e5 km**; "km, km/s" ≥ **5.41e7 km** | must exceed 1 km | discriminating |
| 2 GM_E + GM_M vs GMB | **1 ulp** | 2 ulp | 0.5 |
| 3 header catalogue vs committed table, 11 GMs | **0** (bit-identical) | exact | — |
| counts | 343 numbered asteroids, 66 unidentified, 409 MA total | exact | — |
| 4 barycentre of the 11 header bodies (report only) | Newtonian **122.689122 km**, 2.158e-8 km/s; μ* 122.689086 km | — | report |
| 8 after 2 μ* rebarycentring passes | 7.5e-12 km, 6.3e-19 km/s | 1e-6 km, 1e-12 km/s | ~0 |
| μ* virial bound | \|μ*/μ − 1\| ≤ 2.9e-10 (Mercury) vs U/(2c²) = 1.3e-8 | < U/(2c²) | — |
| 5 typed round trips (f64 and DD casts) | exact | exact | — |
| 6 compile-fail (2 cases) | do not compile (E0308) | — | — |

**OI-23 evidence (for the maintainer, not a closure).** With CENTER = 0, every header.440 state
equals de440.bsp at JDEPOC to < 1e-6 km and < 1e-14 km/s when read as SSB-centred ICRF in au and
au/day (AU = 149 597 870.7 km, day = 86 400 s); `XB` is the EMB, `XM` is Moon − Earth, `XS` the
Sun; the system suffixes 1, 2, 4–9 are NAIF 1, 2, 4–9. The heliocentric reading misses by
6.7e5 km and the km reading by ≥ 5.4e7 km. `XC…ZDC` are zero and stay unexplained.
**NEEDS HUMAN REVIEW:** close OI-23 (and unblock REF-002's state part) on this evidence, or not.

### WP11 real-data results (data tier, run 2026-10-15)
| check | measured | tolerance | note |
|---|---|---|---|
| 7 Horizons DE441 vectors at JDEPOC (11 queries, `signature.version` 1.2, source DE441, frame ICRF) vs header ICs | **9.54e-7 km** (Neptune), **1.07e-14 km/s** (Mars) | 10 km, 1e-3 km/s (gross) | DE441 reproduces DE440's ICs at JDEPOC at the 1e-6 km level, i.e. the two ephemerides share their initial conditions at 1969 (consistent with WP10's growing DE440−DE441 difference away from 1969). |
| 4 with asteroids: all 343 numbered asteroids placed from sb441-n373s.bsp at JDEPOC (Sun-centred segments + header Sun) | barycentre offset **122.650 km** with, 122.689 km without (Newtonian; μ* differs by 3.6e-5 km) | report only (WP15) | the 343 asteroids move it by only 0.04 km. |

**Hypothesis, UNVERIFIED (for WP15):** the remaining ~123 km offset is of the size that the 66
unidentified `MA8xxx` masses (Σ GM/GMS = 8.46e-8) would produce if they sat at Kuiper-belt
distances (8.5e-8 × ~45 au ≈ 570 km at most). Nothing here identifies them; OI-24 owns that.

**Long tiers, full run** (`tools/run_long_local.sh`, log `target/long-local/20261015T095648Z-42d1a84643dd.log`,
commit `42d1a84`, clean tree): the new fetch step verified all 7 manifest products (all cache hits,
SHA-256 equal to the pins) and logged every provenance sidecar before the nightly tier. Nightly
4/4 pass (NUM-023 long ensemble still the known failure at 0.629 / 1.590). Data **23/23 pass on
the first pass**; every earlier number is unchanged to the printed digit (Park17 frame
575.309163, jack-knife σ 0.001531; SW92 dog-leg 571.968047; D5 42.980085160; GR from DE441
42.9806442). Caveat: every Horizons answer in this run was a cache hit (52 hits, 0 misses), so it
does not show that the F4 test group prevents HTTP 503 under live load. The 11 live Horizons
queries of check 7 were made earlier the same day in a direct `--profile data` run, serially
through the group, with no error.

### Mutation probes (WP11; each reverted, tree clean afterwards)
| mutant | result |
|---|---|
| EMRAT fractions swapped in `split` | killed (unit, geocentric round trip, check 1) |
| header velocities not divided by 86 400 s | killed (check 1, rebarycentring) |
| header `S` block read from `1` | killed (check 1, rebarycentring) |
| Moon GM with the Earth fraction | killed (catalogue vs table) |
| `heliocentric()` adds the Sun instead of subtracting | killed (unit) |
| `translate` skips velocities | killed (rebarycentring residual) |
| μ* potential sign flipped; μ* v² term dropped | **survived the first suite** (the barycentre is report-only) → added the virial-bound test, which kills both |
| asteroid NAIF id off by one | **survived the first suite** → the catalogue test now checks 2 000 000 + n for every `MA` symbol; killed |
| frames: Park17 y = X × z; solar node flipped (F1) | killed by the new axis-sign test |
| testdata: SHA-256 re-check disabled (F2) | killed by both new cache tests |

### F5 (μ convention) — not applied in WP11, with reasons
F5 is not a `ferroastro-system` change: it touches `ferroastro-ladder` (`SingleBody`) and the
callers of `ferroastro-analytic::schwarzschild_advance`. I record the analysis for WP12/WP13, which
come before WP14's NUM-013(b) differentials as F5 requires:
- `SingleBody` integrates a test particle in the field of GM☉ alone, so its analytic rate uses the
  period with μ = GM☉ (the dynamics' own Keplerian period). Every `schwarzschild_advance` caller
  passes `mu_period = GM☉ + GM☿` (the GM of Horizons' osculating elements). The difference is the
  3.57e-6 ″/cy the WP10 review measured.
- Either single choice moves one side away from its own dynamics: GM☉ everywhere makes the
  Horizons-element helpers inconsistent with the elements' GM; GM☉ + GM☿ everywhere makes the
  single-body analytic rate inconsistent with its test-particle dynamics (by 3.6e-6 ″/cy, inside
  every current tolerance).
- Suggested rule for the human decision: "n uses the μ of the Keplerian problem that produced the
  elements" (test particle: GM☉; Sun–Mercury relative motion and Horizons elements: GM☉ + GM☿),
  printed in the rung metadata. The N-body differential runs of WP14 compare FerroAstro runs with
  each other and need no analytic n at all. **NEEDS HUMAN REVIEW**, since the review asked for one
  value and this note proposes one rule instead.

### Deviations and NEEDS HUMAN REVIEW (WP11)
1. **OI-23 closure** on the check-1 evidence above (maintainer decision). REF-002's text still marks
   CENTER/units UNVERIFIED: a requirement-text change for PR #2.
2. **μ* transcription** (Park et al. 2021 Eqs. 1–2 / Folkner et al. 2014) was written from memory;
   no PDF was re-opened. Only its sign and size are tested (virial bound), not its exact form.
3. **Unidentified MA entries.** `MA8001…MA8236` (66 entries, Σ GM/GMS = 8.46e-8) are kept but not
   placed. Their identity, and whether they explain the 122.7 km barycentre offset, belongs to OI-24.
4. **Check-1 velocity tolerance** (1e-9 km/s) was chosen after a disclosed exploration (see the
   plan); the position tolerance is the plan's.
5. **Asteroid states are DE441-consistent** (sb441-n373s.bsp), not DE440. Check 7 shows the two
   share their ICs at JDEPOC to 1e-6 km for the major bodies; the asteroid ICs of DE440 itself are
   not in any pinned product (OI-24).
6. **CI job name.** The wasm32 job now also builds `ferroastro-system`, but its display name was
   left unchanged so that any required-check configuration keeps matching.
7. **Requirement-text suggestions for PR #2 (not edited here):** REF-002 notes: replace the
   UNVERIFIED CENTER/units sentence with the check-1 result once OI-23 is closed; NUM-009: say that
   header.440 has 409 `MA` entries, of which 343 are the numbered asteroids; NUM-018: cite the
   exact equation numbers of μ* after a PDF check.
8. **F2 code choice.** A substituted cache file is reported as FA-E0204 ("present but has SHA-256
   …"), not FA-E0201, because FA-E0201's fix text ("upstream changed, re-pin") would be wrong for a
   local substitution. The tests assert FA-E0204 plus both digests.

### WP11 adversarial review (2026-10-15)
The reviewer re-ran every AGENTS.md command plus `check_layers.py`, `pytest`, `lint_literals.py`,
the wasm32 build and the system data tier. All passed: nextest 191 passed and 27 skipped, 13
doctests, trace 119 requirements. The Horizons check was re-run **live**, with an empty Horizons
cache (`FA_DATA_DIR` holding only symlinks to the JPL products): 11 answers with cache=miss,
signature.version 1.2, DE441. The result is unchanged at 9.537e-7 km and 1.066e-14 km/s. Check 1
reproduced 4.768e-7 km / 7.1e-15 km/s in f64 and 5.663e-7 km / 5.7e-15 km/s in double-double.
That level is the header's own 16–17-digit floor at Neptune's distance.

Two changes came out of the review:
- **The μ* magnitude was not pinned.** Two further mutants survived the whole suite: (a) the
  relativistic model silently returning the Newtonian weights, and (b) 1/c² in place of 1/(2c²).
  The virial-bound test now also requires μᵢ*/μᵢ − 1 = (vᵢ² − Uᵢ)/(2c²) for every body, each side
  computed independently from the raw states, to within 8 ulp of 1. Both mutants are now killed.
  This pins the implementation to the transcription. Whether the transcription matches Park et al.
  2021 is still NEEDS HUMAN REVIEW item 2.
- **The `Tier::Minor` doc said Minor bodies "attract nothing".** NUM-009 does not say that, and
  PHY-013's N-AST row (0.0012 ″/cy) is the asteroids' pull on Mercury. A force model built from
  that doc in WP13/WP15 would lose N-AST. The doc now leaves the reaction undecided and states
  that it must be kept. **NEEDS HUMAN REVIEW:** NUM-009 should say explicitly whether Minor bodies
  act on the Majors (DE440 practice). This is a requirement-text change for PR #2.

Noted only: the WP15 plan text already names MA8201–MA8236 as the 36-point KBO ring. That makes
MA8001–MA8030 plausibly the 30 KBOs (30 + 36 = 66). Item 3 above stays open until OI-24 is read
from IOM 392R-21-005.

## WP12 — [M3] `ferroastro-forces`: Newtonian N-body kernels, geocentric Moon, J2 with reaction

Builder agent, 2026-10-15. Branch `m3-newtonian-nbody`, stacked on `m0-foundation`, draft PR #13.

### Plan and pre-registered checks (written 2026-10-15, before any WP12 code or run)
**Clean room (DEP-008).** The N-body equations are the textbook Newtonian point-mass sums. The
solar-J2 field and its reaction on the Sun follow the DE440 description (Park et al. 2021, the
form cited by NUM-019 as Eq. 29) through the existing `SolarJ2` term (Journal #4 R1); the reaction
is `a_Sun = −Σᵢ (μᵢ/μ_Sun) aᵢ`, which is the momentum balance of a potential that depends on
`rᵢ − r_Sun` only. The geocentric-Moon differential acceleration is the classical third-body
expansion (Battin-type `q` form): with `d = r_j − r_E`, `ρ = r_M − r_E`,
`q = ρ·(ρ − 2d)/d²`, `(d − ρ)/|d − ρ|³ − d/d³ = [d F(q) − ρ (1 + F(q))]/d³`, and
`F(q) = (1 + q)^{−3/2} − 1 = Σ_{k≥1} c_k q^k`, `c_k = c_{k−1}·(−(2k+1)/(2k))`, `c_0 = 1`. I wrote it
from this derivation, not from any code. No REBOUND/REBOUNDx/ASSIST/Swiftest/Mercury6 source is
read; IAS15 is the existing WP3 clean-room integrator (Rein & Spiegel 2015).

**Design (to be built).**
* `ForceTerm<R>` (object-safe): `name`, `gauge`, `parameters` (name, value, unit), `tiers` (the
  tiers it acts on), `bind` (resolve body indices once) and `accumulate` (add accelerations in a
  fixed order). Terms: `NewtonianMutual` (Major↔Major, pairs `i < j` in storage order),
  `MinorFromMajors` (Minor feels the Majors; the reaction on the Majors is **on by default**,
  configurable, NEEDS HUMAN REVIEW for NUM-009's text, see the WP11 review), `RingMasses` (Ring
  bodies attract every moving body and never move), `GeocentricMoon` (the Moon's geocentric
  Newtonian acceleration with the Taylor differential, default **12** terms, minimum 4 enforced
  by NUM-008; the closed form is used when |q| > 0.05, never reached by the planets), and
  `SolarJ2WithReaction` (configurable pole; acts on the Majors other than the Sun).
* `NBodyModel<F, R>`: a `SecondOrderSystem<R>` + `ForceModel<R>` whose flat coordinates are
  **component-major SoA** (`x₀…x_{n−1}, y₀…, z₀…`); IAS15 is layout-agnostic. With the geocentric
  Moon on, the Earth slot holds the Earth–Moon barycentre and the Moon slot holds Moon − Earth
  (DE's variables, header `XB`/`XM`). Optional Neumaier accumulation of the accelerations.
* `NBodyPropagator<F, R>`: IAS15 + the model + a TDB start epoch; typed `SystemState` in and out.

**Checks** (margin = |Δ| / tolerance). Tolerances are fixed here, before any run.
| # | check | tier | oracle | tolerance | rationale |
|---|---|---|---|---|---|
| U1 | barycentric `NewtonianMutual` vs the existing `NewtonianPointMasses` on the 11 header bodies | PR | existing M0 kernel | **bitwise** | same pair order and arithmetic |
| U2 | `Σ μᵢ aᵢ` of `SolarJ2WithReaction` (J2 = 1e-3 synthetic, 5 bodies) | PR | 0 | ≤ 8 ε Σ μᵢ\|aᵢ\| | reaction makes the sum vanish algebraically |
| U3 | Taylor differential (12 terms) vs the closed form, and vs a double-double direct difference, Sun/Venus/Jupiter geometries | PR | DD direct difference | ≤ 4 ε × \|tidal term\| (12 terms); 4 terms: reported | truncation of 12 terms is < 1e-20 relative at q ≤ 0.02 |
| 1a | NUM-019 momentum: Sun + 8 planets + Moon (header.440 ICs, geocentric Moon, J2 on: DE440 J2SUN, ASUN, IAU WGCCRE 2015 pole), 100 yr | PR | 0 | \|P(t) − P(0)\| / Σ μᵢ\|vᵢ\| ≤ **64 ε √N_steps** | random-walk round-off bound with 64 ulp per step |
| 1b | NUM-023 energy slope with J2 on: ensemble of Sun + 8 planets (EMB as one body) runs differing by 1e-13 relative IC perturbations; fit from 10 Mercury orbits on | nightly | Brouwer ½ (`NUM023-ENERGY-SLOPE`) | ±0.1 | NUM-023; the run length is the longest nightly-feasible one, chosen from a **cost-only** timing run (steps/s, no energy printed) |
| 2 | TST-005: the 1a run twice (f64) and a 1-yr run twice (double-double) | PR | itself | **bitwise** (states and step counts) | determinism |
| 3 | geocentric vs barycentric Moon, 10 yr from header.440 ICs, Sun + 8 planets + Moon: (a) the two formulations in double-double at ε = 1e-12 agree on Moon − Earth | PR or nightly (by cost) | each other | ≤ **1e-6 km** (PROVISIONAL) | both are the same ODE; DD truncation at ε = 1e-12 is far below 1 mm |
|   | (b) round-off error (f64 minus its DD truth) of Moon − Earth, barycentric over geocentric = **gain factor** | same | — | factor > 1 asserted, value reported | NUM-008's rationale (\|ρ\| ≈ \|r\|/400) |
|   | (c) f64 geocentric vs f64 barycentric Moon − Earth | same | — | ≤ 2 × the barycentric round-off error | "agree to within the round-off gain" |
| 4 | real data: Newtonian + J2, Sun + 8 planets + Moon from header.440, JDEPOC → +10 yr (1969-06-28 → 1979), vs de440.bsp every 10 d; residual table (barycentric and heliocentric, per body) | PR (cached JPL products, CI-005) | de440.bsp | Jupiter–Neptune barycentric **≤ 100 km** (PROVISIONAL, plan); all others reported | the model omits 1PN (mean-motion terms ~μ/(c²a)), asteroids and the KBO ring; Mercury/Earth/Moon are expected at the 10²–10³ km level and are not asserted |

**Addendum to 1b (written after the cost-only timing, before any energy was computed).** The
example `crates/ferroastro-forces/examples/nbody_cost.rs` (structural circular orbits, prints steps
and wall time only) ran 1000 yr in 317 499 steps at 26 848 steps/s on one thread (release). 1b is
therefore fixed at **32 runs × 100 000 yr** (≈ 3.2e7 steps per run, ≈ 1.0e9 steps in total, about
20 min on the 32-thread local machine), with 32 log-spaced checkpoints from 10 Mercury orbits to
100 000 yr. The ensemble members differ by multiplying Mercury's initial x coordinate by
`1 + k·1e-13`, k = 0…31. The energy slope is asserted (±0.1 of ½); the momentum and the
angular-momentum slopes are reported. The test runs alone (nextest `threads-required`) in the
nightly tier. Total ensemble steps reach NUM-001's 1e9; a single run of 1e9 steps (~10 h) is not
nightly-feasible.

Expectations written before running (order of magnitude only, for the reviewer): 1PN mean-motion
drift over 10 yr ≈ 3·2π·N_orbits·μ/(c²a)·a gives Jupiter ~20 km, Earth ~300 km, Mercury
≳ 10³ km. A run with Pluto added is reported as a second row set (not asserted).

### What was built (WP12)
- **`ferroastro-forces::nbody`** (new deps: `ferroastro-system`, `ferroastro-ephem`; layer check
  green, wasm32 build green):
  - `ForceTerm<F, R>` (object-safe): `name`, `gauge`, `parameters`, `tiers`, `bind`,
    `accumulate`, `potential_over_g`, `supplies_moon_relative`.
  - Terms: `NewtonianMutual` (Major↔Major, pairs `i < j`), `MinorFromMajors` (`MinorReaction::On`
    by default), `RingMasses` (fixed masses), `GeocentricMoon` (Taylor differential, 12 terms by
    default, minimum 4, closed form above |q| = 0.05), `SolarJ2WithReaction` (configurable pole,
    Majors only). `SolarJ2`'s field moved into a shared `j2_field` without changing its arithmetic.
  - `Layout` / `View` / `Accelerations`: component-major `SoA` flat vectors, DE Earth–Moon
    variables (EMB + Moon − Earth), optional Neumaier accumulation (`with_compensation`); ordinary
    bodies accumulate directly in the integrator's output slice (no allocation unless compensated).
  - `NBodyModel<F, R>`: `SecondOrderSystem` + `ForceModel`, `pack`/`unpack`/`earth_moon`,
    `energy_over_g` (kinetic + every term's potential), `momentum_over_g`, `describe`.
  - `NBodyPropagator<F, R>`: the IAS15 hook (TDB epoch in, typed `SystemState` out).
  - `examples/nbody_cost.rs`: cost-only timing (used to size 1b).
- **`ferroastro-integrators`: unchanged.** IAS15 is layout-agnostic, so the hook needed no
  integrator change.
- Tests: `src/nbody/tests.rs` (U2, U3, tiers, geocentric vs DD barycentric accelerations,
  compensation, coded errors, pack/unpack), `tests/nbody_de440.rs` (U1, 1a, 2, 4),
  `tests/nbody_moon_formulations.rs` (3), `tests/nbody_brouwer.rs` (1b, nightly). Vectors:
  `tests/vectors/nbody.toml`. The nightly profile runs 1b alone (`threads-required`), which
  changes scheduling only.

### WP12 results (PR tier, cached pinned JPL products, 2026-10-15, toolchain 1.95.0)
Provenance: header.440 sha256 `0f4636b6…6ac3359`; de440.bsp sha256 `a4ce9bf9…fff7c4b5`, both
retrieved 2026-10-15 (cache hit, re-verified on load).

| check | measured | tolerance | margin |
|---|---|---|---|
| U1 `NewtonianMutual` vs M0 kernel, 11 header bodies | **0** (0 differing words) | bitwise | — |
| U2 J2 reaction `|Σμa|/(ε Σμ|a|)` | **0.100** | 8 | 0.013 |
| U3 Taylor (12 terms) vs DD direct difference, worst of 4 geometries | **0.568 ε** of the tidal term | 4 ε | 0.14 |
| U3, 4 terms (reported) | 0.45 ε (Sun-like, q = 1.5e-4) … **6.4e6 ε** (Sun aligned with ρ, q = −5.1e-3) | — | — |
| U3, direct f64 difference (reported) | 15 … 349 ε | — | — |
| geocentric vs DD-barycentric accelerations (synthetic) | Moon − Earth 1.4e-16 rel., EMB 5.3e-16 | 1e4 ε / 1e3 ε | — |
| 1a momentum, 100 yr, 51 756 steps | `|ΔP|/Σμ|v|` = 5.2e-16 = **0.010 ε√N** | 64 ε√N | 1.6e-4 |
| 2 TST-005 f64 (100 yr ×2) / DD (1 yr ×2, 516 steps) | **0 / 0** differing words, equal step counts | exact | — |
| 3a DD geocentric vs DD barycentric Moon − Earth, 10 yr (ε = 1e-12; 14 066 / 14 047 steps) | **7.2e-13 km** | 1e-6 km | 7e-7 |
| 3b f64 round-off of Moon − Earth, 10 yr: geocentric / barycentric | 1.47e-7 km / 2.19e-5 km: **gain 148.9** | > 1 | — |
| 3c f64 geocentric vs f64 barycentric | 2.19e-5 km = **1.001 ×** the barycentric round-off | ≤ 2 × | 0.50 |

**Check 4 (real data): Newtonian + solar J2 vs de440.bsp, 1969-06-28 → +10 yr, 365 samples
every 10 d, IAS15 default (5 346 steps).** Maximum position residual over the span, km:

| body | barycentric | heliocentric | final (barycentric) | with Pluto added (barycentric, reported) |
|---|---|---|---|---|
| Sun | 1.381 | — | 1.281 | 2.362 |
| Mercury | 3408.9 | 3409.9 | 2059.7 | 3410.7 |
| Venus | 877.3 | 878.2 | 877.3 | 879.0 |
| Earth | 527.8 | 526.9 | 505.8 | 529.0 |
| Moon | 696.1 | 695.1 | 674.9 | 697.1 |
| Mars system | 387.9 | 389.0 | 370.5 | 387.2 |
| Jupiter system | **32.39** | 31.91 | 31.83 | 34.52 |
| Saturn system | **6.79** | 5.55 | 6.79 | 7.49 |
| Uranus system | **11.47** | 11.00 | 11.47 | 3.87 |
| Neptune system | **6.39** | 7.12 | 6.39 | 7.20 |
| Pluto system | — | — | — | 7.13 |
| Moon − Earth (geocentric) | 177.2 | — | 175.4 | 177.2 |

Asserted (pre-registered): outer planets ≤ 100 km → **32.39 km (Jupiter), margin 0.32. Pass.**
The inner-planet and Moon residuals have the size the plan expected from the missing 1PN terms
(Mercury ≳ 10³ km, Earth ~ 300–500 km). This is a sanity check, not a fidelity claim. Pluto's
absence mostly shows in Uranus (11.5 → 3.9 km).

### Mutation probes (WP12; each reverted with `git checkout`, tree clean afterwards)
| mutant | result |
|---|---|
| J2 reaction on the Sun removed | killed (U2, 1a) |
| Taylor coefficient sign (`c_k = +c_{k−1}(2k+1)/2k`) | killed (U3, geocentric accelerations, 3) |
| Newtonian Earth/Moon terms via `add` (double-counted in Moon − Earth) | killed (geocentric accelerations, 3) |
| EMB mass fractions swapped | killed (geocentric accelerations, 1a, 3, 4) |
| geocentric Moon term drops the third bodies | killed (geocentric accelerations, 3) |
| J2 evaluated at `r_Sun − rᵢ` | killed (U2) |
| `MinorReaction::Off` ignored | killed (tiers) |
| Ring bodies not frozen in `finish` | **survived: equivalent mutant.** No built-in term ever adds to a Ring body, so the freeze only guards user-written terms. |

### Progress-review corrections (status in WP12)
- **F1–F4** were applied in WP11 and are unchanged here. F4's test-group filter does not need
  WP12's tests, which make no Horizons queries.
- **F5 (μ convention): not applied in WP12.** It changes `ferroastro-ladder`/`ferroastro-analytic`,
  not this WP's crates. The WP11 analysis still holds: the N-body tests here compute no analytic
  rate, so no WP12 number depends on the convention. It must be settled before WP14's NUM-013(b)
  differentials. The natural place is WP13, whose PHY-005 scenario rung is the first consumer.
  **NEEDS HUMAN REVIEW:** one μ, or the WP11 rule "μ of the Keplerian problem that produced the
  elements".
- **`cargo check --workspace` before every push:** done for every WP12 commit.
- WP14/PHY-014/WP16/WP17 items: not applicable to WP12; nothing in WP12 touches them.

### WP12 1b — N-body Brouwer ensemble with J2 (nightly, release, local; logs `target/wp12/brouwer-20261015T102158Z-6595c61.log` and `target/wp12/brouwer2-*-e12ba0e.log`)
32 runs × 100 000 yr, **1 010 886 862 IAS15 steps in total**. Wall time was 2 585 s on the first
run and 3 332 s on the second (the machine was shared). The pre-registered size was estimated at
~20 min; the real-IC runs took ~45–55 min, so the nightly override allows 60 min.

| quantity (ensemble RMS, fit from 10 Mercury orbits to 1e5 yr) | slope | status |
|---|---|---|
| relative energy error, energy evaluated in f64 (**pre-registered**) | **0.050 ± 0.027** | **FAIL** vs 0.5 ± 0.1 |
| \|ΔP\| / Σμ\|v\| | 0.813 ± 0.014 | reported (final 2.4e-13) |
| relative \|L\| error | 1.004 ± 0.011 | reported. **Physics, not round-off:** a fixed-pole J2 conserves only L·k̂ |
| post-hoc: energy of the same f64 states evaluated in double-double | 0.278 ± 0.023 | reported only, added after the failure |
| post-hoc: (L − L₀)·k̂ / \|L₀\| | 0.102 ± 0.017 | reported only |

The second run reproduced the first bit for bit in every pre-registered column (TST-005
evidence at 1e9 steps).

**Reading (NEEDS HUMAN REVIEW).** The f64-evaluated relative energy error is 1.0e-15 at 10 Mercury
orbits and 1.9e-15 at 1e5 yr. It sits on the f64 evaluation floor of the total energy for the
whole span, so the pre-registered fit measures the floor and not round-off growth. Two things make
the observable insensitive:
- Mercury, which sets the global IAS15 step, carries ~1e-5 of the system's energy.
- Jupiter's energy is integrated with ~10⁴ steps per orbit.

The double-double evaluation removes the evaluation floor. It shows the true energy error staying
at ~1e-16 for ~800 yr and then growing to 1.3e-15 by 1e5 yr. Fitted by eye over 820 yr → 35 700 yr,
that segment is roughly t^0.57. This is a **post-hoc window**: it is reported, not claimed.
Neither diagnostic shows the systematic (slope ≈ 1) drift that WP3/WP6 found for Sun + Mercury.

**What changed in the test.** Following the `nightly_num023_long_ensemble_known_failure`
precedent, the test is now a **known-failure guard**. It passes while the pre-registered slope
still fails NUM-023's ±0.1, and it fails if that ever stops reproducing. It cites no requirement.
The tolerance (±0.1) and expected value (½) are unchanged.

Decisions for humans:
- whether NUM-023's energy diagnostic for N-body runs should evaluate the energy in double-double;
- whether it should use a per-body observable (e.g. Mercury's Keplerian energy is not conserved,
  so this needs thought);
- or whether it should use a fit window that starts after the floor. Any of these would be a new
  pre-registration.

**NUM-023 is NOT shown for the N-body model.**

### Deviations and NEEDS HUMAN REVIEW (WP12)
1. **1b failed as pre-registered** (above). The test is converted to a known-failure guard
   (precedent: NUM-023 long ensemble). The diagnostics were added post hoc.
2. **`MinorFromMajors` reaction defaults to on** (Minor bodies attract the Majors). NUM-009's text
   does not say; the WP11 review asked that the N-AST pull be kept. A requirement-text change
   for PR #2.
3. **Solar J2 acts on Majors only** (NUM-009 literal: Minor bodies feel the Majors' Newtonian
   forces only). Whether DE440 applies J2 to the asteroids was not checked against the paper.
4. **Solar pole for J2:** the tests use the IAU WGCCRE 2015 pole (`ferroastro-frames`). That DE440
   uses this pole for its J2 term is **unverified here** (the DE440 paper was not re-opened). The
   pole is configurable.
5. **DE440 Eq. 29.** The reaction form `a_Sun = −Σ (μᵢ/μ_Sun) aᵢ` follows NUM-019's citation and
   the momentum argument; the equation number and its typography were not re-read from the paper
   (same status as OI-50).
6. **NUM-008's "four terms suffice in f64".** In this `q` formulation, 4 terms leave a truncation
   of up to 6.4e6 ε of the tidal term for a Sun-aligned geometry (|q| = 5e-3); 12 terms reach
   < 1 ε. The default is 12. That 4 terms suffice for the 2 mm/40 yr target in a different
   expansion (Subbotin et al. 2023) is not contradicted, since periodic errors of that size stay
   sub-mm. The journal statement needs a source check.
7. **Ring tier:** Ring bodies are frozen (acceleration 0). The total momentum of the moving bodies is
   then not conserved (external field). The Ring-freeze mutant is equivalent for the built-in terms.
8. **TST-005 docs clause** (a float-reproducibility section) is not addressed here.
9. **Check 4 runs in the PR tier** (pinned JPL cache, CI-005), like WP11's check 1, so WP12 adds no
   `data_`-tier test. The data tier was re-run as a regression check: 23/23 passed, all cache hits.
10. **F5** is deferred to WP13 (see above).

### WP12 gate results (2026-10-15, local, toolchain 1.95.0)
| command | result |
|---|---|
| `cargo fmt --all --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo nextest run --workspace` + `cargo test --doc --workspace` | **203 passed, 28 skipped** (the `data_`/`nightly_` ignores); 13 doctests pass |
| `cargo deny check` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace` | pass |
| ruff check / ruff format --check / mypy strict / pytest | pass / pass / pass / 35 passed |
| `reuse lint` | pass |
| `trace.py --check` | pass: 119 requirements, 394 citing tests (TRACE.md regenerated) |
| `check_layers.py`, `lint_literals.py` | 16 crates, 67 edges, 0 errors; 0 findings |
| wasm32 build of `ferroastro-forces` | pass |
| data tier (`--profile data --run-ignored only`) | 23/23 pass (regression; cache hits) |
| nightly 1b (`nbody_brouwer`) | known-failure guard passes; the pre-registered assertion **fails** (slope 0.050) |
