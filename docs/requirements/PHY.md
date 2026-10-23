<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# PHY — Physics truth data for the Mercury perihelion ladder

**Scope.** These requirements fix the expected value, tolerance, and primary source of every rung of the Mercury perihelion precession ladder (ORG-002). They also fix the canonical constants, the metadata every reported rate must carry, and the rule that no secondary or unverified value becomes a test oracle. Source: Journal #1 (`docs/strategy/journal/01-physics-truth-data.md`). The headline number is Park et al. (2017)'s **575.3100 ± 0.0015 ″/cy** in Mercury's mean-orbit frame. The ≈5600 ″/cy equinox-referred figure appears only as a constructed historical comparison (PHY-017). All tolerances are in arcseconds per Julian century (″/cy) unless stated otherwise.

Evidence labels carried over from the journal: **[P]** read in the primary source, **[C]** computed from [P] constants, **[S]** secondary transcription, **UNVERIFIED** not yet confirmed. See [OPEN-ITEMS.md](OPEN-ITEMS.md) for everything that still has to be checked.

Rungs in the Journal #1 table that have no dedicated requirement yet: N-SELF (0.0050, meaning UNVERIFIED), N-X cross terms (Σ = +0.0127), N-MOON (no primary value), J2-DE440 (0.02792, covered inside PHY-010) and GR-1PN-2B (finite-mass-ratio shift ≈ 7×10⁻⁶ ″/cy, covered by the μ note in PHY-001/PHY-004). N-AST (0.0012) has its own differential rung, PHY-019 (added 2026-10-14 after the red-team review, Journal #9). N-SELF, N-AST and N-X enter PHY-013 as part of the N-SUM total.

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
  Integrator: IAS15 (NUM-001) in f64 with compensated summation and its
  default adaptive step control; the integrator name and its settings are
  recorded in the output metadata. Over an integration span of at least
  1000 yr, the fitted |ϖ̇| < 1×10⁻⁶ ″/cy. The test output labels any residual
  rate as numerical error.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 (rung N-2B, analytic closed
  Kepler ellipse) and Requirements implied PHY-001.
parent: ORG-002
related: [NUM-001, NUM-015, NUM-023]
notes: >-
  Caveat C7: the gravitational parameter may be μ = GM☉ + GM_Mercury or GM☉
  alone, but the choice SHALL be documented. The two-body mass correction
  (≈7×10⁻⁶ ″/cy) does not affect this rung, which is exactly zero either way.
  Bound rationale (red-team review 2026-10-14, Journal #9): 1×10⁻⁶ ″/cy
  (≈1.2×10⁻¹⁴ rad/orbit) is far below the smallest asserted rung (N-AST
  0.0012 ± 0.0003, PHY-019; LT −0.0020, PHY-011), so integrator drift cannot
  masquerade as physics in any rung. The bound is integrator-specific: other
  integrators (e.g. DOP853 at rtol 10⁻¹²) can show secular truncation drift
  near 10⁻⁵ ″/cy and need their own stated bound. Integrators that propagate
  the Kepler problem analytically (Wisdom–Holman / WHFast Kepler drift,
  analytic Kepler propagators) are EXCLUDED from this test: they pass
  trivially, so a pass proves nothing.
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
  GMSDOT (−9.0×10⁻¹⁴; adopted inference 2026-10-20: a fractional rate in yr⁻¹,
  confirmed at M5 by a fit against de440.bsp, OI-04; it no longer blocks this
  requirement, because the table carries it with open_item and never asserts it);
  whether the DE440 paper quotes a GM☉ uncertainty or a J2 rationale (OI-06,
  qualifies only). Closed 2026-10-20: the exact au (OI-05: IAU 2012
  Resolution B2 read, au = 149 597 870 700 m exactly) and the header state
  frame and units (OI-23: SSB-centred ICRF, au and au/day; see REF-002 notes).
  Build evidence (WP2, m3-newtonian-nbody): crates/ferroastro-constants/data/de440-constants.toml is generated from header.440 by the gen_de440_table
  example; the data test data_de440_table_matches_header checks 23/23 entries
  token for token. Planetary GMs (km³ s⁻²):
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
  HYPOTHESIS (UNVERIFIED): the 0.0006 ″/cy gap to Park17 comes from mean versus
  osculating a and e. Closing it needs a shifted by ≈ +312 km (8×10⁻⁶
  relative), the right order for Venus-driven short-period terms but not shown;
  other candidates are Park's fit/projection procedure and the element source.
  Journal #4 §4 quoted 42.981 ″/cy; that is a rounding error (42.98048 rounds to
  42.980), not an effect of its different elements (a = 0.38709893 au,
  e = 0.20563069), which change the result by only 2×10⁻⁶ ″/cy (corrected
  2026-10-14, red-team review, Journal #9). The ±0.0001 tolerance here is
  stated for the SW92 elements named above. The μ choice (GM☉ alone or
  GM☉ + GM_Mercury) SHALL be documented (caveat C7).
  μ CONVENTION, build evidence (sync 2026-10-20; WP11, WP13, WP14 on
  m3-newtonian-nbody): the analytic rate splits into a per-orbit advance and
  a mean motion n that converts it to ″/cy, and the two may use different μ.
  WP13 (review correction F5, commit 9fd3b40) adopted one convention for
  every analytic rate in ferroastro-analytic and ferroastro-ladder: the
  per-orbit advance uses the μ of the integrated dynamics (GM☉ for the test
  particle), and n = √((GM☉ + GM☿)/a³), as this acceptance states; the
  constant RATE_CONVENTION is printed in each rung's metadata. The shift
  against GM☉-only n is 3.567e-6 ″/cy. WP11 had proposed instead the rule
  "n uses the μ of the Keplerian problem that produced the elements" (test
  particle: GM☉; Sun–Mercury relative motion and Horizons elements:
  GM☉ + GM☿), and the WP14 NUM-013(a) scenario check
  (crates/ferroastro-diffrun/tests/num013a_scenario.rs, vector
  NUM013-ISOLATION-DIFF-SCENARIO) uses n from GM☉. The code therefore uses
  more than one convention. DECIDED 2026-10-21 (OI-77 closed): every ″/cy
  conversion uses n = √((GM☉ + GM☿)/a³), the two-body Keplerian μ, which
  matches the "Keplerian GM" Horizons prints. The WP14 harness is to be
  aligned. Every current tolerance exceeds the 3.6e-6 ″/cy difference.
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
  For |γ−1| ≤ 10⁻³ and |β−1| ≤ 10⁻³, differential runs from identical
  initial conditions give rate(β, γ) / rate(1, 1) = (2+2γ−β)/3 to within 10⁻⁶
  relative. No absolute rate literal (e.g. 42.98) enters the assertion.
  Illustration only, not asserted: γ = 1, β = 1 + 3.9×10⁻⁵ shifts the rate by
  about −0.00056 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung PPN-βγ and PHY-006;
  Will 2014, Living Rev. Relativ. 17, 4, Eq. 65.
parent: ORG-002
related: [REF-010, NUM-015, NUM-017]
notes: >-
  No external N-body code exposes PPN β/γ (Journal #3), so this rung is
  validated analytically only (REF-010). Sensitivity scale: β ±0.00056 ″/cy per
  Park17 σ_β; γ ±0.00066 ″/cy per Cassini σ_γ. Acceptance corrected 2026-10-14
  (red-team review, Journal #9): the earlier form "42.98 × (2+2γ−β)/3 to 10⁻⁶
  relative" could not pass, because 42.98 differs from the computed 42.98048 by
  1.1×10⁻⁵ relative.
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
  (2+2γ−β)/3 × GR-SCHW is coordinate-independent ("42.98" is shorthand for the
  GR-SCHW rate, never a test value; PHY-004 holds the asserted value). The
  areal-gauge run is a labelled exemption from NUM-017's
  no-Schwarzschild-coordinate rule.
```

### PHY-009 — Term-by-term 1PN coefficient check

```yaml
id: PHY-009
title: "Per unit coefficient, 1PN Lagrangian terms A, B, C, D give 8, 4, 2, 2 units"
status: draft
criticality: high
verification: test
statement: >-
  For the test-particle Lagrangian L = ½v² + U + ε[A v⁴ + B U v² + C U² +
  D U (n·v)²], each coefficient switched on alone SHALL reproduce its
  per-orbit advance in units of πGM/(c²p).
acceptance: >-
  Per unit of coefficient, A, B, C and D individually give 8, 4, 2 and 2 ×
  πGM/(c²p) per orbit (so the SR kinetic value A = 1/8 gives 1) to within 10⁻³
  relative.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §5 [C] and PHY-009 (numerically
  confirmed in-session with sympy + DOP853 at e = 0.2, ε = 10⁻⁴: 1.000 at
  A = 1/8, then 3.999, 2.000, 2.000 for unit B, C, D).
parent: ORG-002
related: [PHY-007, PHY-008, NUM-017]
notes: >-
  Corrected 2026-10-14 (red-team review, Journal #9): the earlier text listed
  A → 1, which holds only at A = 1/8; a test that sets A = 1 must expect 8.
  The D term U(n·v)² is the areal-gauge form, so the D-term check is a labelled
  exemption from NUM-017's no-Schwarzschild-coordinate rule.
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
  SOLAR POLE, build evidence (sync 2026-10-20; WP9 and WP12 on
  m3-newtonian-nbody): the default solar pole is the IAU WGCCRE 2015 pole,
  RA 286.13°, Dec 63.87° (Pole::sun_wgccre2015 in ferroastro-frames), read
  from NAIF pck00011.tpc (BODY10_POLE_RA/DEC, sha256 3dff7b1d…e28bb1, citing
  Archinal et al. 2018, which itself was not opened) into vectors
  SUN-POLE-IAU-RA/DEC; data test data_naif_pck_solar_pole checks them, and
  data_frame_rotations_reproduce_horizons… reproduces Horizons' IAU_SUN frame
  to 4.7e-16. The pole is configurable and travels with the rotation as
  metadata (crates/ferroastro-frames/tests/num014_frame_definitions.rs::solar_equator_pole_is_configurable_and_recorded). In that real geometry
  (i = 3.381087°) the numerical J2 rung at Park's J2 gives 0.0284524 ″/cy
  against Park17 Table 3 0.0286 ± 0.0011 (WP4). Not verified: that DE440's
  own J2 term uses this pole, and whether DE440 applies solar J2 to the
  asteroids; FerroAstro's N-body SolarJ2WithReaction acts on Major bodies
  only (WP12 deviations 3 and 4; OI-76).
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
  and the INPOP21a Eq. 1 acceleration (equivalently DE440 §3.4 Eqs. 37–39).
acceptance: >-
  The rung yields −0.0020 ± 0.0002 ″/cy. The analytic check
  ϖ̇ = −4GS/(c²a³(1−e²)^(3/2)) at I ≈ 0 gives −0.00202 ″/cy.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung LT and PHY-011; Park
  et al. 2017; Park et al. 2021; INPOP21a (Fienga et al. 2021, Notes Sci. Tech.
  IMCCE S110) Eq. 1; Park et al. 2021 §3.4 Eqs. 37–39.
parent: ORG-002
related: [REF-010, NUM-013, NUM-015, NUM-019]
notes: >-
  LT is ~7% of the oblateness precession (Park21); INPOP21a says ~10% of the
  oblateness acceleration.
  OI-35 CLOSED 2026-10-21 (maintainer sign-off, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280:
  "Confirm; Sun reaction on; LT all bodies"). Read from the typeset pages:
  DE440 §3.4 Eqs. 37–39 (Park et al. 2021 p. 7, sha256 96c3945d…e8f0ee0)
  and INPOP21a Eq. 1 (Fienga et al. 2021, IMCCE note S110, p. 5, sha256
  5fc3b7295a9664dadb25867cdc5893ce91cff0205bd03c972a2a8d67fffb89a5). Both
  equal Journal #4 R3b: a_LT = (1+γ)GS/(c²r³)[3(r·ŝ)(r×v)/r² + v×ŝ], with S
  the total solar angular momentum and r, v heliocentric (DE440's
  (1+γ)G/(2c²r³) inside Ω is cancelled by the factor 2 of Eq. 37; DE440 does
  not define v, INPOP21a defines it relative to the Sun). The statement's
  "INPOP21a Eq. 1" therefore stands. Park et al. 2017 Eq. 2 (p. 2, sha256
  24fb2153…2433380b), ϖ̇_LT = −2(1+γ)GS cos i/(c²a³(1−e²)^(3/2)), is the
  orbit-averaged in-plane rate of this acceleration: it is the analytic rate
  check used in the acceptance, not a check of the acceleration's form.
  Decisions for the N-body use (NUM-019): the Sun receives the LT reaction
  (on, the default), and LT applies to all non-Sun bodies.
  NUMERICAL EVIDENCE (WP4 on m3-newtonian-nbody): the implemented R3b
  acceleration (LenseThirring in ferroastro-forces) reproduces Park et al.
  2017 Eq. 2 to 1.25e-6 relative in equatorial geometry (passing NUM-015
  only through its 1e-7 ″/cy absolute floor) and 7.7e-8 relative in the
  real solar-equator geometry (data tier). The rung gives −0.0020184 ″/cy
  (equatorial) and −0.0020148 ″/cy (real geometry), within −0.0020 ± 0.0002.
  G·S = 1.2680765843106158e16 km⁵/s³ from header.440 tokens (S = 1.8999e41
  kg m² s⁻¹), recomputed independently by data_solar_spin_from_fresh_header_440.
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
  Protocol "report both" (ADR-0014; maintainer decision 2026-10-20,
  confirmed 2026-10-21 against the Park 2017 text, OI-38): both protocols are computed from identical initial conditions
  with the NUM-013 harness and one estimator, Newtonian. ASSERTED (P-remove):
  the contribution of planet X is the rate of the full system minus the rate
  of the full system without X; each value within ±0.005 ″/cy of:
  Venus 277.4176, Earth/Moon 90.8881, Mars 2.4814, Jupiter 153.9899,
  Saturn 7.3227, Uranus 0.1425, Neptune 0.0424. REPORTED, not asserted
  (P-add): Sun + Mercury + X minus Sun + Mercury, printed next to the
  asserted value with its difference from Park17.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 and PHY-012; Park et al.
  2017, AJ 153, 121, Table 3 [P].
parent: ORG-002
related: [NUM-011, NUM-013, CI-009]
notes: >-
  AUTHORITATIVE over NUM-013 for expected values and tolerances of the
  per-planet contributions; NUM-013 is authoritative for the differential-run
  harness that produces them. OI-38 CLOSED 2026-10-21: the maintainer
  confirmed P-remove against the Park 2017 text (Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280;
  Park 2017 §4 p. 5: "the Venus row represents the difference in Mercury's
  precession rate when the mass of Venus is set to zero"; ADR-0014
  Confirmation 2026-10-21). The P-remove assertion is no longer provisional
  once the provisional field is removed from the Park 2017 Table 3 vectors in
  the same PR (#13, ADR-0004 revision 2026-10-21). The choice is still
  post hoc in its order (below), P-add misses stay reported as information,
  and the interaction-row arithmetic remains a post-hoc operational
  definition. HISTORY: before the confirmation, Park's protocol was open and
  the oracle was ill-posed without a fixed protocol: cross terms
  reach 0.0411 (J+S) and −0.0209 (V+E/M), so toggling a planet off from the
  full model versus on from Sun + Mercury can differ by up to ~0.04 ″/cy, 8×
  the tolerance (red-team review 2026-10-14, Journal #9). PROTOCOL CHANGE
  2026-10-20 (maintainer decision; ADR-0014). The protocol written down
  before any run was P-add (Sun + Mercury + X minus Sun + Mercury; this
  requirement's earlier acceptance and the WP14 pre-registration in
  tests/vectors/park2017-table3.toml). P-remove was chosen AFTER the WP14 and
  WP17 results were known, so this choice is post hoc and is stated as such.
  Evidence on m3-newtonian-nbody (crates/ferroastro-diffrun/tests/nightly_phy012.rs::nightly_phy012_park17_table3_newtonian_contributions,
  WP14, re-run identically in the progress review after WP18): P-add missed
  4 of 7 rows: Venus 277.394894 (−0.022706), Earth/Moon 90.869103
  (−0.018997), Jupiter 154.028732 (+0.038832), Saturn 7.365749 (+0.043049);
  within ±0.005 only Mars (−0.002016), Uranus (−0.002898) and Neptune
  (−0.000332). P-remove passes all 7 within 0.0007 ″/cy (largest Earth/Moon
  +0.000695). Four independent integrators (FerroAstro IAS15, REBOUND 5.2.2,
  Mercury6 RA15, Swiftest WHM) give the same P-add Venus value to 2e-8 ″/cy
  through one estimator (WP17 check 4,
  crates/ferroastro-oracle/tests/nightly_oracle_differentials.rs), and two
  independent scipy DOP853 integrations reproduce it to ≤ 1.7e-4 ″/cy
  (progress reviews after WP15 and WP18), so the P-add misses are a protocol difference, not an
  integrator error. Park17 §4 (WP14 notes quote it verbatim) describes rows
  made by setting a GM to zero in the nominal ephemeris, and Park's pair rows
  equal P-remove's joint removal minus the two single removals to
  ≤ 1.2e-4 ″/cy for all six pairs (progress review after WP18, R1; that
  comparison is post hoc). REVERT CONDITION (not triggered; OI-38 closed
  2026-10-21): the P-remove assertion was PROVISIONAL until a maintainer
  confirmed OI-38 against the Park 2017 text; had the text said P-add, the
  assertion would have reverted to P-add and the 4 misses above would have
  been treated as real discrepancies (type:physics-discrepancy). SCOPE
  OF THE GATE: the ±0.005 ″/cy tolerance remains tighter than the protocol
  ambiguity (cross terms up to 0.041 ″/cy), so a pass establishes
  "consistent with Park 2017 under P-remove", not agreement independent of
  the protocol. The OI-38 tick and the vector change (removing the
  provisional/blocked_by "OI-38" fields from the Park rows) land in the same
  PR, never the tick alone (the WP15 review's F13). The tolerance and the expected values are unchanged; the
  protocol SHALL NOT be tuned further to make a test pass. Clemence 1947
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
title: "Rung EIH-FULL: DE440-equivalent physics reproduces 575.3093 ″/cy (DE440 J2)"
status: draft
criticality: high
verification: test
statement: >-
  DE440-equivalent physics (EIH N-body with γ = β = 1, solar J2, Lense–Thirring,
  343 asteroids, 30 KBOs and the 36-point KBO ring) SHALL reproduce Park17's
  total secular perihelion rate in the Park17 mean-orbit frame. The 1PN EIH
  acceleration SHALL be evaluated for every moving body (the Sun, planets,
  the Moon, asteroids and KBOs), each with its own 1PN terms, with the sums
  over all bodies (DE440 p. 5; DE430 p. 12).
acceptance: >-
  Run with the DE440 constants (J2 = 2.19614×10⁻⁷), the rate is
  575.3093 ± 0.005 ″/cy. Derivation (maintainer decision 2026-10-15): Park17's
  575.3100 ± 0.0015 used its fitted J2 = 2.25×10⁻⁷; the J2 term is 0.028601 at
  Park's J2 vs 0.027917 at DE440's, Δ = −0.000684 ″/cy (PHY-010), so the
  expected value is 575.3100 − 0.0007 = 575.3093. Run metadata records the J2/S☉
  set. The tolerance is 3× Park's σ plus margin for the DE430-to-DE440 model
  difference.
source: >-
  docs/strategy/journal/01-physics-truth-data.md §1, §2 rung EIH-FULL and
  PHY-014; Park et al. 2017 Table 3; Park et al. 2021 (body set).
parent: ORG-002
related: [NUM-011, NUM-017, NUM-020, CI-007, REF-001, REF-003, PHY-011, PHY-019]
notes: >-
  AUTHORITATIVE tolerance for the 575.31 headline; NUM-011 quotes ±0.01 as the
  estimator-level acceptance. The PHY-014 journal text names "343 asteroids, KBO
  ring"; §4 of the same journal lists 30 KBOs plus the ring, so both are
  included here. Frame pole: α ≈ 280.9876°, δ ≈ 61.4481° (ICRF). Fit model
  B + ϖ̇t + Qt² + 14 periodic terms, Q ≈ −0.04478132 ″/cy². Depends on the R5 EIH
  transcription check (NUM-020) and on asteroid initial conditions (REF-003,
  UNVERIFIED). PHY-014 cannot detect a dropped component smaller than its
  tolerance: LT (−0.0020), N-AST (0.0012) and most cross terms lie inside
  ±0.005. Missing LT is caught by the differential rung PHY-011 and missing
  asteroids by PHY-019, not by PHY-014 (red-team review 2026-10-14, Journal #9).
  The Park17 "Mercury" self row (0.0050) equals the whole tolerance and its
  meaning is UNVERIFIED (OI-09).
  EIH SCOPE AND TRANSCRIPTION (sign-off 2026-10-21; maintainer, Journal #12,
  https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/1#issuecomment-6306718280).
  NUM-020 is signed off for the EIH equation (OI-33 closed; pinned m4-eih
  78d5a4c7fd7141515d3d9aa6bdfa7218624bad81). D1: DE430 p. 12 says "The
  summation is over all bodies: Sun, Moon, planets and asteroids"; DE440
  p. 5 says "the summations are over all bodies". The maintainer chose
  "Match DE440: all bodies", so the 1PN EIH terms apply to every body,
  the Moon, asteroids and KBOs included. This supersedes the Majors-only EIH
  of m4-eih (eih.rs Tier::Major, marked NEEDS HUMAN REVIEW). The ruling on
  #13 (2026-10-21, https://github.com/Proper-Time-Physics-Institute/FerroAstro/pull/13#issuecomment-6306750993)
  repeats it: the Moon and asteroids are required, and #17 does not merge
  with a Majors-only EIH. WP22 and WP23 are unblocked on this basis.
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
criticality: high
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
criticality: high
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

### PHY-019 — Rung N-AST: asteroid differential contribution

```yaml
id: PHY-019
title: "Rung N-AST: asteroid differential contribution reproduces 0.0012 ″/cy"
status: draft
criticality: high
verification: test
statement: >-
  The Newtonian contribution of the DE440 asteroid set to Mercury's perihelion
  rate (rung N-AST) SHALL be computed as a differential run and SHALL reproduce
  the Park17 Table 3 value in the Park17 mean-orbit frame with the Park17 fit.
acceptance: >-
  Protocol as PHY-012 ("report both", ADR-0014): ASSERTED (P-remove) planets
  + asteroid set minus planets; REPORTED (P-add) Sun + Mercury + asteroid
  set minus Sun + Mercury; both Newtonian, from identical initial conditions
  (NUM-013 harness). The asserted differential rate is 0.0012 ± 0.0003 ″/cy.
  This rung gates independently of OI-38, because its protocol dependence is
  about 1% of the tolerance (notes).
source: >-
  docs/strategy/journal/01-physics-truth-data.md §2 rung N-AST; Park et al.
  2017, AJ 153, 121, Table 3 [P]; docs/strategy/journal/09-red-team-review.md
  (PHY-014 finding).
parent: ORG-002
related: [PHY-012, PHY-013, PHY-014, NUM-013, REF-003]
notes: >-
  Added 2026-10-14 after the red-team review (Journal #9): PHY-014's ±0.005
  cannot detect a dropped asteroid set, so this differential rung catches it.
  OI-38 DECOUPLING (maintainer decision 2026-10-20): the earlier text blocked
  this rung on OI-38. Evidence that the protocol does not matter here:
  crates/ferroastro-diffrun/tests/nightly_phy013.rs::nightly_phy013_n_sum_and_asteroid_differential (WP15, re-run identically by
  the WP15 reviewer and the progress review after WP18) gives P-add 0.001242
  and P-remove 0.001245 ″/cy; the protocol dependence of about 3e-6 ″/cy is
  1% of ±0.0003 (progress review after WP15, finding F9; restated in WP16).
  Both values pass. The ±0.0003 tolerance and 0.0012 are unchanged. The
  vector PARK17-T3-ASTEROIDS still carries blocked_by = "OI-38" and cites
  PHY-013/NUM-013 but not this requirement, and the nightly test does not yet
  cite PHY-019 (progress review after WP18, R8); unblocking and citing are
  build-PR work. Initial conditions: the 343 numbered asteroids are placed
  from sb441-n373s.bsp, a DE441 product, because DE440's own asteroid ICs are
  in no pinned product (REF-003 notes; OI-24 still qualifies this rung).
  Which bodies Park's row covers: the WP15 builder re-read Park17 (PDF sha256
  24fb2153…2433380b): its ephemeris has no KBOs (they arrived in DE440), and
  the row's uncertainty "comes from the estimated mass parameters of the 343
  asteroids", which points to the 343 asteroids only; a human reading of that
  text is still pending (OI-24). The KBOs + ring add +0.000019 ″/cy in
  FerroAstro's full model (WP15, reported only).
```
