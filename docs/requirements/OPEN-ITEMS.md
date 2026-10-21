<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Open items: UNVERIFIED and secondary-source claims

This checklist collects every claim in Journals #0–#6 (plus items raised by the Journal #9 red-team review and the 2026-10-20 build-evidence sync) that is marked **UNVERIFIED** or **[S]** (secondary transcription), plus the internal inconsistencies found while consolidating them. Each item lists the requirement(s) it blocks or qualifies and the journal it came from. An item is checked off only when the primary source has been opened and the result recorded, either in a `tests/vectors/` entry with `verified_by` (TST-002, PHY-018) or in an ADR. Per NUM-020, each Journal #4 item also needs a tracking issue.

**Blocks** names requirements whose acceptance cannot be asserted until the item is closed. **Qualifies** names requirements that can proceed but carry the caveat.

---

## Journal #1 — Physics truth data

- [ ] **OI-01** Clemence 1947 (Rev. Mod. Phys. 19, 361) Table II values are [S]: observed 5599.74 ± 0.41, general precession 5025.645 ± 0.50, theory sum 5557.18 ± 0.85, residual 42.56 ± 0.94, GR 43.03 ± 0.03, and the per-planet values. Obtain the scan. **Blocks:** PHY-017, PHY-018. **Qualifies:** PHY-012, PHY-013 (informational comparison only).
- [ ] **OI-02** IAU 2006 general precession p_A linear term 5028.796195 ″/cy (Capitaine, Wallace & Chapront 2003) is UNVERIFIED; it was seen only in a search snippet. Journal #4 repeats it as UNVERIFIED. **Blocks:** PHY-017. *Update (build-evidence sync 2026-10-20, WP9 on `m3-newtonian-nbody`):* NUM-014 no longer depends on this item. Its check takes p_A from `ferroastro-erfa`'s port of ERFA `p06e`, verified against ERFA's `t_erfa_c` reference values and the pyerfa 2.0.1.5 goldens (bitwise in f64), never from the snippet. The item stays open because PHY-017 still needs the published coefficient. **Qualifies:** NUM-014 (historical note only).
- [ ] **OI-03** EPM β, γ, J2 (β = 0.99998 ± 0.00003, γ = 1.00004 ± 0.00006, J2 = (2.0 ± 0.2)×10⁻⁷ for EPM2011; J2 = (2.22 ± 0.23)×10⁻⁷ for EPM2013) come from secondary sources only. Read PP14 / EPM2017 (Pitjeva & Pitjev 2018). **Qualifies:** PHY-018.
- [ ] **OI-04** The units of DE440 GMSDOT = −9.0×10⁻¹⁴ are not stated in any public primary source. **Research 2026-10-20 (Journal #11):**
  - It is absent from the DE440 paper (Park et al. 2021) and from JPL's DE440 web page.
  - The header and the NAIF tech-comments list it without units. It first appears in DE440; it is absent from DE421–DE438 and present in DE441 and DE442.
  - The probable primary source is JPL IOM 392R-20-002, which is not public.
  - **Adopted inference (maintainer decision 2026-10-20):** a fractional rate in yr⁻¹, GM☉(t) = GMS·[1 + GMSDOT·(t − JDEPOC)/365.25], with t in TDB days. Only this reading matches the physics: radiation ≈ −6.8×10⁻¹⁴/yr plus solar wind gives ≈ −9×10⁻¹⁴/yr. Every other unit reading is off by ≥400× or is negligible.
  - **Impact:** none on M0 or the precession rungs. In M5, a t² along-track drift of about 28 km (Earth) and 46 km (Mercury) at the 1550/2650 span ends, which the initial conditions cannot absorb.
  - **No longer blocks** PHY-002 or REF-002 (the value is parsed and carried, never asserted with units). **Blocks** the M5 DE440 reproduction (CI-007): fit both conventions (per-year fractional vs none) against de440.bsp over 1550–2650, adopt the one that matches, and close on that evidence.
  - Source hashes: header.440 sha256 0f4636b6…6ac3359; Park 2021 PDF sha256 96c3945d…34f68ee0.
  - **Qualifies:** PHY-018.
- [x] **OI-05** The IAU 2012 Resolution B2 text (exact au = 149597870.700 km) is UNVERIFIED. The value itself is [P] from header.440. **Qualifies:** PHY-002, PHY-018. **Closed 2026-10-20** (maintainer decision A). Evidence: the Resolution B2 PDF from the SYRTE mirror, https://syrte.obspm.fr/IAU_resolutions/Res_IAU2012_B2.pdf (the iau.org `static/resolutions` URL returned 404), sha256 `3489ebb1e51f7c278d56c9e0f9cb49c142e036ac34e21defc87414507afec984`. It reads: "recommends 1. that the astronomical unit be re-defined to be a conventional unit of length equal to 149 597 870 700 m exactly". The WP1 reviewer opened it on 2026-10-14 and recorded it in the `verified_by` of vector `CONST-AU-IAU2012` (`tests/vectors/defined-constants.toml`); the progress review after WP5 re-downloaded it and found the same hash and text (`docs/strategy/IMPLEMENTATION-NOTES.md`, WP1 adversarial review and "Progress review: M0-M2 after WP5" §3, on `m0-foundation` and `m3-newtonian-nbody`). header.440's `AU` equals this value exactly (`data_de440_header_defined_constants`).
- [ ] **OI-06** DE440 paper (Park et al. 2021) past character 100k is unread: does it quote a GM☉ uncertainty or a J2 rationale? **Qualifies:** PHY-002.
- [ ] **OI-07** The attribution of the SR 1/6 result to Sommerfeld and to a Goldstein Ch. 7 problem is UNVERIFIED. Journal #4 likewise flags the Sommerfeld 1916 attribution of the exact formula. **Qualifies:** PHY-007, NUM-016.
- [ ] **OI-08** Caveat C3: that Clemence's "Earth" means the Earth–Moon system with 1947 masses (1/329300) is UNVERIFIED. So is the 1985 *Nature* comment that Clemence used unconventional au and c. **Qualifies:** PHY-013.
- [ ] **OI-09** Caveat C6: the physical meaning of the Park17 "Mercury" self row (0.0050 ″/cy) is UNVERIFIED. **Qualifies:** PHY-012, PHY-013, PHY-014 (the row equals the whole ±0.005 PHY-014 tolerance).
- [ ] **OI-10** Rung N-MOON (Moon resolved separately from Earth) has no primary value; Park17 gives only E/M combined. **Blocks:** any future N-MOON rung under ORG-002.
- [ ] **OI-11** Caveat C5, an inconsistency rather than an unverified claim: INPOP19a GM☉ − 132712440000 is 40.042 ± 0.01 in IMCCE note S109 but 42.0322 ± 0.13 in S110. INPOP21a also lists Park17's J2 as 2.28 ± 0.06, whereas Park17 gives 2.25 ± 0.09. Rule: cite each value from its own source. **Qualifies:** PHY-002, PHY-010.

## Journal #2 — Rust ecosystem and licenses

- [ ] **OI-12** Can a DD type (qd, twofloat or our own) implement `simba::RealField` cheaply enough to reuse nalgebra in the hot loop? **Qualifies:** DEP-007, NUM-006.
- [ ] **OI-13** Does `uom` accept a custom (DD) storage type? **Closed by Journal #6** (uom not used, API-001); kept for the record. **Qualifies:** API-001.
- [ ] **OI-14** Do `faer` and `anise` build for wasm32-unknown-unknown? **Qualifies:** DEP-006, DEP-011.
- [ ] **OI-15** Does nightly Rust `f128` provide math functions (sin, exp, …) on wasm32 and x86_64? What is its current stabilisation status (rust-lang/rust#116909)? **Qualifies:** DEP-007, NUM-006.
- [ ] **OI-16** Do the IAU SOFA licence terms bind `sofars` users, and transitively `anise` users? This needs a legal read. **Blocks:** any future anise MPL exception in DEP-002 (anise is not a dependency; decision deferred to M5, OI-72). **Qualifies:** DEP-009, LIC-004.
- [ ] **OI-17** The NAIF CSPICE licence terms (via `rust-spice`) are UNVERIFIED. **Qualifies:** DEP-003, DEP-015.
- [ ] **OI-18** JPL DE440/DE441 kernel redistribution terms are UNVERIFIED (also in Journals #3 and #5). **Blocks:** any relaxation of DEP-017 or LIC-003 (e.g. shipping a DE segment in the WASM demo). **Qualifies:** REF-001.
- [ ] **OI-19** Was `thor-rs-assist` derived from GPL REBOUND/ASSIST code? **Blocks:** removing it from the DEP-003 ban list (DEP-015).
- [ ] **OI-20** Is UMFPACK, as linked by `russell_sparse`, GPL-licensed ("per our knowledge")? **Qualifies:** DEP-016.
- [ ] **OI-21** `rmcp`'s licence: crates.io says Apache-2.0, GitHub reports NOASSERTION. Read the LICENSE file. **Blocks:** DEP-013 under DEP-015.
- [ ] **OI-22** `astro-float` is assumed, not benchmarked, to be too slow for production runs. **Qualifies:** DEP-012.

## Journal #3 — Reference implementations and CI

- [x] **OI-23** header.440: does `CENTER = 0` mean barycentric ICRF? What does the `XC…` block mean? Are the state units au and au/day (inferred)? **Blocks:** REF-002 and PHY-002 (both M0 exit requirements; red-team review 2026-10-14), PHY-014. **Closed 2026-10-20** (maintainer decision B): with `CENTER = 0` the header states are SSB-centred, ICRF-aligned, in au and au/day (AU = header `AU` km, day = 86 400 s TDB); `XB` is the Earth–Moon barycentre, `XM` is Moon − Earth, `XS` the Sun. Evidence (WP11 check 1, `m3-newtonian-nbody`): `crates/ferroastro-system/tests/ref002_header_initial_conditions.rs::ref002_header_initial_conditions_equal_de440_at_jdepoc` matches de440.bsp at JDEPOC for every body to < 1e-6 km and < 1e-14 km/s (4.77e-7 km, 7.1e-15 km/s in f64), while the heliocentric reading misses by 6.7e5 km and the km reading by ≥ 5.4e7 km; JPL Horizons DE441 at JDEPOC agrees with the header states to 9.5e-7 km (WP11 check 7); `tools/oracles/header440.py`, an independent header reader run with jplephem in the jpl oracle image, reproduces 4.768e-7 km (WP16 check 3). Notes: `docs/strategy/IMPLEMENTATION-NOTES.md` WP11 ("OI-23 evidence"). The unexplained `XC…ZDC` block (all zero in DE440, not used) is split off as **OI-73**.
- [ ] **OI-24** How does JPL supply asteroid initial conditions at JDEPOC? What does `sb441-n373.bsp` contain, and does n373 = 343 asteroids + 30 KBOs? Read IOM 392R-21-005. **Blocks:** REF-003, PHY-014, CI-007. **Qualifies:** PHY-019 (initial conditions are DE441's; which bodies Park's asteroid row covers). *Evidence, not a closure (WP10, WP11, WP15 on `m3-newtonian-nbody`):* only `sb441-n373s.bsp` (982 MB, an SPKMERGE subset over 1550–2650, DE441) is pinned; it holds 373 targets = the 343 numbered header asteroids + 30 TNOs. 3 of the 30 header KBO masses are identified by a pre-registered rule (MA8001 Eris, MA8007 Orcus, MA8009 2002 UX25); 27 are not placed; the 36 ring masses have no state and their geometry is FerroAstro's. DE440's own asteroid ICs are in no pinned product. Park 2017's text (re-read by the WP15 agent) attributes its asteroid row's uncertainty to "the 343 asteroids", and its ephemeris has no KBOs. PHY-019 is no longer blocked by this item (maintainer decision 2026-10-20); it remains qualified.
- [ ] **OI-25** Original Mercury6 licence (the mirror is labelled GPL-3.0), Swifter availability, licence and GR support, and the NOVAS F3.1 licence. **Qualifies:** REF-008, LIC-001, LIC-003. *Finding (WP17), not a closure:* the GPL-3.0 `LICENSE` of `smirik/mercury@aee9e0f6` was added by the mirror's maintainers; no licence statement exists in the original files; the mirror is a modified (double-precision) derivative. Swifter and NOVAS were not examined.
- [ ] **OI-26** Which Orekit release introduced `J2OnlyPerturbation`? Can `LenseThirringRelativity` be re-pointed at the Sun? Is there really no built-in coupled multi-body propagator? **Qualifies:** REF-006, REF-010. *Finding (WP17), not a closure:* `J2OnlyPerturbation` exists in 13.1.9 (no `@since`); `LenseThirringRelativity(gm, bodyFrame)` is `@since 10.3`, spin model not determined; the multi-body question was not examined.
- [ ] **OI-27** The exact JPL binary DE file path for Orekit (`Linux/de440/linux_p1550p2650.440`), and the conda-forge status of the JCC Orekit wrapper. **Qualifies:** REF-006. *Finding (WP17), not a closure:* the path exists (102 272 352 B) and Orekit 13.1.9 loads it; pinned as `de440-linux-binary`. Orekit's Sun inertially oriented frame is rotated 0.535 rad from ICRF. The JCC wrapper was not examined.
- [ ] **OI-28** Do REBOUND/REBOUNDx publish manylinux wheels, or does the image need a compiler? **Qualifies:** REF-005, CI-001. *Answer recorded (WP16), awaiting a maintainer tick:* rebound 5.2.2 has a cp312 manylinux wheel; reboundx 5.1.0 is an sdist only, so the image pins a C compiler. Swiftest 2026.7.1 ships manylinux wheels (WP17).
- [ ] **OI-29** Which DE does Horizons use internally now? Journal #1's live call reported `DE441`. **Qualifies:** PHY-016; the source string is recorded per run.
- [ ] **OI-30** Full citation of the TRACE integrator paper (Lu, Hernandez & Rein 2024). **Qualifies:** none (bibliography only).
- [ ] **OI-31** Whether Swiftest still supports a plain J2 input (`j2rp2`). **Qualifies:** REF-007. *Finding (WP17), not a closure:* the 2026.7.1 API documents `add_body(..., j2rp2=, j4rp4=)`; not exercised.
- [ ] **OI-32** No maintained, openly licensed Fortran Gauss–Jackson planetary integrator was found; this is treated as a gap. **Qualifies:** none.

## Journal #4 — Numerical methods

- [ ] **OI-33** Second, independent transcription of the EIH equation (DE430 Eq. 27 / DE440 §3.1) against the typeset PDFs. **Blocks:** NUM-020, NUM-017, PHY-014.
- [ ] **OI-34** The Saha & Tremaine 1994 H_PN coefficients (Eq. 26) need checking against the typeset paper. **Blocks:** NUM-005.
- [ ] **OI-35** The R3b Lense–Thirring acceleration: does the IERS-style typography match the DE440 equation and the INPOP21a Eq. 1 form named in PHY-011? **Blocks:** NUM-019, NUM-015 (R3b oracle). **Qualifies:** PHY-011. **Evidence available, pending sign-off** (WP4 on `m3-newtonian-nbody`): INPOP21a (IMCCE S110) could not be retrieved, so Eq. 1 equivalence is not shown; the implemented R3b reproduces Park et al. 2017 Eq. 2 (sign, (1+γ), cos i) to 1.25e-6 relative (equatorial) and 7.7e-8 (real geometry). Whether that closes the item, or PHY-011 should name Park17 Eq. 2 instead of INPOP21a Eq. 1, is a human call.
- [ ] **OI-36** IERS Conventions 2010 Ch. 10 equation numbers for the R3 Schwarzschild term. **Qualifies:** NUM-015, NUM-017.
- [ ] **OI-37** Citation for the inclined-orbit J₂ rates ω̇, Ω̇. **Blocks:** NUM-015 (inclined J₂ oracle). **Qualifies:** PHY-010.
- [ ] **OI-38** Park 2017's exact switch-on/switch-off protocol for the per-body decomposition. Provisional protocol (red-team review 2026-10-14): Sun + Mercury + body X minus Sun + Mercury, Newtonian. Cross terms (up to 0.0411 ″/cy) exceed the ±0.005 tolerance, so the oracle is ill-posed until this is confirmed. **Qualifies:** PHY-012, NUM-013 part (b), PHY-019 (no longer **Blocks**, maintainer decision C of 2026-10-20). *Update 2026-10-20:* the requirements now use "report both" (ADR-0014): assert P-remove (full system minus full system without X), report P-add. P-remove was chosen after the WP14/WP17 results: P-add (pre-registered) missed 4 of 7 rows (Venus −0.022706, Earth/Moon −0.018997, Jupiter +0.038832, Saturn +0.043049 ″/cy); P-remove passes all 7 within 0.0007; four tools agree on P-add Venus to 2e-8. Remaining question: **confirm the protocol against the Park 2017 text** (§4, quoted in the WP14 notes). **Revert condition:** if the Park 2017 text says P-add, PHY-012/NUM-013(b) revert to asserting P-add and the 4 misses are treated as real discrepancies (`type:physics-discrepancy`). Ticking this box also arms the `blocked_by = "OI-38"` vectors (ADR-0004 revision), so the tick must land with the matching harness/vector change.
- [ ] **OI-39** The slope-bias constant for an unmodelled sinusoid (~A·P/T²) and the achievable-precision estimates (10⁻⁴–10⁻⁵ ″/cy differential, ~10⁻³ ″/cy total) are derived and UNVERIFIED until the NUM tests run. **Qualifies:** NUM-011, NUM-013.
- [ ] **OI-40** The form of a 1PN-corrected LRL vector. **Qualifies:** NUM-012.
- [ ] **OI-41** Whether production EPM uses exactly ABM13 PECEC with h = 1/16 d. **Qualifies:** NUM-002.
- [ ] **OI-42** Cost estimates: f64 + compensated summation ~1.1–1.3×; emulated f128 in full use; EIH ~3–5× Newtonian flops per pair; typical IAS15 step ~1 d for Mercury. **Qualifies:** NUM-006, NUM-009, NUM-024. *Evidence (WP15):* with Newtonian Majors the tiered run costs 28.2× the 16-Major run (pair count predicts ≈ 46× for the forces), so NUM-009's < 2× bound can only be tested with EIH Majors (WP19); its vector is `blocked_by = "OI-42"`.
- [ ] **OI-43** Extrapolated f64 error growth (t^(3/2): ~0.3 km at 10³ yr, ~3×10² km at 10⁵ yr). **Qualifies:** NUM-007.
- [ ] **OI-44** The CPU used for Subbotin's 81 s / 246 s timings is unspecified, so hardware normalisation is UNVERIFIED. **Qualifies:** NUM-024.
- [ ] **OI-45** Feasibility of a ≥ 2.5× SIMD speedup at N = 16; current API status of stable SIMD crates (`wide`, `pulp`). **Qualifies:** NUM-021.
- [ ] **OI-46** The threading crossover inside a step, and heyoka's batch mode. **Qualifies:** NUM-022.
- [ ] **OI-47** The Taylor-integrator jet cost factor. **Qualifies:** NUM-025.
- [ ] **OI-48** Inner-solar-system Lyapunov time ~5 Myr (Laskar 1989); La2010 and La2004 integrator details. **Qualifies:** none (context).
- [ ] **OI-49** Citation details: Rein & Tamayo 2015 (MNRAS 452, 376 vol/page), Wisdom & Holman 1991, Everhart 1985 page, Quinlan & Tremaine 1990, Berry & Healy 2004 page range, Moyer 2003 DOI, La2010 volume. **Qualifies:** NUM-004 and bibliography.
- [ ] **OI-50** Direction conflict: Journal #4 says the DE430 J₂ (2.1106088532726840×10⁻⁷), estimated without LT, was "~7% too high". Journal #1 says a J₂ fitted without LT is biased **low** (INPOP19a 2.01). Also, DE430's value is below DE440's 2.196×10⁻⁷. Re-read Park et al. 2021. **Provisional resolution (maintainer, 2026-10-14):** biased **low**. Journal #4 and NUM-019 are corrected; keep this open until the Park 2021 sentence is read. *Caveat (red-team review 2026-10-14):* INPOP's J₂ shift 2.01→2.218 (9.4%) exceeds the ~7% LT share, and the releases also differ in data and GM☉ (C5), so the INPOP and DE430/DE440 values are **consistent in sign** only; the full shift is not attributed to LT. **Qualifies:** NUM-019, PHY-010 (narrative only; no acceptance number depends on it).

> **Legal items deferred (maintainer decision 2026-10-14):** the CLA text, JPL DE / NAIF redistribution terms and SOFA terms via `sofars` are deferred until before the first release. Conservative defaults apply meanwhile: DCO, fetch-on-demand data with SHA-256 pins, and `anise` not a dependency (decision deferred to M5, OI-72).

## Journal #5 — Organization and standards

- [ ] **OI-51** Which NPR 7150.2D SWEs apply at Class D/E (Appendix C not reviewed). **Qualifies:** ORG-007.
- [ ] **OI-52** ECSS-Q-ST-80C Table D-1 criticality definitions, and whether Q-ST-80C is now at Rev.2. **Qualifies:** ORG-007.
- [ ] **OI-53** Effort needed for the OpenSSF Best Practices "passing" badge. **Qualifies:** STD-012.
- [ ] **OI-54** Maintenance status of `EndBug/label-sync`. **Qualifies:** ORG-006.
- [ ] **OI-55** Toolchain channel: 1.95.0 is a placeholder (ADR-0006), bumped to the then-current stable at M0; stabilisation of rustfmt `imports_granularity` / `group_imports`. **Qualifies:** STD-001, STD-005.
- [ ] **OI-56** `#[expect]` stabilisation version (1.81), and the clippy group membership of float_cmp_const, lossy_float_literal, suboptimal_flops, imprecise_flops, unwrap_used and indexing_slicing. **Qualifies:** STD-002, TST-001.
- [ ] **OI-57** Whether nextest still skips doctests. **Qualifies:** STD-008.
- [ ] **OI-58** The orekit-jpype packaging choice for `tools/`, and the pre-commit hook IDs and revisions. **Qualifies:** STD-009, STD-010.
- [ ] **OI-59** The legal status of CLAs for AI-generated contributions (needs counsel). Who signs for agent PRs? **Blocks:** ORG-010. **Qualifies:** ORG-008 (DCO sign-off by the operating human or org).
- [ ] **OI-60** Whether `CLAUDE.md` containing `@AGENTS.md` is still the recommended Claude Code import pattern. **Qualifies:** AGT-001.
- [ ] **OI-61** Stabilisation status of rustdoc JSON output. **Qualifies:** none (CI artifact only).
- [ ] **OI-62** MCP tool errors via the `isError` result flag (check the spec's error-handling section). **Qualifies:** AGT-004.
- [ ] **OI-63** Benchmark variance figures on shared GitHub runners. **Qualifies:** CI-008.
- [ ] **OI-64** Whether release-please bumps Cargo-workspace internal dependencies (evaluate release-plz). **Qualifies:** STD-011.
- [ ] **OI-65** Whether optional signed commits interact cleanly with agent workflows. **Qualifies:** ORG-009.
- [ ] **OI-66** Fit of StrictDoc and OpenFastTrace (ADR-0010, `docs/adr/0010-requirements-tooling.md`, revisited at M2). **Qualifies:** ORG-003, ORG-004.
- [ ] **OI-67** IERS EOP redistribution terms in test fixtures (open question 5). **Qualifies:** DEP-017, LIC-003.

## Consolidation inconsistencies needing maintainer confirmation

These are not UNVERIFIED claims. They are conflicts between journals, provisionally resolved in the requirement `notes`.

- [ ] **OI-68** CI-009 (LRL-vector estimator) versus NUM-011 (Park2017 canonical). Provisional resolution: NUM-011 is authoritative and LRL is secondary. **Affects:** CI-009, NUM-011, NUM-012.
- [ ] **OI-69** DEP-014 says criterion runs on every PR; CI-008 and Journal #5's bench.yml measure only on the fixed runner class. Provisional resolution: compile benchmarks per PR, measure under CI-008. **Affects:** DEP-014, CI-008, NUM-024.
- [ ] **OI-70** NUM-013's 10⁻⁴ ″/cy differential tolerance versus Journal #4 R4's ≲ 10⁻² ″/cy planet-interaction residuals in N-body GR differentials. Provisional resolution: 10⁻⁴ applies to single-body isolation only. **Affects:** NUM-013, PHY-005.
- [ ] **OI-71** CI-010's resume-equivalence criterion (bitwise identity) is derived from TST-005/NUM-021, not stated in Journal #6. **Affects:** CI-010.

## Journal #9 — Red-team review

- [ ] **OI-72** Whether to adopt `anise` at all; decision deferred to **M5** (maintainer decision 2026-10-14). Until then anise is not a dependency and deny.toml has no anise exception. Inputs to the decision: SOFA terms via `sofars` (OI-16); anise 0.10.6's graph fails the current allow-list (BlueOak-1.0.0 via `minicbor`, CDLA-Permissive-2.0 via `webpki-roots`) and has a RustSec advisory (`proc-macro-error2` unmaintained); anise's `build.rs` has a build-time network path (build-dependency `ureq`); and cargo-deny licence exceptions ignore dependency kind, so "dev-only" would need a second config with `[graph] exclude-dev = true` and no anise exception (red-team L1/L2, Journal #9). **Blocks:** the ANISE cross-validation in DEP-011, any anise entry in deny.toml. **Qualifies:** DEP-002. *2026-10-20:* DEP-011's acceptance now uses Horizons + the DE440 ASCII products + jplephem bit-identity until this decision (WP10).

## Build-evidence sync 2026-10-20

Raised while syncing the catalogue with the build branches (`m0-foundation`, `m3-newtonian-nbody`; `docs/strategy/IMPLEMENTATION-NOTES.md`, WP1–WP18). Each one is a question the evidence does not settle.

- [ ] **OI-73** header.440's `XC…ZDC` block is all zero in DE440 and its meaning is unexplained (split from OI-23 when that item closed). FerroAstro does not read it (WP11). **Qualifies:** REF-002.
- [ ] **OI-74** Two NUM-011 decisions. (a) Scope of "canonical observable for every absolute rate": the single-body rungs report the initial-orbit-plane passage estimator, which agrees with Park2017 to ≤ 2e-8 ″/cy on them (WP4, WP6); decide whether every rung must report Park2017 or the clause is restricted. (b) Whether Park 2017 used heliocentric or barycentric perihelion passages; FerroAstro uses heliocentric (WP5 "Centre", carried in every progress review). **Qualifies:** NUM-011, PHY-016.
- [ ] **OI-75** Human second reading of the relativistic barycentre μᵢ* = GMᵢ [1 + vᵢ²/(2c²) − (1/(2c²)) Σⱼ≠ᵢ GMⱼ/rᵢⱼ] against Park et al. 2021 Eq. 2, and of the equation numbers NUM-018 cites ("Eqs. 1–2"). WP11 wrote it from memory; the WP15 agent compared it with the printed Eq. 2 and found it equal. **Qualifies:** NUM-018 (sign-off).
- [ ] **OI-76** DE440's solar J2 term: which solar pole it uses (FerroAstro defaults to IAU WGCCRE 2015, 286.13°/63.87°), and whether it acts on the asteroids (FerroAstro applies it to Major bodies only). Read Park et al. 2021 §3 (WP12 deviations 3 and 4). **Qualifies:** PHY-010, NUM-019, NUM-009, PHY-014.
- [x] **OI-77** μ convention for analytic rates: the ″/cy conversion uses n = √((GM☉ + GM☿)/a³) in `ferroastro-analytic`/`ferroastro-ladder` (WP13 review correction F5), but n from GM☉ in the WP14 NUM-013(a) scenario check, and WP11 proposed the rule "n uses the μ of the Keplerian problem that produced the elements". Choose one rule for requirement text. The difference is 3.57e-6 ″/cy. **Qualifies:** PHY-004, NUM-015, NUM-013. **Closed 2026-10-21 (maintainer decision):** the ″/cy conversion uses n = √((GM☉ + GM☿)/a³), the two-body Keplerian μ, which is also the "Keplerian GM" Horizons prints. The WP14 NUM-013(a) scenario harness must be changed to match (build branch follow-up).
- [ ] **OI-78** NUM-009 text: (a) which 16 bodies are "Major" (FerroAstro: 11 DE440 majors + the 5 largest-GM asteroids 1, 4, 2, 10, 511; check Subbotin et al. 2023); (b) whether Minor bodies act on the Majors (FerroAstro default: yes, configurable; DE440 practice not checked) (WP11 review, WP12 deviation 2, WP15 deviation 6). **Qualifies:** NUM-009, PHY-019.
