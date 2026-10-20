## 📓 Journal #9 — Red-team review of PR #2

> **Lead summary:** the physics numbers all hold up under independent recomputation. The real risks are in the governance and licensing text. Two licensing findings were **confirmed by running cargo-deny** on a test crate: the "anise dev-only" rule cannot be enforced as written, and anise 0.10.6 brings in BlueOak-1.0.0 and CDLA-Permissive-2.0 licences. The "data over humans" merge policy needs a small human-owned set of meta files plus base-branch-pinned checks, or a PR can switch off its own evidence checks. A fix batch follows. Key claims spot-checked by the lead: PHY-006 acceptance text, the MSRV ≥/≤ conflict, and the ROADMAP M4/REF-003 inversion.

Red-team report on FerroAstro PR #2 (read-only; nothing was edited, committed or posted). Findings are ranked; "CONFIRMED" and "PLAUSIBLE" are marked on each.

Method:
- I did §1 myself: I recomputed every ladder number in Python from the cited DE440/SW92 constants and re-derived the 1PN coefficients analytically. A second independent agent reproduced the same numbers.
- Subagents did §2–4.
- cargo-deny 0.19.8 was run on a throwaway canary crate in the scratchpad.
- crates.io and PyPI licences were queried live.
- I spot-checked the subagent claims that matter, and they hold: ORG-009 text, missing ORG-011, MSRV ≥/≤, ROADMAP M4/M5, NUM-017 vs PHY-008, NUM.md:343 citing Journal #8, deny.toml lines.

=== TOP OF LIST (cross-cutting, ranked) ===

CRITICAL
C1 [CONFIRMED] "Data over humans" contradicts existing text in at least 8 places. Agents will pick whichever rule passes.
- ORG.md:236-242 (ORG-009: ≥1 human approval; agents cannot approve)
- AGENTS.md:49 (rule 10)
- .github/PULL_REQUEST_TEMPLATE.md:16
- AGT.md:6
- CI.md:96,99 (golden change needs reviewed PR)
- TST.md:95-99, ADR-0004:13, standards/testing.md:21 (CODEOWNERS review for goldens)
- requirements/README.md:66 ("approved" moved by CODEOWNERS review)

ORG-011 does not exist anywhere in the repo yet. The proposal must amend all of these in one PR.

C2 [PLAUSIBLE] Gate self-modification. If required checks run the PR-head workflow or scripts, one PR can neuter its own evidence. Ways to do it:
- edit trace.py to `return 0`
- add `paths-ignore` to a workflow
- change the deny.toml exceptions
- regenerate MANIFEST.json, or bump an oracle Dockerfile digest. The digest pin is in-repo, and CI-003 checks only field presence and SHA self-match.

Nothing exists yet: no CODEOWNERS, no workflows, no ruleset. Every gate is prose.

C3 [PLAUSIBLE] The golden value and its citation can be edited together. The TST-004 rewrite turns "updated provenance" into the *permission*. Nothing checks that the cited source contains the value. Ways to exploit it:
- swap in a real but irrelevant table or frame
- for computed-reference goldens, regenerate with a modified runner and record the new generated_by SHA, which TST-003 then blesses
- for β/γ≠1, where "no external code" exists (ARCHITECTURE.md:81), edit the in-repo analytic oracle and the implementation in the same PR; self-consistency is an accepted tier (testing.md:16)

C4 [CONFIRMED] DCO accountability breaks under 0 reviews. ADR-0005:14 says the human signs off and takes responsibility. With no review, the agent writes `Signed-off-by: <maintainer>` and the CI trailer check (ORG-008) verifies text, not person. This is a legal act, which the proposed ORG-011 "legal files" carve-out does not cover.

HIGH
H1 [PLAUSIBLE] Tolerance loosening with a fluent rationale, e.g. 0.005 → 0.05 citing "estimator noise, cf. NUM-011 ±0.01". No ratchet exists. Authoritative tolerances also live in PHY.md/NUM.md YAML, not only tests/vectors/, so a gate scoped to tests/vectors misses them.

H2 [PLAUSIBLE] Test disabling. Routes:
- #[ignore] is already the sanctioned nightly label (testing.md:19), so it looks normal
- cfg gating, nextest profile/filter edits, an early return on an env var
- float_cmp allow "for exact-by-construction" (TST.md:33)

H3 [PLAUSIBLE] Self-applied labels as evidence.
- `oracle` re-runs are opt-in by label (CI.md:118, ARCHITECTURE.md:77).
- An agent token with pull-requests:write can add golden-change or remove `hold`.
- With auto-merge and a PR-fast run under 10 min (CI-005), merges race `hold`.
- Nightly drift only "flags" (CI.md:143,145), and the headline rung runs weekly.

H4 [PLAUSIBLE] Requirement self-citation. An agent adds a requirement, a vector and a test in one PR. trace.py is green by construction; it counts IDs, not adequacy. Requirements can also stay `draft` forever to escape coverage, since coverage is enforced only for `approved`.

H5 [PLAUSIBLE] UNVERIFIED laundering. An agent ticks an OI box and sets verified_by. PHY-018 is verification: inspection, with no machine gate.

MINIMUM CONTROLS to keep "data over humans" honest:
1. Human-only CODEOWNERS on a small "meta" surface. Everything else can be 0-review. The surface:
   - tests/vectors/**, ci/**, .github/**
   - tools/trace.py and literal linters, vector schema
   - deny.toml, rust-toolchain.toml, REUSE.toml, LICENSE*, NOTICE
   - AGENTS.md, CLAUDE.md, docs/adr/**
   - PHY/NUM value/tolerance lines and requirement status changes past draft
2. Required checks pinned to workflows from the default branch (ruleset "required workflows"). Checker scripts run from the base SHA. No pull_request_target with head checkout. GITHUB_TOKEN read-only. Agent tokens cannot write labels or rulesets.
3. A base-branch ratchet that fails without human review on any of:
   - a widened tolerance
   - a downgraded oracle kind
   - |Δvalue| > k·old_tol
   - a deleted vector or test
   - a fall in executed-test count (nextest JSON)
   - a new #[ignore] or cfg-gate not on a human-owned allowlist
4. Oracle independence:
   - Oracle images built by a protected workflow and signed.
   - Published vectors carry DOI + table/page locator + a hash of the primary PDF where licensable.
   - Reject computed-reference goldens whose generator SHA comes from the same PR.
5. Mandatory oracle runs on physics paths (forces, integrators, ci/scenarios) instead of an opt-in label. Nightly drift opens a blocking issue or auto-reverts.
6. A merge queue with a delay window (12–24 h) for physics paths. `hold` is a required check re-evaluated at queue time, and only maintainers can add or remove it.
7. Mutation testing (cargo-mutants) on changed physics functions, with a surviving-mutant ratchet. This catches self-consistent formula+impl edits.
8. DCO: accept sign-off only from a commit signed with a human key, or with an agent key registered to an operator with recorded delegation. Draft ORG-011 explicitly.


## §2 is above (evidence-gating). §1 PHYSICS:
## §1 PHYSICS (lead-reviewed; recomputed in Python from the cited DE440/SW92 constants, and the coefficients re-derived analytically)

### Verified correct (no action needed)
- **GR-SCHW.** Recomputed 42.980479″/cy (5.018660e-7 rad/orbit, 415.200918 orbits/cy). Using μ = GM☉ + GM_Me instead of GM☉ changes it by 7e-6. PHY-004 is correct.
- **SR rungs.**
  - Re-derived L = −c²dτ/dt for the PPN metric: A, B, C = 1/8, γ+½, ½−β, D = 0.
  - Re-derived it for areal Schwarzschild: 1/8, ½, ½, 1.
  - Per-coefficient advances, via the Delaunay average ⟨δH⟩ = −⟨δL⟩ and ∂/∂G: v⁴ → 8A, Uv² → 4B, U² → 2C, U(n·v)² → 2D, in units of πGM/(c²p). For the D term, ⟨U ṙ²⟩ = μ⁴[1/(L³G) − 1/L⁴], so Δω = 2πDμ/(c²p).
  - Totals: 6 in both gauges. SR-a = 7.16341, 7/6 GR = 50.14389, 5/6 GR = 35.81707. All match.
- **"SR-a gauge-independent, SR-b not": CORRECT.** Position-only 1PN coordinate changes (r → r(1+αU/c²)), or a time shift t → t + χ(x)/c², alter only B, C and D; the shift adds terms odd in v. They cannot generate v⁴, so A = 1/8 is fixed. B and D are not fixed, so the velocity-only sum (A+B+D) is gauge-dependent. Harmonic and isotropic coincide at 1PN for a test particle, so writing "isotropic/harmonic" is fine.
- **Equation transcriptions.**
  - **R2:** re-derived from d(γv)/dt = F: a = [F − (F·v)v/c²]/γ. The exact Sommerfeld formula with h = γ|r×v| is correct.
  - **R3:** matches IERS 2010 Eq. 10.12 form.
  - **R3b:** matches the IERS LT form, with J per unit mass.
  - **R5 (EIH):** matches DE430 Eq. 27 / Explanatory Supplement 8.1 term by term (recall-based check, not a PDF check; NUM-020 still stands).
  - **R5 → R3 limit:** checked by hand (massless planet, static Sun).
- **J2.** 0.028601 (J2 = 2.25e-7) and 0.027917 (DE440 J2). **LT:** S☉ = 1.8999e41 and LT = −0.0020183″/cy. **PPN sensitivities:** −0.000559 (β) and 0.000659 (γ). All match.
- **Ladder sum.** The Park17 rows sum to N-SUM = 532.3035, and N-SUM + 42.9799 + 0.0286 − 0.0020 = **575.3100 exactly**. The table is internally consistent and the decomposition is additive.
- **OI-50 sign ("J2 without LT biased low"): CORRECT.**
  - Perihelion: LT contributes −0.0020 and J2 +0.0286, so a fit without LT absorbs −0.0020 into J2, which comes out about 7% lower.
  - Node: J2 regression is negative and LT is positive. Omitting LT again lowers |J2|, so the two signatures agree.
  - Caveat (PLAUSIBLE): INPOP19a → 21a moved J2 by 9.4% (2.01 → 2.218). That is larger than the 7% LT share predicts. The two releases also differ in data and GM☉ (C5), so "INPOP agrees" overstates attribution. Say "consistent in sign" rather than citing the full shift as LT.

### Findings

**[MEDIUM, CONFIRMED] PHY-006 acceptance contradicts its own statement and cannot pass** (`docs/requirements/PHY.md:174-176`).
- The statement scales relative to "the γ = β = 1 value". The acceptance asserts a match to `42.98 × (2+2γ−β)/3` at 1e-6 relative.
- 42.98 vs the actual 42.98048 is already 1.1e-5 relative, about 11× the tolerance, so a correct implementation fails.
- Fix: assert rate(β, γ)/rate(1, 1) = (2+2γ−β)/3 to 1e-6, using differential runs with identical ICs. The same 42.98 shorthand appears in journal 01:92 and PHY-008 notes.

**[MEDIUM, PLAUSIBLE] NUM-015's ≤1e-6 relative cannot be met for the LT (and probably the J2) single-body oracles** (`docs/requirements/NUM.md:403-404`).
- The LT advance is about 2.4e-11 rad/orbit, so 1e-6 relative over 10³ orbits means pinning ω to about 2.4e-14 rad.
- f64 Brouwer round-off in the eccentricity-vector direction after about 1e5 steps is about ε√N/e ≈ 1.7e-13 rad, which is 7× worse.
- Separately, the planar LT/J2 formulas assume I = 0, but Mercury is about 3.4° from the solar equator. That gives a 0.5% difference for J2 (0.02846 vs 0.02860) if the test is not strictly equatorial.
- Fix:
  - Use an absolute floor, e.g. max(1e-6 rel, 1e-7″/cy), or a differential run with/without the term from identical ICs.
  - Require equatorial geometry for the planar oracle, and use the inclined formula otherwise.

**[MEDIUM, CONFIRMED] PHY-012 tolerance (±0.005) is tighter than the definitional ambiguity it admits** (`PHY.md:336-352`).
- The notes say Park's switch-on/off protocol is UNVERIFIED, and the cross terms are J+S 0.0411 and V+E/M −0.0209.
- Toggling Jupiter "off from full" vs "on from Sun+Mercury" differs by up to about 0.04, which is 8× the tolerance.
- As written, the oracle is ill-posed. The test either fails, or someone tunes the protocol until it passes, which is a garden of forking paths.
- Fix: fix the protocol now. The most likely protocol is Sun + Mercury + planet X (plus GR?) vs Sun + Mercury, given that cross terms are listed separately as pairs. Mark PHY-012 blocked until the Park17 text confirms it. Also apply the same rule to NUM-013(b).

**[MEDIUM, CONFIRMED] PHY-014's headline tolerance cannot detect missing LT or asteroids; the oracle is also off by a known bias** (`PHY.md:396-398`).
- ±0.005 is wider than LT (0.0020), N-AST (0.0012) and most cross terms. An EIH-FULL with LT or asteroids silently dropped still passes.
- "DE440-equivalent physics" uses J2 = 2.196e-7, but Park17's 575.3100 used its fitted J2 = 2.25e-7. That is a known −0.0007″/cy offset, about half of Park's σ, that the "plus margin" hand-waves.
- Fix:
  - Either run EIH-FULL with J2 = 2.25e-7 and Park17's S☉ when comparing to 575.3100, or shift the expected value to 575.3093 with a documented derivation.
  - State explicitly that missing components are caught by the differential rungs (PHY-011, N-AST), not by PHY-014, and add an N-AST differential assertion (±0.0003). Today N-AST has no requirement at all (PHY.md:10).

**[LOW, CONFIRMED] PHY-009 / journal 01 §5 table mislabels coefficient A** (`PHY.md:254-256`; `journal/01:150-153`).
- The column says "advance per unit coefficient", but per unit of A the advance is **8**, not 1. The value 1 holds only at A = 1/8 (the "8A = 1" parenthetical).
- A test author who sets A = 1 gets 8 and "fails" a correct implementation.
- Fix: "per unit of 8A" or list A → 8.

**[LOW, CONFIRMED] Journal 04 rounds 42.98048 to "42.981"** (`journal/04:250`).
- To 3 decimals it is 42.980.
- PHY-004 notes (`PHY.md:131-134`) say 42.981 was "computed from different elements (a = 0.38709893, e = 0.20563069)". I recomputed with those elements: 42.980477, a difference of only 2e-6.
- The elements do not explain anything. It is a rounding error. Fix the note.

**[LOW, PLAUSIBLE] "The 0.0006 gap to Park17 comes from mean vs osculating a and e"** (`journal/01:67`, `PHY.md:131`).
- This is asserted, not shown. Closing the gap needs a shifted by about +312 km (8e-6 relative). That is the right order for Venus-driven short-period terms, but it is unverified.
- Other candidates: Park's fit or projection procedure, or element source.
- Fix: label it as a hypothesis (UNVERIFIED).

**[LOW, PLAUSIBLE] PHY-001 (|ϖ̇| < 1e-6″/cy over ≥ 1000 yr) is integrator-dependent** (`PHY.md:27`).
- 1e-6″/cy is 1.2e-14 rad/orbit.
- IAS15 with compensated summation should pass. A DOP853 at rtol 1e-12 can show secular ω truncation drift at about 1e-5″/cy.
- A Kepler-drift (WH) propagator passes trivially, so the test proves nothing there.
- Fix: name the integrator and its settings. Justify the bound as ≪ the smallest asserted rung (1e-5 would still be 100× below N-AST). Exclude the analytic-drift integrators or make the test integrator-specific.

**[LOW, CONFIRMED wording] Journal 04 says "DE440 adds … LT for all bodies except the Sun"** (`journal/04` R5 companion bullet).
- This is ambiguous. The Sun's spin acts *on* all other bodies; it isn't "LT of all bodies".
- NUM-019 inherits the wording. Clarify: "LT acceleration from solar angular momentum, applied to every non-Sun body."

**[INFO] Tolerances that are fine but worth knowing.**
- PHY-004 ±0.0001 is a formula check. It cannot detect GM☉ vs IAU nominal (3e-10 rel), which PHY-003 covers by design. That's OK.
- PHY-011 ±0.0002 (10%) catches a sign error and a −6 vs −4 node omission, but not 5% form errors. NUM-013(a)'s 1e-4 is the real guard, so keep it.

## §3 LICENSING

HIGH
L1 [CONFIRMED by canary] "anise dev-only" is not enforceable as written.
- deny.toml:27 is a comment. cargo-deny licence exceptions are per-crate and ignore dependency kind: anise in normal [dependencies] passed. DEP.md:49-51 implies enforcement.
- Fix: a second config (e.g. deny-release.toml) with [graph] exclude-dev = true and no anise exception, run in CI. A non-dev anise then fails.

L2 [CONFIRMED by canary] Adding anise (0.10.6) fails `cargo deny check` against the current allow-list. Causes:
- minicbor (BlueOak-1.0.0, via serde_dhall/dhall)
- webpki-roots (CDLA-Permissive-2.0, via a build-dep ureq, which means anise's build.rs has a network path)
- RustSec: proc-macro-error2 is unmaintained

DEP-002's "exactly this allow-list" (DEP.md:53-55) cannot be met. Decide on scoped exceptions, add an advisories ignore with a reason, and document the build-time network access.

L3 [PLAUSIBLE, HIGH] Pushing GPL oracle images to GHCR (CI.md:25, journal 03:186) is distribution if the packages are public.
- ADR-0009:10 and LIC.md:48-52 forbid only GPL "release artifacts". Public images carry compiled REBOUND/REBOUNDx/ASSIST/Swiftest, which triggers GPLv3 §6 (Corresponding Source or a written offer).
- The same images would redistribute Mercury6, the SOFA tarball and de440.bsp, whose terms are UNVERIFIED.
- Fix: keep the images private, or rebuild and cache them; or ship source/offer and keep licence-unverified tools out. Reword the rule to "no publicly distributed image".

L4 [PLAUSIBLE, HIGH] The process-boundary argument fails if a GPL container also runs a FerroAstro build. A runner that imports both rebound and a FerroAstro PyO3 wheel makes a combined GPLv3 work. Add a LIC rule: GPL containers never contain a FerroAstro build; they exchange files with a separate job.

MEDIUM
L5 [CONFIRMED] cargo-deny reads licence files only when Cargo.toml has no `license` field.
- sofars passed as plain MIT. The SOFA text in its LICENSE is invisible to the gate, contrary to DEP.md:32-34 and journal 02:107.
- Fix: a [[licenses.clarify]] entry with the LICENSE file hash (a file change then forces review), or a human DEP-015 checklist item.

L6 [CONFIRMED] The rug ban (deny.toml:36) contradicts DEP.md:91-92 ("opt-in out-of-tree dev oracle"). Bans include dev deps. Say "separate repo or excluded workspace only".

L7 [PLAUSIBLE] LIC.md:24-25 ("no GPL source … outside ci/oracles/ scripts") implies GPL source is OK inside ci/oracles/. That contradicts "never vendored". Reword.

L8 [PLAUSIBLE] ERFA obligations are understated (DEP.md:228-229, LIC.md:97). BSD-3 clause 1 needs the notice, conditions and disclaimer in source redistributions, so NOTICE alone is weak.
<!-- REUSE-IgnoreStart -->
- Fix: per-file `SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause` plus the NumFOCUS copyright, and add LICENSES/BSD-3-Clause.txt (reuse lint needs this).
<!-- REUSE-IgnoreEnd -->
- Clause 3 bars SOFA/IAU names in promotion.

The ERFA-not-SOFA renaming rationale itself is correct.

LOW
L9 [CONFIRMED] DEP-003 says "nine names" but deny.toml has ten (erfars). nyx_space vs nyx-space is harmless.

L10 deny.toml syntax is valid on cargo-deny 0.19.8; no deprecated keys.

L11 Clean-room: journal 03:69 shows a research session read GPL Swiftest source (swiftest_gr.f90). No GPL code is quoted in docs/. Add to DEP-008 that sessions which read GPL source must not implement GR or integrators.

L12 tools/pyproject runtime deps are all permissive (verified on PyPI). reuse (partly GPL-3) runs only as a CLI, and tools/ is not distributed. Add a pip-licenses guard so no one later adds rebound to it.

Verified correct: hifitime 4.3.1 and anise are MPL-2.0; nyx-space 2.6.0 is AGPL-3.0-or-later (the ban is right, including the network clause for MCP/web); the MPL "don't vendor-and-modify" rule; rebound, reboundx, assist and swiftest are all GPL-3.

## §4 INTERNAL CONSISTENCY

Script results (scratchpad audit_reqs.py):
- 119 requirements (ORG 10, STD 12, PHY 18, NUM 25, TST 5, API 3, AGT 4, DEP 17, REF 11, CI 10, LIC 4). All are draft. Criticality is 69 high / 35 medium / 15 low.
- Clean: duplicate IDs, enums, mandatory fields, dangling parent/related links, OI-01..71 continuity, ADR refs (0000–0013).
- Nothing is marked verified, so there are no [V]-without-artifact cases in requirements.
- 63 asymmetric related links (informational).
- 18 requirements are on no ROADMAP milestone: CI-009, DEP-004/008/012/015/016/017, LIC-004, NUM-003/004/005, ORG-001/002/005/006/007/009/010.

HIGH
I1 [CONFIRMED] Milestone inversion. ROADMAP.md:17 has M4 exit on PHY-014 (575.310 with 343 asteroids + 30 KBOs). That needs the REF-003 initial conditions, which ROADMAP.md:18 schedules in M5, and REF-003 is blocked by OI-24. Move REF-003 to M4, or make M4's exit a planets-only EIH value.

I2 [CONFIRMED] MSRV direction contradiction. STD.md:124 says "MSRV ≥ latest stable minus 2"; ADR-0006:10 says "never more than pinned toolchain minus two" (≤). Make ADR-0006 authoritative.

I3 [CONFIRMED] NUM.md:343 cites "Journal #8", which does not exist (the journals stop at 07). The 2026-10-14 maintainer-decision batch (216c58e) has no journal entry, contrary to the CLAUDE.md journaling rule.

I4 [CONFIRMED] NUM-017 (NUM.md:448-452) bans every standard-Schwarzschild-coordinate EOM except the geodesic oracle. PHY-008 requires an areal-gauge SR-b (35.817), and PHY-009's D-term is areal-only. Add labelled exemptions.

I5 [CONFIRMED, plausible-blocker] M0 cannot exit as scheduled. PHY-002 and REF-002 are M0 exit requirements but are blocked by OI-23 (header.440 CENTER/units) and OI-04 (GMSDOT units).

MEDIUM
I6 [CONFIRMED] STD-007 (STD.md:155-163) says "from M5" using Journal #5's numbering; in ROADMAP it is M6.

I7 [CONFIRMED] NUM-013 says "RESOLVED" while OI-70 is still open (OPEN-ITEMS.md:102). OI-71 is also still open, although ADR-0013 is accepted.

I8 [CONFIRMED] A stale "000x-requirements-tooling" ADR placeholder persists in README.md:103, ORG.md:116,146 and OI-66; ADR-0010 exists. ORG.md:142-146 seed ADR numbering doesn't match the files.

I9 [PLAUSIBLE] MISSION.md:30,35 quotes Park σ values (±0.0015, ±0.0011) next to rungs whose test tolerances are ±0.005 and ±0.0005. Readers will take them as pass criteria.

I10 [PLAUSIBLE, rule 8] OI-09 (meaning of the "Mercury self" row 0.0050) qualifies PHY-012/013 but not PHY-014, although that row equals the whole PHY-014 tolerance. NUM-015 uses OI-35 (R3b form) and OI-37 (inclined J2 citation) as oracles at 1e-6, but OI-35 blocks only NUM-019.

LOW
I11 [CONFIRMED] STD.md:29 and OI-55 still say toolchain 1.90.0, but the pin is 1.95.0.

I12 [CONFIRMED] DEP-002 acceptance includes MPL-2.0 in the allow-list; deny.toml correctly does not.

I13 [CONFIRMED] Archive policy is inconsistent. Journal 04:232 was edited in place, but 04:330 (blanket 1e-4 for NUM-013) and 02:113 (anise not dev-only) were not. Either annotate every superseded passage or none.
- Note: the inverted "~7% too high" claim carried a [V] tag. That is the one proven-wrong [V], and it shows the tag was applied without checking.

I14 [CONFIRMED] ROADMAP.md:27 "confirm" JPL/SOFA/NAIF terms vs OPEN-ITEMS.md:74 "defer until before first release".

I15 [PLAUSIBLE] REF-003 is medium criticality but feeds high-criticality PHY-014. PHY-016/017 are medium, but the README says all PHY rungs are high.

