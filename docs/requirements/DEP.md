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
  LICENSE carries extra SOFA text).
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
  MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0 / Unicode-DFS-2016,
  CC0-1.0 and MPL-2.0. MPL-2.0 SHALL be allowed only for crates named in a
  reviewed exception list, initially `hifitime` and `anise`.
acceptance: >-
  deny.toml contains exactly this allow-list and an MPL-2.0 exception list of
  {hifitime, anise}. A canary MPL-2.0 crate not on the exception list fails
  `cargo deny check`.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-002.
parent: DEP-001
related: [STD-006, DEP-004, DEP-009]
notes: >-
  Journal #5 §3.4 shows a shorter illustrative allow-list ending in "..."; this
  list is authoritative. anise depends on sofars, whose LICENSE carries the
  SOFA Board header; whether SOFA terms bind sofars and hence anise users is
  UNVERIFIED and needs a legal read before the anise exception is approved.
  lox-* and erfa are MPL-2.0 but not on the initial exception list.
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
  libassist-sys, adam_assist, rebound-rs, and thor-rs-assist (the last until
  DEP-015 clears its provenance).
acceptance: >-
  deny.toml [bans] lists all nine names. A canary PR adding any one fails CI.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Audit table, License
  risks and DEP-003.
parent: DEP-001
related: [LIC-001, DEP-008, DEP-015]
notes: >-
  nyx-space is AGPL-3.0 and would impose source-offer obligations on the
  network-served MCP server and web demo. Whether thor-rs-assist derives from
  GPL REBOUND code is UNVERIFIED. rug may appear only as an opt-in, out-of-tree
  dev oracle that is never shipped.
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
  GPL REBOUND/ASSIST source, and the PR template SHALL say so.
acceptance: >-
  The PR template contains the clean-room attestation; integrator source files
  cite only the published papers.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 2
  and DEP-008.
parent: null
related: [LIC-001, DEP-003, NUM-001, NUM-004]
notes: >-
  No permissive, maintained IAS15 exists in Rust as of 2026-10-14.
```

### DEP-009 — ERFA-derived routines

```yaml
id: DEP-009
title: "SOFA-equivalent routines ported from ERFA (BSD-3), no iau prefix, ERFA test values"
status: draft
criticality: high
verification: test
statement: >-
  SOFA-equivalent routines SHALL be ported from ERFA (BSD-3-Clause) with its
  copyright notice kept in NOTICE. Function names SHALL NOT use the SOFA `iau`
  prefix. Each ported routine SHALL have a test reproducing ERFA's reference
  values.
acceptance: >-
  NOTICE contains the ERFA BSD-3 notice; no public symbol starts with `iau`;
  every ported routine has a test citing its ERFA reference values.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 5
  and DEP-009.
parent: null
related: [LIC-004, REF-009, DEP-010]
notes: >-
  OVERLAP with LIC-004: DEP-009 is AUTHORITATIVE (more specific: NOTICE, naming,
  test vectors); LIC-004 states the same ERFA-not-SOFA policy at license level.
  sofars is a test oracle only until its SOFA-terms status is cleared
  (UNVERIFIED).
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
  scale pair, in both f64 and DD.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Build vs. reuse item 4
  and DEP-010.
parent: null
related: [REF-009, DEP-009, API-001]
notes: >-
  The tolerance is not specified in the journals and SHALL be set in the test
  vector with a rationale (TST-002). hifitime's ET is SPICE's approximation,
  slightly different from true TDB.
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
  over `Real`. In CI it SHALL be cross-validated against ANISE (and optionally
  satkit) to within a documented tolerance.
acceptance: >-
  CI comparison against ANISE passes at the documented tolerance; REF-004's
  jplephem comparison (≤ 1 mm / 1 µm s⁻¹ target) also passes.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md Audit table
  (Ephemerides) and DEP-011.
parent: null
related: [REF-004, DEP-007]
notes: >-
  ANISE evaluates in f64, so the extended-precision path needs this reader.
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
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-012.
parent: null
related: [NUM-006, DEP-007]
notes: >-
  astro-float being too slow for production is assumed, not benchmarked.
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
  thor-rs-assist) SHALL be rejected.
acceptance: >-
  The PR template has the provenance checklist, and every PR that changes
  Cargo.lock dependencies has it completed.
source: >-
  docs/strategy/journal/02-rust-ecosystem-licenses.md DEP-015.
parent: null
related: [DEP-003, STD-006]
notes: null
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
