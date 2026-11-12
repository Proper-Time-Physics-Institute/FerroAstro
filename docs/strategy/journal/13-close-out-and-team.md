<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

## 📓 Journal #13: #13 close-out decisions and a new team shape (2026-11-07/08)

### Team
ferroastro-3b (Claude) is now **red team and merge judge**. **Samson**, a Kilo CLI agent on a local gemma4-12b, implements, running scripts and exact commands. Because one session now holds both review roles, an independent Claude sub-agent reviewed today's #13 commits; its valid findings were fixed in `8001e27`.

### Maintainer decisions (quoted from the questionnaire answers)
| Item | Decision | Evidence |
|---|---|---|
| Swiftest stage A (REF-007) | "**1/16 d, 2 km**": golden at dt = 1/16 d, bound 2 km for every body | dt scan: worst body 7.92 / 1.97 / **1.46** / 4.11 / 8.34 / 28.68 km for dt = 1/4 … 1/128 d. WHM truncation (∝ dt², Mercury) trades against Swiftest's own per-step energy drift (outer planets). `docs/strategy/evidence/swiftest-dt-scan-2026-11-08.md` |
| Park 2017 pair rows (PHY-012) | "**Tighten to ±5e-4**", replacing ±0.005, under which four of six rows could not detect a sign error | measured worst difference 1.18e-4; PHY-012 nightly passes |
| ADR-0004 `provisional` | kept; schema, parser and fixture tests cover Confirmation 1–5 | `33b7c6a` |
| NUM-023 / B5 | per the 2026-10-21 ruling, `Ias15Config::pc_b_from_g` becomes the default and the long ensemble is asserted | energy/phase slopes 0.466/1.442 (long), 0.436/1.411 (default) against 0.5/1.5 ± 0.1 |

### Also on #13 today
- OI-38 vectors and pair rows: `6a68abc`, `1b5e3d1`, with a negative check.
- PO-5: IC epoch and gauge in diffrun reports.
- F8 citation fix.
- Two stale review markers removed (OI-23 and OI-75 are closed).
- The must-fix audit found LIC-2, R13-CODE-2/3/4 and F5 already fixed.

### Open
- WP12-1B N-body Brouwer guard under the new predictor (solo rerun in progress).
- 14 "NEEDS HUMAN REVIEW" vector items, grouped for the maintainer.
- The M4 (#17) merge is staged; a reconciler caught and fixed a doubled cross-term sign flip from git's auto-merge.

