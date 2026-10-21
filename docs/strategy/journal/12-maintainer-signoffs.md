<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

## 📓 Journal #12 — Maintainer sign-offs (2026-10-21)

The maintainer recorded these sign-offs in the plan-owner session (ferroastro-2f) through a direct questionnaire, after reviewing the checklists. Their answers are quoted exactly. Per the judge's rule, agent findings do not close items; these sign-offs do.

| Item | Maintainer's answer (verbatim) | Reviewed against |
|---|---|---|
| **NUM-020** (EIH transcription) | "**Signed off**" (the option read: "I reviewed it; Eq. 27 transcription in journal R5 and eih.rs matches DE430/DE440. Unblock WP22/23 (with D1: all bodies).") | Checklist pinned to `m4-eih` 78d5a4c, rows 0–17. DE430 IPN PR 42-196 p.12 (sha256 76df7694…), DE440 p.5 (sha256 96c3945d…). |
| NUM-020 D1 (EIH scope) | "**Match DE440: all bodies**" | DE430 p.12 says "Sun, Moon, planets and asteroids"; DE440 says "all bodies". |
| **OI-38** (Park 2017 protocol) | "**Confirm P-remove**" | Park 2017 §4 p.5: "the Venus row represents the difference in Mercury's precession rate when the mass of Venus is set to zero" (sha256 24fb2153…). |
| **OI-35** (Lense–Thirring form) | "**Confirm; Sun reaction on; LT all bodies**" | DE440 §3.4 Eqs. 37–39 p.7; INPOP21a Eq. 1 (sha256 5fc3b729…); journal R3b. |
| **OI-75 / NUM-018** (μ*) | "**Include dμ*/dt; scope to ephemeris mode**" | DE440 Eq. 2; DE430 Eqs. 1–3 pp. 4–6 (DE405 drifted ~0.5 m/cy when dμ*/dt was neglected); M5 opt-in reaches 2.92 µm per 40 yr in double-double. |

### Follow-through
- **WP22 and WP23 are unblocked.** EIH sums run over all bodies, and asteroids and KBOs get their own 1PN terms (D1).
- **OI-38 closes inside #13,** in the same PR that removes `provisional` from the Park rows (ADR-0004 revision). ADR-0014 becomes *confirmed*, still disclosing that the choice was made after the results.
- **A docs PR to main** records OI-35, OI-75/NUM-018, NUM-020/OI-33 and the D1 scope (PHY-014/NUM-009), and removes the stale "INPOP21a unavailable" text.
- **Earlier decisions (2026-10-21):** #10 and #12 are deferred until M4 lands; M5 continues; the PR-title lint has been added (#27).

