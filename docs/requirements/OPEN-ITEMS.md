<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Open items: UNVERIFIED and secondary-source claims

This checklist collects every claim in Journals #0–#6 that is marked **UNVERIFIED** or **[S]** (secondary transcription), plus the internal inconsistencies found while consolidating them. Each item lists the requirement(s) it blocks or qualifies and the journal it came from. An item is checked off only when the primary source has been opened and the result recorded, either in a `tests/vectors/` entry with `verified_by` (TST-002, PHY-018) or in an ADR. Per NUM-020, each Journal #4 item also needs a tracking issue.

**Blocks** names requirements whose acceptance cannot be asserted until the item is closed. **Qualifies** names requirements that can proceed but carry the caveat.

---

## Journal #1 — Physics truth data

- [ ] **OI-01** Clemence 1947 (Rev. Mod. Phys. 19, 361) Table II values are [S]: observed 5599.74 ± 0.41, general precession 5025.645 ± 0.50, theory sum 5557.18 ± 0.85, residual 42.56 ± 0.94, GR 43.03 ± 0.03, and the per-planet values. Obtain the scan. **Blocks:** PHY-017, PHY-018. **Qualifies:** PHY-012, PHY-013 (informational comparison only).
- [ ] **OI-02** IAU 2006 general precession p_A linear term 5028.796195 ″/cy (Capitaine, Wallace & Chapront 2003) is UNVERIFIED; it was seen only in a search snippet. Journal #4 repeats it as UNVERIFIED. **Blocks:** PHY-017, NUM-014.
- [ ] **OI-03** EPM β, γ, J2 (β = 0.99998 ± 0.00003, γ = 1.00004 ± 0.00006, J2 = (2.0 ± 0.2)×10⁻⁷ for EPM2011; J2 = (2.22 ± 0.23)×10⁻⁷ for EPM2013) come from secondary sources only. Read PP14 / EPM2017 (Pitjeva & Pitjev 2018). **Qualifies:** PHY-018.
- [ ] **OI-04** The units of DE440 GMSDOT = −9.0×10⁻¹⁴ (presumably au³/d² per year) are UNVERIFIED. **Blocks:** PHY-002 (if GMSDOT is used), PHY-018.
- [ ] **OI-05** The IAU 2012 Resolution B2 text (exact au = 149597870.700 km) is UNVERIFIED. The value itself is [P] from header.440. **Qualifies:** PHY-002, PHY-018.
- [ ] **OI-06** DE440 paper (Park et al. 2021) past character 100k is unread: does it quote a GM☉ uncertainty or a J2 rationale? **Qualifies:** PHY-002.
- [ ] **OI-07** The attribution of the SR 1/6 result to Sommerfeld and to a Goldstein Ch. 7 problem is UNVERIFIED. Journal #4 likewise flags the Sommerfeld 1916 attribution of the exact formula. **Qualifies:** PHY-007, NUM-016.
- [ ] **OI-08** Caveat C3: that Clemence's "Earth" means the Earth–Moon system with 1947 masses (1/329300) is UNVERIFIED. So is the 1985 *Nature* comment that Clemence used unconventional au and c. **Qualifies:** PHY-013.
- [ ] **OI-09** Caveat C6: the physical meaning of the Park17 "Mercury" self row (0.0050 ″/cy) is UNVERIFIED. **Qualifies:** PHY-012, PHY-013.
- [ ] **OI-10** Rung N-MOON (Moon resolved separately from Earth) has no primary value; Park17 gives only E/M combined. **Blocks:** any future N-MOON rung under ORG-002.
- [ ] **OI-11** Caveat C5, an inconsistency rather than an unverified claim: INPOP19a GM☉ − 132712440000 is 40.042 ± 0.01 in IMCCE note S109 but 42.0322 ± 0.13 in S110. INPOP21a also lists Park17's J2 as 2.28 ± 0.06, whereas Park17 gives 2.25 ± 0.09. Rule: cite each value from its own source. **Qualifies:** PHY-002, PHY-010.

## Journal #2 — Rust ecosystem and licenses

- [ ] **OI-12** Can a DD type (qd, twofloat or our own) implement `simba::RealField` cheaply enough to reuse nalgebra in the hot loop? **Qualifies:** DEP-007, NUM-006.
- [ ] **OI-13** Does `uom` accept a custom (DD) storage type? **Closed by Journal #6** (uom not used, API-001); kept for the record. **Qualifies:** API-001.
- [ ] **OI-14** Do `faer` and `anise` build for wasm32-unknown-unknown? **Qualifies:** DEP-006, DEP-011.
- [ ] **OI-15** Does nightly Rust `f128` provide math functions (sin, exp, …) on wasm32 and x86_64? What is its current stabilisation status (rust-lang/rust#116909)? **Qualifies:** DEP-007, NUM-006.
- [ ] **OI-16** Do the IAU SOFA licence terms bind `sofars` users, and transitively `anise` users? This needs a legal read. **Blocks:** approval of the anise MPL exception in DEP-002. **Qualifies:** DEP-009, LIC-004.
- [ ] **OI-17** The NAIF CSPICE licence terms (via `rust-spice`) are UNVERIFIED. **Qualifies:** DEP-003, DEP-015.
- [ ] **OI-18** JPL DE440/DE441 kernel redistribution terms are UNVERIFIED (also in Journals #3 and #5). **Blocks:** any relaxation of DEP-017 or LIC-003 (e.g. shipping a DE segment in the WASM demo). **Qualifies:** REF-001.
- [ ] **OI-19** Was `thor-rs-assist` derived from GPL REBOUND/ASSIST code? **Blocks:** removing it from the DEP-003 ban list (DEP-015).
- [ ] **OI-20** Is UMFPACK, as linked by `russell_sparse`, GPL-licensed ("per our knowledge")? **Qualifies:** DEP-016.
- [ ] **OI-21** `rmcp`'s licence: crates.io says Apache-2.0, GitHub reports NOASSERTION. Read the LICENSE file. **Blocks:** DEP-013 under DEP-015.
- [ ] **OI-22** `astro-float` is assumed, not benchmarked, to be too slow for production runs. **Qualifies:** DEP-012.

## Journal #3 — Reference implementations and CI

- [ ] **OI-23** header.440: does `CENTER = 0` mean barycentric ICRF? What does the `XC…` block mean? Are the state units au and au/day (inferred)? **Blocks:** REF-002, PHY-014.
- [ ] **OI-24** How does JPL supply asteroid initial conditions at JDEPOC? What does `sb441-n373.bsp` contain, and does n373 = 343 asteroids + 30 KBOs? Read IOM 392R-21-005. **Blocks:** REF-003, PHY-014, CI-007.
- [ ] **OI-25** Original Mercury6 licence (the mirror is labelled GPL-3.0), Swifter availability, licence and GR support, and the NOVAS F3.1 licence. **Qualifies:** REF-008, LIC-001, LIC-003.
- [ ] **OI-26** Which Orekit release introduced `J2OnlyPerturbation`? Can `LenseThirringRelativity` be re-pointed at the Sun? Is there really no built-in coupled multi-body propagator? **Qualifies:** REF-006, REF-010.
- [ ] **OI-27** The exact JPL binary DE file path for Orekit (`Linux/de440/linux_p1550p2650.440`), and the conda-forge status of the JCC Orekit wrapper. **Qualifies:** REF-006.
- [ ] **OI-28** Do REBOUND/REBOUNDx publish manylinux wheels, or does the image need a compiler? **Qualifies:** REF-005, CI-001.
- [ ] **OI-29** Which DE does Horizons use internally now? Journal #1's live call reported `DE441`. **Qualifies:** PHY-016; the source string is recorded per run.
- [ ] **OI-30** Full citation of the TRACE integrator paper (Lu, Hernandez & Rein 2024). **Qualifies:** none (bibliography only).
- [ ] **OI-31** Whether Swiftest still supports a plain J2 input (`j2rp2`). **Qualifies:** REF-007.
- [ ] **OI-32** No maintained, openly licensed Fortran Gauss–Jackson planetary integrator was found; this is treated as a gap. **Qualifies:** none.

## Journal #4 — Numerical methods

- [ ] **OI-33** Second, independent transcription of the EIH equation (DE430 Eq. 27 / DE440 §3.1) against the typeset PDFs. **Blocks:** NUM-020, NUM-017, PHY-014.
- [ ] **OI-34** The Saha & Tremaine 1994 H_PN coefficients (Eq. 26) need checking against the typeset paper. **Blocks:** NUM-005.
- [ ] **OI-35** The R3b Lense–Thirring acceleration: does the IERS-style typography match the DE440 equation and the INPOP21a Eq. 1 form named in PHY-011? **Blocks:** NUM-019. **Qualifies:** PHY-011.
- [ ] **OI-36** IERS Conventions 2010 Ch. 10 equation numbers for the R3 Schwarzschild term. **Qualifies:** NUM-015, NUM-017.
- [ ] **OI-37** Citation for the inclined-orbit J₂ rates ω̇, Ω̇. **Qualifies:** NUM-015, PHY-010.
- [ ] **OI-38** Park 2017's exact switch-on/switch-off protocol for the per-body decomposition. **Qualifies:** PHY-012, NUM-013.
- [ ] **OI-39** The slope-bias constant for an unmodelled sinusoid (~A·P/T²) and the achievable-precision estimates (10⁻⁴–10⁻⁵ ″/cy differential, ~10⁻³ ″/cy total) are derived and UNVERIFIED until the NUM tests run. **Qualifies:** NUM-011, NUM-013.
- [ ] **OI-40** The form of a 1PN-corrected LRL vector. **Qualifies:** NUM-012.
- [ ] **OI-41** Whether production EPM uses exactly ABM13 PECEC with h = 1/16 d. **Qualifies:** NUM-002.
- [ ] **OI-42** Cost estimates: f64 + compensated summation ~1.1–1.3×; emulated f128 in full use; EIH ~3–5× Newtonian flops per pair; typical IAS15 step ~1 d for Mercury. **Qualifies:** NUM-006, NUM-009, NUM-024.
- [ ] **OI-43** Extrapolated f64 error growth (t^(3/2): ~0.3 km at 10³ yr, ~3×10² km at 10⁵ yr). **Qualifies:** NUM-007.
- [ ] **OI-44** The CPU used for Subbotin's 81 s / 246 s timings is unspecified, so hardware normalisation is UNVERIFIED. **Qualifies:** NUM-024.
- [ ] **OI-45** Feasibility of a ≥ 2.5× SIMD speedup at N = 16; current API status of stable SIMD crates (`wide`, `pulp`). **Qualifies:** NUM-021.
- [ ] **OI-46** The threading crossover inside a step, and heyoka's batch mode. **Qualifies:** NUM-022.
- [ ] **OI-47** The Taylor-integrator jet cost factor. **Qualifies:** NUM-025.
- [ ] **OI-48** Inner-solar-system Lyapunov time ~5 Myr (Laskar 1989); La2010 and La2004 integrator details. **Qualifies:** none (context).
- [ ] **OI-49** Citation details: Rein & Tamayo 2015 (MNRAS 452, 376 vol/page), Wisdom & Holman 1991, Everhart 1985 page, Quinlan & Tremaine 1990, Berry & Healy 2004 page range, Moyer 2003 DOI, La2010 volume. **Qualifies:** NUM-004 and bibliography.
- [ ] **OI-50** Direction conflict: Journal #4 says the DE430 J₂ (2.1106088532726840×10⁻⁷), estimated without LT, was "~7% too high". Journal #1 says a J₂ fitted without LT is biased **low** (INPOP19a 2.01). Also, DE430's value is below DE440's 2.196×10⁻⁷. Re-read Park et al. 2021. **Provisional resolution (maintainer, 2026-10-14):** biased **low**. Journal #4 and NUM-019 are corrected; keep this open until the Park 2021 sentence is read. **Qualifies:** NUM-019, PHY-010 (narrative only; no acceptance number depends on it).

> **Legal items deferred (maintainer decision 2026-10-14):** the CLA text, JPL DE / NAIF redistribution terms and SOFA terms via `sofars` are deferred until before the first release. Conservative defaults apply meanwhile: DCO, fetch-on-demand data with SHA-256 pins, and `anise` as an oracle only.

## Journal #5 — Organization and standards

- [ ] **OI-51** Which NPR 7150.2D SWEs apply at Class D/E (Appendix C not reviewed). **Qualifies:** ORG-007.
- [ ] **OI-52** ECSS-Q-ST-80C Table D-1 criticality definitions, and whether Q-ST-80C is now at Rev.2. **Qualifies:** ORG-007.
- [ ] **OI-53** Effort needed for the OpenSSF Best Practices "passing" badge. **Qualifies:** STD-012.
- [ ] **OI-54** Maintenance status of `EndBug/label-sync`. **Qualifies:** ORG-006.
- [ ] **OI-55** Toolchain channel 1.90.0 as "latest stable" (choose at bootstrap); stabilisation of rustfmt `imports_granularity` / `group_imports`. **Qualifies:** STD-001, STD-005.
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
- [ ] **OI-66** Fit of StrictDoc and OpenFastTrace (ADR `000x-requirements-tooling.md`, revisited at M2). **Qualifies:** ORG-003, ORG-004.
- [ ] **OI-67** IERS EOP redistribution terms in test fixtures (open question 5). **Qualifies:** DEP-017, LIC-003.

## Consolidation inconsistencies needing maintainer confirmation

These are not UNVERIFIED claims. They are conflicts between journals, provisionally resolved in the requirement `notes`.

- [ ] **OI-68** CI-009 (LRL-vector estimator) versus NUM-011 (Park2017 canonical). Provisional resolution: NUM-011 is authoritative and LRL is secondary. **Affects:** CI-009, NUM-011, NUM-012.
- [ ] **OI-69** DEP-014 says criterion runs on every PR; CI-008 and Journal #5's bench.yml measure only on the fixed runner class. Provisional resolution: compile benchmarks per PR, measure under CI-008. **Affects:** DEP-014, CI-008, NUM-024.
- [ ] **OI-70** NUM-013's 10⁻⁴ ″/cy differential tolerance versus Journal #4 R4's ≲ 10⁻² ″/cy planet-interaction residuals in N-body GR differentials. Provisional resolution: 10⁻⁴ applies to single-body isolation only. **Affects:** NUM-013, PHY-005.
- [ ] **OI-71** CI-010's resume-equivalence criterion (bitwise identity) is derived from TST-005/NUM-021, not stated in Journal #6. **Affects:** CI-010.
