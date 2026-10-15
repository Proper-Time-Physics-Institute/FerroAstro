<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# REF — Reference implementations and oracles

**Scope.** These requirements choose the external reference codes ("oracles") and ground-truth data that CI uses to check FerroAstro's accuracy on the Mercury ladder, and fix the rules for running them. The scenario ladder is: A Newtonian, B Mercury in the DE440 field, C coupled 1PN, D full DE440 reproduction. JPL DE440 is ground truth. REBOUND/REBOUNDx is the primary coupled N-body and 1PN-EIH oracle. Orekit covers single-body propagation in an ephemeris-driven field. Swiftest and Mercury6 are Fortran-ecosystem cross-checks, and ERFA covers time and frames. No external code exposes PPN β/γ, so those rungs are validated analytically (REF-010). Source: Journal #3 (`docs/strategy/journal/03-reference-implementations-ci.md`), with versions checked on 2026-10-14. How oracles run in CI is in [CI.md](CI.md). License isolation of GPL oracles is in [LIC.md](LIC.md).

---

### REF-001 — DE440 is the accuracy ground truth

```yaml
id: REF-001
title: "JPL DE440 (de440.bsp, header.440, testpo.440) is the accuracy ground truth, fetched and pinned"
status: draft
criticality: high
verification: test
statement: >-
  CI SHALL use JPL DE440 (de440.bsp, ASCII header.440 and testpo.440) as the
  accuracy ground truth. Each file SHALL be fetched by URL and pinned by
  SHA-256, and SHALL NOT be committed to git.
acceptance: >-
  The jpl oracle image fetches the three files from
  https://ssd.jpl.nasa.gov/ftp/eph/planets/ and verifies a pinned SHA-256 for
  each; a repository scan finds none of them in git.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.4 and REF-001;
  Park et al. 2021, AJ 161, 105.
parent: null
related: [LIC-003, DEP-017, PHY-014, CI-007]
notes: >-
  OVERLAP: REF-001 is AUTHORITATIVE for which files are ground truth and how CI
  obtains them; LIC-003 is authoritative for the prohibition on committing them
  (and also covers SOFA and NOVAS sources); DEP-017 for released artifacts. File
  sizes on 2026-10-14: de440.bsp 119,799,808 B; header.440 22.3 K; testpo.440
  838 K. Redistribution terms are UNVERIFIED.
```

### REF-002 — Initial conditions and constants loaded from header.440

```yaml
id: REF-002
title: "Initial conditions and constants loaded directly from header.440, never hand-transcribed"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL load initial conditions and constants (JDEPOC,
  planet/Moon/Sun states, GM*, MA*, J2SUN, ASUN, CLIGHT, AU, EMRAT, BETA,
  GAMMA) directly from header.440 GROUP 1040/1041. They SHALL NOT be
  transcribed by hand.
acceptance: >-
  A test parses header.440 (pinned SHA-256) and asserts JDEPOC = 2440400.5,
  CLIGHT = 299792.458, BETA = 1.0, GAMMA = 1.0, AU = 149597870.7,
  J2SUN = 2.1961391516529825e-7, ASUN = 696000, and 409 MA entries. No
  constants or state literals are hand-entered in source.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.4 and REF-002.
parent: null
related: [PHY-002, TST-002, TST-003]
notes: >-
  AUTHORITATIVE over PHY-002 for the loading mechanism; PHY-002's versioned
  constants table SHALL be generated from header.440. UNVERIFIED: that
  CENTER = 0 means barycentric ICRF; the meaning of the XC block; the au and
  au/day units of the states (inferred from X1 = 0.36176…). 645 constants in
  the header.
```

### REF-003 — Asteroid and KBO initial states from sb441-n373

```yaml
id: REF-003
title: "Asteroid/KBO initial states sampled from sb441-n373*.bsp at JDEPOC (provisional)"
status: draft
criticality: medium
verification: test
statement: >-
  Asteroid and KBO initial states SHALL be sampled from sb441-n373*.bsp at
  JDEPOC, with provenance recorded. This requirement is provisional until the
  open question on how JPL supplies asteroid initial conditions is closed.
acceptance: >-
  The sampled initial-state file records the source file name, its SHA-256 and
  JDEPOC, and covers every MA entry with a state.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.4, §5 item 2 and
  REF-003.
parent: null
related: [PHY-014, CI-003]
notes: >-
  BLOCKED: how JPL supplied asteroid ICs, the contents of sb441-n373.bsp, and
  that n373 = 343 asteroids + 30 KBOs are all UNVERIFIED; IOM 392R-21-005 must
  be read. The 36-point KBO ring masses (MA8201–MA8236) have no state in
  sb441-n373.
```

### REF-004 — jplephem as reference SPK reader

```yaml
id: REF-004
title: "jplephem 2.24 is the reference .bsp reader; FerroAstro's reader matches it to 1 mm / 1 µm/s"
status: draft
criticality: high
verification: test
statement: >-
  jplephem==2.24 (MIT) SHALL be the reference reader for generating golden
  states from .bsp files. FerroAstro's own SPK reader SHALL match it.
acceptance: >-
  Target: position difference ≤ 1 mm and velocity difference ≤ 1 µm s⁻¹ at
  every golden epoch.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.4 and REF-004.
parent: null
related: [DEP-011, CI-005]
notes: >-
  The 1 mm / 1 µm s⁻¹ figure is labelled a target in the journal.
```

### REF-005 — REBOUND + REBOUNDx as primary N-body / 1PN oracle

```yaml
id: REF-005
title: "REBOUND 5.2.2 + REBOUNDx 5.1.0 are the primary coupled N-body and 1PN-EIH oracle"
status: draft
criticality: high
verification: test
statement: >-
  REBOUND 5.2.2 with REBOUNDx 5.1.0 (gr_full, gr, gr_potential,
  gravitational_harmonics) SHALL be the primary coupled N-body and 1PN-EIH
  oracle.
acceptance: >-
  The rebound oracle image pins rebound==5.2.2 and reboundx==5.1.0, and
  scenario stages A and C produce goldens with MANIFEST.json entries.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.3 and REF-005.
parent: null
related: [LIC-001, LIC-002, CI-001, CI-003]
notes: >-
  GPL-3.0: separate container only, files-only interface (LIC-001). gr_full has
  only parameter c (β = γ = 1 fixed). Whether REBOUND/REBOUNDx publish
  manylinux wheels or need a compiler in the image is UNVERIFIED.
```

### REF-006 — Orekit as single-body Apache-2.0 oracle

```yaml
id: REF-006
title: "Orekit 13.1.9 / Hipparchus 4.0.3 are the single-body, ephemeris-driven oracle"
status: draft
criticality: medium
verification: test
statement: >-
  Orekit 13.1.9 with Hipparchus 4.0.3 SHALL be the Apache-2.0 oracle for
  single-body propagation in an ephemeris-driven field (Relativity,
  J2OnlyPerturbation, ThirdBodyAttraction / SingleBodyAbsoluteAttraction).
acceptance: >-
  The orekit oracle image pins orekit 13.1.9, hipparchus 4.0.3 and orekit-data
  at a git commit, and scenario stage B produces a golden with a MANIFEST.json
  entry.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.1 and REF-006.
parent: null
related: [CI-001, REF-011]
notes: >-
  Orekit Relativity is single-central-body Schwarzschild with β = γ = 1 and no
  planet–planet 1PN terms. UNVERIFIED: the first release containing
  J2OnlyPerturbation; whether LenseThirringRelativity can be re-pointed at the
  Sun; absence of a coupled multi-body propagator; exact JPL binary file path
  (Linux/de440/linux_p1550p2650.440); conda-forge status of the JCC wrapper.
  Orekit reads JPL binary DE files, not .bsp.
```

### REF-007 — Swiftest as symplectic Fortran oracle

```yaml
id: REF-007
title: "Swiftest 2026.7.1 is the symplectic Fortran-ecosystem oracle"
status: draft
criticality: medium
verification: test
statement: >-
  Swiftest 2026.7.1 SHALL be the symplectic Fortran-ecosystem oracle (WHM/HELIO
  with general_relativity).
acceptance: >-
  The swiftest oracle image pins swiftest==2026.7.1 and produces goldens for
  scenario stages A and C-lite.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.2 and REF-007;
  Wishard et al. 2023, JOSS, doi:10.21105/joss.05409.
parent: null
related: [LIC-001, LIC-002, CI-001]
notes: >-
  GPL-3.0-or-later; isolated container. general_relativity defaults to True
  and is central-body 1PN, not EIH. Whether a plain J2 input (j2rp2) is still
  supported is UNVERIFIED.
```

### REF-008 — Mercury6 as Newtonian-only legacy oracle

```yaml
id: REF-008
title: "Mercury6 (pinned commit) is a Newtonian-only legacy oracle; its GR is never used"
status: draft
criticality: low
verification: inspection
statement: >-
  Mercury6, at a pinned commit, MAY serve as a legacy Newtonian oracle (RA15/BS)
  only. GR results from it SHALL NOT be used, because GR would come from our
  own mfo_user code.
acceptance: >-
  No scenario with a GR force switch is routed to the Mercury6 runner (its
  runner refuses such scenarios per REF-011).
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.2 and REF-008;
  Chambers 1999, MNRAS 304, 793.
parent: null
related: [REF-011, LIC-001]
notes: >-
  The original Chambers license is UNVERIFIED (the smirik/mercury mirror is
  labelled GPL-3.0); treated as GPL and never vendored.
```

### REF-009 — ERFA as time and frame oracle

```yaml
id: REF-009
title: "ERFA / pyerfa 2.0.1.5 (SOFA 20231011) is the time-scale and frame oracle"
status: draft
criticality: high
verification: test
statement: >-
  ERFA via pyerfa 2.0.1.5 (matching SOFA 20231011) SHALL be the oracle for
  time-scale and frame transformations.
acceptance: >-
  The erfa oracle image pins pyerfa==2.0.1.5 and produces TT↔TDB,
  ICRS↔ecliptic and J2000 rotation goldens used by PR-fast CI.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.2 and REF-009.
parent: null
related: [DEP-009, DEP-010, LIC-004]
notes: null
```

### REF-010 — Analytic validation where no oracle exists

```yaml
id: REF-010
title: "PPN β/γ ≠ 1 and solar Lense–Thirring are validated analytically"
status: draft
criticality: high
verification: test
statement: >-
  PPN β/γ ≠ 1 and solar Lense–Thirring have no external N-body oracle. They
  SHALL be validated against analytic secular-precession formulas and against
  Park et al. 2021's LT value for Mercury.
acceptance: >-
  β/γ: rate matches (2+2γ−β)/3 × 42.98 ″/cy (PHY-006 tolerances). LT: Mercury
  rate ≈ −0.0020 ″/cy (PHY-011 tolerance ±0.0002).
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 scenario C and
  REF-010; Park et al. 2021.
parent: ORG-002
related: [PHY-006, PHY-011, NUM-015]
notes: >-
  PHY-006 and PHY-011 are authoritative for the tolerances.
```

### REF-011 — Oracle runners refuse unsupported forces

```yaml
id: REF-011
title: "Every oracle runner refuses a scenario requesting a force it does not implement"
status: draft
criticality: high
verification: test
statement: >-
  Every oracle runner SHALL refuse a scenario that requests a force it does not
  implement, and SHALL never silently omit the force.
acceptance: >-
  For each runner, a test feeds a scenario with an unsupported switch (e.g.
  ppn{beta,gamma} to REBOUNDx, gr_eih to Mercury6) and asserts a non-zero exit
  with a message naming the switch.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 principles and
  REF-011.
parent: null
related: [CI-002, REF-008]
notes: null
```
