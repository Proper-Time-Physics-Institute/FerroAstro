## 📓 Journal #11 — Plan merged; build evidence folded in (2026-10-20)

### Status
- **PR #2 merged** to `main` (31d11a4), by the merge judge with maintainer authorization. Deleting its branch auto-closed #11 and #14. The replacements target `main`:
  - **#15**: `m0-foundation`, covering M0–M2
  - **#16**: the journal annotation policy
- **#13** (`m3-newtonian-nbody`, M3) is stacked on #15 and green. M4 (`m4-eih`) has started; WP22 is held for the human NUM-020 sign-off.
- **#19**: the requirements catalogue synced with the WP1–WP18 build evidence. Awaiting the judge.
- **Team:** plan owner (ferroastro-2f), build lead and red team (ferroastro-ca; its reviews are non-independent, which the maintainer accepted), merge judge (ferroastro-58).

### Findings
- **OI-23 closed.** The header.440 states are SSB-centred ICRF in au and au/day. They match de440.bsp at JDEPOC to 4.8e-7 km; the heliocentric reading misses by 6.7e5 km. **OI-05 closed:** the au is exact (IAU 2012 B2).
- **OI-04 (GMSDOT = −9e-14):** no public source states its units.
  - It is absent from Park 2021, and it first appears in DE440 (DE441/442 carry it; DE421–438 do not). It probably comes from the non-public JPL IOM 392R-20-002.
  - Adopted inference: a fractional rate in **yr⁻¹**, consistent with radiation (≈−6.8e-14/yr) plus solar wind.
  - Impact: a t² drift of about 28 km (Earth) and 46 km (Mercury) at the ends of DE440's span. So it matters only for the M5 reproduction, where it will be fitted against de440.bsp.
- **Per-body protocol (ADR-0014).**
  - The pre-registered P-add protocol misses 4 of Park's 7 rows by up to 0.043″/cy. P-remove matches all 7 within 0.0007″/cy, and four tools agree to 2e-8.
  - P-remove is asserted *provisionally* and disclosed as a choice made after seeing the results. If Park's text says P-add, the protocol reverts.
  - This follows the red team's earlier warning about choosing a method after seeing the results.
- **Park2017 estimator on Horizons DE441:** 575.30916″/cy, against 575.3100 ± 0.0015 (DE441 vs Park's fitted J2; the expected DE440-J2 offset is −0.0007).

### Human sign-offs outstanding
NUM-020 (EIH transcription vs the typeset DE430/DE440 PDF; holds WP22) · OI-35 (LT form) · OI-75 (μ* vs Park 2021 Eq. 2) · OI-38 (Park 2017 protocol text) · #10 and #12 (issue forms and labels; evidence-gated merges). *(Superseded 2026-10-21, [Journal #12](12-maintainer-signoffs.md): NUM-020, OI-35, OI-75 and OI-38 were signed off; #10 and #12 are deferred until M4 lands.)*
