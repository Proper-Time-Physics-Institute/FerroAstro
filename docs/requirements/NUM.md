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
  benchmark is unchanged relative to a non-generic f64 baseline.
source: >-
  docs/strategy/journal/04-numerical-methods.md §2 and NUM-006.
parent: null
related: [DEP-007, DEP-012, API-001]
notes: >-
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
  acceleration with at least 4 terms.
acceptance: >-
  Lunar round-off ≤ 2 mm over 40 yr in NUM-007 mode.
source: >-
  docs/strategy/journal/04-numerical-methods.md §2 item 2 and NUM-008.
parent: NUM-007
related: [PHY-014]
notes: >-
  |r_M − r_E| is ~400× smaller than barycentric vectors [V]; DE, EPM and INPOP
  all integrate the Moon geocentrically.
```

### NUM-009 — Body tiering

```yaml
id: NUM-009
title: "Body tiering: Major (EIH, mutual), Minor (Newtonian from Majors), Ring (fixed masses)"
status: draft
criticality: medium
verification: test
statement: >-
  Body tiering SHALL be a first-class model setting: Major bodies interact
  mutually under EIH; Minor bodies feel Newtonian forces from the Majors only
  and do not interact with each other; Ring bodies are fixed point masses.
acceptance: >-
  With the DE440 asteroid set (343 Minor bodies), the run costs < 2× the
  16-Major-body run.
source: >-
  docs/strategy/journal/04-numerical-methods.md §5 and NUM-009; Subbotin et al.
  2023; Folkner et al. 2014, IPN PR 42-196.
parent: null
related: [NUM-024, PHY-014]
notes: >-
  Tiering costs < 1 cm (planets) / < 1 mm (Moon) over 40 yr [V]. Interaction
  count 16×343 + 120 ≈ 5.6×10³ versus ~6.4×10⁴ (derived here). EIH cost per pair
  ~3–5× Newtonian is UNVERIFIED.
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
  times match the analytic values to round-off.
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.2 and NUM-010.
parent: null
related: [NUM-011]
notes: null
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
  formulas within 10⁻⁴ ″/cy; (b) N-body differential contributions, using the
  PROVISIONAL protocol Sun + Mercury + planet X minus Sun + Mercury (Newtonian)
  as in PHY-012, agree with the Park 2017 Table 3 values within ±0.005 ″/cy,
  with interaction cross terms reported separately. Part (b) is BLOCKED until
  OI-38 confirms Park 2017's switch-on/switch-off protocol.
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
  comparison basis SHALL be documented. Park's switch-on/off protocol is
  UNVERIFIED (OI-38); without a fixed protocol part (b) is ill-posed, because
  cross terms (up to 0.0411 ″/cy) exceed the tolerance (PHY-012 notes).
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
source: >-
  docs/strategy/journal/04-numerical-methods.md §3.1 and NUM-014.
parent: null
related: [PHY-015, PHY-017]
notes: >-
  BLOCKED on p_A = 5028.796195 ″/cy (IAU 2006, Cap03), UNVERIFIED. No numeric
  tolerance was given in the journal. Journal #1 warns that 575.31 (Mercury
  mean-orbit plane) + p_A does not reproduce an equinox-referred value, so this
  check must not mix planes.
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
  O(c⁻⁴) difference. Initial-condition barycentring SHALL use the relativistic
  SSB with μᵢ*.
acceptance: >-
  The iterated-minus-default difference is within the expected O(c⁻⁴)
  magnitude. SSB drift is at the μm level in DD mode (Subbotin reports
  2.6 μm / 40 yr).
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R5 and NUM-018; Park et al.
  2021 Eqs. 1–2.
parent: NUM-017
related: [NUM-007, PHY-014]
notes: null
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
  angular momentum SHALL be applied to every non-Sun body, with its
  γ-dependence.
acceptance: >-
  Total linear momentum is conserved with J₂ on. J₂ and LT contributions match
  Park17 Table 3 (0.0286 ± 0.0011 and −0.0020 ± 0.0002 ″/cy).
source: >-
  docs/strategy/journal/04-numerical-methods.md §4 R1/R3b and NUM-019; Park et
  al. 2021 Eq. 29.
parent: null
related: [PHY-010, PHY-011, NUM-013]
notes: >-
  The LT form used here (IERS-style, Journal #4 R3b) and PHY-011's INPOP21a
  Eq. 1 form SHALL be shown to agree; R3b typography versus DE440 is
  UNVERIFIED. A J₂ fitted without LT is biased LOW: the LT precession is
  negative, so the fit absorbs it by lowering J₂. INPOP (2.01→2.218×10⁻⁷ once
  LT is modelled) and DE430 2.11 < DE440 2.196×10⁻⁷ are consistent in sign
  only: the INPOP shift (9.4%) exceeds the ~7% LT share and the releases also
  differ in data and GM☉ (C5), so the full shift is not attributed to LT.
  Wording "LT for all bodies except the Sun" clarified 2026-10-14: it is the
  LT acceleration from solar angular momentum, applied to every non-Sun body.
  Journal #4's original "~7% too
  high" wording was corrected on 2026-10-14. Confirmation against the Park 2021
  text is pending (OI-50).
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
notes: null
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
notes: null
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
