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
  BUILD EVIDENCE (sync 2026-10-20; branch m3-newtonian-nbody, WP10 and WP16):
  the products are fetched by tools/fetch_jpl.py (retries on network errors
  only, --verify-only checks the cache offline) and by the Rust
  ferroastro-testdata crate, both against the SHA-256 pins in
  tests/data/MANIFEST.toml, and each download gets a .provenance.toml sidecar.
  The MANIFEST pins were taken by an agent on first retrieval and await human
  confirmation (WP10 deviation 2). Pins: de440.bsp a4ce9bf9…fff7c4b5,
  testpo.440 45084a7f…6693429f, header.440 0f4636b6…6ac3359. The reader
  threshold is the one in JPL's testeph.f ("agreement is considered okay if
  DEL is less than 1e-13", au and au/day): crates/ferroastro-ephem/tests/ref001_testpo440.rs evaluates 11 354 body points (1 847 nutation/libration
  points skipped) with max DEL 7.105e-15 (WP10 check 2; an independent Python
  reader in the WP10 progress review got 1.42e-14). The vector check is ≤,
  testeph's is <; they differ only at exactly 1e-13 (WP10 deviation 7). The
  "no product in git" scan is tools/tests/test_fetch_jpl.py (WP10). The jpl
  oracle image (WP16, tools/oracles/jpl_runner.py) fetched all three into an
  empty cache and matched every pin (ci/golden/jpl/ref001-pins/,
  crates/ferroastro-oracle/tests/jpl_pins.rs).
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
  constants table SHALL be generated from header.440. 645 constants in the
  header. STATE FRAME AND UNITS (OI-23 closed 2026-10-20 by maintainer
  decision): with CENTER = 0 the header states are SSB-centred, ICRF-aligned
  (the DE frame, NAIF frame 1), positions in au and velocities in au/day
  (AU = header AU in km, day = 86 400 s TDB). XB is the Earth–Moon barycentre,
  XM is Moon − Earth (the DE geocentric Moon), XS is the Sun, and the suffixes
  1, 2, 4–9 are NAIF 1, 2, 4–9 (system barycentres). Evidence (WP11 check 1,
  branch m3-newtonian-nbody): crates/ferroastro-system/tests/ref002_header_initial_conditions.rs::ref002_header_initial_conditions_equal_de440_at_jdepoc reproduces de440.bsp
  at JDEPOC for every body to 4.77e-7 km and 7.1e-15 km/s in f64 (5.66e-7 km,
  5.7e-15 km/s in double-double; vectors SYSTEM-HEADER-VS-DE440-POS/VEL-MAX-DIFF,
  1e-3 km and 1e-9 km/s), while the heliocentric reading misses by
  ≥ 6.735e5 km and the km/km-s reading by ≥ 5.41e7 km (both asserted to exceed
  1e3 × the tolerance). JPL Horizons DE441 at JDEPOC agrees with the header
  states to 9.54e-7 km (WP11 check 7, data tier). An independent header reader,
  tools/oracles/header440.py, run with jplephem in the jpl oracle image,
  reproduces the 4.768e-7 km (WP16 check 3). The XC…ZDC block is all zero in
  DE440 and its meaning is still unexplained; it is not used (OI-73). The
  state-vector part of this requirement is therefore no longer blocked;
  OI-04 (GMSDOT units) still blocks PHY-002's GMSDOT entry only.
```

### REF-003 — Asteroid and KBO initial states from sb441-n373

```yaml
id: REF-003
title: "Asteroid/KBO initial states sampled from sb441-n373*.bsp at JDEPOC (provisional)"
status: draft
criticality: high
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
related: [PHY-014, PHY-019, CI-003]
notes: >-
  BLOCKED: how JPL supplied asteroid ICs, the contents of sb441-n373.bsp, and
  that n373 = 343 asteroids + 30 KBOs are all UNVERIFIED; IOM 392R-21-005 must
  be read. The 36-point KBO ring masses (MA8201–MA8236) have no state in
  sb441-n373. Criticality raised to high 2026-10-14 (red-team review, Journal
  #9): it feeds the high-criticality PHY-014 and PHY-019. Scheduled in M4,
  whose exit (PHY-014) needs these initial conditions.
  BUILD EVIDENCE (sync 2026-10-20; WP10, WP11, WP15 on m3-newtonian-nbody).
  File: the 14.13 GB sb441-n373.bsp is not pinned; the pinned product is
  sb441-n373s.bsp (982 106 112 B, SHA-256 21431132…ca8da10b, directory
  small_bodies/asteroids_de441, i.e. a DE441 product), whose comment area
  says it is an SPKMERGE subset of sb441-n373.bsp over 1550–2650. It holds
  374 type-2 segments, all Sun-centred, frame 1, for 373 targets: the 343
  numbered asteroids of header.440 plus 30 trans-Neptunian objects (Pluto is
  not among them). That the n373s JDEPOC states equal those of the full file
  is UNVERIFIED. DE440's own asteroid initial conditions are in no pinned
  product (WP11 deviation 5, WP15 deviation 4), so the asteroid states used
  are DE441-consistent, not DE440's. Sampled file:
  tests/goldens/sb441-n373s-jdepoc-states.csv (373 rows, Sun-centred ICRF km
  and km/s; header records the product name, URL, SHA-256, retrieval date,
  header.440 SHA-256, JDEPOC, centre, frame, units and generator commit),
  written by tools/sample_sb441.py with jplephem 2.24 and checked by
  crates/ferroastro-system/tests/ref003_small_bodies.rs (PR tier) and
  data_system_initial_conditions.rs (ferroastro-ephem round trip, 0 km and
  0 km/s for all 373 targets). Coverage gap against this acceptance: of the
  409 MA entries, the 343 numbered asteroids and 3 KBOs identified by a
  pre-registered rule (MA8001 Eris, MA8007 Orcus, MA8009 2002 UX25; ids
  confirmed by Horizons) have states; the other 27 MA80nn KBO masses are not
  identified and are not placed, and the 36 ring masses MA8201–MA8236 have no
  state (placed by FerroAstro at 44 au in ECLIPJ2000 at k·10°, a geometry
  that is UNVERIFIED). The acceptance is therefore not yet met in full; OI-24
  stays open (IOM 392R-21-005 still unread).
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
  only parameter c (β = γ = 1 fixed).
  BUILD EVIDENCE (sync 2026-10-20; WP16, m3-newtonian-nbody): packaging is
  answered (OI-28): rebound 5.2.2 has a cp312 manylinux wheel, but reboundx
  5.1.0 is an sdist only, so ci/oracles/rebound/Dockerfile installs a pinned C
  compiler (gcc=4:12.2.0-3, libc6-dev=2.36-9+deb12u14) and builds reboundx with
  --no-build-isolation; every pip package is installed with --require-hashes.
  Stage A exists: ci/golden/rebound/stage-a-sun-8planets-newton-100yr/ (with
  MANIFEST.json) is matched by FerroAstro to 7.132e-4 km (Mercury) against
  ORACLE-STAGE-A-REBOUND-MAX-DR (1e-3 km, PROVISIONAL), in
  crates/ferroastro-oracle/tests/stage_a.rs. The difference is f64 round-off,
  dominated by REBOUND's own (WP16 diagnosis); the 1e-3 km bound is only 1.4×
  above it and is NEEDS HUMAN REVIEW (WP16 item 1). Stage C (gr_full) is not
  built yet (WP21), so the acceptance is met for stage A only. The runner
  (tools/oracles/rebound_runner.py) uses only REBOUND's published
  documentation; REBOUND 5 dropped the ri_ias15 structs, so IAS15 settings go
  through sim.integrator (WP16 item 5).
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
  Sun; absence of a coupled multi-body propagator; conda-forge status of the
  JCC wrapper. Orekit reads JPL binary DE files, not .bsp.
  BUILD EVIDENCE (sync 2026-10-20; WP17, m3-newtonian-nbody; OI-26 and OI-27
  findings recorded, neither closed): the JPL binary file path is confirmed:
  https://ssd.jpl.nasa.gov/ftp/eph/planets/Linux/de440/linux_p1550p2650.440
  exists (102 272 352 B) and Orekit 13.1.9 loads it through a DirectoryCrawler;
  it is pinned in tests/data/MANIFEST.toml as de440-linux-binary
  (29915576…9ad062), and crates/ferroastro-oracle/tests/data_de440_linux_binary.rs
  shows its 400 record-1 constants equal header.440 bitwise. orekit-data's own
  lnxp1990.440 subset is removed from the image. Frame finding: Orekit's
  getSun().getInertiallyOrientedFrame() is rotated 0.535 rad from ICRF (it
  follows the solar pole), so the journal 03 sketch "Sun-centred ICRF
  (getInertiallyOrientedFrame)" is wrong; ci/oracles/orekit StageB.java
  rotates its output to ICRF axes, and this rotation is recorded with the
  runner. J2OnlyPerturbation exists in 13.1.9 (its javadoc has no @since);
  LenseThirringRelativity(gm, bodyFrame) is @since 10.3, and whether its spin
  model is Earth-specific was not determined. Stage B golden:
  ci/golden/orekit/oracle-stage-b-mercury-de440-field-100yr/; FerroAstro's
  ephemeris-field run matches it to 0.131 km (ORACLE-STAGE-B-OREKIT-MAX-DR,
  1 km), but only after Orekit's DP853 tolerances were tightened following a
  FAILED pre-registered run (1.102 km); this post-measurement oracle change is
  NEEDS HUMAN REVIEW (WP17 item 3; progress review after WP18, R6).
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
  and is central-body 1PN, not EIH.
  BUILD EVIDENCE (sync 2026-10-20; WP17, m3-newtonian-nbody; OI-28 and OI-31
  findings, OI-31 not closed): swiftest 2026.7.1 ships cp311–cp313
  manylinux_2_28 wheels, so ci/oracles/swiftest/Dockerfile installs it and
  its 64-package closure with --only-binary and SHA-256 pins, no compiler.
  The documented 2026.7.1 API has add_body(..., j2rp2=, j4rp4=, ...) and the
  output carries j2rp2/j4rp4, so a plain J2 input is documented; it was not
  exercised (the runner refuses sun_j2). Settings
  (ci/oracles/swiftest/settings.toml): WHM, dt = 1/128 d. Stage A golden
  ci/golden/swiftest/stage-a-sun-8planets-newton-100yr/: the pre-registered
  1 km bound FAILED (max 28.68 km, Mars), traced to a per-step f64 energy bias
  of about −1.1e-16 per step, and ORACLE-STAGE-A-SWIFTEST-MAX-DR was widened
  to 60 km after the measurement, with that derivation in its rationale;
  NEEDS HUMAN REVIEW (WP17 item 2; the WP18 progress review, R5, notes the
  drift model fails for Mercury, 0.38 km measured, so a Mercury-only 1 km
  bound is an alternative). The rate checks are sharp: C-lite GR
  contribution 42.980094 vs FerroAstro 42.980085 (Δ +9.2e-6, bound 1e-3) and
  P-add Venus within 1.4e-8 of FerroAstro. Swiftest 2026.7.1 segfaults
  intermittently for Sun + one planet; tools/oracles/swiftest_runner.py runs
  each attempt in a fresh interpreter and records the attempts in the
  manifest.
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
  BUILD EVIDENCE (sync 2026-10-20; WP17, m3-newtonian-nbody; OI-25 finding,
  not closed): the pinned commit is smirik/mercury@aee9e0f6b8e4d359a9ed3607ee12e34a2e2dafac,
  fetched at image build time by ADD --checksum and built with make build
  (gfortran 12.2.0 pinned). Its GPL-3.0 LICENSE was added by the mirror's
  maintainers in that commit ("Add a GNU license (addresses issue #26)");
  JOHN_E_CHAMBERS.md, mercury6.man and README.md contain no licence or
  copyright statement, so the original Chambers licence is still
  UNVERIFIED. The mirror is also not the original package but a modified
  derivative: its manual says it was "modified by I. Dovgalev to provide true
  double precision" (2016), and its README describes updated constants. Only
  the manual, README and *.in.sample files were read (no .for/.inc source);
  k² is measured black-box. tools/oracles/mercury6_runner.py refuses every GR
  switch (gr_1pn_central, gr_eih, ppn) citing this requirement
  (tools/tests/test_oracles.py). Stage A golden
  ci/golden/mercury6/stage-a-sun-8planets-newton-100yr/ is matched to
  0.104 km (bound 1 km, pre-registered).
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
  ICRS↔ecliptic and J2000 rotation goldens used by PR-fast CI. The acceptance
  is met in three parts, each with its own evidence: (1) pinned time-scale
  goldens (TT↔TAI/TDB/TCB/TCG, dtdb, jd2cal, TAI−UTC) generated with pyerfa
  2.0.1.5 and used by PR-fast tests; (2) pinned ICRS↔ecliptic, IAU 2006
  precession and ECLIPJ2000 rotation goldens; (3) the erfa oracle image
  regenerates the data rows of both golden sets bit-identically.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §2.2 and REF-009.
parent: null
related: [DEP-009, DEP-010, LIC-004]
notes: >-
  Acceptance split recorded 2026-10-20 (build-evidence sync; the original
  sentence is unchanged, the three parts say how it is evidenced). Evidence on
  m3-newtonian-nbody: (1) WP8: tests/goldens/erfa-time-pyerfa.csv (223
  epochs 1550–2650) and erfa-dat-pyerfa.csv (219 instants), written by
  tools/goldens/erfa_time.py, which refuses any pyerfa but 2.0.1.5; checked by
  crates/ferroastro-time/tests/ref009_pyerfa_goldens.rs against the PYERFA-*
  vectors (1 ns PROVISIONAL; f64 bitwise except TT→TDB at 9.6e-12 s). (2) WP9:
  tests/goldens/erfa-frames-pyerfa.csv (113 epochs) from
  tools/goldens/erfa_frames.py, checked by crates/ferroastro-frames/tests/ref009_pyerfa_frames.rs (1e-12 rad PROVISIONAL; f64 bitwise identical).
  (3) WP16: the erfa image (ci/oracles/erfa/Dockerfile, run by
  tools/oracles/run_oracles.sh) regenerated all three files with identical
  data rows. The committed golden stays under tests/goldens/ (no second copy
  under ci/golden/; WP16 item 4). Until WP16 the goldens came from a local uv
  dependency group (goldens), not from the image (WP8 item 5).
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
  with a message naming the switch. FerroAstro's own scenario runner obeys the
  same rule: a switch known to the scenario spec but not implemented is
  refused with FA-E1201 and an unknown switch with FA-E1202, each naming the
  switch.
source: >-
  docs/strategy/journal/03-reference-implementations-ci.md §4 principles and
  REF-011.
parent: null
related: [CI-002, REF-008]
notes: >-
  FerroAstro clause added 2026-10-20 (build-evidence sync; a tightening).
  Evidence on m3-newtonian-nbody: WP13 crates/ferroastro-scenario/tests/ci_scenarios.rs (unknown_switch_is_refused_by_name,
  unsupported_switch_is_refused_by_name), crates/ferroastro-propagate/tests/scenario_runs.rs::run_refuses_unimplemented_switches_by_name (FA-E1201
  naming forces.gr_eih, forces.asteroids, forces.gr_1pn_central in N-body
  mode, integrator.abm) and crates/ferroastro-cli/tests/cli_scenario.rs::cli_refuses_unsupported_and_unknown_switches_by_name (exit 2). Oracle
  runners exit 3 naming every unsupported switch and option, before importing
  the tool (WP16/WP17, tools/oracles/common.py refuse_unsupported,
  tools/tests/test_oracles.py); canaries ci/oracles/canaries/canary-{ppn-gr-eih,sun-j2}.toml are refused by REBOUND, Swiftest and
  Mercury6.
```
