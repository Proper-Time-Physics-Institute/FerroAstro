<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

## 📓 Journal #14: maintainer rulings on the review items; M3 merged (2026-11-08 to 11)

### Rulings on the "NEEDS HUMAN REVIEW" vector items (maintainer, questionnaire, 2026-11-08)
| Group | Items | Ruling (verbatim option) |
|---|---|---|
| Post-hoc tolerances | IAS15-EQ11-DEADBEAT, TIME-TDB-SECONDS-DD-ROUNDTRIP, REAL-DD-EXP-REL-ERR (domain), ORACLE-STAGE-B-OREKIT-MAX-DR (settings) | "**Accept as recorded**" |
| Provisional engineering bounds | ORACLE-STAGE-A-REBOUND-MAX-DR, -MERCURY6-, -STAGE-B-OREKIT-, ORACLE-PADD-VENUS-TOOL-AGREEMENT, ORACLE-CLITE-SWIFTEST-GR-AGREEMENT, DIFFRUN-IC-SOURCE-AGREEMENT, NUM006-F64-BENCH-RATIO | "**Accept; issue for REBOUND**" (#33) |
| WP12-1B N-body Brouwer (1e9 steps) | known failure under both predictors (0.050, then 0.065 ± 0.025) | "**Merge; redesign at M5**" (#34) |
| OI-24 and NUM015-ABS-FLOOR | | "**Issues; don't block #13**" (#35, #36) |

Recorded in the vectors in `4fa7936`. Only the policy wording in the REAL-DD rationales still contains the marker text.

### Merges and state
- **#13 (M3) merged to main as `b161b63`** (2026-11-11). The judge verified every condition from the 2026-10-21 review. #21 closed.
  - Follow-ups #37–#40: F7, F9, F10, LIC-1.
  - Found during close-out: #31 (diffrun store key), #32 (Orekit image), #33–#36.
- **#17 (M4)** now targets main. `m4-eih` has the final M3 merged in. A reconciling sub-agent found that git's auto-merge had doubled the cross-term sign flip, and fixed it. The gate passes with 289 tests. The N-body Lense–Thirring row now gates, since OI-35 is closed (`26b65fe`).
- **WP22/23** (EIH for all bodies per D1, plus the relativistic barycentre, for PHY-014): maintainer chose a Claude sub-agent builder, with Samson running gates and ferroastro-3b judging. In progress.
