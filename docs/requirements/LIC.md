<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# LIC — License isolation of oracles and third-party sources

**Scope.** FerroAstro is Apache-2.0, © Proper-Time Physics Institute. These requirements keep GPL reference codes, licence-restricted data and non-OSI sources out of FerroAstro's code and release artifacts. GPL tools may still serve as separate CI oracles, talking to FerroAstro only through files and process boundaries. Source: Journal #3 §3 (`docs/strategy/journal/03-reference-implementations-ci.md`). *This is engineering guidance, not legal advice.* Dependency-graph licence policy is in [DEP.md](DEP.md). Contributor sign-off and the CLA are in [ORG.md](ORG.md) (ORG-008, ORG-010).

---

### LIC-001 — No GPL code in FerroAstro

```yaml
id: LIC-001
title: "GPL oracles are never linked, bound, compiled by build.rs or vendored"
status: draft
criticality: high
verification: inspection
statement: >-
  No GPL code (REBOUND, REBOUNDx, ASSIST, Swiftest, Mercury6) SHALL be linked,
  bound via FFI, compiled by build.rs, or vendored into FerroAstro.
  Interaction SHALL be through plain files (CSV/NetCDF/JSON state vectors) and
  process boundaries only.
acceptance: >-
  cargo-deny bans (DEP-003) pass. A repository scan finds no GPL-licensed
  source file (e.g. gr_full.c, swiftest_gr.f90) anywhere in the repository,
  ci/oracles/ included: ci/oracles/ holds only FerroAstro's own Apache-2.0
  Dockerfiles and scripts, which download and install GPL software at image
  build time. Oracle runners exchange only files.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §3 items 1–2 and
  LIC-001; GNU GPL FAQ.
parent: null
related: [DEP-003, DEP-008, REF-005, REF-007, REF-008]
notes: >-
  Golden outputs produced by GPL tools may be committed (GPL FAQ: program
  output is not generally covered). The CI-only Dockerfiles and install
  scripts under ci/oracles/ are FerroAstro's own Apache-2.0 files; GPL source is
  never vendored there either. Mercury6's original license is UNVERIFIED and is
  treated as GPL. Python under tools/ SHALL NOT take GPL runtime dependencies
  (e.g. rebound); a pip-licenses guard in the tools/ CI job enforces this
  (STD-009).
```

### LIC-002 — GPL oracle images are not release artifacts

```yaml
id: LIC-002
title: "GPL oracle images are never published publicly or as FerroAstro release artifacts"
status: draft
criticality: high
verification: inspection
statement: >-
  GPL oracle images SHALL NOT be published publicly or as FerroAstro release
  artifacts. They SHALL be kept in a private GHCR registry or rebuilt (with
  caching) in CI (maintainer decision 2026-10-14). No combined FerroAstro +
  GPL-oracle image SHALL be built or released (LIC-005).
acceptance: >-
  The release workflow's artifact list contains no ci/oracles/ image; every
  GHCR oracle package has private visibility, or CI rebuilds the image from
  its cached layers instead of pulling a public one; oracle images are
  documented as CI-only.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §3 item 1 and
  LIC-002; docs/strategy/journal/09-red-team-review.md (L3).
parent: null
related: [CI-001, DEP-005, LIC-005]
notes: >-
  A public image would distribute compiled REBOUND/REBOUNDx/ASSIST/Swiftest
  and trigger GPLv3 §6 obligations (Corresponding Source or a written offer).
  It would also redistribute Mercury6, the SOFA tarball and de440.bsp, whose
  terms are UNVERIFIED (OI-18, OI-25). Engineering guidance, not legal
  advice.
```

### LIC-003 — No JPL, SOFA or NOVAS files in the repository

```yaml
id: LIC-003
title: "JPL ephemeris files, SOFA and NOVAS sources are not committed; CI fetches with SHA-256"
status: draft
criticality: high
verification: inspection
statement: >-
  JPL ephemeris files, SOFA sources and NOVAS sources SHALL NOT be committed to
  the repository. CI SHALL fetch them with SHA-256 verification.
acceptance: >-
  A repository scan finds no .bsp, DE ASCII, SOFA or NOVAS source file; each
  fetch step verifies a pinned SHA-256.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §3 items 5–6 and
  LIC-003.
parent: null
related: [REF-001, DEP-017]
notes: >-
  OVERLAP: LIC-003 is AUTHORITATIVE for repository contents; REF-001 for which
  DE440 files are ground truth; DEP-017 for released artifacts. JPL
  redistribution terms, NOVAS F3.1 license and the original Mercury6 license
  are UNVERIFIED.
```

### LIC-004 — Ports come from ERFA, not SOFA

```yaml
id: LIC-004
title: "Time/frame routines ported into Rust come from ERFA (BSD-3), not SOFA"
status: draft
criticality: high
verification: inspection
statement: >-
  Any routine ported into Rust from a time/frame library SHALL come from ERFA
  (BSD-3-Clause), not from SOFA. Each ported file SHALL carry
  an SPDX license-identifier tag of `Apache-2.0 AND BSD-3-Clause`, the ERFA (NumFOCUS
  Foundation) copyright and the BSD-3-Clause disclaimer; attribution SHALL also
  go in NOTICE, and LICENSES/BSD-3-Clause.txt SHALL exist before the first
  port lands. SOFA/IAU names SHALL NOT be used to promote FerroAstro (BSD-3
  clause 3).
acceptance: >-
  Each ported routine cites its ERFA source and its file carries the SPDX
  expression, ERFA copyright and disclaimer; NOTICE carries the ERFA BSD-3
  attribution; `reuse lint` passes with LICENSES/BSD-3-Clause.txt present.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §3 item 4 and
  LIC-004.
parent: null
related: [DEP-009, REF-009]
notes: >-
  OVERLAP with DEP-009, which is AUTHORITATIVE (adds no-iau-prefix naming and
  ERFA test-vector reproduction). SOFA Fortran remains usable as an oracle
  only. The SOFA licence (SPDX SOFA) requires derived works to be renamed.
  Per-file notice added 2026-10-14 (red-team L8, Journal #9).
```

### LIC-005 — GPL oracle containers never contain a FerroAstro build

```yaml
id: LIC-005
title: "A GPL oracle container never contains a FerroAstro build; exchange is files between separate jobs"
status: draft
criticality: high
verification: inspection
statement: >-
  No container image or CI job that runs GPL oracle code SHALL contain,
  install, import or execute a FerroAstro build (crate, binary or PyO3 wheel).
  GPL oracle runs and FerroAstro runs SHALL be separate jobs that exchange only
  files (scenario in; state vectors and goldens out) passed as workflow
  artifacts.
acceptance: >-
  Inspection of every ci/oracles/ Dockerfile and oracle workflow finds no
  FerroAstro artifact installed, mounted or imported in a GPL oracle image or
  job; each oracle job and each FerroAstro job is separate and passes data only
  as files.
source: >-
  docs/strategy/journal/09-red-team-review.md (L4);
  docs/strategy/journal/03-reference-implementations-ci.md §3; GNU GPL FAQ.
parent: null
related: [LIC-001, LIC-002, CI-001, CI-002, REF-005, REF-007]
notes: >-
  The process-boundary argument of LIC-001 fails if one runner imports both
  rebound and a FerroAstro PyO3 wheel: that makes a combined GPLv3 work.
  Engineering guidance, not legal advice.
```
