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
