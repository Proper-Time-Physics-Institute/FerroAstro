<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# DEP — Dependencies, crate reuse and dependency licensing

**Scope.** These requirements govern which third-party Rust crates FerroAstro may depend on, which it must build itself, and how the dependency graph is policed. The rule is "reuse the plumbing, build the physics". Interface and tooling crates (clap, serde, rmcp, wasm-bindgen, pyo3/maturin, cargo-deny, nextest, criterion, proptest, approx) are adopted. The flagship integrators, the SPK reader and the ERFA subset are clean-room or in-crate implementations generic over a crate-owned `Real` scalar. Source: Journal #2 (`docs/strategy/journal/02-rust-ecosystem-licenses.md`); crate facts were queried on 2026-10-14. Repository-content and oracle-isolation licensing rules are in [LIC.md](LIC.md).

---

### DEP-001 — cargo-deny rejects copyleft and unlisted licenses

```yaml
id: DEP-001
title: "cargo-deny rejects GPL, LGPL, AGPL, SSPL and any license not on the allow-list"
status: draft
criticality: high
verification: test
statement: >-
  cargo-deny SHALL reject GPL-*, LGPL-*, AGPL-*, SSPL and any license not on
  the explicit allow-list (DEP-002) anywhere in the dependency graph, including
  dev- and build-dependencies of published crates. CI SHALL fail on any
  violation.
acceptance: >-
  `cargo deny check licenses` passes on main. A canary PR that adds a GPL
  dependency (dev- or build-) fails CI.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md License risks and DEP-001.
parent: null
related: [DEP-002, DEP-003, STD-006, LIC-001]
notes: >-
  DEP-001 to DEP-003 are AUTHORITATIVE for license policy content; STD-006 is
  the CI gate that enforces it. cargo-deny must check the actual license files,
  not only the crates.io SPDX field (rmcp: GitHub reports NOASSERTION; sofars:
  LICENSE carries extra SOFA text). cargo-deny reads LICENSE files only when
  Cargo.toml declares no `license`, so this needs the DEP-015 clarify entries.
```

### DEP-002 — License allow-list

```yaml
id: DEP-002
title: "License allow-list, with MPL-2.0 only for reviewed exceptions"
status: draft
criticality: high
verification: test
statement: >-
  The license allow-list SHALL be Apache-2.0, Apache-2.0 WITH LLVM-exception,
  MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016
  and CC0-1.0. MPL-2.0 SHALL NOT be on the allow-list; it SHALL be permitted
  only through per-crate exceptions for reviewed crates, initially `hifitime`
  (production) only.
acceptance: >-
  deny.toml `allow` contains exactly this list (no MPL-2.0) and the MPL-2.0
  exception list is {hifitime}. A canary MPL-2.0 crate not on the exception
  list fails `cargo deny check`.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-002.
parent: DEP-001
related: [STD-006, DEP-004, DEP-009]
notes: >-
  Journal #5 §3.4 shows a shorter illustrative allow-list ending in "..."; this
  list is authoritative. anise is not a dependency; decision deferred to M5
  (OI-16, OI-72, and red-team L2, Journal #9: anise 0.10.6 brings in
  BlueOak-1.0.0 via minicbor and CDLA-Permissive-2.0 via webpki-roots, has a
  RustSec advisory (proc-macro-error2 unmaintained), and its build.rs has a
  build-time network path via the ureq build-dependency). anise depends on
  sofars, whose LICENSE carries the SOFA Board header; whether SOFA terms bind
  sofars and hence anise users is UNVERIFIED. The earlier "anise dev-only"
  exception was removed 2026-10-14: cargo-deny licence exceptions ignore
  dependency kind, so dev-only cannot be enforced by an exception (red-team
  L1); adopting anise at M5 would need a second config with
  `[graph] exclude-dev = true` and no anise exception. lox-* and erfa are
  MPL-2.0 but not on the initial exception list.
```

### DEP-003 — Named crate bans

```yaml
id: DEP-003
title: "Ban AGPL/GPL/LGPL crates and unresolved-provenance crates by name"
status: draft
criticality: high
verification: test
statement: >-
  cargo-deny SHALL ban by name, whatever their declared license:
  nyx-space / nyx_space, rug, gmp-mpfr-sys, cspice-sys, librebound-sys,
  libassist-sys, adam_assist, rebound-rs, thor-rs-assist (until DEP-015 clears
  its provenance) and erfars (archived, DEP-015).
acceptance: >-
  deny.toml [bans] lists all ten names. A canary PR adding any one fails CI.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Audit table, License
  risks and DEP-003.
parent: DEP-001
related: [LIC-001, DEP-008, DEP-015]
notes: >-
  nyx-space is AGPL-3.0 and would impose source-offer obligations on the
  network-served MCP server and web demo. Whether thor-rs-assist derives from
  GPL REBOUND code is UNVERIFIED. rug is banned in every dependency graph of
  this workspace, including dev-dependencies (cargo-deny bans apply to dev
  deps). An arbitrary-precision oracle built on rug may live only in a separate
  repository or in a crate excluded from the workspace, never shipped.
```

### DEP-004 — No vendor-and-modify of MPL files

```yaml
id: DEP-004
title: "MPL-2.0 dependency files are never vendored and modified in-tree"
status: draft
criticality: high
verification: inspection
statement: >-
  MPL-2.0 dependency files SHALL NOT be vendored and modified in-tree. Any fix
  SHALL go upstream or into a separately published MPL fork.
acceptance: >-
  No MPL-2.0-licensed source file exists in the repository tree (checked by
  `reuse lint` / SPDX scan); CODEOWNERS review covers any vendored directory.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md License risks (MPL-2.0)
  and DEP-004.
parent: null
related: [DEP-002, DEP-005, STD-010]
notes: null
```

### DEP-005 — THIRD_PARTY_LICENSES in every artifact

```yaml
id: DEP-005
title: "Every distributed artifact ships a generated THIRD_PARTY_LICENSES file"
status: draft
criticality: high
verification: test
statement: >-
  Every distributed artifact (crates, CLI binaries, PyPI wheels, WASM bundle,
  MCP server image) SHALL ship a generated THIRD_PARTY_LICENSES file (e.g. via
  cargo-about or `cargo deny list`). For MPL components it SHALL state where
  their source can be obtained.
acceptance: >-
  A release-pipeline test unpacks each artifact type and finds
  THIRD_PARTY_LICENSES, including a source location for every MPL-2.0
  component.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-005.
parent: null
related: [DEP-004, STD-010, LIC-002]
notes: null
```

### DEP-006 — Pure-Rust stable wasm32 build of the core

```yaml
id: DEP-006
title: "Physics core builds on stable Rust for wasm32-unknown-unknown with no native toolchain"
status: draft
criticality: medium
verification: test
statement: >-
  The physics core crate (ferroastro-core or equivalent) SHALL build on stable
  Rust for wasm32-unknown-unknown with no C, C++ or Fortran toolchain and no
  system libraries.
acceptance: >-
  A CI job builds the core crate for wasm32-unknown-unknown on the pinned
  stable toolchain in an image without C/C++/Fortran compilers.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-006.
parent: null
related: [DEP-007, DEP-016, STD-005]
notes: >-
  WASM builds of faer and anise are UNVERIFIED.
```

### DEP-007 — Core components generic over `Real`

```yaml
id: DEP-007
title: "Integrator, force model, ephemeris evaluator and time arithmetic generic over Real"
status: draft
criticality: high
verification: test
statement: >-
  The integrator, force model, ephemeris evaluator and time arithmetic SHALL
  be generic over a crate-owned `Real` scalar trait, with implementations for
  f64 and a double-double type. A nightly-only f128 implementation SHALL sit
  behind a non-default feature.
acceptance: >-
  Each of the four components compiles and passes its tests with f64 and with
  the DD type; the default feature set builds on stable Rust.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 3
  and DEP-007.
parent: null
related: [NUM-006, DEP-011, DEP-012, API-001]
notes: >-
  NUM-006 is AUTHORITATIVE for the numerical contract (backends, FMA TwoProd,
  zero-cost f64, no x87). DEP-007 fixes which components must be generic. The
  DD backend (qd, twofloat or own ~500 LOC, estimate) is chosen by benchmark
  and accuracy audit; twofloat's transcendentals are "very preliminary".
  UNVERIFIED: whether a DD type can implement simba::RealField cheaply enough
  to use nalgebra in the hot loop; nightly f128 math functions on wasm32 and
  x86_64.
```

### DEP-008 — Clean-room integrators

```yaml
id: DEP-008
title: "IAS15/Gauss-Radau and symplectic integrators are clean-room from the papers"
status: draft
criticality: high
verification: inspection
statement: >-
  The IAS15/Gauss-Radau and symplectic integrators SHALL be clean-room
  implementations from the published papers. Contributors SHALL NOT consult
  GPL REBOUND/ASSIST source, and the PR template SHALL say so. A human or agent
  session that has read GPL source (e.g. the Journal #3 research session that
  read Swiftest's swiftest_gr.f90) SHALL NOT implement GR force terms or
  integrators; that work goes to a session with no GPL source in its context.
acceptance: >-
  The PR template contains the clean-room attestation; integrator source files
  cite only the published papers.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 2
  and DEP-008.
parent: null
related: [LIC-001, DEP-003, NUM-001, NUM-004]
notes: >-
  No permissive, maintained IAS15 exists in Rust as of 2026-10-14. The
  session rule was added 2026-10-14 after the red-team review (L11, Journal #9);
  no GPL code is quoted in docs/.
```

### DEP-009 — ERFA-derived routines

```yaml
id: DEP-009
title: "SOFA-equivalent routines ported from ERFA (BSD-3), no iau prefix, ERFA test values"
status: draft
criticality: high
verification: test
statement: >-
  SOFA-equivalent routines SHALL be ported from ERFA (BSD-3-Clause). Each
  ported file SHALL carry an SPDX license-identifier tag of
  `Apache-2.0 AND BSD-3-Clause`, the ERFA copyright line (NumFOCUS Foundation, copied from the
  ERFA LICENSE at the pinned version) and the BSD-3-Clause conditions and
  disclaimer; NOTICE SHALL also carry the ERFA notice. LICENSES/BSD-3-Clause.txt
  SHALL be added before the first ported file lands. Per BSD-3 clause 3, the
  SOFA Board, IAU and ERFA contributor names SHALL NOT be used to endorse or
  promote FerroAstro (factual attribution only). Function names SHALL NOT use
  the SOFA `iau` prefix. Each ported routine SHALL have a test reproducing
  ERFA's reference values.
acceptance: >-
  Every ported file carries the SPDX expression, ERFA copyright and BSD-3
  disclaimer; LICENSES/BSD-3-Clause.txt exists and `reuse lint` passes; NOTICE
  contains the ERFA BSD-3 notice; no public symbol starts with `iau`; every
  ported routine has a test citing its ERFA reference values.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 5
  and DEP-009.
parent: null
related: [LIC-004, REF-009, DEP-010]
notes: >-
  OVERLAP with LIC-004: DEP-009 is AUTHORITATIVE (more specific: NOTICE, naming,
  test vectors); LIC-004 states the same ERFA-not-SOFA policy at license level.
  sofars is a test oracle only until its SOFA-terms status is cleared
  (UNVERIFIED). Per-file notice requirement added 2026-10-14 (red-team L8,
  Journal #9): BSD-3 clause 1 needs the notice, conditions and disclaimer in
  source redistributions, so NOTICE alone is not enough.
  BUILD EVIDENCE (sync 2026-10-20; WP8, WP9 on m3-newtonian-nbody). Source:
  ERFA 2.0.1 release tarball (sha256 45111f0a…8c20e4, tag v2.0.1, the version
  pyerfa 2.0.1.5 bundles, SOFA 20231011); no SOFA file was opened or
  downloaded. Crate ferroastro-erfa (licence "Apache-2.0 AND BSD-3-Clause",
  no_std, generic over Real). Naming: public functions keep ERFA's names
  without ERFA's own era prefix (e.g. taitt, not eraTaitt) and never use
  SOFA's iau prefix; crates/ferroastro-erfa/tests/dep009_attribution.rs::no_public_symbol_uses_the_sofa_iau_prefix scans the whole workspace (it
  caught Pole::iau_sun, renamed sun_wgccre2015 in WP9). Ported routines,
  WP8 (time): cal2jd, jd2cal, taitt, tttai, dtdb (full 787-term
  Fairhead–Bretagnon series plus topocentric terms, the table copied
  mechanically by tools/erfa_dtdb_table.py), tttdb, tdbtt, tdbtcb, tcbtdb,
  tttcg, tcgtt and the erfam.h constants they use. WP9 (precession and
  frames): obl06, p06e, pfw06, fw2m, pmat06, ecm06, eqec06, eceq06, and the
  helpers ir, rx, rz, rxr, tr, rxp, trxp, s2c, c2s, anp, anpm. Each ported
  file carries the SPDX expression, the ERFA copyright (2013-2023 NumFOCUS
  Foundation), its source routine and the ERFA notice, conditions and
  disclaimer verbatim; NOTICE lists the routines; LICENSES/BSD-3-Clause.txt
  exists; reuse lint passes. Reference values: 23 (WP8) + 110 (WP9) ERFA
  t_erfa_c.c vectors (ERFA-T-*), passed in f64 and double-double with ERFA's
  own dval (dep009_t_erfa_c.rs, dep009_t_erfa_c_precession.rs), plus the
  split-order test dep009_split_order.rs added by the WP8 review.
```

### DEP-010 — Time-scale coverage and cross-validation

```yaml
id: DEP-010
title: "TAI, TT, TDB, TCB, TCG and UTC, cross-validated against hifitime and ERFA"
status: draft
criticality: high
verification: test
statement: >-
  Time-scale support SHALL cover TAI, TT, TDB, TCB, TCG and UTC with leap
  seconds. Conversions SHALL be cross-validated against hifitime and an
  ERFA-derived reference to a stated tolerance, at both f64 and DD precision.
acceptance: >-
  Round-trip and cross-library tests pass at the documented tolerance for every
  scale pair, in both f64 and DD. Documented tolerances: against the
  ERFA-derived reference (pyerfa 2.0.1.5 goldens) 1 ns per date for
  TT↔TAI/TDB, TDB↔TCB and TT↔TCG (PROVISIONAL); round trips A→B→A for all
  10 pairs of {TAI, TT, TDB, TCB, TCG} 2e-10 s in f64 and 1e-20 s in DD;
  TAI−UTC exact (0 s) against ERFA dat and against the IERS and IANA
  leap-second files. The hifitime TDB difference is a bound, not an
  agreement: |hifitime TDB − ERFA TDB| ≤ 4.0e-4 s over 1550–2650.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 4
  and DEP-010.
parent: null
related: [REF-009, DEP-009, API-001]
notes: >-
  The tolerance is not specified in the journals and SHALL be set in the test
  vector with a rationale (TST-002). hifitime's ET is SPICE's approximation,
  slightly different from true TDB. TOLERANCES STATED 2026-10-20
  (build-evidence sync; WP8 item 10). They come from the vectors in
  tests/vectors/time-scales.toml on m3-newtonian-nbody, each pre-registered
  before its first run (WP8 plan), and loosen nothing. Measured: pyerfa f64
  bitwise except TT→TDB at 9.6e-12 s (1 ulp of the day fraction, from the
  exact-inverse definition), DD ≤ 8.9e-12 s; round trips f64 9.6e-12 s, DD
  1.8e-27 s; TAI−UTC 0 s at 219 instants and 55 IERS/IANA instants
  (crates/ferroastro-time/tests/dep010_round_trips.rs, dep010_hifitime.rs,
  ref009_pyerfa_goldens.rs, data_leap_seconds.rs). The 1 ns band is
  PROVISIONAL and NEEDS HUMAN REVIEW (WP8 item 1). hifitime TDB: 1.957e-4 s
  at 2600 (3.9e-5 s over 1950–2050) against an analytic bound of 4.0e-4 s
  written before running hifitime (vector HIFITIME-TDB-VS-ERFA-MAX-DIFF):
  hifitime uses the one-term SPICE model, whose frequency differs from
  ERFA's. UTC: hifitime 4.3.1 (MPL-2.0, optional feature hifitime) applies
  each leap second seconds early (upstream #417, #255; 27 of 219 instants
  wrong), so the adapter uses only hifitime's leap-second table, leap-free
  calendar and ns instant and does the boundary lookup itself; whether this
  reading of ADR-0003 is acceptable is NEEDS HUMAN REVIEW (WP8 item 4). The
  hifitime cross-check runs on hifitime's integer-nanosecond grid whichever
  backend is used (WP8 item 6). TDB→TT is the exact inverse of ERFA's TT→TDB
  (3 fixed-point iterations); it differs from pyerfa's one-shot use by
  ≈ 6e-13 s (WP8 item 3).
```

### DEP-011 — In-crate SPK type-2 reader

```yaml
id: DEP-011
title: "In-crate JPL DE440/DE441 SPK type-2 reader, generic over Real, cross-validated against ANISE"
status: draft
criticality: high
verification: test
statement: >-
  JPL DE440/DE441 SPK type-2 reading SHALL be implemented in-crate and generic
  over `Real`. In CI it SHALL be cross-validated against ANISE (if adopted at
  M5, OI-72; optionally satkit) to within a documented tolerance.
acceptance: >-
  REF-004's jplephem comparison (≤ 1 mm / 1 µm s⁻¹ target) passes. If anise
  is adopted at M5 (OI-72), the CI comparison against ANISE passes at the
  documented tolerance. Until that decision, the ANISE comparison is replaced
  by three independent checks: every Chebyshev coefficient of the DE440 ASCII
  products (ascpNNNNN.440) equals the SPK coefficient (0 mismatches), the
  geocentric Moon from the SPK matches the ASCII series (≤ 1e-8 km,
  ≤ 1e-12 km/s), and JPL Horizons DE441 barycentric states agree with the
  DE440 chain within a gross bound (10 km, 1e-3 km/s) that catches frame,
  centre and time-scale errors.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Audit table
  (Ephemerides) and DEP-011.
parent: null
related: [REF-004, DEP-007]
notes: >-
  ANISE evaluates in f64, so the extended-precision path needs this reader.
  anise is not a dependency; whether to adopt it is deferred to M5 (OI-72).
  ACCEPTANCE CHANGED 2026-10-20 (build-evidence sync). Reason: the earlier
  acceptance required an ANISE comparison that the M5 deferral (OI-72; no
  anise dependency of any kind, not even the Python package) makes
  impossible before M5, so it could never pass in M3/M4. The ANISE clause is
  kept, conditional on adoption; the substitute checks are not weaker for
  correctness: the ASCII check is exact. Evidence (WP10, m3-newtonian-nbody):
  crates/ferroastro-ephem/tests/ref004_jplephem_de440.rs is bit-identical
  to jplephem 2.24 (0 km, 0 km/s) at 784 epochs over all 14 de440 segments,
  because the Clenshaw order and the three-way divmod of the two-part date
  copy jplephem's (the asserted tolerance stays REF-004's 1 mm / 1 µm/s;
  bit-identity is an observation, not a property; WP10 deviation 5);
  data_de440_products.rs::data_de440_ascii_coefficients_equal_the_spk finds 0
  mismatches in 1 727 208 coefficients of ascp01550/01950/02550 and a
  geocentric Moon difference of 1.746e-10 km and 8.88e-16 km/s;
  data_horizons_de441_barycentric_states_vs_de440_gross_bound gives Mercury
  1.49e-8 km, Earth 5.14e-3 km, Moon 0.418 km (1600) against 10 km;
  data_sb441_asteroids_match_jplephem is bit-identical on a second SPK file.
  The Horizons bound is weak by design (DE441 is not DE440) and does not
  catch TT vs TDB (≤ 80 m); the testpo check (REF-001) covers the time
  argument. Generic over Real: f64 and double-double evaluations agree to
  ρ = 0.428 (golden epochs) and 0.650 (every record, nightly) of the
  Clenshaw round-off bound n·u·Σ|c_k| (a FerroAstro engineering bound, ρ ≤ 2;
  WP10 deviation 6). Type 3 and big-endian files are tested on synthetic SPKs
  against jplephem. NAIF frame 1 is mapped to ICRF, an interpretation to keep
  in mind at frame-bias level (WP10 deviation 8).
```

### DEP-012 — Extended precision validated against an arbitrary-precision oracle

```yaml
id: DEP-012
title: "Extended-precision arithmetic property-tested against astro-float with ulp bounds"
status: draft
criticality: high
verification: test
statement: >-
  Extended-precision arithmetic SHALL be validated against an
  arbitrary-precision oracle (astro-float, pure Rust, MIT) in proptest
  property tests covering +, ×, ÷, sqrt and every transcendental the force
  model uses, with documented ulp error bounds.
acceptance: >-
  Property tests pass for every listed operation at its documented ulp bound.
  Bounds for double-word (double-double) results are stated in units of
  u² = 2⁻¹⁰⁶, where u = 2⁻⁵³ is the binary64 unit roundoff, relative unless
  marked absolute; f64 bounds are in ulp of binary64.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-012.
parent: null
related: [NUM-006, DEP-007]
notes: >-
  astro-float being too slow for production is assumed, not benchmarked.
  UNIT NAMED 2026-10-20 (build-evidence sync; WP7 item 10). It records the
  unit the vectors already use and loosens nothing. Evidence
  (m3-newtonian-nbody, crates/ferroastro-real/tests/dep012_ulp_bounds.rs,
  vectors in tests/vectors/real-backends.toml, all 26 bounds pre-registered,
  oracle astro-float at 384 bits): DD add/sub ≤ 1.43 u² (bound 3, JMP 2017
  Thm 3.1), mul 3.06 (5), div 3.99 (9.8), sqrt 2.41 (3.125, Lefèvre et al.
  2023); elementary functions 128 u² (powf 256) are FerroAstro engineering
  bounds, labelled as such; f64 ops ≤ 0.5 ulp (IEEE 754). The
  REAL-DD-EXP-REL-ERR test domain was amended to [−600, 700] after a first-run failure caused by
  the subnormal trailing word (bound unchanged; NEEDS HUMAN REVIEW, WP7
  item 3).
```

### DEP-013 — MCP server on rmcp

```yaml
id: DEP-013
title: "MCP server uses rmcp pinned to a 3.x minor and declares its spec revision"
status: draft
criticality: medium
verification: inspection
statement: >-
  The MCP server SHALL use the official rmcp SDK pinned to a 3.x minor
  version and SHALL declare the MCP spec revision it targets (currently
  2026-07-28).
acceptance: >-
  Cargo.toml pins rmcp to a 3.x minor; the server's declared protocol
  revision is 2026-07-28 (or a later revision recorded in an ADR).
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Audit table (Interfaces)
  and DEP-013.
parent: null
related: [AGT-004]
notes: >-
  rmcp is Apache-2.0 on crates.io but GitHub reports NOASSERTION; the LICENSE
  file must be read (DEP-015). DEP-013 is authoritative for the spec revision
  pin; AGT-004 sets the floor (2025-06-18 or later) for structured output.
```

### DEP-014 — Tooling baseline

```yaml
id: DEP-014
title: "Tooling baseline: nextest, criterion, proptest, approx, cargo-deny"
status: draft
criticality: medium
verification: test
statement: >-
  The project tooling baseline SHALL be cargo-nextest (tests), criterion
  (benchmarks with tracked baselines), proptest (invariants), approx (float
  assertions) and cargo-deny (licenses, advisories, sources). These SHALL run in
  CI on every PR.
acceptance: >-
  The PR workflow runs nextest (with proptest and approx-based tests) and
  cargo-deny on every PR. Criterion benchmarks build on every PR and are
  measured under CI-008 conditions.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-014.
parent: null
related: [STD-006, STD-008, CI-008, NUM-024]
notes: >-
  CONFLICT resolved: DEP-014 says all five run on every PR, but CI-008
  (Journal #3, refined by Journal #6) and Journal #5 bench.yml put benchmark
  measurement on a fixed larger-runner class, compared only within the same
  run. Resolution here: criterion is compiled on every PR and measured only
  under CI-008. Maintainer confirmation pending.
```

### DEP-015 — Provenance check for new dependencies

```yaml
id: DEP-015
title: "New dependencies need a recorded license, maintenance and WASM provenance check"
status: draft
criticality: high
verification: inspection
statement: >-
  Any new dependency SHALL have a provenance check recorded in the PR: license
  file read (not only the crates.io SPDX field), maintenance status (last
  release under 24 months or a justification) and WASM compatibility. Archived
  crates (e.g. erfars) and crates with unresolved provenance (e.g.
  thor-rs-assist) SHALL be rejected. For any crate whose LICENSE file carries
  terms beyond its declared SPDX expression (e.g. sofars' SOFA Board text),
  deny.toml SHALL carry a [[licenses.clarify]] entry pinned to that LICENSE
  file's hash, so a changed file fails `cargo deny check` and forces review.
acceptance: >-
  The PR template has the provenance checklist, and every PR that changes
  Cargo.lock dependencies has it completed. Every dependency whose LICENSE file
  adds terms to its declared license has a hash-pinned [[licenses.clarify]]
  entry in deny.toml.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-015.
parent: null
related: [DEP-003, STD-006, DEP-001]
notes: >-
  cargo-deny ignores LICENSE files when Cargo.toml declares a license
  (confirmed with cargo-deny 0.19.8: sofars passed as plain MIT, its SOFA text
  invisible to the gate; red-team L5, Journal #9). Hence the hash-pinned
  clarify rule.
```

### DEP-016 — No native-library features

```yaml
id: DEP-016
title: "Optional native-library features (SuiteSparse, LAPACK) are never enabled"
status: draft
criticality: high
verification: test
statement: >-
  Optional native-library features of otherwise-permissive crates (e.g.
  diffsol's suitesparse_sys, LAPACK backends) SHALL NOT be enabled anywhere in
  the workspace. cargo-deny [bans] feature checks or a CI
  `cargo tree -e features` audit SHALL enforce this.
acceptance: >-
  The CI feature audit passes on main and fails on a canary PR enabling
  suitesparse_sys.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md License risks (native
  toolchain contamination) and DEP-016.
parent: DEP-001
related: [DEP-006]
notes: >-
  UMFPACK being GPL-licensed is "per our knowledge", UNVERIFIED for the
  version russell_sparse links.
```

### DEP-017 — JPL ephemeris data not bundled in releases

```yaml
id: DEP-017
title: "JPL ephemeris data not bundled in released artifacts until redistribution terms are confirmed"
status: draft
criticality: high
verification: inspection
statement: >-
  JPL ephemeris data SHALL NOT be bundled in released artifacts until its
  redistribution terms are confirmed. The default SHALL be fetch-on-demand with
  pinned SHA-256 checksums.
acceptance: >-
  No released artifact (crate, wheel, WASM bundle, image) contains a .bsp or
  DE ASCII file; the fetcher verifies a pinned SHA-256 before use.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md License risks (data) and
  DEP-017.
parent: null
related: [LIC-003, REF-001]
notes: >-
  OVERLAP: DEP-017 is AUTHORITATIVE for released artifacts; LIC-003 for
  repository contents; REF-001 for which files are ground truth. JPL DE
  redistribution terms are UNVERIFIED, and so are IERS EOP terms (Journal #5
  open question 5). Once confirmed, a truncated, explicitly attributed segment
  may ship in the WASM demo.
```
