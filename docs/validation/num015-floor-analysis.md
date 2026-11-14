<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# NUM-015 absolute floor: analysis for issue #36

Status: analysis only. No vector, tolerance or requirement text is changed by this document. The
principal decides; the exact edits each option implies are in [§6](#6-exact-changes-implied).

Base: `origin/main` at `826c298` (2026-11-14).

## 1. Summary

- **Recommendation: keep 1.17×10⁻¹⁵ rad/orbit** (= 10⁻⁷ ″/cy at 415.2 orbits/cy). NUM-015's acceptance text
  has said `max(10⁻⁶ relative, 10⁻⁷ ″/cy absolute)` since `7811132` (2026-10-19), so the vector and the
  requirement **already agree**. Only wording is stale.
- **2.9×10⁻¹⁷ rad/orbit is a measurement, not a floor.** It is the LT deviation measured in WP4 (1.25×10⁻⁶
  relative × 2.36×10⁻¹¹ rad/orbit). The NUM-015 notes quote it in a parenthesis right after "the 1e-7 ″/cy
  floor", and issue #36 read that parenthesis as a second floor. Used as a floor, it would sit **below** the
  measured f64 noise of the estimator (1σ ≈ 3–4×10⁻¹⁷ rad/orbit, single runs up to 9.8×10⁻¹⁷), so the test
  would pass or fail at random.
- **The floor is needed.** In f64, whether LT passes the 10⁻⁶ relative bound is a coin flip. Measured LT
  relative deviations over 250, 500, 1000 and 2000 orbits are 2.3×10⁻⁷, **2.9×10⁻⁶**, 7.8×10⁻⁷ and 6.0×10⁻⁷.
  WP4 measured 1.25×10⁻⁶ on the code of that time.
- **Implied change:** rewrite one sentence in the NUM-015 notes and the stale clause in the
  `NUM015-ABS-FLOOR` `tolerance_rationale`. The value and tolerance stay the same.

## 2. Where the two numbers come from (`git log -S`)

| Number | Commit | Date | What it is |
|---|---|---|---|
| `1.17e-15` | `e7acb1a` docs: WP4 plan and pre-registered oracles | 2026-10-15 | Pre-registered floor: 10⁻⁷ ″/cy ÷ 415.2 orbits/cy = 2.41×10⁻¹⁰ ″ = 1.168×10⁻¹⁵ rad, rounded up. |
| `1.17e-15` | `2953c76` feat(ladder,cli): single-body rung catalogue | 2026-10-15 | Vector `NUM015-ABS-FLOOR` created with that tolerance. |
| `1.17e-15`, `2.9e-17` | `c901f86` docs: WP4 implementation notes, interpretations and results | 2026-10-15 | IMPLEMENTATION-NOTES WP4 table: "NUM-015 R3b LT equatorial — 1.25e-6 relative, 2.9e-17 rad/orbit — PASS (floor only)". Here 2.9e-17 is the **measured deviation**. |
| `10⁻⁷ ″/cy absolute` | `7811132` docs: apply red-team fixes and 2026-10-15 maintainer decisions | 2026-10-19 | NUM-015 acceptance becomes `max(10⁻⁶ relative, 10⁻⁷ ″/cy absolute)`. This is the red-team §1 fix (Journal #9, "use an absolute floor, e.g. max(1e-6 rel, 1e-7″/cy)"). |
| `2.9e-17`, `26× the measured` | `9728be7` docs(requirements): sync catalogue with build evidence (WP1-WP18) | 2026-10-20 | WP4 result copied into the NUM-015 notes: "(passing only through the 1e-7 ″/cy floor; 2.9e-17 rad/orbit absolute, 26× the measured estimator noise floor)". The 26× is floor ÷ noise = 1.17×10⁻¹⁵ / 4.5×10⁻¹⁷. |
| `to issue #36` | `4fa7936` test(vectors): record the maintainer rulings of 2026-11-08 | 2026-11-11 | Rationale gains "deferred … to issue #36: this floor is a red-team recommendation, not yet in NUM-015's text". This was already out of date: the acceptance had carried 10⁻⁷ ″/cy since 2026-10-19. |

The red-team source is Journal #9 §1 (`docs/strategy/journal/09-red-team-review.md`, "NUM-015's ≤1e-6 relative
cannot be met for the LT (and probably the J2) single-body oracles").

## 3. The estimator as implemented

The NUM-015 checks (`crates/ferroastro-ladder/tests/common/mod.rs::check_num015`) compare
`RungResult::per_orbit_difference()` with `NUM015-REL-TOL` (relative) **or** `NUM015-ABS-FLOOR` (absolute).
The measured quantity is computed as follows:

1. `SingleBody::run` integrates 10³ anomalistic orbits (scenario `orbits = 1000`) with IAS15 (ε = 10⁻⁹,
   PC tolerance 10⁻¹⁶, compensated summation) in f64. `run_in::<DoubleDouble>` does the same in double-double.
2. `find_perihelia` locates each passage as a rising root of `r·v`, refined by Newton's method. Time is an
   absolute f64 (seconds from the start of the run).
3. `ferroastro_precession::passages::fit_per_orbit` takes the longitude of **r** at each passage in the fixed
   initial osculating plane (`atan2`), unwraps it, and fits an OLS line against the passage index. The slope is
   the per-orbit advance. Sums are Neumaier-compensated (`fit::fit_line`).

### 3.1 Round-off floor, derived

Per-orbit advances (Mercury, a = 0.387 au, e = 0.2056, P = 7.60×10⁶ s, v_p/r_p = 58.98 km/s ÷ 4.600×10⁷ km
= 1.28×10⁻⁶ s⁻¹):

| Term | Analytic per-orbit advance | 10⁻⁶ of it | Floor ÷ advance |
|---|---|---|---|
| J₂ (Park17 J₂, planar) | 3.340×10⁻¹⁰ rad | 3.3×10⁻¹⁶ | 3.5×10⁻⁶ |
| J₂ (DE440 J₂, planar) | 3.260×10⁻¹⁰ rad | 3.3×10⁻¹⁶ | 3.6×10⁻⁶ |
| LT (DE440 spin, equatorial) | −2.357×10⁻¹¹ rad | 2.4×10⁻¹⁷ | 5.0×10⁻⁵ |

Error sources in the f64 run, largest first:

- **Passage-time quantisation (dominant).** The passage time t_k ≈ kP is an absolute f64. Its rounding error is
  uniform in ±ulp(t_k)/2 with ulp(t) ≤ ε·t (ε = 2.22×10⁻¹⁶). This shifts the measured direction of **r** by
  (v_p/r_p)·δt. The per-passage noise therefore grows linearly, σ_k ≈ s·k with
  s = (v_p/r_p)·ε·P/√12 ≈ 6.2×10⁻¹⁶ rad (upper end; ulp/t averages ≈ 0.7ε over a binade).
  For OLS over k = 0…N with σ_k = s·k, Var(slope) = (s²N⁵/30)/(N³/12)² = 4.8 s²/N, so

  **σ_slope ≈ 2.2·s/√N ≈ 3–4×10⁻¹⁷ rad/orbit at N = 10³.**

  The measurements in §4 match three predictions of this model:
  - The formal slope error falls as N^−½: 5.7, 3.9, 2.8 and 1.9×10⁻¹⁷ for N = 250, 500, 1000 and 2000.
  - The RMS fit residual grows ∝ N: 6.5×10⁻¹⁴, 1.3×10⁻¹³, 2.6×10⁻¹³ and 5.0×10⁻¹³.
  - The largest passage residual |r·v|/(|r||v|) grows ∝ N: 2.6×10⁻¹⁴ … 2.1×10⁻¹³. The model predicts
    (v_p/r_p)·e/(1+e)·ulp(t)/2 ≈ 1×10⁻¹³ at N = 10³.

  I infer that this mechanism dominates from these three scalings. I did not test it in isolation.
- **Integrator round-off (Brouwer random walk of ω).** The red-team bound ε√N_steps/e ≈ 1.7×10⁻¹³ rad after
  ~10⁵ steps corresponds to about √1.2 × 1.7×10⁻¹³ / 10³ ≈ 1.9×10⁻¹⁶ rad/orbit in the slope. The measured noise
  is 4–6× smaller, so IAS15 with compensated summation does better than that pessimistic bound and this source
  is not dominant.
- **Rounding of the passage position and `atan2`.** About 10⁻¹⁶ rad per passage, independent between passages,
  so the slope error is ≈ 10⁻¹⁶·√12/N^1.5 ≈ 10⁻²⁰ rad/orbit. This is what remains in double-double (§4.2).
- **Oracle truncation (physics, not round-off).** The first-order J₂ oracle omits O(J₂(R/p)²) terms.
  Double-double shows a systematic +1.3×10⁻¹⁰ relative for J₂ (4×10⁻²⁰ rad/orbit) and −5×10⁻¹² for LT. Both are
  negligible.

**Floors for this estimator:**
- **f64:** 1σ ≈ 3–4×10⁻¹⁷ rad/orbit at 10³ orbits. A 5σ bound is ≈ 2×10⁻¹⁶. Averaging over more orbits helps
  only as N^−½.
- **Double-double:** ≈ 10⁻²⁰ rad/orbit (estimator rounding plus oracle truncation). In double-double the plain
  10⁻⁶ relative bound passes for every rung with 4–5 orders of magnitude to spare.

## 4. What the tests achieve today

Run on 2026-11-14 as workflow_user on the shared host (Rust 1.95.0, `profile.test` opt-level 1, base
`826c298`).

### 4.1 The committed tests

`cargo test -p ferroastro-ladder --test phy010_j2 --test phy011_lt --test estimator_floor -- --nocapture`
passed. Every test passed.

| Check | Numerical (rad/orbit) | Analytic | Relative | Absolute | Passes via |
|---|---|---|---|---|---|
| NUM-015 R1 J2 (Park17 J₂, equatorial) | 3.339681645031e-10 | 3.339681470621e-10 | 5.2e-8 | 1.7e-17 | relative |
| NUM-015 R1 J2-DE440 (equatorial) | 3.259736144932e-10 | 3.259735658525e-10 | 1.5e-7 | 4.9e-17 | relative |
| NUM-015 R1 J2-INCL (3.4°, Park17 Eq. 3) | 3.322061503999e-10 | 3.322061748947e-10 | −7.4e-8 | −2.4e-17 | relative |
| NUM-015 R3b LT (equatorial) | −2.356748009678e-11 | −2.356746178829e-11 | 7.8e-7 | −1.8e-17 | relative (by chance; see 4.2) |
| Estimator floor (Newtonian, true value 0) | −2.49e-17 (formal 1σ 2.8e-17) | 0 | — | 2.5e-17 | floor |

At WP4 (2026-10-15) LT gave 1.25×10⁻⁶ relative and passed only through the floor. The integrator has changed
since then, and the result now falls on the other side of 10⁻⁶. That is the noise of §3.1, not an improvement.

### 4.2 Temporary probe (deleted afterwards, not committed)

A throw-away test called `SingleBody::run` with N ∈ {250, 500, 1000, 2000} and `run_in::<DoubleDouble>` at
N = 1000 on the same rungs. The table shows the absolute deviation in rad/orbit, with the relative deviation in
brackets.

| Rung | f64 N=250 | f64 N=500 | f64 N=1000 | f64 N=2000 | double-double N=1000 |
|---|---|---|---|---|---|
| Newtonian (0) | −5.1e-17 | −6.5e-17 | −2.5e-17 | −3.1e-17 | 0 (exact) |
| J2 | −7.3e-17 [−2.2e-7] | +9.1e-17 [+2.7e-7] | +1.7e-17 [+5.2e-8] | −8.0e-18 [−2.4e-8] | +4.8e-20 [+1.4e-10] |
| J2-DE440 | −1.8e-17 [−5.5e-8] | +6.9e-17 [+2.1e-7] | +4.9e-17 [+1.5e-7] | −3.0e-17 [−9.1e-8] | +4.1e-20 [+1.3e-10] |
| J2-INCL | −9.8e-17 [−2.9e-7] | +7.0e-17 [+2.1e-7] | −2.4e-17 [−7.4e-8] | +1.0e-17 [+3.1e-8] | +4.3e-20 [+1.3e-10] |
| LT | −5.5e-18 [+2.3e-7] | +6.7e-17 [**−2.9e-6**] | −1.8e-17 [+7.8e-7] | +1.4e-17 [−6.0e-7] | +1.2e-22 [−4.9e-12] |

Notes on the probe:
- f64 formal 1σ: 5.7, 3.9, 2.8 and 1.9×10⁻¹⁷. Double-double formal 1σ: 2.1×10⁻²¹.
- f64 RMS fit residuals: 6.5×10⁻¹⁴ … 5.0×10⁻¹³ rad. Double-double: 1.9×10⁻¹⁷.
- 76 468 steps at N = 1000. The double-double runs took ≈ 8 s per rung and the f64 runs ≈ 1.5 s.
- **Across the 20 f64 runs, the largest |deviation| is 9.8×10⁻¹⁷ rad/orbit. The floor of 1.17×10⁻¹⁵ is 12× that,
  and 27–42× the 1σ.**

### 4.3 Not run

`data_single_body::data_j2_and_lt_rungs_in_real_solar_equator_geometry` (data tier, `#[ignore]`) panicked in
`tests/horizons/mod.rs:61` because the Horizons download is not reachable from the build host and is not in the
`FA_DATA_DIR` cache. The figures from WP4 for the real geometry still stand: J₂ and LT pass the relative bound
directly, with LT at 7.7×10⁻⁸.

## 5. Options

| Option | Floor (rad/orbit) | Margin over f64 noise | Assessment |
|---|---|---|---|
| **A. Keep 10⁻⁷ ″/cy = 1.17×10⁻¹⁵** (recommended) | 1.17e-15 | 27–42× the 1σ, 12× the worst of 20 runs | Already in the NUM-015 acceptance and the vector. A round number in the unit the ladder reports. Robust to estimator and integrator changes of a few times. It still detects any LT error above 5×10⁻⁵ relative and any J₂ error above 3.5×10⁻⁶. Every oracle error that matters for the ladder is O(1): a sign, the (1+γ) factor, cos i, or the planar versus inclined formula (0.5%). No golden value changes. |
| B. Use 2.9×10⁻¹⁷ | 2.9e-17 | about 1σ | **Not justified.** It is one past measurement, not a bound. It is below the measured noise, and 11 of the 20 f64 runs above exceed it. It would make NUM-015 flaky. |
| C. Tighten to a derived ~5σ bound, e.g. 2×10⁻¹⁶ | 2e-16 | ≈ 5σ, 2× the worst run | Physically defensible, and it tightens LT to ≈ 10⁻⁵ relative. But it is tied to the present time representation and to N = 10³, so an estimator or integrator change could break it. It is also not a round ″/cy number (≈ 1.7×10⁻⁸ ″/cy). Needs a golden-change. Little gain for the ladder. |
| D. Drop the floor; for J₂ and LT, use double-double runs or the differential run that NUM-015 already allows | none | — | Double-double meets 10⁻⁶ relative by about 10⁴×. It costs ≈ 5× the run time and changes the tests. This is a larger redesign, outside the scope of #36. |

## 6. Exact changes implied

### Option A (recommended): wording only, no value or tolerance change

**`docs/requirements/NUM.md`, NUM-015 `notes`.** Replace

```text
  (R3b): evidence available, pending human sign-off (PHY-011 notes); R3b
  matches Park 2017 Eq. 2 to 1.25e-6 relative in equatorial geometry
  (passing only through the 1e-7 ″/cy floor; 2.9e-17 rad/orbit absolute,
  26× the measured estimator noise floor) and 7.7e-8 in the real geometry
  (WP4).
```

with

```text
  (R3b): evidence available, pending human sign-off (PHY-011 notes); R3b
  matched Park 2017 Eq. 2 to 1.25e-6 relative in equatorial geometry at WP4
  (a measured deviation of 2.9e-17 rad/orbit, which passed only through the
  absolute floor) and 7.7e-8 in the real geometry. ABSOLUTE FLOOR (issue
  #36; docs/validation/num015-floor-analysis.md): the 10⁻⁷ ″/cy of the
  acceptance is applied per orbit as 1.17×10⁻¹⁵ rad/orbit (10⁻⁷ ″/cy at
  415.2 orbits/cy, rounded up; vector NUM015-ABS-FLOOR). It is 3.5×10⁻⁶ of
  the J₂ and 5×10⁻⁵ of the LT per-orbit advance, and about 30× the f64
  estimator noise (1σ ≈ 3–4×10⁻¹⁷ rad/orbit over 10³ orbits, set by f64
  passage-time rounding). Whether LT passes 10⁻⁶ relative in f64 is noise
  (2.3e-7 to 2.9e-6 across 250–2000 orbits), so the floor is needed. In
  double-double every rung passes 10⁻⁶ relative directly (≤1.4e-10).
```

(The "BLOCKED by OI-35" and "pending human sign-off" parts of the text are out of scope here. They are left
as they are.)

**`tests/vectors/single-body-rungs.toml`, `NUM015-ABS-FLOOR`.** `value` and `tolerance` stay unchanged.
Replace the tail of `tolerance_rationale`

```text
deferred by the maintainer 2026-11-08 to issue #36: this floor is a red-team recommendation, not yet in NUM-015's text.
```

with

```text
In NUM-015's acceptance since 7811132 (2026-10-19) as max(1e-6 relative, 1e-7 arcsec/cy absolute). Issue #36 (docs/validation/num015-floor-analysis.md): f64 estimator noise 1-sigma ~3-4e-17 rad/orbit at 1e3 orbits (passage-time rounding), worst of 20 runs 9.8e-17, so the floor sits 12x above the worst run; 2.9e-17 in the NUM-015 notes is the WP4 LT measurement, not a floor. Kept by the maintainer on <date>.
```

and append to `verified_by`: `"; issue #36 analysis <date>: re-measured, see docs/validation/num015-floor-analysis.md"`.

**Golden-change process.** TST-004 and AGENTS.md rule 1 require the `golden-change` label and a CODEOWNERS
review when a value or tolerance changes. Option A changes neither. It edits only the provenance text of a
golden entry, so the label is not mechanically required. Applying it anyway is a cheap way to make the
CODEOWNERS of `tests/vectors/` see the change. Close #36 with a link to the PR.

### Option C (if chosen instead)

`NUM015-ABS-FLOOR` `tolerance = { abs = 2.0e-16 }`, with the rationale rewritten around the derivation in §3.1.
NUM-015's acceptance changes from `10⁻⁷ ″/cy absolute` to `2×10⁻¹⁶ rad/orbit absolute (≈ 1.7×10⁻⁸ ″/cy)`.
This changes a tolerance, so it needs the `golden-change` label and a CODEOWNERS review (TST-004). The data
tier must be re-run before merge.

## 7. Questions for the principal

1. Which floor applies to NUM-015?
   - (a) Keep 10⁻⁷ ″/cy = 1.17×10⁻¹⁵ rad/orbit (recommended).
   - (b) Tighten to a derived bound of about 2×10⁻¹⁶.
   - (c) Drop the floor and use double-double or differential runs for J₂ and LT.
2. Should the NUM-015 acceptance also state the floor in rad/orbit, not only in the notes? The per-orbit
   comparison does not depend on n.
3. Should the provenance-only edit of `NUM015-ABS-FLOOR` carry the `golden-change` label anyway?
4. Should a follow-up issue cover making the passage time relative to the previous passage? The analysis
   suggests this would lower the f64 floor by orders of magnitude. It is not needed for NUM-015.
