---
status: accepted
date: 2026-11-14
decision-makers: maintainers (decisions D1–D7 of 2026-11-14, Bernard, recorded on #59)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Comparison models outside the GR ladder (conformal gravity)

Related: [#59](https://github.com/Proper-Time-Physics-Institute/FerroAstro/issues/59), [relativistic-dynamics#88](https://github.com/Proper-Time-Physics-Institute/relativistic-dynamics/issues/88), ADR-0011 (precession observable and frames), ADR-0014 (per-body contribution protocol), NUM-017 (gauge), PHY-018 (no unverified oracle), PHY-020, API-004, CI-002, OI-79, AGENTS.md rule 4 (state the gauge).

## Context and Problem Statement
FerroAstro's force models form a ladder that reproduces Mercury's observed 575.3100 ± 0.0015 ″/cy (Park et al. 2017) from Newtonian planets, solar J2, GR 1PN/EIH and Lense–Thirring (ORG-002, PHY-001…PHY-019). Every term on that ladder is GR or Newtonian physics. relativistic-dynamics#88 asks for Mannheim's conformal gravity as a toggle, so that its Mercury prediction can be compared with GR and with ephemeris bounds. This is the first beyond-GR term FerroAstro would carry. How should a model that is not part of the ladder enter the engine, the scenarios and the viewer?

The Mannheim–Kazanas static exterior is ds² = B dt² − dr²/B − r²dΩ², in the simplified form used for orbit work, with B = 1 − 2β/r + γr − κr² (Mannheim & Kazanas 1989), β = GM/c². The exact form is B = 1 − β(2 − 3βγ)/r − 3βγ + γr − κr²; the dropped terms are ≤ 10⁻²⁴ in the Solar System. With γ = κ = 0 it is Schwarzschild, so the β term gives GR's +42.98 ″/cy. In the weak field the two extra terms add, to Newtonian order, two radial accelerations:
- a constant inward acceleration, (γ/2)c²;
- an outward, Hooke-like acceleration, κc²r (repulsive for κ > 0, because B contains −κr²).

The galactic fits write γ = N\*γ\* + γ0, with γ\* per solar mass of the source and γ0 a cosmological term (Mannheim & O'Brien 2012). At Mercury only γ0 is large enough to matter, and the sources do not settle where it is centred:
- **Sun-centred**, it gives −0.183 ″/cy, about 60× the INPOP15a bound of 0.0 ± 3.1 mas/cy on any supplementary Mercury advance (Fienga et al. 2016);
- **as a uniform background field**, it accelerates the Sun and the planets equally and gives ≈ 0.

These Mercury numbers come from the rd#88 research notes (two-body Gauss averages checked there by numerical integration). They have not been reproduced in FerroAstro. The γ\* and κ contributions are 10⁻¹¹ to 10⁻¹² ″/cy in magnitude, of opposite signs (about −3.2×10⁻¹² and +9.9×10⁻¹²).

## Decision Drivers
* The GR ladder and its reference values must not change, by default or by accident.
* Every force model states its gauge (AGENTS.md rule 4, NUM-017). The model constructor refuses two different relativistic gauges (FA-E0502), and "full model + conformal gravity" must still assemble.
* No unverified value becomes a test oracle (PHY-018). The published fit values have not been checked against the primary papers (OI-79).
* The scientific point of the toggle is the γ0 centring question, so both readings must be computable, not just labelled.
* Existing scenario digests and the default viewer configuration must stay unchanged.

## Considered Options
* Comparison model: default-off term, outside the ladder, labelled "comparison model" (chosen).
* No beyond-GR physics in FerroAstro; keep conformal gravity in research notes only.
* A new ladder rung for conformal gravity.

## Decision Outcome
Chosen option: **comparison model**. The maintainer decided D1–D7 on 2026-11-14 (Bernard; recorded on #59).

1. **Status of the model (D4).** Conformal gravity enters as a **comparison model**. It is default-off and is never part of the GR ladder, of the 575.31 ″/cy total or of any reference value. Wherever it is shown (reports, scenario output, viewer) it carries the label "comparison model".
2. **Force term.** `ConformalPotential` is a Sun-sourced, velocity-independent radial term with reaction on the Sun and an implemented potential, modelled on `GrPotential`. Its acceleration on a body at heliocentric position r is −(γc²/2) r̂ + κc² r. A matching `CentralTerm` serves the Rung lab.
3. **Signs (D3).** γ and κ are signed in the API. γ > 0 is attractive and gives a retrograde advance. κ > 0 is repulsive, matching the −κr² term in B, and gives a prograde advance. (An earlier integration note for rd#88 assumed an attractive κ; the rd#88 physics note supersedes it.)
4. **Gauge (D1).** The term declares a new `Gauge::StaticWeakFieldPotential`, which `Gauge::is_relativistic()` excludes. The term is a Newtonian-order potential, not a metric written in a PN gauge, so it has no PN gauge to conflict with. It therefore combines with `PpnIsotropicHarmonic` (GR 1PN, EIH) without FA-E0502. This differs from `GrEffectivePotential`, which also has no PN gauge but stands in for the 1PN advance and stays relativistic. The model's reported gauge label (PHY-015) names both the relativistic gauge, if any, and the comparison term.
5. **γ0 centring (D2).** The scenario selects `gamma0_centre = "sun" | "uniform"`, with default `"uniform"` when the term is on.
   - `"sun"` adds γ0 to the Sun-sourced linear term: γ = N\*γ\* + γ0, with N\* the Sun's mass in solar masses.
   - `"uniform"` applies the same acceleration γ0c²/2 to every body, along a fixed direction given in the scenario. It is implemented, not just reported as 0. Relative motion is then unchanged, so with velocity-independent forces the heliocentric observable is unchanged to integrator precision. With velocity-dependent terms (EIH, Lense–Thirring) the common drift of about 0.04 m/s per century enters only at O(v·Δv/c²), far below any resolvable rate.
   - The Sun's own γ\* term stays Sun-centred in both settings.
6. **Parameters (D7).** The fit values γ0 = 3.06×10⁻³⁰ cm⁻¹, γ\* = 5.42×10⁻⁴¹ cm⁻¹ per solar mass and κ = 9.54×10⁻⁵⁴ cm⁻² are tracked in OI-79 as unverified until checked against Mannheim's primary papers. They are **never** test oracles. Tests assert the analytic Gauss secular rates per orbit, with β = GM☉/c²:
   - linear term: Δϖ = −πγa²√(1−e²)/β;
   - quadratic term: Δϖ = +3πκa³√(1−e²)/β.

   The tests use exaggerated parameters so that the rate is resolved in a short PR-tier run. They use √(1−e²), not the (1−e²)² and (1−e²)³ factors printed by Sultana, Kazanas & Said (2012); for Mercury the printed linear factor differs by about 7 %.

   β = GM☉/c² is the Sun's GM only, and a is the relative-orbit semi-major axis. The Sun's reaction raises the relative acceleration by (1 + m/M☉) and the two-body μ = G(M☉ + m) in the Gauss average divides it out, so the two cancel; for a massless test body they coincide. OI-77's two-body μ applies only to the per-orbit → ″/cy conversion.
7. **Viewer ABI (D5).** In a later viewer PR, bit 12 becomes `CONFORMAL` and parameters 13–15 carry γ0, γ\* and κ, plus the centring choice. The bit changes from reserved to defined, so the ABI becomes v2. The default mask does not change. Nothing in this ADR changes the viewer yet.
8. **Further comparison models (D6).** The Proper Time framework's 1/r³ kernel (−7.17 ″/cy) is the next comparison model under this ADR, in a later PR. Every comparison model needs its own PHY requirement, a default-off switch, the "comparison model" label and analytic, non-fit test oracles.

### Consequences
* Good, because the ladder, its tolerances and its reference values are untouched, and the default viewer mask and existing scenario digests stay as they are. The new optional `forces.conformal` table is skipped when absent (CI-002).
* Good, because the viewer can show both centrings side by side: one gives ≈ 0, the other −0.183 ″/cy against INPOP15a's ±3.1 mas/cy.
* Good, because the tests rest on closed-form secular rates, so no unverified fit value becomes an oracle.
* Bad, because the contribution table and the per-effect wedges must render negative contributions. This also fixes today's "Not drawn (≤ 0)" Lense–Thirring row.
* Bad, because a beyond-GR term now lives in the flagship crates and viewer. The label, the default-off switch and PHY-020's "never in the ladder" clause are the guard.
* Neutral: β is an integration constant matched to the Keplerian GM, so the +42.98 ″/cy from the β term is reproduced, not independently predicted. FerroAstro's GR terms already supply it; the comparison term adds only the γ and κ accelerations.

### Confirmation
PHY-020. Once the engine lands, its tests confirm the decision:
- a two-body integration with exaggerated γ, of both signs, matches −πγa²√(1−e²)/β per orbit;
- an exaggerated κ > 0 matches +3πκa³√(1−e²)/β per orbit, prograde;
- `gamma0_centre = "uniform"` gives a heliocentric rate of 0 within the integrator floor;
- GR 1PN + conformal gravity and EIH + conformal gravity assemble without FA-E0502;
- existing scenario digests are byte-identical.

The default-off clause is confirmed by the default scenario and viewer mask tests. All of these tests are PR-tier; none is `#[ignore]`.

## Pros and Cons of the Options
### Comparison model (chosen)
* Good, because it answers rd#88 with FerroAstro's own integrator and estimator, under the same metadata rules as every rate.
* Bad, because it adds a gauge variant and a term that no ladder requirement covers, so it needs its own requirement (PHY-020).

### No beyond-GR physics in FerroAstro
* Good, because the codebase stays GR-only.
* Bad, because the centring question could then be shown only by a separate two-body script, outside the N-body engine and the viewer.

### A new ladder rung
* Bad, because the ladder reproduces an observed total. A term that is either ≈ 0 or excluded by ephemerides at 60× the bound does not belong in it, and the fit values could not serve as a rung's oracle.

## More Information
References. Metadata checked against Crossref (journal DOIs) and arXiv on 2026-11-14; the papers' full text has not been read for this ADR, and OI-79 tracks that.
* P. D. Mannheim and D. Kazanas, "Exact vacuum solution to conformal Weyl gravity and galactic rotation curves", ApJ 342, 635 (1989), doi:10.1086/167623.
* P. D. Mannheim and J. G. O'Brien, "Fitting galactic rotation curves with conformal gravity and a global quadratic potential", Phys. Rev. D 85, 124020 (2012), doi:10.1103/PhysRevD.85.124020, arXiv:1011.3495. Its abstract states γ0 = 3.06×10⁻³⁰ cm⁻¹ and κ = 9.54×10⁻⁵⁴ cm⁻²; γ\* was not found in the abstract.
* J. Sultana, D. Kazanas and J. L. Said, "Conformal Weyl gravity and perihelion precession", Phys. Rev. D 86, 084008 (2012), doi:10.1103/PhysRevD.86.084008, arXiv:1910.06118.
* A. Fienga, J. Laskar, H. Manche and M. Gastineau, "Tests of GR with INPOP15a planetary ephemerides: estimations of possible supplementary advances of perihelia for Mercury and Saturn", Proceedings of the 14th Marcel Grossmann Meeting, arXiv:1601.00947 (2016). The proceedings volume and page numbers have not been checked.
