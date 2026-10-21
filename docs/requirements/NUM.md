<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# NUM — Numerical methods, precision and the precession estimator

**Scope.** These requirements cover the integrators FerroAstro provides, when arithmetic beyond f64 is used and how, how a perihelion-precession rate is extracted from a trajectory so that it matches the published observable, the equations of motion for each ladder rung, and performance for N ≈ 10–350 bodies. Source: Journal #4 (`docs/strategy/journal/04-numerical-methods.md`). Architectural decisions adopted in the Journal #4 lead review: IAS15 is the default integrator; ABM13 with the mixed double-double scheme is the "ephemeris mode" for DE440 reproduction; the Moon is integrated geocentrically; bodies are tiered (Major/Minor/Ring); the Park 2017 estimator is the canonical observable; the 80-bit x87 path is dropped.

Evidence labels from the journal: **[V]** checked against the primary source; **UNVERIFIED** from memory, a secondary source or an estimate; *(derived here)* the author's own arithmetic. See [OPEN-ITEMS.md](OPEN-ITEMS.md).

---

### NUM-001 — IAS15-class Gauss–Radau integrator (default)

```yaml
id: NUM-001
title: "IAS15-class adaptive 15th-order Gauss–Radau integrator with compensated summation"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide an IAS15-class adaptive 15th-order Gauss–Radau
  integrator with Rein–Spiegel step control and compensated summation of the
  state update. It SHALL accept velocity-dependent forces through the
  predictor–corrector iteration, with at most 12 iterations and convergence
  judged on δb₆. It is the default integrator for science runs and all rungs.
acceptance: >-
  Passes the Kepler and Kozai tests of Rein & Spiegel 2015. In f64 the energy
  error grows as t^(1/2) (Brouwer's law) over ≥ 10⁹ steps; NUM-023 fits the
  slope.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.2, TL;DR 1 and NUM-001;
  Rein & Spiegel 2015, MNRAS 446, 1424, doi:10.1093/mnras/stu2164.
parent: null
related: [NUM-023, DEP-008, LIC-001, PHY-001, PHY-005]
notes: >-
  Clean-room implementation from the paper only (DEP-008). Typical cost ~15
  force evaluations per step (derived here: ~7 × ~2 iterations).
```

### NUM-002 — Fixed-step ABM PECE/PECEC integrator (ephemeris mode)

```yaml
id: NUM-002
title: "Fixed-step Adams–Bashforth–Moulton PECE/PECEC, order 8–14, default 13"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide a fixed-step ABM PECE/PECEC integrator with
  configurable order 8–14 (default 13) and a high-order Runge–Kutta start-up
  (e.g. DP8 at h/8). The accumulators SHALL be generic over `Real`.
acceptance: >-
  On a model equivalent to Subbotin et al. 2023 (40-yr EIH, ABM13 PECEC,
  h = 1/16 d) the f64-versus-mixed-DD trend of their Table 1 is reproduced
  (Mercury: f64 2.47 m versus mixed DD 8.4 mm).
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.1, §1.2 and NUM-002;
  Subbotin, Kodukov & Pavlov 2023, CeMDA, doi:10.1007/s10569-023-10139-2,
  Table 1.
parent: null
related: [NUM-006, NUM-007, NUM-024]
notes: >-
  Whether production EPM uses exactly order 13 and h = 1/16 d is UNVERIFIED;
  the EPM-based study configuration is [V].
```

### NUM-003 — Gauss–Legendre IRK reference integrator

```yaml
id: NUM-003
title: "Gauss–Legendre IRK (s = 8, order 16) reference integrator"
status: draft
criticality: medium
verification: test
statement: >-
  FerroAstro SHALL provide a Gauss–Legendre implicit Runge–Kutta reference
  integrator (s = 8, order 16) using the Hairer–McLachlan–Razakarivony
  round-off-aware implementation.
acceptance: >-
  Integrating forward then backward returns the initial conditions to
  round-off, and results agree with NUM-001.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.2 and NUM-003; Hairer,
  McLachlan & Razakarivony 2008, BIT 48, 231, doi:10.1007/s10543-008-0170-3;
  Antoñana et al. 2022, CeMDA, doi:10.1007/s10569-022-10081-9.
parent: null
related: [NUM-001, NUM-023]
notes: >-
  Derived here: Gauss collocation on the velocity-form ODE in (x, v) is
  symmetric but not canonically symplectic once p ≠ mv.
```

### NUM-004 — Wisdom–Holman / WHFast fast rung

```yaml
id: NUM-004
title: "Wisdom–Holman / WHFast with symplectic correctors for Newtonian + GR-potential rungs"
status: draft
criticality: medium
verification: test
statement: >-
  FerroAstro SHALL provide a Wisdom–Holman / WHFast integrator (Jacobi or
  democratic-heliocentric coordinates) with symplectic correctors for the
  Newtonian and GR-potential rungs. The documentation SHALL state that full EIH
  is not supported in WH.
acceptance: >-
  Newtonian Solar-System energy error stays bounded over 10⁶ yr. The docs
  contain the EIH-unsupported statement.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.2, §1.3 and NUM-004;
  Wisdom & Holman 1991, AJ 102, 1528; Rein & Tamayo 2015, arXiv:1506.01084.
parent: null
related: [NUM-005, DEP-008]
notes: >-
  Clean-room from the papers (DEP-008). WH 1991 and the Rein & Tamayo
  volume/page are UNVERIFIED citations.
```

### NUM-005 — Saha–Tremaine pseudo-velocity 1PN extension of WH

```yaml
id: NUM-005
title: "Saha & Tremaine 1994 pseudo-velocity 1PN extension of WH (optional rung)"
status: draft
criticality: medium
verification: test
statement: >-
  FerroAstro SHALL implement the Saha & Tremaine 1994 pseudo-velocity 1PN
  extension of WH (Kepler-drift time rescale, p⁴ sub-drift, 1/r² kick) as an
  optional rung.
acceptance: >-
  Mercury precession = 42.98 ″/cy ± 10⁻³, and the mean motion is correct when
  compared with R3 integrated under NUM-001.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.3 and NUM-005; Saha &
  Tremaine 1994, AJ 108, 1962, doi:10.1086/117210.
parent: NUM-004
related: [PHY-004, PHY-005]
notes: >-
  The coefficients of the extracted H_PN must be cross-checked against the
  typeset paper (UNVERIFIED). Covers only the dominant Sun–planet 1PN terms, not
  full EIH.
```

### NUM-006 — Arithmetic generic over a `Real` trait

```yaml
id: NUM-006
title: "All arithmetic generic over Real with f64, double-double and optional f128 backends"
status: draft
criticality: high
verification: test
statement: >-
  All arithmetic SHALL be generic over a crate-owned `Real` trait with backends
  f64, double-double (FMA-based TwoProd) and optional f128. The precision mode
  SHALL be a type parameter resolved by monomorphisation, not a runtime
  switch. There SHALL be no dependency on x87 80-bit arithmetic.
acceptance: >-
  The crate compiles with each backend by monomorphisation, and the f64-path
  benchmark is unchanged relative to a non-generic f64 baseline. "Unchanged"
  is measured as the ratio of medians (candidate / baseline) of alternated
  Criterion runs and must lie within ±2 % (PROVISIONAL band, vector
  NUM006-F64-BENCH-RATIO).
source: >-
  docs/strategy/journal/04-numerical-methods.md §2 and NUM-006.
parent: null
related: [DEP-007, DEP-012, API-001]
notes: >-
  Benchmark band added 2026-10-20 (build-evidence sync; it quantifies
  "unchanged" and loosens nothing). Evidence (WP7, m3-newtonian-nbody):
  tools/bench_num006.py runs a baseline and a candidate commit in detached
  worktrees, alternating Criterion ias15_sun_mercury/propagate_1_julian_year
  for 5 rounds, and compares medians of medians with the vector. Measured
  against the non-generic baseline c5fd5a0: 0.9922, 0.9846 and 1.0048 for
  three WP7 candidates. The machine is shared (load ~13 on 32 cores; a single
  round showed a 6.5 % spread), so the 2 % band is PROVISIONAL and NEEDS HUMAN
  REVIEW (WP7 item 2). Monomorphisation: crates/ferroastro-real/tests/num006_backends.rs (f64 and double-double on stable; f128 matches DD to
  ≤ 8.2e-33 on nightly only, compiled out on stable).
  AUTHORITATIVE over DEP-007 for the numerical contract (backends, FMA TwoProd,
  zero-cost f64, no x87). DEP-007 remains the dependency-side statement of
  which components must be generic and that f128 is nightly-only behind a
  non-default feature. Cost of f64 + compensated summation (~1.1–1.3×) and of
  emulated f128 are UNVERIFIED estimates; Rust f128 is nightly-only and its
  math/wasm32 support is UNVERIFIED.
```

### NUM-007 — Ephemeris precision mode (mixed double-double)

```yaml
id: NUM-007
title: "Ephemeris precision mode: Subbotin mixed double-double scheme"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide an "ephemeris precision mode" implementing the
  Subbotin mixed scheme: state, integrator sums and coefficients in
  double-double; forces in f64.
acceptance: >-
  Mercury round-off over 40 yr ≤ 1 cm relative to a full double-double run.
source: >-
  docs/strategy/journal/04-numerical-methods.md §2 and NUM-007; Subbotin et al.
  2023 (mixed DD ≈ 3× f64 cost; Mercury 8.4 mm / 40 yr).
parent: NUM-006
related: [NUM-002, NUM-008, NUM-024]
notes: >-
  Derived-here extrapolation (UNVERIFIED): pure t^(3/2) f64 error growth gives
  ~0.3 km at 10³ yr and ~3×10² km at 10⁵ yr, which is why this mode exists for
  DE reproduction.
```

### NUM-008 — Geocentric Moon and Taylor-expanded tidal acceleration

```yaml
id: NUM-008
title: "Moon integrated geocentrically with ≥4-term Taylor-expanded differential acceleration"
status: draft
criticality: high
verification: test
statement: >-
  The Moon SHALL be integrated geocentrically (the Earth–Moon vector is a
  state variable), using the Taylor-expanded differential third-body
  acceleration with at least 4 terms, and with enough terms that the series
  truncation stays below f64 round-off of the tidal term in the expansion
  actually used.
acceptance: >-
  Lunar round-off ≤ 2 mm over 40 yr in NUM-007 mode. In FerroAstro's
  q-formulation, F(q) = (1 + q)^(−3/2) − 1 = Σ c_k q^k with
  q = ρ·(ρ − 2d)/d², the default series (12 terms) agrees with a
  double-double direct difference to ≤ 4 ε of the tidal term in every tested
  geometry (vector NBODY-MOON-TAYLOR-VS-DD-ULPS).
source: >-
  docs/strategy/journal/04-numerical-methods.md §2 item 2 and NUM-008.
parent: NUM-007
related: [PHY-014]
notes: >-
  |r_M − r_E| is ~400× smaller than barycentric vectors [V]; DE, EPM and INPOP
  all integrate the Moon geocentrically. ACCEPTANCE TIGHTENED 2026-10-20
  (build-evidence sync), because 4 terms are not enough in the q-formulation.
  Evidence (WP12, m3-newtonian-nbody, crates/ferroastro-forces/src/nbody/tests.rs::geocentric_moon_expansion_matches_double_double_direct_difference):
  12 terms reach 0.568 ε of the tidal term (worst of 4 geometries) against
  the 4 ε bound, while 4 terms leave up to 6.4e6 ε for a Sun-aligned geometry
  (q = −5.1e-3; 0.45 ε at q = 1.5e-4). That 4 terms suffice for the 2 mm /
  40 yr target in the expansion Subbotin et al. 2023 use is not contradicted
  (periodic errors of that size stay sub-mm) but needs a source check (WP12
  deviation 6). FerroAstro enforces a minimum of 4 terms and switches to the
  closed form above |q| = 0.05, which the planets never reach. Geocentric vs
  barycentric Moon (crates/ferroastro-forces/tests/nbody_moon_formulations.rs,
  10 yr): the two formulations agree to 7.2e-13 km in double-double, and the
  geocentric form reduces f64 round-off of Moon − Earth by a factor of 148.9.
  The 2 mm / 40 yr acceptance needs NUM-007 mode, which is not built yet.
```

### NUM-009 — Body tiering

```yaml
id: NUM-009
title: "Body tiering: EIH 1PN for all bodies; Newtonian Minor from Majors only; Ring fixed masses"
status: draft
criticality: medium
verification: test
statement: >-
  Body tiering SHALL be a first-class model setting for the Newtonian
  interactions: Major bodies interact mutually; Minor bodies feel Newtonian
  forces from the Majors only and do not interact with each other; Ring
  bodies are fixed point masses. When EIH is on, the 1PN EIH acceleration
  (DE440 Eq. 27 minus its Newtonian part) SHALL be evaluated for every
  moving body, Major and Minor (the Sun, planets, the Moon, asteroids and
  KBOs), each with its own 1PN terms, with the sums over all bodies;
  tiering does not restrict the 1PN terms.
acceptance: >-
  With the DE440 asteroid set (343 Minor bodies), the run costs < 2× the
  16-Major-body run. The bound applies when EIH is on, as the statement
  requires; with Newtonian Majors the cost ratio is measured and reported
  only.
source: >-
  docs/strategy/journal/04-numerical-methods.md §5 and NUM-009; Subbotin et al.
  2023; Folkner et al. 2014, IPN PR 42-196.
parent: null
related: [NUM-024, PHY-014]
notes: >-
  EIH SCOPE (maintainer sign-off 2026-10-21, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280;
  ruling on #13, https://github.com/Proper-Time-Physics-Institute/FerroAstro/pull/13#issuecomment-6306750993).
  DE430 p. 12 states "The summation is over all bodies: Sun, Moon, planets
  and asteroids" and DE440 p. 5 "the summations are over all bodies"; the
  maintainer chose "Match DE440: all bodies". This supersedes the earlier
  "Major = EIH, Minor = Newtonian from Majors only" reading of this
  requirement (and the Majors-only EIH on m4-eih): the 1PN EIH acceleration
  is evaluated for every moving body with the sums over all bodies, and the
  asteroids and KBOs receive their own 1PN terms. Tiering continues to
  govern the Newtonian part only (no Minor–Minor Newtonian interaction). Not
  decided by the sign-off, to be recorded in WP22: (a) whether the 1PN sums
  for a Minor receiver include other Minor sources, whose Newtonian pull the
  tiered model omits ("all bodies" read literally includes them); (b)
  whether the fixed Ring masses enter the 1PN sums as sources (they receive
  no acceleration). The < 2× cost bound was set for Majors-only EIH; with
  EIH evaluated for all bodies the ratio MUST be re-measured. The bound is
  not changed here.
  Tiering costs < 1 cm (planets) / < 1 mm (Moon) over 40 yr [V]. Interaction
  count 16×343 + 120 ≈ 5.6×10³ versus ~6.4×10⁴ (derived here). EIH cost per pair
  ~3–5× Newtonian is UNVERIFIED.
  BUILD EVIDENCE (sync 2026-10-20; WP11, WP12, WP15 on m3-newtonian-nbody).
  (1) Scope of the 2× bound: the EIH-only sentence above makes explicit what
  the statement already implies; it is recorded with its evidence because it
  stops a Newtonian-Major measurement from counting as a failure. Measured
  with Newtonian Majors (crates/ferroastro-forces/tests/nightly_num009_tier_cost.rs::nightly_num009_tiered_minor_bodies_cost, 10 yr
  from JDEPOC, median of 5): tiered/16-Major = 28.2 (reviewer re-run 28.5),
  as predicted from the pair count (≈ 46× for the forces), while tiered /
  all-mutual = 0.117 (< 1, asserted). The vector NUM009-TIER-COST-RATIO
  carries blocked_by = "OI-42" and is non-gating until EIH Majors exist
  (WP19; OI-42 holds the EIH cost estimate). (2) "16 Majors" is read as the
  11 DE440 major bodies (Sun, Mercury, Venus, Earth, Moon, Mars–Pluto system
  barycentres) plus the 5 header asteroids with the largest GMs (1 Ceres,
  4 Vesta, 2 Pallas, 10 Hygiea, 511 Davida), leaving 338 Minor bodies; the
  acceptance's "343 Minor" would count those 5 twice (WP15 deviation 6;
  Subbotin et al. 2023 should be checked for their 16, OI-78). (3) header.440
  has 409 MA entries: 343 numbered asteroids (MAnnnn, nnnn < 8000, NAIF
  2 000 000 + nnnn) and 66 others, MA8001–MA8030 (30 KBO masses, 3 of them
  identified) and MA8201–MA8236 (36 equal ring masses) (WP11, WP15;
  crates/ferroastro-system/tests/ref002_header_initial_conditions.rs). (4)
  Minor bodies act on the Majors: MinorFromMajors applies the reaction of
  each Minor body on the Majors by default (MinorReaction::On,
  configurable), because N-AST (PHY-019) is the asteroids' pull on Mercury;
  that DE440 does the same was not checked against the paper (WP11 review,
  WP12 deviation 2; OI-78). (5) Ring bodies are frozen (zero acceleration),
  so the moving bodies' momentum is not conserved in their presence (WP12
  deviation 7). Tests: crates/ferroastro-forces/src/nbody/tests.rs::tiers_follow_num009; crates/ferroastro-propagate/tests/scenario_runs.rs::small_body_gms_equal_their_header_values_bitwise and
  small_bodies_carry_their_gm_and_conserve_total_momentum.
```

### NUM-010 — Dense output and perihelion event location

```yaml
id: NUM-010
title: "Dense output at integrator order and event location for r·v = 0"
status: draft
criticality: high
verification: test
statement: >-
  Each integrator SHALL provide dense output (a continuous interpolant at
  integrator order) and event location for r·v = 0 (heliocentric perihelion).
acceptance: >-
  Perihelion events are located to ≤ 10⁻⁶ s; on a Kepler orbit the perihelion
  times match the analytic values to round-off. The 10⁻⁶ s bound is the
  location error (the located time against the integrated trajectory's own
  r·v root), not the accumulated drift from the analytic times. "To
  round-off" is backend-dependent: it is asserted in double-double, where
  round-off is negligible, against the same 10⁻⁶ s; the f64 drift from the
  analytic times is reported.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.2 and NUM-010.
parent: null
related: [NUM-011]
notes: >-
  Reading recorded 2026-10-20 (build-evidence sync; WP7 item 10). It states
  which error each bound applies to and loosens no number. Evidence on
  m3-newtonian-nbody, crates/ferroastro-integrators/tests/num010_dense_events.rs (Keplerian Mercury-like orbit, 100 orbits): f64
  dense-output location error 1.69e-12 s (NUM010-EVENT-LOCATION, 1e-6 s);
  double-double event times vs analytic P/2 + kP 1.16e-15 s
  (NUM010-KEPLER-PERIHELION-TIMES, 1e-6 s); f64 vs analytic 2.18e-11 s,
  reported. N-body mode (WP13, crates/ferroastro-propagate/tests/scenario_runs.rs::nbody_perihelion_events_match_analytic_kepler_times,
  Sun + Mercury, 100 yr, 416 passages): 1.287e-5 s against analytic times
  (NUM010-NBODY-PERIHELION-VS-KEPLER, 1e-4 s PROVISIONAL, which covers the
  accumulated phase round-off over 415 orbits) and a location residual of
  2.38e-7 s (NUM010-NBODY-EVENT-LOCATION, 1e-6 s; this metric sits on the f64
  recomputation floor of r·v, the true bracket is ~1e-11 s). Event location
  is Illinois regula falsi on the last step's collocation polynomial
  (Ias15::locate_event); dense output is opt-in (Ias15Config::dense_output).
```

### NUM-011 — Park2017 precession estimator (canonical observable)

```yaml
id: NUM-011
title: "Precession estimator Park2017: mean-orbit frame, perihelion passages, quadratic + periodic LSQ fit"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide the "Park2017" precession estimator: a mean-orbit
  frame with configurable pole (default RA 280.9876°, Dec 61.4481°, ICRF),
  perihelion directions at each perihelion passage projected onto that plane,
  and a least-squares fit of B + ϖ̇t + Qt² + Σ(Sᵢ sin νᵢt + Cᵢ cos νᵢt) with
  user-supplied or FFT-derived frequencies. It SHALL report both the formal
  and the jack-knife σ of ϖ̇. It is the canonical observable for every absolute
  rate FerroAstro reports.
acceptance: >-
  For a DE440-class model over 1000–3000 CE the estimator returns
  575.31 ± 0.01 ″/cy, with both σ values reported.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.1 and NUM-011; Park et al.
  2017, AJ 153, 121 (2000-yr span, epoch 2000-01-01, 14 periodic terms with
  amplitude ≥ 0.5″).
parent: null
related: [PHY-012, PHY-013, PHY-014, CI-009, NUM-010, NUM-012]
notes: >-
  PHY-014 (±0.005) is authoritative for the physics acceptance of 575.31;
  ±0.01 here is the estimator-level criterion. CI-009 (Journal #3) specified an
  LRL-vector-longitude estimator for cross-tool comparison; per the Journal #4
  lead review this estimator is canonical and LRL is a secondary estimator
  (NUM-012). The slope-bias constant (~A·P/T² for an unmodelled sinusoid) is
  UNVERIFIED. Achievable precision (~10⁻³ ″/cy total) is a derived, UNVERIFIED
  estimate.
  BUILD EVIDENCE AND OPEN DECISIONS (sync 2026-10-20; WP5, WP6, WP14 on
  m3-newtonian-nbody). The acceptance is met on DE441 via JPL Horizons
  (ferroastro-precession park2017; data_park17_rate_from_horizons_de441_mean_orbit):
  575.309163 ″/cy, formal σ 0.00083, jack-knife σ 0.00153, against
  575.31 ± 0.01; an independent Python estimator in the progress review after
  WP5 got 575.309174. Two points are recorded as an open decision (OI-74)
  rather than settled, because the evidence does not settle them. (a) Scope
  of "canonical for every absolute rate": WP5 delivered the estimator but not
  that clause. The single-body rungs still report the initial-orbit-plane
  passage estimator (WP4), and the WP6 cross-estimator table shows passages
  and Park2017 agree to ≤ 2e-8 ″/cy on those rungs, so the gap is one of
  labelling, not of value; the N-body differential runs (WP14 onward) do use
  Park2017. Whether to make every rung report Park2017, or restrict
  "canonical" to N-body and absolute rates, is for a maintainer. (b)
  Heliocentric versus barycentric passages: FerroAstro uses heliocentric
  passages (Mercury − Sun; Horizons centre 500@10), as PHY-016 specifies, but
  Park 2017 does not say which it used (WP5 "Centre"; kept for humans in
  every progress review since). Also unresolved and reported, not asserted:
  Park17 Table 2 amplitudes are not reproduced, because once-per-orbit
  sampling aliases k·n_M + j·n_P pairs; the rate is orthogonal to them (11
  FFT-derived terms give the same ϖ̇ to 1e-5 ″/cy) (WP5).
```

### NUM-012 — Alternative estimators with labelled definitions

```yaml
id: NUM-012
title: "Alternative estimators: osculating ϖ, LRL-vector angle, per-orbit mean ellipse"
status: draft
criticality: medium
verification: test
statement: >-
  FerroAstro SHALL provide alternative precession estimators (osculating
  ϖ = Ω + ω, Laplace–Runge–Lenz vector angle, per-orbit mean ellipse). Every
  estimator output SHALL label its definition, frame and span.
acceptance: >-
  A test rejects estimator output lacking definition, frame or span labels. The
  docs report the cross-estimator spread for the Mercury ladder.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.2 and NUM-012; Pogossian
  2022, CeMDA, doi:10.1007/s10569-022-10085-5.
parent: null
related: [PHY-015, CI-009, NUM-011]
notes: >-
  A 1PN-corrected LRL vector form is UNVERIFIED. Osculating elements from
  harmonic-coordinate states carry ~5 mas periodic terms under 1PN (derived
  here).
```

### NUM-013 — Differential-run harness

```yaml
id: NUM-013
title: "Differential-run harness: identical ICs, rung toggled, Δϖ̇ with decomposition"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide a differential-run harness that runs identical
  initial conditions with one rung toggled and outputs Δϖ̇ with a per-body and
  interaction-term decomposition.
acceptance: >-
  Split by rung type (maintainer decision 2026-10-14, Journal #8):
  (a) isolated single-body terms (Sun + Mercury only: GR 42.98 ″/cy, J₂
  0.0286 ″/cy at J₂ = 2.25×10⁻⁷, LT −0.0020 ″/cy) agree with the analytic
  formulas within 10⁻⁴ ″/cy; (b) N-body differential contributions follow
  PHY-012's PROVISIONAL "report both" protocol (ADR-0014): the harness
  computes P-remove (full system minus full system without X) and P-add
  (Sun + Mercury + X minus Sun + Mercury), Newtonian, from identical initial
  conditions; the P-remove values agree with the Park 2017 Table 3 values
  within ±0.005 ″/cy (asserted), the P-add values are reported, and
  interaction cross terms are reported separately with their sign
  convention labelled.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.2 and NUM-013.
parent: null
related: [PHY-005, PHY-010, PHY-011, PHY-012, PHY-019, CI-009]
notes: >-
  OVERLAP with PHY-012/PHY-005/PHY-010/PHY-011: the PHY- requirements are
  authoritative for the expected values and ladder tolerances; NUM-013 is
  authoritative for the harness mechanics. The 10⁻⁴ ″/cy target is tighter than
  the PHY tolerances (GR ±0.001, J2 ±0.0005, LT ±0.0002). Journal #4 R4 notes
  that in an N-body run, planet-interaction residuals in the GR differential
  are ≲ 10⁻² ″/cy, so 10⁻⁴ is achievable only when the comparison isolates the
  single-body term (e.g. Sun + Mercury only). The split acceptance above is the
  provisional resolution; OI-70 stays open until a maintainer confirms it. The
  comparison basis SHALL be documented. Park's switch-on/off protocol is not
  yet confirmed against the Park 2017 text (OI-38 stays open); without a
  fixed protocol part (b) is ill-posed, because cross terms (up to
  0.0411 ″/cy) exceed the tolerance (PHY-012 notes).
  PART (b) PROTOCOL, 2026-10-20 (maintainer decision; ADR-0014): part (b) is
  no longer blocked on OI-38. P-remove is asserted and P-add reported; this
  was chosen after the WP14/WP17 results were known (P-add, the
  pre-registered protocol, missed 4 of 7 rows by 0.019–0.043 ″/cy; P-remove
  passes all 7 within 0.0007 ″/cy; four tools agree on P-add Venus to
  2e-8 ″/cy). The assertion is PROVISIONAL and reverts to P-add, with the 4
  misses treated as physics discrepancies, if a maintainer reading of the
  Park 2017 text says P-add. Full evidence and values are in the PHY-012
  notes. Cross-term sign: the harness's cross term is the inclusion–exclusion
  second difference; Park17's pair rows are the joint removal minus the two
  single removals, i.e. its negative, and match P-remove's to ≤ 1.2e-4 ″/cy
  in all six pairs (progress review after WP18, R1, post hoc). Reports SHALL
  label which convention a cross-term column uses. BUILD EVIDENCE for the
  harness (m3-newtonian-nbody): ferroastro-diffrun (WP14) checks identical
  per-body IC records and one shared IC hash before any run (FA-E1401 on a
  one-ulp tamper), runs long legs in checkpointed segments (CI-010), and
  combines rates by exact signed sums (crates/ferroastro-diffrun/tests/harness.rs). Part (a): crates/ferroastro-ladder/tests/num013_isolation.rs (WP6; GR
  −6.6e-6, J2 +2.7e-9, LT −6.3e-9 ″/cy at J2 = 2.25e-7, equatorial) and the
  scenario path crates/ferroastro-diffrun/tests/num013a_scenario.rs (WP14;
  DE440 J2, real geometry; GR −1.233e-5, J2 +3.6e-8, LT −2.7e-9 ″/cy), all
  within 10⁻⁴.
```

### NUM-014 — Frame utilities, inertial default

```yaml
id: NUM-014
title: "Frame utilities (ICRF, J2000 ecliptic, mean-orbit, optional ecliptic of date); inertial default"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide frame utilities for ICRF, J2000 ecliptic, the
  Mercury mean-orbit frame and an optional ecliptic of date with IAU 2006
  precession. The default reported rate SHALL be inertial (ICRF-fixed).
acceptance: >-
  The ecliptic-of-date result minus the inertial result ≈ p_A, with both
  measured in the same reference plane. The default output frame is inertial.
  The reference plane is the fixed IAU 2006 mean ecliptic of J2000: the
  inertial longitude is measured from that plane's fixed x-axis, and the
  ecliptic-of-date longitude from the projection of the IAU 2006 equinox of
  date onto it, after rotating the state into the ecliptic of date. The
  fitted rate difference agrees with the linear-fit slope of p_A(t) from an
  ERFA-derived p06e at the same epochs within 1e-3 ″/cy, and with the slope
  of the exact projected-equinox longitude predicted from p06e's π_A, Π_A
  and p_A within 1e-5 ″/cy.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.1 and NUM-014.
parent: null
related: [PHY-015, PHY-017]
notes: >-
  No numeric tolerance was given in the journal. Journal #1 warns that 575.31
  (Mercury mean-orbit plane) + p_A does not reproduce an equinox-referred
  value, so this check must not mix planes. PLANE AND TOLERANCES named
  2026-10-20 (build-evidence sync; WP9 items 1 and 7). The earlier text left
  both open, so this adds precision and loosens nothing. Evidence on
  m3-newtonian-nbody: crates/ferroastro-ladder/tests/num014_ecliptic_of_date.rs
  (offline, SW92 ICs at J2000, 831 passages: difference − p_A slope
  +5.68e-4, difference − exact geometry +8.6e-7 ″/cy; data tier, header.440
  ICs at JDEPOC: +4.431e-4 and +8.42e-7), vectors NUM014-RATE-DIFF-VS-PA
  (1e-3, the WP9 brief's number) and NUM014-RATE-DIFF-VS-GEOMETRY (1e-5,
  ERFA's documented 1 µas agreement of the P03 parameterisations). The 1e-3
  check passes with only ~2× margin, because the moving ecliptic adds a
  second-order projection term ≈ (π_A²/4)·sin 2(Π_A + p_A); the 1e-5
  geometry check carries the discriminating power (a +1e-4 ″/cy mutant of
  the p_A coefficient fails it but passes the 1e-3 check). The expected
  values were computed with pyerfa before the Rust run (non-blind; WP9 item
  2). With this plane the difference depends on Mercury's orbit only through
  the passage epochs, so it tests frames, not dynamics (WP9 review). Measured
  in the moving ecliptic itself (dog-leg) the difference would be off by
  ~2 ″/cy. p_A SOURCE: the check uses ferroastro-erfa's port of ERFA p06e,
  verified against ERFA's t_erfa_c reference values and pyerfa 2.0.1.5
  goldens (bitwise in f64), never the OI-02 snippet value, so OI-02 no
  longer blocks this requirement; it still blocks PHY-017. Not covered:
  velocities rotated into the ecliptic of date omit the frame-rotation term
  (≈ 1e-5 relative; this check uses positions only). The JPL/NAIF
  EclipticJ2000 frame (ECLIPJ2000, no frame bias) is 0.041″ (pole) from the
  IAU 2006 J2000 ecliptic used here and must not be mixed with it.
```

### NUM-015 — Analytic test oracles

```yaml
id: NUM-015
title: "Analytic oracles: J₂ (planar and inclined), SR exact, PPN 1PN, Lense–Thirring"
status: draft
criticality: high
verification: test
statement: >-
  FerroAstro SHALL provide analytic test oracles for: R1 solar J₂ (planar and
  inclined); R2 SR exact 2π[(1−μ²/c²h²)^(−1/2) − 1]; R3
  (2+2γ−β)/3 · 6πμ/(c²p); R3b Lense–Thirring.
acceptance: >-
  Single-body numerical per-orbit advance, averaged over 10³ orbits, matches
  each analytic value to within max(10⁻⁶ relative, 10⁻⁷ ″/cy absolute), or the
  term's effect is measured by differential runs with and without the term from
  identical initial conditions and matched at the same tolerance. The planar R1
  J₂ and R3b LT oracles are used only in strictly equatorial geometry (orbit in
  the solar equatorial plane); otherwise the inclined formula is the oracle.
  BLOCKED by OI-35 (R3b form) and OI-37 (inclined J₂ citation).
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 and NUM-015.
parent: ORG-002
related: [PHY-004, PHY-006, PHY-007, PHY-010, PHY-011, REF-010]
notes: >-
  Inclined J₂ rates ω̇ = 3nJ₂R²/(4p²)(5cos²i − 1), Ω̇ = −3nJ₂R²/(2p²) cos i are
  standard but the citation is UNVERIFIED. R3 form matches IERS Conventions 2010
  Ch. 10 (equation number UNVERIFIED). R3b typography versus DE440 UNVERIFIED.
  Journal #5's example (≥100 orbits, 10⁻⁶ relative) is superseded by this
  requirement's 10³-orbit average. Tolerance revised 2026-10-14 (red-team
  review, Journal #9): 10⁻⁶ relative alone is unreachable for the small terms.
  The LT advance is ~2.4×10⁻¹¹ rad/orbit, so 10⁻⁶ relative needs ω pinned to
  ~2.4×10⁻¹⁴ rad, while f64 Brouwer round-off after ~10⁵ steps is
  ~ε√N/e ≈ 1.7×10⁻¹³ rad. Mercury is ~3.4° from the solar equator, so the
  planar J₂ formula is off by ~0.5% (0.02846 vs 0.02860 ″/cy) unless the test
  geometry is strictly equatorial.
  μ IN THE ORACLES (sync 2026-10-20; WP11, WP13): the per-orbit oracles use
  the μ of the integrated dynamics (GM☉ for the test particle); the mean
  motion that converts them to ″/cy is a separate choice, currently
  n = √((GM☉ + GM☿)/a³) in ferroastro-analytic and ferroastro-ladder
  (RATE_CONVENTION, WP13 F5) but GM☉ in the WP14 scenario harness check. The
  maintainer decided on 2026-10-21 (OI-77 closed) that n = √((GM☉ + GM☿)/a³)
  is used for every ″/cy conversion (details in PHY-004 notes). The
  per-orbit comparison this acceptance makes does not depend on n. OI-35
  (R3b): evidence available, pending human sign-off (PHY-011 notes); R3b
  matches Park 2017 Eq. 2 to 1.25e-6 relative in equatorial geometry
  (passing only through the 1e-7 ″/cy floor; 2.9e-17 rad/orbit absolute,
  26× the measured estimator noise floor) and 7.7e-8 in the real geometry
  (WP4).
```

### NUM-016 — SR rung documents its Hamiltonian

```yaml
id: NUM-016
title: "SR rung documents H = γmc² − μm/r and the model dependence of the SR fraction"
status: draft
criticality: medium
verification: inspection
statement: >-
  The SR rung SHALL document its Hamiltonian H = γmc² − μm/r and SHALL state
  that other SR couplings give different fractions of GR (e.g. 1/3).
acceptance: >-
  Documentation review finds the Hamiltonian and the model-dependence
  statement in the SR rung docs and example.
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R2 and NUM-016; Lemmon &
  Mondragon, arXiv:1012.5438.
parent: ORG-002
related: [PHY-007]
notes: >-
  Attribution of the exact formula to Sommerfeld 1916 is UNVERIFIED.
```

### NUM-017 — 1PN gauge, time argument and units

```yaml
id: NUM-017
title: "All 1PN EOMs in PPN isotropic/harmonic gauge, TDB time, TDB-compatible GM"
status: draft
criticality: high
verification: test
statement: >-
  All 1PN equations of motion SHALL be written in PPN isotropic/harmonic
  gauge, with TDB as time argument and GM in TDB-compatible units. No
  standard-Schwarzschild-coordinate EOM SHALL be used except these labelled
  exemptions: (i) the exact-geodesic oracle; (ii) PHY-008's SR-b areal-gauge
  rung (35.817 ″/cy); (iii) PHY-009's D-term coefficient check, whose
  U(n·v)² term is the areal-gauge form. Exempt runs carry their gauge label in
  the output and are never production EOMs.
acceptance: >-
  The R5 → R3 limit test passes: with massless planets the EIH equations
  reduce to the Schwarzschild test-particle equations, checked symbolically and
  numerically.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.2, §4 R3/R5 and NUM-017.
parent: ORG-002
related: [NUM-018, NUM-020, PHY-005, PHY-008, PHY-009, PHY-014]
notes: >-
  The secular per-orbit precession is gauge-invariant at 1PN, but a, n and
  osculating elements are not. TCB/TDB rate difference L_B ≈ 1.55×10⁻⁸ is
  negligible for ″/cy.
```

### NUM-018 — EIH implementation details

```yaml
id: NUM-018
title: "EIH with Newtonian r̈ⱼ on the RHS, optional fixed-point iteration, relativistic SSB"
status: draft
criticality: high
verification: test
statement: >-
  The EIH implementation SHALL use the Newtonian r̈ⱼ on the right-hand side by
  default, with an optional one-step fixed-point iteration to quantify the
  O(c⁻⁴) difference. In ephemeris mode, initial-condition barycentring SHALL
  use the relativistic SSB with μᵢ*: Σ μᵢ* rᵢ = 0 and the velocity condition
  Σ (μ̇ᵢ* rᵢ + μᵢ* vᵢ) = 0, which includes the time derivative of μᵢ*. This
  is REQUIRED for DE440 reproduction (ephemeris mode) and optional in other
  modes (for example differential runs that use the header.440 initial
  conditions unchanged).
acceptance: >-
  The iterated-minus-default difference is within the expected O(c⁻⁴)
  magnitude. In ephemeris mode with the relativistic SSB (including dμ*/dt),
  SSB drift is at the μm level in DD mode (Subbotin reports 2.6 μm / 40 yr).
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R5 and NUM-018; Park et al.
  2021 Eqs. 1–2; Folkner et al. 2014 (DE430, IPN PR 42-196) p. 12
  (Newtonian r̈ⱼ), Eqs. 2–4 and pp. 5–6; Subbotin et al. 2023 Eq. 5.
parent: NUM-017
related: [NUM-007, PHY-014]
notes: >-
  μ* TRANSCRIPTION (sync 2026-10-20; WP11, WP15 on m3-newtonian-nbody): the
  relativistic barycentre is implemented in ferroastro-system as
  μᵢ* = μᵢ [1 + vᵢ²/(2c²) − (1/(2c²)) Σⱼ≠ᵢ μⱼ/rᵢⱼ], with Σ μᵢ* rᵢ = 0. WP11
  wrote it from memory; the WP15 builder agent compared it with the printed
  Eq. 2 of Park et al. 2021 and found it equal. Tests pin the implementation
  to the transcription:
  crates/ferroastro-system/tests/ref002_header_initial_conditions.rs
  (μᵢ*/μᵢ − 1 = (vᵢ² − Uᵢ)/(2c²) to 8 ulp per body; Σμ*r 7.5e-12 km after two
  rebarycentring passes).
  SIGN-OFF 2026-10-21 (OI-75 closed; maintainer, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280:
  "Include dμ*/dt; scope to ephemeris mode"). μᵢ* is confirmed against
  DE440 Eq. 2 (Park et al. 2021 p. 2, sha256 96c3945d…e8f0ee0; vᵢ is the
  barycentric speed, the sum is over all bodies with finite mass) and DE430
  Eq. 2 (Folkner et al. 2014 p. 4, sha256 76df7694…3ed196f). The source's
  "Park et al. 2021 Eqs. 1–2" is correct (Eq. 1 r_ssbc, Eq. 2 μ*); DE430
  numbers the same content Eqs. 2–4. DE440 states no velocity condition.
  The velocity condition is DE430 Eq. 3, P = d/dt[Σ μ* r] (p. 5), with
  V = P/𝓜 set to zero, equivalently Subbotin et al. 2023 Eq. 5,
  Σ (μ̇ᵢ* rᵢ + μᵢ* ṙᵢ) = 0 (arXiv:2304.04458v1 p. 5, sha256 e65d152a…).
  Neglecting dμ*/dt is not acceptable: DE430 pp. 5–6 state that DE405
  neglected the rate of change of μ* when computing the Sun's initial
  velocity and that its barycentre consequently moved by about
  0.5 m/century, about 10⁵ times this requirement's μm acceptance. The
  earlier text of these notes, which described Σ μᵢ* vᵢ = 0 with dμ*/dt
  neglected, is withdrawn; ferroastro-system's rebarycentre (μ* only in the
  velocity condition) does not comply in ephemeris mode.
  Scope: required in ephemeris mode, where DE440 is reproduced (NUM-007);
  optional elsewhere. Differential runs deliberately keep the header.440
  initial conditions unchanged, so that the two runs of a pair start
  identically (header.440 is already DE440-barycentric).
  BUILD EVIDENCE (m5-performance a8795f85, WP25 N3, IMPLEMENTATION-NOTES):
  ferroastro-eih relativistic_barycentre_with_rate implements the Eq. 5
  condition (mu_star_rate) and is selected by the scenario key
  initial_conditions.barycentre = "relativistic" (opt-in; default none). SSB
  drift over 40 yr: 2.92 µm in double-double (2.9173 µm; mixed 2.9243 µm;
  f64 0.405 mm), against Subbotin's 2.6 µm. This evidence is on a build
  branch and is cited here, not yet merged.
```

### NUM-019 — Solar J₂ reaction and Lense–Thirring for all bodies

```yaml
id: NUM-019
title: "Solar J₂ with reaction on the Sun; solar-spin LT on every non-Sun body, with γ-dependence"
status: draft
criticality: high
verification: test
statement: >-
  Solar J₂ SHALL include the reaction on the Sun (DE440 Eq. 29) and a
  configurable solar pole. The Lense–Thirring acceleration from the solar
  angular momentum (DE440 Eqs. 37–39, equivalently INPOP21a Eq. 1) SHALL be
  applied to every non-Sun body, Major and Minor, with its γ-dependence, and
  its reaction SHALL be applied to the Sun by default (configurable).
acceptance: >-
  Total linear momentum is conserved with J₂ on. J₂ and LT contributions match
  Park17 Table 3 (0.0286 ± 0.0011 and −0.0020 ± 0.0002 ″/cy).
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R1/R3b and NUM-019; Park et
  al. 2021 Eq. 29 and §3.4 Eqs. 37–39; INPOP21a (Fienga et al. 2021, Notes
  Sci. Tech. IMCCE S110) Eq. 1.
parent: null
related: [PHY-010, PHY-011, NUM-013]
notes: >-
  LT FORM (OI-35 closed 2026-10-21; maintainer, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280:
  "Confirm; Sun reaction on; LT all bodies"). The Journal #4 R3b form,
  a_LT = (1+γ)GS/(c²r³)[3(r·ŝ)(r×v)/r² + v×ŝ] with S the total solar
  angular momentum and r, v heliocentric, equals DE440 §3.4 Eqs. 37–39
  (Park et al. 2021 p. 7, sha256 96c3945d…e8f0ee0) and INPOP21a Eq. 1 (p. 5,
  sha256 5fc3b729…a8d67fffb89a5), read from the typeset pages. DE440 does not
  define v; INPOP21a's heliocentric definition is used. Park et al. 2017
  Eq. 2 is the orbit-averaged in-plane rate of this acceleration, the
  analytic rate check of PHY-011, not a check of the form. Decisions: the
  Sun receives the reaction −Σ (GMᵢ/GM☉) a_LT,i (on, the default; Off is the
  literal DE440 reading, which states no reaction), consistent with this
  requirement's momentum acceptance; LT applies to all non-Sun bodies, Minor
  bodies included (DE440: "each body other than the Sun"). The build
  branch's SolarLenseThirring (m4-eih / m5-performance) applies LT to Major
  bodies only, so the Minor-body part is not yet met. A J₂ fitted without LT is biased LOW: the LT precession is
  negative, so the fit absorbs it by lowering J₂. INPOP (2.01→2.218×10⁻⁷ once
  LT is modelled) and DE430 2.11 < DE440 2.196×10⁻⁷ are consistent in sign
  only: the INPOP shift (9.4%) exceeds the ~7% LT share and the releases also
  differ in data and GM☉ (C5), so the full shift is not attributed to LT.
  Wording "LT for all bodies except the Sun" clarified 2026-10-14: it is the
  LT acceleration from solar angular momentum, applied to every non-Sun body.
  Journal #4's original "~7% too
  high" wording was corrected on 2026-10-14. Confirmation against the Park 2021
  text is pending (OI-50).
  BUILD EVIDENCE (sync 2026-10-20; WP9, WP12, WP13 on m3-newtonian-nbody).
  Pole: the default solar pole is the IAU WGCCRE 2015 pole (RA 286.13°,
  Dec 63.87°, from NAIF pck00011.tpc; PHY-010 notes) and is configurable;
  that DE440's J2 term uses this pole is UNVERIFIED (WP12 deviation 4).
  Reaction: SolarJ2WithReaction applies a_Sun = −Σ (μᵢ/μ_Sun) aᵢ
  (crates/ferroastro-forces/src/nbody/tests.rs::solar_j2_reaction_balances_momentum, |Σμa| = 0.100 ε Σμ|a| against 8);
  over 100 yr with Sun + 8 planets + Moon and J2 on, |ΔP|/Σμ|v| = 5.2e-16
  = 0.010 ε√N against 64 ε√N (WP12 check 1a). The form follows this
  requirement's citation and the momentum argument; Eq. 29's number and
  typography were not re-read in the paper (WP12 deviation 5). Bodies acted
  on: J2 acts on the Major bodies other than the Sun; whether DE440 applies
  solar J2 to the asteroids (Minor bodies) was not checked (WP12 deviation 3;
  OI-76). LT is implemented for the test-particle rungs only; N-body
  scenarios refuse lense_thirring with FA-E1201 until it is applied to every
  non-Sun body (WP13), so the LT half of this statement is not yet met.
  Single-body J2/LT vs Park17 Table 3 in the real geometry: 0.0284524 and
  −0.0020148 ″/cy (WP4).
```

### NUM-020 — Second independent check of the EIH transcription

```yaml
id: NUM-020
title: "Independent second check of DE430 Eq. 27 / DE440 §3.1 against the typeset PDFs"
status: draft
criticality: high
verification: inspection
statement: >-
  Before release, the EIH equation (DE430 Eq. 27 / DE440 §3.1) SHALL be
  transcribed and checked a second time, independently, against the typeset
  PDFs. Every UNVERIFIED item in Journal #4 SHALL be tracked as an issue.
acceptance: >-
  A signed review record of the second transcription exists, and each Journal
  #4 UNVERIFIED item in OPEN-ITEMS.md has a linked issue.
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R5 and NUM-020; Folkner et
  al. 2014, IPN PR 42-196; Park et al. 2021 §3.1.
parent: null
related: [PHY-014, PHY-018, NUM-017]
notes: >-
  SIGN-OFF 2026-10-21 (EIH equation; maintainer, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280).
  The maintainer answered "Signed off" ("I reviewed it; Eq. 27 transcription
  in journal R5 and eih.rs matches DE430/DE440. Unblock WP22/23 (with D1: all
  bodies).") after reviewing the agent-prepared checklist, rows 0–17
  (typeset term, Journal #4 R5 line, code line). Pinned code: m4-eih
  78d5a4c7fd7141515d3d9aa6bdfa7218624bad81; crates/ferroastro-eih/src/eih.rs
  blob 9f8a3fb82763f8f3438c3164d8ada28c1be4c247, barycentre.rs blob
  9b43722da143629853327f8b12ae292e709414ff, lib.rs blob
  adc3085c59615ef399106aa0296ffff81ba64531,
  crates/ferroastro-forces/src/nbody/terms.rs blob
  5c4dea6370911c298eb76f28807bdc97d7e2fbca; journal
  docs/strategy/journal/04-numerical-methods.md blob 37a85701…, lines
  269–285. Sources, read from 200 dpi page images, not the text layer:
  DE430 (Folkner et al. 2014, IPN PR 42-196, sha256
  76df76941ccc2602df03daa5bd1a3a0d5a54ba20e313c78f42f572fac3ed196f) p. 12
  (§III.A, Eq. 27 and the paragraph after it), p. 4 (Eqs. 1–2), p. 5
  (Eqs. 3–4); DE440 (Park et al. 2021, sha256
  96c3945deb12e0f0843c2bf0703d3542d950cf85ff903b1fe11a501634f68ee0) p. 5
  (§3.1, Eq. 27), p. 2 (Eqs. 1–2). No sign, coefficient or index difference
  was found between the two PDFs, Journal #4 R5 and eih.rs. If eih.rs,
  barycentre.rs or terms.rs changes blob after this pin, the affected rows
  must be re-checked against the diff. OI-33 is closed by this sign-off.
  The second clause of the statement (a linked issue per Journal #4
  UNVERIFIED item) is not covered by this sign-off.
  Decisions recorded with the sign-off. D1 (body scope): DE430 p. 12 states
  "The summation is over all bodies: Sun, Moon, planets and asteroids", and
  DE440 p. 5 "the summations are over all bodies". The maintainer chose
  "Match DE440: all bodies": the 1PN EIH acceleration is evaluated for every
  body (Sun, planets, the Moon, asteroids and KBOs), each with its own 1PN
  terms, with the sums over all bodies. This supersedes the Majors-only
  interpretation at the pin (eih.rs Tier::Major); see PHY-014 and NUM-009.
  The maintainer's ruling on #13 (2026-10-21,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/pull/13#issuecomment-6306750993)
  repeats it: the Moon and the asteroids are required, and #17 does not
  merge with a Majors-only EIH. D2 (provenance of the Newtonian
  r̈ⱼ substitution): DE430 p. 12 states both the summation scope and that
  using the Newtonian acceleration in the two aⱼ terms is accurate to
  O(c⁻²); DE440 Eq. 27 and its text say neither. The citation for NUM-018's
  default is therefore DE430 p. 12 (crates/ferroastro-eih/TRANSCRIPTION.md
  T12/T13 should be corrected on the build branch). D3 (geocentric Moon):
  Eih accumulates the Earth and Moon corrections barycentrically, and the
  Moon − Earth variable receives their difference, while NewtonianMutual
  skips the Earth–Moon pair, which GeocentricMoon supplies. This was judged
  consistent (the 1PN correction of the relative acceleration is the
  difference of the two barycentric corrections); it is noted for code
  review of the WP22/WP23 wiring, not as a transcription issue.
```

### NUM-021 — SIMD force kernels with bitwise reproducibility

```yaml
id: NUM-021
title: "SoA SIMD force kernels, exact 1/r³, fixed reduction order, bitwise reproducible"
status: draft
criticality: medium
verification: test
statement: >-
  Force kernels SHALL use a structure-of-arrays layout with SIMD (f64x4/x8),
  an exact 1/r³ (≤ 1 ulp), a fixed reduction order and an explicit FMA policy,
  and SHALL be bitwise reproducible across thread counts.
acceptance: >-
  A CI test shows bitwise-identical results across thread counts. Target:
  speedup ≥ 2.5× versus scalar at N = 16.
source: >-
  docs/strategy/journal/04-numerical-methods.md §5 and NUM-021.
parent: null
related: [TST-005, STD-003, NUM-023]
notes: >-
  The 2.5× speedup target has UNVERIFIED feasibility. Stable SIMD crate APIs
  (wide, pulp) are UNVERIFIED; std::simd is nightly. Any unsafe SIMD code needs
  an ADR (STD-003).
```

### NUM-022 — Run-level parallelism

```yaml
id: NUM-022
title: "Run-level parallelism (rayon) and optional SIMD batch-lane integration"
status: draft
criticality: low
verification: test
statement: >-
  FerroAstro SHALL provide run-level parallelism (rayon) for ensembles and β/γ
  sweeps, plus optional SIMD batch-lane integration of independent initial
  conditions.
acceptance: >-
  Throughput scales linearly with core count when the number of runs is at
  least the number of cores.
source: >-
  docs/strategy/journal/04-numerical-methods.md §5 and NUM-022.
parent: null
related: [NUM-021]
notes: >-
  The within-step threading crossover and heyoka's batch mode are UNVERIFIED.
  BUILD STATUS (sync 2026-10-20; WP13 on m3-newtonian-nbody): run_batch in
  ferroastro-propagate (rayon behind the parallel feature) is implemented, and
  crates/ferroastro-propagate/tests/scenario_runs.rs::same_scenario_twice_and_batch_thread_counts_are_bitwise asserts that a
  batch of 8 runs gives bitwise-identical results with 1 and 8 threads and
  equals the single runs. The linear-scaling acceptance is NOT asserted and
  NOT met yet: the build machine is shared with other users' jobs, so a
  scaling measurement there is not evidence, and CI-008 confines speed
  claims to the one recorded machine under controlled load. The single
  measurement (4.57 s on 1 thread vs 1.04 s on 8, speed-up 4.38 against an
  ideal of 4.5 for that mix of run lengths) is reported only (WP13 deviation
  9). The acceptance is unchanged; SIMD batch lanes are not built.
```

### NUM-023 — Round-off diagnostics

```yaml
id: NUM-023
title: "Round-off diagnostics: Brouwer-law slope fits and forward–backward reversibility"
status: draft
criticality: medium
verification: test
statement: >-
  FerroAstro SHALL provide round-off diagnostics: energy, angular-momentum and
  SSB drift versus t with a Brouwer-law slope fit (expected ½ and 3/2), and the
  forward–backward reversibility error.
acceptance: >-
  In NUM-001 f64 runs the fitted slopes are within ±0.1 of the Brouwer values.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.4 and NUM-023; Brouwer 1937,
  AJ 46, 149.
parent: null
related: [NUM-001, NUM-003]
notes: >-
  MAINTAINER RULING 2026-10-21 (on #13,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/pull/13#issuecomment-6306750993):
  NUM-023 BLOCKS M3 and #13. #13 does not merge until NUM-023 passes with an
  asserting test at the long-ensemble length (≥ 5e7 steps), where the build
  branch measured an energy slope of 0.629 against ½ ± 0.1. A known-failure
  guard (nightly_num023_long_ensemble_known_failure) no longer satisfies
  this requirement. If the fix is the pc_b_from_g predictor, it must become
  the asserted default path, not a report-only knob. The ±0.1 tolerance is
  not widened. Evidence history: docs/strategy/IMPLEMENTATION-NOTES.md
  (WP3 NUM-023 section; the NUM-023 investigation).
```

### NUM-024 — Benchmark suite against Subbotin 2023

```yaml
id: NUM-024
title: "Benchmark: 40-yr EIH 16-major + 343-minor run in f64 and mixed DD"
status: draft
criticality: medium
verification: demonstration
statement: >-
  FerroAstro SHALL maintain a benchmark suite with a 40-yr EIH run of 16 Major
  + 343 Minor bodies in f64 and in mixed DD, and SHALL publish wall times
  against Subbotin's 81 s (f64) and 246 s (mixed DD), hardware-normalised.
acceptance: >-
  Criterion benchmarks run in CI (CI-008 conditions). Target: within 1× of
  81 s / 246 s on comparable hardware.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.4, §5 and NUM-024; Subbotin
  et al. 2023 Table 1.
parent: null
related: [CI-008, NUM-007, NUM-009, DEP-014]
notes: >-
  Subbotin's CPU is unspecified in what was read, so hardware normalisation is
  UNVERIFIED. Reference speed ≈ 0.35 ms/step (233,760 steps).
```

### NUM-025 — Taylor-series integrator (stretch)

```yaml
id: NUM-025
title: "Stretch: Taylor-series integrator with AD expression graph and variational equations"
status: draft
criticality: low
verification: test
statement: >-
  (Stretch goal.) FerroAstro SHALL provide a Taylor-series integrator with an
  automatic-differentiation expression graph (heyoka/TaylorIntegration.jl
  class) supporting variational equations.
acceptance: >-
  Agrees with NUM-001 to round-off, and passes an Apophis 2029 encounter test.
source: >-
  docs/strategy/journal/04-numerical-methods.md §1.2 and NUM-025; Biscani &
  Izzo 2021, MNRAS 504, 2614; Pérez-Hernández & Benet 2022.
parent: null
related: [NUM-001]
notes: >-
  The jet cost factor is UNVERIFIED. The Apophis acceptance has no numeric
  tolerance in the journal yet.
```
