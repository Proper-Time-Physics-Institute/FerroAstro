<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# PHY — Physics truth data for the Mercury perihelion ladder

**Scope.** These requirements fix the expected value, tolerance, and primary source of every rung of the Mercury perihelion precession ladder (ORG-002). They also fix the canonical constants, the metadata every reported rate must carry, and the rule that no secondary or unverified value becomes a test oracle. Source: Journal #1 (`docs/strategy/journal/01-physics-truth-data.md`). The headline number is Park et al. (2017)'s **575.3100 ± 0.0015 ″/cy** in Mercury's mean-orbit frame. The ≈5600 ″/cy equinox-referred figure appears only as a constructed historical comparison (PHY-017). All tolerances are in arcseconds per Julian century (″/cy) unless stated otherwise.

Evidence labels carried over from the journal: **[P]** read in the primary source, **[C]** computed from [P] constants, **[S]** secondary transcription, **UNVERIFIED** not yet confirmed. See [OPEN-ITEMS.md](OPEN-ITEMS.md) for everything that still has to be checked.

Rungs in the Journal #1 table that have no dedicated requirement yet: N-SELF (0.0050, meaning UNVERIFIED), N-AST (0.0012), N-X cross terms (Σ = +0.0127), N-MOON (no primary value), J2-DE440 (0.02792, covered inside PHY-010) and GR-1PN-2B (finite-mass-ratio shift ≈ 7×10⁻⁶ ″/cy, covered by the μ note in PHY-001/PHY-004). N-SELF, N-AST and N-X enter PHY-013 as part of the N-SUM total.

---

### PHY-001 — Rung N-2B: Newtonian two-body shows zero precession

```yaml
id: PHY-001
title: "Rung N-2B: Newtonian Sun + Mercury shows zero perihelion precession"
status: draft
criticality: high
verification: test
statement: >-
  A Newtonian Sun + Mercury point-mass integration (rung N-2B) SHALL produce a
  fitted perihelion precession rate indistinguishable from zero. Any nonzero
  fitted rate SHALL be reported as integrator error, not as physics.
acceptance: >-
  Over an integration span of at least 1000 yr, the fitted |ϖ̇| < 1×10⁻⁶ ″/cy.
  The test output labels any residual rate as numerical error.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 (rung N-2B, analytic closed
  Kepler ellipse) and Requirements implied PHY-001.
parent: ORG-002
related: [NUM-001, NUM-015, NUM-023]
notes: >-
  Caveat C7: the gravitational parameter may be μ = GM☉ + GM_Mercury or GM☉
  alone, but the choice SHALL be documented. The two-body mass correction
  (≈7×10⁻⁶ ″/cy) does not affect this rung, which is exactly zero either way.
```

### PHY-002 — DE440 canonical constants table

```yaml
id: PHY-002
title: "Precision rungs use DE440 constants from a single versioned, cited table"
status: draft
criticality: high
verification: test
statement: >-
  All precision rungs SHALL use the DE440 constants GMS, CLIGHT, AU, the
  planetary GMs, EMRAT, ASUN, J2SUN, MOISUN and OMGSUN exactly as published.
  The constants SHALL be loaded from a single versioned constants table in
  which every entry records its source citation.
acceptance: >-
  A unit test reads the constants table and asserts
  GM☉ = 132712440041.279419 km³ s⁻² (= 2.95912208284119561×10⁻⁴ au³ d⁻²) and
  c = 299792.458 km/s. The test also asserts au = 149597870.700 km,
  ASUN = 696000 km, MOISUN = 0.06884, OMGSUN = 0.247564432907 rad/day and
  EMRAT = 81.3005682214972154. Every table entry has a non-empty citation
  (DE440hdr or Park21). The table matches header.440 entry by entry (REF-002).
source: >-
  docs/strategy/journal/01-physics-truth-data.md §4 and PHY-002; Park et al.
  2021, AJ 161, 105, doi:10.3847/1538-3881/abd414; DE440 ASCII header.440
  (groups 1040/1041).
parent: ORG-002
related: [REF-002, PHY-003, TST-002, TST-003]
notes: >-
  Overlap with REF-002: REF-002 is authoritative for HOW the constants are
  obtained (parsed from header.440, never hand-transcribed). PHY-002 is
  authoritative for WHICH constants are used and the asserted values. The
  versioned table SHALL be generated from header.440. UNVERIFIED: the units of
  GMSDOT (−9.0×10⁻¹⁴, presumably au³/d² per year); the IAU 2012 Resolution B2
  text behind the exact au; whether the DE440 paper quotes a GM☉ uncertainty or a
  J2 rationale (text past character 100k not yet read). Planetary GMs (km³ s⁻²):
  Mercury 22031.868551, Venus 324858.592, Earth 398600.435507, Moon 4902.800118,
  Mars 42828.375816, Jupiter 126712764.1, Saturn 37940584.8418, Uranus 5794556.4,
  Neptune 6836527.10058, Pluto 975.5 (Park21 [P]).
```

### PHY-003 — IAU 2015 nominal values are quarantined

```yaml
id: PHY-003
title: "IAU 2015 B3 nominal values only in a separate nominal namespace"
status: draft
criticality: high
verification: test
statement: >-
  IAU 2015 Resolution B3 nominal values SHALL be available only under a
  separate `nominal` namespace, and ladder rungs SHALL NOT use them.
acceptance: >-
  A test asserts that no ladder-rung code path references the `nominal`
  namespace. Nominal values present: (GM)☉ᴺ = 1.3271244×10²⁰ m³ s⁻²,
  R☉ᴺ = 6.957×10⁸ m, (GM)ᴇᴺ = 3.986004×10¹⁴ m³ s⁻²,
  (GM)ᴊᴺ = 1.2668653×10¹⁷ m³ s⁻².
source: >-
  docs/strategy/journal/01-physics-truth-data.md §4 and PHY-003; Prša et al.
  2016, AJ 152, 41, doi:10.3847/0004-6256/152/2/41, Table 1.
parent: ORG-002
related: [PHY-002, TST-002]
notes: >-
  These are conversion constants, not best estimates. R☉ᴺ = 695700 km differs
  from the DE440 ASUN = 696000 km used for J2. Journal #5's illustrative vector
  file tagged the nominal GM☉ with requirement PHY-001; the correct tag is
  PHY-003.
```

### PHY-004 — Rung GR-SCHW, analytic

```yaml
id: PHY-004
title: "Rung GR-SCHW analytic: 6πGM/(c²a(1−e²)) gives 42.9805 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  The analytic Schwarzschild 1PN perihelion advance 6πGM/(c²a(1−e²)) per orbit,
  evaluated with the DE440 GM☉, c and au and the SW92 J2000 mean elements of
  Mercury, SHALL be computed and reported as rung GR-SCHW (analytic).
acceptance: >-
  With GM☉ = 132712440041.279419 km³ s⁻², c = 299792.458 km/s,
  au = 149597870.700 km, a = 0.38709927 au, e = 0.20563593 and 415.2009 orbits
  per Julian century (period from GM☉ + GM_Mercury), the result is
  5.01866×10⁻⁷ rad/orbit and 42.9805 ± 0.0001 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 [C] and PHY-004; cross-check
  Park et al. 2017, AJ 153, 121, doi:10.3847/1538-3881/aa5be2, Table 3
  (42.9799 ± 0.0009); Will 2014, Living Rev. Relativ. 17, 4, Eq. 65.
parent: ORG-002
related: [PHY-005, PHY-006, NUM-015, NUM-017, REF-010]
notes: >-
  The 0.0006 ″/cy gap to Park17 comes from mean versus osculating a and e.
  Journal #4 §4 quotes 42.981 ″/cy computed from different elements
  (a = 0.38709893 au, e = 0.20563069); the ±0.0001 tolerance here holds only for
  the SW92 elements named above. The μ choice (GM☉ alone or GM☉ + GM_Mercury)
  SHALL be documented (caveat C7).
```

### PHY-005 — Rung GR-SCHW, numerical

```yaml
id: PHY-005
title: "Rung GR-SCHW numerical: 1PN test-particle integration reproduces 42.980 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  Integrating the 1PN Schwarzschild test-particle equations of motion with
  γ = β = 1 and fitting the secular perihelion rate SHALL reproduce the GR-SCHW
  value.
acceptance: >-
  The fitted rate is 42.980 ± 0.001 ″/cy and differs from the PHY-004 analytic
  value by at most 0.001 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md PHY-005; Park et al. 2017
  Table 3 (42.9799 ± 0.0009).
parent: ORG-002
related: [PHY-004, NUM-001, NUM-013, NUM-015, NUM-017]
notes: >-
  Equations of motion in PPN isotropic/harmonic gauge (NUM-017, Journal #4 R3).
```

### PHY-006 — Rung PPN: (2+2γ−β)/3 scaling

```yaml
id: PHY-006
title: "Rung PPN: GR-SCHW rate scales as (2+2γ−β)/3"
status: draft
criticality: high
verification: test
statement: >-
  With β and γ as free parameters, the GR-SCHW precession rate SHALL scale as
  (2+2γ−β)/3 relative to the γ = β = 1 value.
acceptance: >-
  For |γ−1| ≤ 10⁻³ and |β−1| ≤ 10⁻³ the numerical rate matches
  42.98 × (2+2γ−β)/3 to within 10⁻⁶ relative. Example check: γ = 1,
  β = 1 + 3.9×10⁻⁵ shifts the rate by −0.00056 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung PPN-βγ and PHY-006;
  Will 2014, Living Rev. Relativ. 17, 4, Eq. 65.
parent: ORG-002
related: [REF-010, NUM-015, NUM-017]
notes: >-
  No external N-body code exposes PPN β/γ (Journal #3), so this rung is
  validated analytically only (REF-010). Sensitivity scale: β ±0.00056 ″/cy per
  Park17 σ_β; γ ±0.00066 ″/cy per Cassini σ_γ.
```

### PHY-007 — Rung SR-a: special-relativistic Kepler gives GR/6

```yaml
id: PHY-007
title: "Rung SR-a: SR kinetic energy in a Newtonian potential gives GR/6 = 7.1634 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  Rung SR-a SHALL use the Lagrangian L = −mc²√(1−v²/c²) + GMm/r (equivalently
  H = γmc² − μm/r) and SHALL name it in its output. Both the numerical rate and
  the analytic per-orbit value πGM/(c²p) SHALL be checked.
acceptance: >-
  Numerical ϖ̇ = GR-SCHW/6 = 7.1634 ± 0.001 ″/cy, and the analytic per-orbit
  advance πGM/(c²p) evaluated with PHY-004 inputs agrees with it.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung SR-a, §5 and PHY-007;
  Lemmon & Mondragon, arXiv:1012.5438 v1 (1/6 factor).
parent: ORG-002
related: [NUM-015, NUM-016, PHY-009]
notes: >-
  Model-dependent: LM10 v6 claims 1/3 once a relativistic potential energy is
  added; Trump & Schieve (arXiv:physics/9805032) obtain zero in Stueckelberg
  covariant mechanics. The attribution of 1/6 to Sommerfeld and to a Goldstein
  Ch. 7 problem is UNVERIFIED. NUM-015 adds the exact-formula check
  2π[(1−μ²/c²h²)^(−1/2) − 1] at 10⁻⁶ relative.
```

### PHY-008 — Rung SR-b: gauge-labelled velocity-only 1PN terms

```yaml
id: PHY-008
title: "Rung SR-b: velocity-dependent 1PN terms only, labelled by gauge"
status: draft
criticality: high
verification: test
statement: >-
  Rung SR-b output SHALL be labelled with its gauge. The documentation SHALL
  state that the split into velocity-dependent terms is not an observable and
  that SR-b is pedagogical, never "the SR part of GR".
acceptance: >-
  Isotropic/harmonic gauge: 7/6 × GR = 50.144 ± 0.002 ″/cy. Schwarzschild areal
  gauge: 5/6 × GR = 35.817 ± 0.002 ″/cy. A test rejects SR-b output that lacks
  a gauge label. The docs contain the not-an-observable statement.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung SR-b, §5 [C] and
  PHY-008.
parent: ORG-002
related: [PHY-009, PHY-015]
notes: >-
  Journal values: 50.1439 (harmonic) and 35.8171 (areal). Only the full 1PN sum
  (2+2γ−β)/3 × 42.98 is coordinate-independent.
```

### PHY-009 — Term-by-term 1PN coefficient check

```yaml
id: PHY-009
title: "Each 1PN Lagrangian coefficient A, B, C, D reproduces 1, 4, 2, 2 units"
status: draft
criticality: high
verification: test
statement: >-
  For the test-particle Lagrangian L = ½v² + U + ε[A v⁴ + B U v² + C U² +
  D U (n·v)²], each coefficient switched on alone SHALL reproduce its
  per-orbit advance in units of πGM/(c²p).
acceptance: >-
  A, B, C and D individually give 1, 4, 2 and 2 × πGM/(c²p) per orbit (A with
  8A = 1) to within 10⁻³ relative.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §5 [C] and PHY-009 (numerically
  confirmed in-session with sympy + DOP853 at e = 0.2, ε = 10⁻⁴: 1.000, 3.999,
  2.000, 2.000).
parent: ORG-002
related: [PHY-007, PHY-008]
notes: null
```

### PHY-010 — Rung J2: solar quadrupole

```yaml
id: PHY-010
title: "Rung J2: solar oblateness gives 0.02860 ″/cy at J2 = 2.25×10⁻⁷"
status: draft
criticality: high
verification: test
statement: >-
  Rung J2 SHALL be checked against the analytic per-orbit advance
  3πJ₂R☉²/p² with R☉ = 696000 km, and the numerical rung SHALL match it. The
  solar pole orientation used SHALL be documented.
acceptance: >-
  Analytic: 0.02860 ″/cy at J2 = 2.25×10⁻⁷ and 0.02792 ″/cy at the DE440
  J2 = 2.19613915165298248×10⁻⁷. Numerical rung matches the analytic value
  within ±0.0005 ″/cy. Pole orientation is recorded in the output metadata.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rungs J2 / J2-DE440 and
  PHY-010; Park et al. 2017 (0.0286 ± 0.0011).
parent: ORG-002
related: [NUM-013, NUM-015, NUM-019, PHY-015]
notes: >-
  The planar formula applies to an orbit in the solar equatorial plane; the
  solar equator is inclined ~3.4° to Mercury's mean orbit (Journal #4 §3.1).
  NUM-015 adds the inclined-orbit check (citation for the inclined formula
  UNVERIFIED). NUM-013 asserts the differential-run value within 10⁻⁴; see the
  NUM-013 notes for how the two tolerances relate.
```

### PHY-011 — Rung LT: solar Lense–Thirring

```yaml
id: PHY-011
title: "Rung LT: solar Lense–Thirring gives −0.0020 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  Rung LT SHALL use S☉ = 1.90×10⁴¹ kg m² s⁻¹ (DE440 MOISUN × M☉R☉² × OMGSUN)
  and the INPOP21a Eq. 1 acceleration.
acceptance: >-
  The rung yields −0.0020 ± 0.0002 ″/cy. The analytic check
  ϖ̇ = −4GS/(c²a³(1−e²)^(3/2)) at I ≈ 0 gives −0.00202 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung LT and PHY-011; Park
  et al. 2017; Park et al. 2021; INPOP21a (Fienga et al. 2021, Notes Sci. Tech.
  IMCCE S110) Eq. 1.
parent: ORG-002
related: [REF-010, NUM-013, NUM-015, NUM-019]
notes: >-
  Journal #4 R3b gives an IERS-style LT acceleration whose typography relative
  to the DE440 equation is UNVERIFIED. The INPOP21a Eq. 1 form named here and the
  NUM-019 implementation SHALL be shown to agree. LT is ~7% of the oblateness
  precession (Park21); INPOP21a says ~10% of the oblateness acceleration.
```

### PHY-012 — Rungs N-VEN … N-NEP: single-planet contributions

```yaml
id: PHY-012
title: "Single-planet Newtonian contributions reproduce Park17 Table 3"
status: draft
criticality: high
verification: test
statement: >-
  Each single-planet Newtonian contribution to Mercury's perihelion rate
  (rungs N-VEN through N-NEP) SHALL reproduce the Park17 Table 3 value when
  computed with the Park17 procedure: Mercury mean-orbit frame, 1000–3000 CE
  span, quadratic + periodic fit.
acceptance: >-
  Each value within ±0.005 ″/cy of: Venus 277.4176, Earth/Moon 90.8881,
  Mars 2.4814, Jupiter 153.9899, Saturn 7.3227, Uranus 0.1425,
  Neptune 0.0424.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 and PHY-012; Park et al.
  2017, AJ 153, 121, Table 3 [P].
parent: ORG-002
related: [NUM-011, NUM-013, CI-009]
notes: >-
  AUTHORITATIVE over NUM-013 for expected values and tolerances of the
  per-planet contributions; NUM-013 is authoritative for the differential-run
  harness that produces them. Park's exact switch-on/switch-off protocol is
  UNVERIFIED (Journal #4 §3.1), and cross terms reach 0.0411 (J+S) and −0.0209
  (V+E/M), so the decomposition definition SHALL be documented. Clemence 1947
  per-planet values are [S] and are not asserted. Journal #5's illustrative
  example used "PHY-012" for the 1PN Schwarzschild advance; that example is
  superseded: the Schwarzschild rung is PHY-004/PHY-005.
```

### PHY-013 — Rung N-SUM: full Newtonian N-body

```yaml
id: PHY-013
title: "Rung N-SUM: full Newtonian N-body rate is 532.30 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  The full Newtonian N-body rate, including asteroids and cross terms, SHALL be
  reproduced in the Park17 frame. A comparison against Clemence 1947 SHALL be
  shown for information only and SHALL NOT be asserted.
acceptance: >-
  532.30 ± 0.01 ″/cy in the Park17 mean-orbit frame (sum of Park17 rows:
  532.3035 [C]).
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung N-SUM and PHY-013;
  Park et al. 2017 Table 3; Clemence 1947, Rev. Mod. Phys. 19, 361 [S].
parent: ORG-002
related: [PHY-012, NUM-011]
notes: >-
  Clemence planets-only value 531.5 ± 0.7 (531.504; 531.535 = 5557.18 − 5025.645)
  is [S] until the RMP scan is checked. Clemence's "Earth" probably meant the
  Earth–Moon system with 1947 masses (UNVERIFIED). The Park17 "Mercury" self
  row (0.0050) is included in the sum but its meaning is UNVERIFIED (C6).
  Literature Newtonian values range 526.7–532.4 ″/cy depending on definition
  (Pogossian 2022).
```

### PHY-014 — Rung EIH-FULL: DE440-equivalent physics

```yaml
id: PHY-014
title: "Rung EIH-FULL: DE440-equivalent physics reproduces 575.310 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  DE440-equivalent physics (EIH N-body with γ = β = 1, solar J2, Lense–Thirring,
  343 asteroids, 30 KBOs and the 36-point KBO ring) SHALL reproduce Park17's
  total secular perihelion rate in the Park17 mean-orbit frame.
acceptance: >-
  575.310 ± 0.005 ″/cy (Park17: 575.3100 ± 0.0015). The tolerance is 3× Park's
  σ plus margin for the DE430-to-DE440 model difference.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §1, §2 rung EIH-FULL and
  PHY-014; Park et al. 2017 Table 3; Park et al. 2021 (body set).
parent: ORG-002
related: [NUM-011, NUM-017, NUM-020, CI-007, REF-001, REF-003]
notes: >-
  AUTHORITATIVE tolerance for the 575.31 headline; NUM-011 quotes ±0.01 as the
  estimator-level acceptance. The PHY-014 journal text names "343 asteroids, KBO
  ring"; §4 of the same journal lists 30 KBOs plus the ring, so both are
  included here. Frame pole: α ≈ 280.9876°, δ ≈ 61.4481° (ICRF). Fit model
  B + ϖ̇t + Qt² + 14 periodic terms, Q ≈ −0.04478132 ″/cy². Depends on the R5 EIH
  transcription check (NUM-020) and on asteroid initial conditions (REF-003,
  UNVERIFIED).
```

### PHY-015 — Mandatory metadata on every reported rate

```yaml
id: PHY-015
title: "Every reported ϖ̇ carries frame, epoch, fit span, fit model and constants version"
status: draft
criticality: high
verification: test
statement: >-
  Every reported perihelion rate SHALL carry metadata stating the frame
  (Mercury mean-orbit, J2000 ecliptic, or equinox of date), epoch, fit span,
  fit model and constants version.
acceptance: >-
  A test rejects any rung output missing any of the five metadata fields.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §1 frame notes, caveat C1 and
  PHY-015.
parent: ORG-002
related: [NUM-012, CI-003, PHY-008]
notes: >-
  5599.74 (moving equinox), 577.72 (fixed J2000 ecliptic) and 575.31 (Mercury
  mean-orbit plane) are each correct only in their own frame. NUM-012 adds the
  estimator definition to the label set.
```

### PHY-016 — JPL Horizons cross-check

```yaml
id: PHY-016
title: "Horizons DE441 cross-check of the EIH-FULL rate within 0.01 ″/cy"
status: draft
criticality: medium
verification: test
statement: >-
  A fetcher SHALL retrieve DE441 Mercury osculating elements
  (COMMAND='199', CENTER='500@10') or state vectors from the JPL Horizons API
  for at least 500 yr. FerroAstro's EIH-FULL propagation SHALL be compared with
  it after both are put in the same frame and fitted the same way.
acceptance: >-
  |ϖ̇(FerroAstro) − ϖ̇(Horizons)| ≤ 0.01 ″/cy over a ≥ 500 yr span, same frame,
  same fit. The test records the Horizons API version (signature.version) and
  the ephemeris source string.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §6 and PHY-016; JPL Horizons
  API documentation v1.3, https://ssd-api.jpl.nasa.gov/doc/horizons.html.
parent: ORG-002
related: [PHY-014, NUM-011, NUM-014]
notes: >-
  Journal #1's live call (2026-10-14) returned source DE441; Journal #3 lists
  "which DE Horizons uses now" as UNVERIFIED. The recorded source string
  resolves this per run. To compare with Park17, rotate vectors into Mercury's
  mean-orbit frame before forming ϖ. Use μ = GMS + GM1
  (2.9591225740912142×10⁻⁴ au³/d²) when converting state vectors. Rate limits
  are undocumented, so responses are cached.
```

### PHY-017 — Equinox-referred showcase number

```yaml
id: PHY-017
title: "The ≈5600 ″/cy figure is built explicitly from a cited precession model"
status: draft
criticality: medium
verification: inspection
statement: >-
  When the 5600 ″/cy-class equinox-referred number is displayed, it SHALL be
  constructed explicitly by adding a cited general-precession model (IAU 2006
  p_A). It SHALL NOT be compared numerically with Clemence's 5599.74 ± 0.41
  beyond that stated uncertainty.
acceptance: >-
  Inspection of the report shows the construction, the cited p_A source and no
  numerical assertion tighter than ±0.41 ″/cy against Clemence.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §1 and PHY-017; Clemence 1947
  [S]; Capitaine, Wallace & Chapront 2003, A&A 412, 567 (Cap03).
parent: ORG-002
related: [NUM-014, PHY-018]
notes: >-
  BLOCKED: the IAU 2006 p_A linear term 5028.796195 ″/cy is UNVERIFIED (search
  snippet only) and Clemence Table II values are [S]. Adding 575.31 and 5028.80
  does NOT reproduce a modern equinox-referred value, because the two refer to
  different planes; that sum must never be used as a test.
```

### PHY-018 — No unverified value becomes a test oracle

```yaml
id: PHY-018
title: "[S] and UNVERIFIED values are confirmed against primary sources before use as oracles"
status: draft
criticality: high
verification: inspection
statement: >-
  Before any value marked [S] or UNVERIFIED in Journal #1 is used as a test
  oracle, it SHALL be confirmed against the primary scan or PDF.
acceptance: >-
  For each such value used in tests/vectors/, the vector entry has
  verified_by set and cites the primary source. Applies at least to Clem47
  Table II, EPM β/γ/J2, Cap03 p_A, IAU 2012 B2 and the GMSDOT units. The
  matching items in OPEN-ITEMS.md are checked off.
source: >-
  docs/strategy/journal/01-physics-truth-data.md PHY-018 and Open items.
parent: ORG-002
related: [NUM-020, TST-002, TST-003]
notes: >-
  Tracked in docs/requirements/OPEN-ITEMS.md.
```
