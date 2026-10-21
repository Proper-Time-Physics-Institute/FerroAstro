<!--
SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
SPDX-License-Identifier: Apache-2.0
-->
# Lense–Thirring transcription record (NUM-019, PHY-011, open item OI-35)

**Status: first transcription and machine checks done (agent, WP20, 2026-10-20). Human
confirmation: NOT DONE. OI-35 stays open.**

OI-35 asks whether the Journal #4 R3b Lense–Thirring acceleration matches the DE440 equation and
the INPOP21a Eq. 1 form named in PHY-011. While it is open, the asserting N-body LT test
(`nightly_num019_solar_j2_and_lense_thirring_contributions_under_eih`, vector
`NUM019-NBODY-LT-CONTRIBUTION`, `blocked_by = "OI-35"`) reports its row without gating on it.

## Sources (read from 300-dpi page images; the PDF text layers garble the equations)
1. **DE440.** Park, R. S., Folkner, W. M., Williams, J. G., & Boggs, D. H. 2021, AJ 161, 105,
   doi:10.3847/1538-3881/abd414, §3.4 "LT Acceleration", **Eqs. 37–39**, p. 7.
2. **INPOP21a.** Fienga, A., et al. 2021, *INPOP21a planetary ephemerides*, Notes Scientifiques et
   Techniques de l'IMCCE S110, §1.2, **Eq. 1**, p. 5. URL
   `https://www.imcce.fr/content/medias/recherche/equipes/asd/inpop/inpop21a.pdf`, sha256
   `5fc3b7295a9664dadb25867cdc5893ce91cff0205bd03c972a2a8d67fffb89a5`, retrieved 2026-10-20.
3. **R3b.** FerroAstro Journal #4 R3b, as implemented by `LenseThirring` (`src/relativity.rs`).

Clean room (DEP-008): no REBOUND/REBOUNDx, ASSIST, Swiftest or Mercury6 source was opened.

## Transcription
| item | DE440 (Eqs. 37–39) | INPOP21a (Eq. 1) | R3b / `LenseThirring` | `SolarLenseThirring` (`src/nbody/terms.rs`) |
|---|---|---|---|---|
| form | `a = 2 Ω × v`, `Ω = (1+γ)G/(2c²r³) [−J + 3(J·r) r/r²]` | `a = (γ+1)G/(c²r³) S [3 (k·r)/r² (r ∧ v) − (k ∧ v)]` | `(1+γ) GS/(c²r³) [(3/r²)(r×v)(r·ŝ) + v×ŝ]` | DE440 as written: Ω first, then `2 Ω × v` |
| spin | `J = C☉ M☉ R☉² ω☉ p̂`, C☉ = 0.06884, R☉ = 696 000 km, ω☉ = 14.1844°/d | `S = S k`, S fitted (Table 1) | `G·S` given | `G·S = GMS · MOISUN · ASUN² · OMGSUN` (header.440) or an override |
| pole | RA 286.13°, Dec 63.87° (Archinal et al. 2018) | IAU pole | given | WGCCRE 2015 (RA 286.13°, Dec 63.87°) or an override |
| r | body relative to the Sun | planet relative to the Sun | relative to the central body | rᵢ − r☉ |
| v | not defined in the text | planet relative to the Sun | relative to the central body | vᵢ − v☉ (INPOP21a's definition) |
| bodies | "each body other than the Sun" | planets | test particle | every Major body except the Sun; Minor and Ring bodies do not feel it |
| reaction on the Sun | not mentioned | not mentioned | — | `LtReaction::On` by default (momentum conserved; NEEDS HUMAN REVIEW), `Off` = the literal reading |

**Algebra.** `2 Ω × v = (1+γ)GS/(c²r³) [−ŝ × v + 3(ŝ·r)(r × v)/r²]`, and `−ŝ × v = v × ŝ =
−(k ∧ v)`. The three forms are the same expression.

## Machine checks (WP20)
- **L2, symbolic:** `tools/lt_forms.py` (sympy) shows DE440 Eq. 37–38 ≡ INPOP21a Eq. 1 ≡ R3b for
  general γ, G·S, c, r, v and pole. A negative control (the `−J` term flipped) is detected.
- **L1b, numerical:** `SolarLenseThirring` equals `LenseThirring` at every body's heliocentric
  state to 2.1e-16 relative (bound 1e-13), at γ = 1 and γ = 0.999.
- **I2:** the N-body LT rate (equatorial Sun + Mercury) equals −4GS′/(c²a³(1−e²)^{3/2}) to
  4.5e-9 ″/cy (bound 1e-4).

The machine checks show that the code implements what is written above. They cannot show that
what is written above is what the PDFs say: that is the human check.

## Human check (to be completed by a reviewer; closes OI-35 if no discrepancy)
Read DE440 Eqs. 37–39 and INPOP21a Eq. 1 in the typeset PDFs. Compare them with the table above,
with `src/nbody/terms.rs` and with `tools/lt_forms.py`. Decide the reaction-on-the-Sun question.

- Reviewer:
- Date:
- PDFs and pages read:
- Rows confirmed:
- Discrepancies found:
- Reaction on the Sun (On / Off):
- Signature (closes OI-35):
