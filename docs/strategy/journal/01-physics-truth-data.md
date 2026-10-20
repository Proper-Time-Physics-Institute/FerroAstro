## 📓 Journal #1 — Physics truth data for the Mercury ladder

> **Lead review notes:**
> - Re-derived §5 independently. In the PPN isotropic/harmonic gauge (A, B, C) = (1/8, γ+½, ½−β), so the sum 1 + 4(γ+½) + 2(½−β) = 2(2+2γ−β) recovers the standard PPN factor. The arithmetic for SR-a = GR/6 = 7.1634, SR-b = 7/6 GR = 50.144 (harmonic gauge) and 5/6 GR = 35.817 (areal gauge) is consistent.
> - **This fixes the SR-rung presentation** (the user chose "show both with caveats"). SR-a is a well-defined *model* result. SR-b is a gauge-labelled teaching rung, not an observable.
> - **The showcase headline number is Park17's 575.31″/cy** in Mercury's mean-orbit frame. The ≈5600″/cy equinox-referred figure appears only as a clearly constructed historical comparison (PHY-017).

**Date:** 2026-10-14 · **Branch:** `1-scientific-computing-strategy` · **Scope:** expected values, uncertainties and primary sources for every rung of the Mercury perihelion ladder, plus the constants and the JPL Horizons workflow used to check them.

**Verification legend**
- **[P]**: read in the primary source during this session (publisher page, arXiv full text, or an official JPL or IMCCE file).
- **[C]**: computed in this session from [P] constants. The formula and inputs are stated so the number can be re-derived.
- **[S]**: taken from a secondary transcription and not yet checked against the original scan.
- **UNVERIFIED**: from memory or an unconfirmed snippet. Do not use it as a test oracle until it has been checked.

---

### 0. Primary references (bibliography)

| Key | Citation | DOI / URL | Status |
|---|---|---|---|
| Park17 | R. S. Park, W. M. Folkner, A. S. Konopliv, J. G. Williams, D. E. Smith, M. T. Zuber, "Precession of Mercury's Perihelion from Ranging to the MESSENGER Spacecraft", *AJ* **153**, 121 (2017) | [10.3847/1538-3881/aa5be2](https://doi.org/10.3847/1538-3881/aa5be2) | [P] |
| Park21 | R. S. Park, W. M. Folkner, J. G. Williams, D. H. Boggs, "The JPL Planetary and Lunar Ephemerides DE440 and DE441", *AJ* **161**, 105 (2021) | [10.3847/1538-3881/abd414](https://doi.org/10.3847/1538-3881/abd414) | [P] |
| DE440hdr | JPL, DE440 ASCII header (`header.440`, groups 1040 and 1041) | https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/header.440 | [P] (parsed) |
| Clem47 | G. M. Clemence, "The Relativity Effect in Planetary Motions", *Rev. Mod. Phys.* **19**, 361–364 (1947) | [10.1103/RevModPhys.19.361](https://doi.org/10.1103/RevModPhys.19.361) | Metadata [P] via Crossref. Table values [S] (transcription at publish.obsidian.md/spaceaudits) |
| Will14 | C. M. Will, "The Confrontation between General Relativity and Experiment", *Living Rev. Relativ.* **17**, 4 (2014) | [10.12942/lrr-2014-4](https://doi.org/10.12942/lrr-2014-4), arXiv:1403.7377 | [P] (§4.2, Eqs. 64–65) |
| Bert03 | B. Bertotti, L. Iess, P. Tortora, "A test of general relativity using radio links with the Cassini spacecraft", *Nature* **425**, 374–376 (2003) | [10.1038/nature01997](https://doi.org/10.1038/nature01997) | Metadata [P]. γ value [P] as quoted by Park17 |
| INPOP19a | A. Fienga, P. Deram, V. Viswanathan, A. Di Ruscio, L. Bernus, D. Durante, M. Gastineau, J. Laskar, "INPOP19a planetary ephemerides", *Notes Sci. Tech. IMCCE* **S109** (2019) | https://www.imcce.fr/content/medias/recherche/equipes/asd/inpop/inpop19a_20191214.pdf | [P] (Table 3) |
| INPOP21a | A. Fienga, P. Deram, A. Di Ruscio, V. Viswanathan, J. I. B. Camargo, L. Bernus, M. Gastineau, J. Laskar, "INPOP21a planetary ephemerides", *Notes Sci. Tech. IMCCE* **S110** (2021) | https://www.imcce.fr/content/medias/recherche/equipes/asd/inpop/inpop21a.pdf | [P] (Tables 1–2, Eq. 1) |
| Fien21 | A. Fienga et al., "Evolution of INPOP planetary ephemerides and Bepi-Colombo simulations", *Proc. IAU* **15** (S364), 31–51 | [10.1017/S1743921321001277](https://doi.org/10.1017/S1743921321001277), arXiv:2111.04499 | [P] (abstract) |
| Pij98 | F. P. Pijpers, "Helioseismic determination of the solar gravitational quadrupole moment", *MNRAS* **297**, L76–L80 (1998) | [10.1046/j.1365-8711.1998.01801.x](https://doi.org/10.1046/j.1365-8711.1998.01801.x) | [P] (ar5iv, Eq. 27) |
| ACG08 | H. M. Antia, S. M. Chitre, D. O. Gough, "Temporal variations in the Sun's rotational kinetic energy", *A&A* **477**, 657–663 (2008) | arXiv:0711.0799 | Value [P] as quoted in INPOP21a Table 1 |
| PP14 | E. V. Pitjeva, N. P. Pitjev, "Development of planetary ephemerides EPM and their applications", *Celest. Mech. Dyn. Astron.* **119**, 237–256 (2014) | [10.1007/s10569-014-9569-0](https://doi.org/10.1007/s10569-014-9569-0) | Metadata [P]. Values [S] |
| Prsa16 | A. Prša et al., "Nominal Values for Selected Solar and Planetary Quantities: IAU 2015 Resolution B3", *AJ* **152**, 41 (2016) | [10.3847/0004-6256/152/2/41](https://doi.org/10.3847/0004-6256/152/2/41), arXiv:1605.09788 | [P] (Table 1) |
| Cap03 | N. Capitaine, P. T. Wallace, J. Chapront, "Expressions for IAU 2000 precession quantities", *A&A* **412**, 567–586 (2003) | [10.1051/0004-6361:20031539](https://doi.org/10.1051/0004-6361:20031539) | Metadata [P]. p_A value UNVERIFIED (search snippet only) |
| LM10 | T. J. Lemmon, A. R. Mondragon, "First-Order Special Relativistic Corrections to Kepler's Orbits", arXiv:1012.5438 (v1 2010, v6 2016). Not published in a journal. | https://arxiv.org/abs/1012.5438 | [P] (abstract) |
| SW92 | E. M. Standish, J. G. Williams, "Orbital Ephemerides of the Sun, Moon and Planets" (Explanatory Supplement, 1992). Keplerian elements table hosted by JPL. | https://ssd.jpl.nasa.gov/planets/approx_pos.html | [P] |
| HzAPI | JPL Horizons API documentation (v1.3) | https://ssd-api.jpl.nasa.gov/doc/horizons.html | [P]. A live call was also tested |

---

### 1. Observed total advance and reference frames

| Quantity | Value (″/cy) | Uncertainty | Frame / definition | Source |
|---|---|---|---|---|
| Observed motion of Mercury's perihelion (classic) | **5599.74** | ±0.41 | Referred to the **moving equinox** (equinox of date), Julian century, epoch about 1850 | Clem47 Table II [S] |
| General precession in longitude (Newcomb, used by Clemence) | **5025.645** | ±0.50 | Precession of the equinox along the ecliptic, 1850 Julian century | Clem47 Table II [S] |
| General precession in longitude, IAU 2006/P03, at J2000 (linear term of p_A) | 5028.796195 | conventional | Equinox of date | Cap03, **UNVERIFIED** (seen only in a search snippet of an A&A table) |
| Clemence theoretical sum (general precession + planets + oblateness) | 5557.18 | ±0.85 | Moving equinox | Clem47 [S] |
| Clemence residual (observed − theory) | **42.56** | ±0.94 | n/a | Clem47 [S] |
| Clemence GR prediction (1947 constants) | 43.03 | ±0.03 | n/a | Clem47 [S] |
| **Modern total secular rate (all effects, inertial)** | **575.3100** | **±0.0015** | Longitude of perihelion projected onto **Mercury's mean-orbit frame**, with pole relative to ICRF at α ≈ 280.9876°, δ ≈ 61.4481°. Epoch J2000. Rate fitted as B + ϖ̇t + Qt² + 14 periodic terms over 1000–3000 CE. Q ≈ −0.04478132 ″/cy² | Park17 Table 3 [P] |
| Mean ϖ̇ in the fixed J2000 ecliptic/equinox frame | 577.7168 (= 0.16047689°/cy) | fit to DE200, valid 1800–2050 | **Fixed** mean ecliptic and equinox of J2000. This is not the Park17 frame. | SW92 [P] |

**Frame notes.** The ≈5600″/cy figure includes the precession of the equinox (≈5025–5029″/cy), because it is measured from a moving reference direction. It is therefore not a dynamical quantity. Ephemeris-era values (≈575″/cy) are measured in an inertial (ICRF-aligned) frame. Even those depend on the reference plane. The difference between 575.31 and 577.72 comes from measuring ϖ = Ω + ω, a "dog-leg" angle, in the J2000 ecliptic instead of Mercury's own orbit plane. Adding 575.31 and 5028.80 does **not** reproduce a modern equinox-referred value, because the two numbers refer to different planes. Do not use that sum as a test.

---

### 2. Rung table (expected values)

GR reference value computed here **[C]** with:
- GM☉ = 132712440041.279419 km³ s⁻² (Park21 / DE440hdr)
- c = 299792.458 km/s and au = 149597870.700 km (DE440hdr)
- Mercury a = 0.38709927 au and e = 0.20563593 (SW92 J2000 mean elements)
- orbital period from (GM☉ + GM_Mercury), 415.2009 orbits per Julian century

Result: Δϖ = 6πGM/(c²a(1−e²)) = 5.01866×10⁻⁷ rad/orbit = **42.9805″/cy**. Park17 gives 42.9799 ± 0.0009, so the two agree within 0.0006″/cy. The residual difference comes from mean versus osculating a and e. *(Annotated 2026-10-14, red-team review, Journal #9: this attribution is a **hypothesis, UNVERIFIED**. Closing the gap needs a shifted by ≈ +312 km (8×10⁻⁶ relative); other candidates are Park's fit/projection procedure and the element source.)*

| Rung ID | Physics | Expected ϖ̇ (″/cy) | Uncertainty | Source / status |
|---|---|---|---|---|
| **N-2B** | Newtonian Sun + Mercury, point masses | **0** (exactly) | numerical only | Analytic (closed Kepler ellipse) |
| N-SELF | "Mercury" term in Park17 (meaning not stated; see caveat C6) | 0.0050 | <0.0001 | Park17 Table 3 [P] |
| **N-VEN** | + Venus | **277.4176** | <0.0001 | Park17 [P]. Clemence: 277.856 ± 0.68 [S] |
| **N-EMB** | + Earth/Moon (combined) | **90.8881** | <0.0001 | Park17 [P]. Clemence "Earth": 90.038 ± 0.08 [S] |
| N-MARS | + Mars | 2.4814 | <0.0001 | Park17 [P]. Clemence 2.536 [S] |
| **N-JUP** | + Jupiter | **153.9899** | <0.0001 | Park17 [P]. Clemence 153.584 ± 0.01 [S] |
| N-SAT | + Saturn | 7.3227 | <0.0001 | Park17 [P]. Clemence 7.302 ± 0.01 [S] |
| N-URA | + Uranus | 0.1425 | <0.0001 | Park17 [P]. Clemence 0.141 [S] |
| N-NEP | + Neptune | 0.0424 | <0.0001 | Park17 [P]. Clemence 0.042 [S] |
| N-X | Planet–planet cross terms: Me+V −0.0053, V+E/M −0.0209, V+J −0.0012, E/M+Ma −0.0016, Ma+J +0.0002, J+S +0.0411, S+U +0.0004 | Σ = +0.0127 | not listed | Park17 [P] |
| N-AST | + asteroids | 0.0012 | <0.0001 | Park17 [P] |
| N-MOON | Moon resolved separately from Earth | — | — | **UNVERIFIED**: no primary value found. Park17 only gives E/M combined. |
| **N-SUM** | Full Newtonian N-body total | **532.3035** [C, Park17 rows summed] | about 0.001 | Clemence planets only: 531.504 [S] (531.535 = 5557.18 − 5025.645) |
| **J2** | + solar quadrupole | **0.0286** at J2 = 2.25×10⁻⁷ | ±0.0011 | Park17 [P]. Same value reproduced [C] from 3πJ₂R☉²/p² per orbit with R☉ = 696000 km |
| J2-DE440 | J2 at the DE440 value 2.19614×10⁻⁷ | 0.02792 | scales linearly with J2 | [C] |
| **SR-a** | Relativistic kinetic energy only, L = −mc²√(1−v²/c²) + GMm/r (Sommerfeld/Goldstein) | **7.1634** (= GR/6) | follows GR value | Analytic: Δϖ = πGM/(c²p) per orbit. 1/6 factor stated by LM10 v1 [P]. Number [C]. Term-by-term values were checked numerically in this session (§5). |
| **SR-b** | "Velocity-dependent 1PN terms only": v⁴/8 + (γ+½)Uv² in isotropic/harmonic gauge | **50.1439** (= 7/6 GR) | **gauge-dependent** | [C], see §5. In Schwarzschild areal coordinates the "velocity-only" value is 35.8171 (= 5/6 GR). |
| **GR-SCHW** | Schwarzschild test particle, (2+2γ−β)/3 = 1 | **42.9805** [C] / **42.9799 ± 0.0009** [P] | ±0.0009 | Park17 Table 3 ("gravitoelectric"). Will14 Eq. 65 gives 42.98″ [P]. |
| GR-1PN-2B | 1PN Sun + Mercury (finite mass ratio) | GR-SCHW + O(η) | the m = m☉ + m_Mercury shift is ≈7×10⁻⁶ | Will14 Eq. 64: the η term vanishes in fully conservative theories [P]. Value [C]. |
| **LT** | Lense–Thirring from solar spin | **−0.0020** | ±0.0002 | Park17 [P], with S☉ = 190×10³⁹ kg m² s⁻¹. Same value repeated in Park21 [P]. [C] −0.00202 from ϖ̇ = −4GS/(c²a³(1−e²)^{3/2}) at I ≈ 0. |
| **EIH-FULL** | Full N-body EIH + J2 + LT (γ = β = 1) | **575.3100** | ±0.0015 | Park17 Table 3 [P], in the Mercury mean-orbit frame (§1) |
| PPN-βγ | Sensitivity of GR-SCHW to β and γ | 42.98 × (2+2γ−β)/3 *(2026-10-14: 42.98 is shorthand; the test asserts the ratio to the β = γ = 1 run, PHY-006)* | β: ±0.00056 per Park17 σ_β. γ: ±0.00066 per Cassini σ_γ. | Will14 Eq. 65 [P]. Arithmetic [C]. |

---

### 3. Solar J2 and PPN constraints

| Quantity | Value | Method | Source |
|---|---|---|---|
| J2☉ | (2.25 ± 0.09)×10⁻⁷. Table 1 subsets: 2.26 ± 0.09 and 2.28 ± 0.09. | MESSENGER ranging, γ fixed to Cassini | Park17 [P] |
| β − 1 | (−2.7 ± 3.9)×10⁻⁵ (abstract). Table 1 total row: −2.6 ± 3.9. | MESSENGER ranging | Park17 [P] |
| γ − 1 | (2.1 ± 2.3)×10⁻⁵ | Cassini Shapiro delay / frequency shift | Bert03 (value as quoted by Park17) [P] |
| β − 1 (review) | (−4.1 ± 7.8)×10⁻⁵ | Planetary fits including MESSENGER | Will14 §4.2 [P] |
| J2☉ (helioseismic, review) | (2.2 ± 0.1)×10⁻⁷ | Helioseismology | Will14 [P] |
| J2☉ | (2.18 ± 0.06)×10⁻⁷ | GONG + MDI rotation inversion | Pij98 Eq. 27 [P] |
| J2☉ | (2.2057 ± 0.007)×10⁻⁷. SOHO 2.22 ± 0.009, GONG 2.18 ± 0.005. | Helioseismology | ACG08, as quoted in INPOP21a [P] |
| J2☉ | (2.010 ± 0.010)×10⁻⁷, fitted **without** Lense–Thirring | Ephemeris fit | INPOP19a Table 3 [P] |
| J2☉ | (2.2180 ± 0.010)×10⁻⁷, fitted **with** LT and S = 1.90×10⁴⁸ g cm² s⁻¹ | Ephemeris fit | INPOP21a Table 1 [P] |
| J2☉ (DE440 adopted) | 2.19613915165298248×10⁻⁷ | Ephemeris | DE440hdr [P] |
| β − 1, γ − 1 limits | about 7.16×10⁻⁵ and 7.49×10⁻⁵ (conservative; INPOP20a) | Ephemeris | Fien21 abstract [P] |
| EPM: β, γ, J2 | β = 0.99998 ± 0.00003, γ = 1.00004 ± 0.00006, J2 = (2.0 ± 0.2)×10⁻⁷ (EPM2011). EPM2013 J2 = (2.22 ± 0.23)×10⁻⁷. | Ephemeris | PP14 / Pitjeva 2013. **UNVERIFIED** (secondary sources only) |

**Lense–Thirring and J2 correlation.** INPOP21a says LT "contribute[s] to about 10% of the dynamical acceleration induced by the shape of the Sun". Park21 gives about 7% of the oblateness precession. A J2 fitted without LT, such as INPOP19a's 2.01, is therefore biased low and should not be compared with helioseismic values.

---

### 4. Canonical constants (use these exactly)

| Constant | Value | Source |
|---|---|---|
| GM☉ (dynamical, TDB-compatible) | 132712440041.279419 km³ s⁻² = 2.95912208284119561×10⁻⁴ au³ d⁻² | Park21 [P], DE440hdr GMS [P] |
| dGM☉/dt | GMSDOT = −9.0×10⁻¹⁴ (header units, presumably au³/d² per year; **UNVERIFIED units**) | DE440hdr [P] |
| GM☉ − 132712440000 km³ s⁻² in other ephemerides | INPOP21a 41.8453 ± 0.11. INPOP19a 40.042 ± 0.01 (S109) **but** 42.0322 ± 0.13 (as listed in S110: an inconsistency between the two IMCCE notes). DE440 41.27942. | INPOP19a/21a [P] |
| au | 149597870.700 km (exact; IAU 2012 Res. B2, resolution text **UNVERIFIED**) | DE440hdr AU [P]. Horizons output [P] |
| c | 299792.458 km/s (exact, SI) | DE440hdr CLIGHT [P] |
| R☉ (used for J2) | 696000 km (ASUN). This is not the IAU nominal radius of 695700 km. | DE440hdr [P] |
| C☉/(M☉R☉²) | 0.06884 | DE440hdr MOISUN, Park21 [P] |
| Ω☉ | 0.247564432907 rad/day (14.1844°/day) | DE440hdr OMGSUN, Park21 [P] |
| S☉ implied by the line above | 0.06884 · M☉R☉²Ω☉ = 1.900×10⁴¹ kg m² s⁻¹ | [C]. Matches Park17's 190×10³⁹. |
| GM (km³ s⁻²): Mercury, Venus, Earth, Moon | 22031.868551, 324858.592, 398600.435507, 4902.800118 | Park21 [P] |
| GM (km³ s⁻²): Mars, Jupiter, Saturn, Uranus, Neptune systems | 42828.375816, 126712764.1, 37940584.8418, 5794556.4, 6836527.10058 | Park21 [P] |
| GM (km³ s⁻²): Pluto system | 975.5 | Park21 [P] |
| EMRAT | 81.3005682214972154 | DE440hdr [P] |
| β, γ in DE440 | 1, 1 (fixed) | DE440hdr, Park21 [P] |
| DE440 / DE441 span | 1550–2650 / −13200 to +17191 | Park21 [P] |
| DE440 small bodies | 343 asteroids, 30 KBOs, and a 36-point ring at 44 au | Park21 [P] |
| IAU 2015 B3 nominal values | (GM)☉ᴺ = 1.3271244×10²⁰ m³ s⁻². R☉ᴺ = 6.957×10⁸ m. (GM)ᴇᴺ = 3.986004×10¹⁴. (GM)ᴊᴺ = 1.2668653×10¹⁷. | Prsa16 Table 1 [P]. These are **conversion constants, not best estimates**, and must not be used for the precision rungs. |
| Mercury J2000 mean elements | a = 0.38709927 au, e = 0.20563593, I = 7.00497902°, ϖ = 77.45779628°, Ω = 48.33076593° | SW92 [P] |

---

### 5. SR rungs: derivation and gauge caveat

Write the general test-particle Lagrangian per unit mass, with U = GM/r:

L = ½v² + U + ε[A v⁴ + B U v² + C U² + D U (n·v)²]

To first order, each coefficient adds a perihelion advance per orbit. In units of πGM/(c²p):

| Coefficient | Advance per unit coefficient (πGM/(c²p)) |
|---|---|
| A (8A = 1 for the SR kinetic term) | 1 |
| B | 4 |
| C | 2 |
| D | 2 |

*(Corrected 2026-10-14, red-team review, Journal #9: per unit coefficient, A gives **8**, not 1; the value 1 holds only at A = 1/8, the SR kinetic value. B, C and D are per unit as listed, so the per-unit advances are 8A, 4B, 2C, 2D. The numerical 1.000 below was obtained at A = 1/8.)*

These coefficients were confirmed numerically in this session by direct integration of the Euler–Lagrange equations, using sympy and DOP853 at e = 0.2 and ε = 10⁻⁴. Results: 1.000, 3.999, 2.000 and 2.000.

Two gauges are compared below. In both, the full GR total is 6 units, which gives 42.98″/cy.

| Gauge | A | B | C | D | Full total | SR-a (A only) | Velocity-dependent terms only |
|---|---|---|---|---|---|---|---|
| PPN isotropic / harmonic (1PN), from expanding dτ/dt with the PPN metric | 1/8 | γ + ½ | ½ − β | 0 | 1 + 4(γ+½) + 2(½−β) = 2(2+2γ−β) → 6 | 1 → **1/6 GR** | 1 + 6 = **7/6 GR** |
| Schwarzschild areal | 1/8 | ½ | ½ | 1 | 1 + 2 + 1 + 2 = 6 | 1 → **1/6 GR** | 1 + 2 + 2 = **5/6 GR** |

Caveats:
- **SR-a, the 1/6 result, is robust in the sense that matters here.** The v⁴/8 term is identical in both gauges. It is a statement about a *model*, special-relativistic kinetic energy in a Newtonian 1/r potential, not about nature. LM10 v1 states the 1/6 factor. LM10 v6 instead claims **1/3** once a "relativistic potential energy" is added. Any SR rung must name its Lagrangian explicitly. Trump & Schieve (arXiv:physics/9805032) obtain **zero** for a Lorentz-invariant two-body interaction in Stueckelberg's covariant mechanics. So "the SR prediction" is model-dependent.
- **SR-b, "velocity-dependent 1PN only", is not gauge-invariant.** It gives 7/6 GR in isotropic/harmonic coordinates and 5/6 GR in areal coordinates. Only the full 1PN sum, (2+2γ−β)/3 × 42.98, is a coordinate-independent observable for a secular orbit-averaged rate. FerroAstro must label SR-b with its gauge and present it as pedagogical, never as "the SR part of GR".
- The textbook attribution of 1/6 to Sommerfeld and to a Goldstein problem (Ch. 7) is **UNVERIFIED** against the original texts. The derivation above was checked independently.

---

### 6. JPL Horizons: retrieving Mercury osculating elements

The call below was tested live on 2026-10-14. It returned `{source: DE441}`, the "Ecliptic of J2000.0" frame, and TDB time.

```
https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND='199'&OBJ_DATA='NO'&MAKE_EPHEM='YES'&EPHEM_TYPE='ELEMENTS'&CENTER='500@10'&START_TIME='2000-01-01'&STOP_TIME='2000-01-03'&STEP_SIZE='1%20d'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='ICRF'&OUT_UNITS='AU-D'&CSV_FORMAT='YES'
```

Notes from HzAPI [P] and the live output [P]:
- **Target:** `COMMAND='199'` selects Mercury itself. `'1'` selects the Mercury *barycenter*, and asteroids need a trailing `;` written as `%3B`.
- **Center:** `CENTER='500@10'` is the Sun body center. `'500@0'` is the solar-system barycenter.
- **Reference plane:**
  - `REF_PLANE` accepts `ECLIPTIC`, `FRAME` (ICRF equator) or `BODY EQUATOR`.
  - The elements are inertial (fixed J2000 ecliptic), so the general precession is **not** included.
  - To compare with Park17's 575.31, rotate the state vectors (`EPHEM_TYPE='VECTORS'`) into Mercury's mean-orbit frame (pole α ≈ 280.9876°, δ ≈ 61.4481°) before forming ϖ.
- **Units and format:**
  - Element tables are always in TDB.
  - Columns: EC, QR, IN, OM, W, Tp, N, MA, TA, A, AD, PR. ϖ = OM + W.
  - Encode spaces as `%20`. Quote values with single quotes. For `format=json`, check `signature.version`.
- **"Keplerian GM" printed:** 2.9591225740912142×10⁻⁴ au³/d², which equals GMS + GM1 (Sun + Mercury). Use the same μ when converting your own state vectors, so that the osculating a and e are comparable.
- **Fitting:**
  - Osculating elements carry large short-period terms (the planetary perturbations).
  - To get a secular rate, sample for centuries (DE441 covers −13200 to +17191), for example 1600–2600 at 1–5 day steps.
  - Fit the same model as Park17: B + ϖ̇t + Qt² + Σ periodic terms.
  - A linear fit over a short span will not reach 10⁻³ ″/cy.

---

### 7. Key caveats

- **C1, frames.** Values of 5599.74 (moving equinox), 577.72 (fixed J2000 ecliptic) and 575.31 (Mercury mean-orbit plane) are all "correct", but each is valid only in its own frame. Every rung output must state its frame.
- **C2, fit procedure.** Park17's per-planet numbers come from a 2000-year integration with a quadratic-plus-14-periodic fit. A different span or fit model shifts the 4th decimal. Tolerances below allow for this.
- **C3, Clemence vs modern.** Clemence's per-planet values differ from Park17 by up to 0.85″/cy (Earth: 90.038 vs 90.888, partly because the Moon is included). Clemence's "Earth" probably meant the Earth–Moon system with 1947 masses (1/329300). That attribution is **UNVERIFIED**. The table values are a secondary transcription [S] until the RMP scan is checked. Clemence's GR value of 43.03 also used non-modern constants: a 1985 *Nature* comment says unconventional au and c values were used (that content is **UNVERIFIED**).
- **C4, J2 and LT degeneracy.** The precession budget cannot separate β, J2 and S☉ from Mercury data alone. Park17 needed Cassini γ, and INPOP J2 moves from 2.01 to 2.218 when LT is modelled.
- **C5, INPOP inconsistencies.** INPOP19a's GM☉ appears as 40.042 ± 0.01 in S109 and as 42.0322 ± 0.13 in S110. INPOP21a also lists Park17's J2 as 2.28 ± 0.06, whereas Park17 itself gives 2.25 ± 0.09. Cite each directly from its own source.
- **C6, Park17 "Mercury" row (0.0050).** Its physical meaning is not explained in what was read. Plausibly it is Mercury's own mass effect in the heliocentric versus barycentric formulation, but that is **UNVERIFIED**.
- **C7, the 7×10⁻⁶ ″/cy two-body mass correction** is far below every other tolerance. N-2B and GR rungs may use μ = GM☉ + GM_Mercury or GM☉ alone, but the choice must be documented.

---

### Requirements implied

| ID | Requirement (testable) | Oracle / source |
|---|---|---|
| **PHY-001** | Rung N-2B: a Sun + Mercury Newtonian integration over ≥1000 yr shows \|ϖ̇\| < 1×10⁻⁶ ″/cy. Any nonzero value is integrator error and must be reported as such. | Analytic |
| **PHY-002** | All precision rungs use the DE440 constants exactly as listed in §4 (GMS, CLIGHT, AU, planetary GMs, EMRAT, ASUN, J2SUN, MOISUN, OMGSUN), loaded from a single versioned constants table with source citations. A unit test asserts GM☉ = 132712440041.279419 km³ s⁻² and c = 299792.458 km/s. | DE440hdr, Park21 |
| **PHY-003** | IAU 2015 B3 nominal values are available only under a separate `nominal` namespace, and a test asserts they are never used by ladder rungs. | Prsa16 |
| **PHY-004** | Rung GR-SCHW, analytic: 6πGM/(c²a(1−e²)) with DE440 GM☉ and SW92 a, e returns 42.9805 ± 0.0001 ″/cy (5.01866×10⁻⁷ rad/orbit). | [C] |
| **PHY-005** | Rung GR-SCHW, numerical: integrating the 1PN Schwarzschild test-particle EOM (γ = β = 1) and fitting ϖ̇ reproduces 42.980 ± 0.001 ″/cy and agrees with PHY-004 to within 0.001. | Park17 42.9799 ± 0.0009 |
| **PHY-006** | Rung PPN: the GR-SCHW rate scales as (2+2γ−β)/3 to within 10⁻⁶ relative for \|γ−1\|, \|β−1\| ≤ 10⁻³. For example, γ = 1, β = 1 + 3.9×10⁻⁵ shifts the rate by −0.00056 ″/cy. | Will14 Eq. 65 |
| **PHY-007** | Rung SR-a: the Lagrangian −mc²√(1−v²/c²) + GMm/r yields ϖ̇ = GR-SCHW/6 = 7.1634 ± 0.001 ″/cy. The analytic per-orbit value πGM/(c²p) must also be checked. | [C], LM10 |
| **PHY-008** | Rung SR-b: the output is labelled with its gauge. In the isotropic/harmonic gauge it gives 7/6 · GR = 50.144 ± 0.002 ″/cy, and in the areal gauge 5/6 · GR = 35.817 ± 0.002 ″/cy. Docs must state that this split is not an observable. | §5 [C] |
| **PHY-009** | Term-by-term check: each 1PN coefficient (A, B, C, D in §5) on its own reproduces 1, 4, 2, 2 × πGM/(c²p) per orbit to within 10⁻³ relative. | §5 [C] |
| **PHY-010** | Rung J2: an analytic check gives 3πJ₂R☉²/p² per orbit, so 0.02860 ″/cy at J2 = 2.25×10⁻⁷ and 0.02792 ″/cy at the DE440 J2 (R☉ = 696000 km). The numerical rung must match within ±0.0005 ″/cy, and the solar pole orientation must be documented. | Park17 0.0286 ± 0.0011 |
| **PHY-011** | Rung LT: with S☉ = 1.90×10⁴¹ kg m² s⁻¹ (DE440 MOISUN × M☉R☉² × OMGSUN) and the INPOP21a Eq. 1 acceleration, the rung yields −0.0020 ± 0.0002 ″/cy. | Park17, Park21 |
| **PHY-012** | Rungs N-VEN … N-NEP: each single-planet contribution reproduces the Park17 Table 3 value within ±0.005 ″/cy when computed with the Park17 procedure (Mercury mean-orbit frame, 1000–3000 CE, quadratic + periodic fit). The expected values are Venus 277.4176, E/M 90.8881, Mars 2.4814, Jupiter 153.9899, Saturn 7.3227, Uranus 0.1425 and Neptune 0.0424. | Park17 |
| **PHY-013** | Rung N-SUM: the full Newtonian N-body rate, including asteroids and cross terms, is 532.30 ± 0.01 ″/cy in the Park17 frame. A comparison against Clemence's 531.5 ± 0.7 is shown only for information and is not asserted. | Park17, Clem47 [S] |
| **PHY-014** | Rung EIH-FULL: DE440-equivalent physics (EIH N-body, γ = β = 1, J2, LT, 343 asteroids, KBO ring) reproduces 575.310 ± 0.005 ″/cy in the Park17 mean-orbit frame. The tolerance is 3× Park's σ, plus margin for the model difference between DE430 and DE440. | Park17 |
| **PHY-015** | Every reported ϖ̇ carries metadata: frame (Mercury mean-orbit, J2000 ecliptic, or equinox of date), epoch, fit span, fit model and constants version. A test rejects any rung output that lacks it. | §1, C1 |
| **PHY-016** | Horizons cross-check: a fetcher retrieves DE441 Mercury elements (`COMMAND='199'`, `CENTER='500@10'`) or vectors for ≥ 500 yr. FerroAstro's EIH-FULL propagation agrees in ϖ̇ to ≤ 0.01 ″/cy after both use the same frame and fit. The test records the Horizons API version and the ephemeris source string. | HzAPI |
| **PHY-017** | Equinox-referred showcase: when the 5600″/cy-class number is displayed, it is built explicitly by adding a cited general-precession model (IAU 2006 p_A, **to be verified in Cap03**). It is never compared numerically with Clemence's 5599.74 ± 0.41 beyond that stated uncertainty. | Clem47 [S], Cap03 |
| **PHY-018** | Before any value marked [S] or UNVERIFIED in this entry is used as a test oracle, it is confirmed against the primary scan or PDF. This applies to Clem47 Table II, EPM β/γ/J2, Cap03 p_A, IAU 2012 B2 and the GMSDOT units. | This entry |

**Open items:** obtain Clem47 scan; read PP14 / EPM2017 (Pitjeva & Pitjev 2018) for EPM β, γ, J2; confirm Cap03 p_A; obtain the full DE440 paper text past character 100k to see whether it quotes a GM☉ uncertainty or a J2 rationale.
