---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Integrator strategy

## Decision Outcome
* **IAS15-class Gauss–Radau** (clean-room from Rein & Spiegel 2015) is the default: it handles velocity-dependent 1PN forces natively, follows Brouwer's law, and uses compensated summation.
* **ABM PECE(C), order 13**, is ephemeris mode for DE440 reproduction (the JPL/EPM/INPOP heritage, about 2 force evaluations per step), with the Moon integrated geocentrically and bodies tiered Major/Minor/Ring.
* **Gauss–Legendre IRK16** (Hairer–McLachlan–Razakarivony round-off fix) is the symmetric reference.
* **WH/WHFast + Saha–Tremaine pseudo-velocity 1PN** covers fast Newtonian and GR-potential rungs only. Full EIH under WH is unsupported.
* No permissively licensed Rust IAS15 exists (all are GPL), hence the clean-room rule (DEP-008).

### Confirmation
NUM-001…005 tests. Brouwer-law slopes (NUM-023). Cross-agreement between integrators.

