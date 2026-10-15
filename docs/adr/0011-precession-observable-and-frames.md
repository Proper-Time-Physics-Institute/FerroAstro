---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Precession observable and frames

## Context and Problem Statement
Published Mercury values differ by frame (5599.74 equinox-of-date; 577.72 fixed J2000 ecliptic; 575.31 Mercury mean-orbit plane) and by estimator (Newtonian values in the literature span 526.7–532.4″/cy).

## Decision Outcome
The canonical observable is the **Park et al. 2017 estimator**: heliocentric perihelion passages from a 2000-yr run, projected onto the ICRF-fixed mean Mercury orbit frame (pole α 280.9876°, δ 61.4481°), fitted with B + ϖ̇t + Qt² + periodic terms. Per-rung contributions come from **differential runs** with identical initial conditions. Every output carries frame, epoch, span, fit model and constants version. The equinox-referred ≈5600″/cy value is only ever constructed explicitly with the IAU 2006 p_A.

### Confirmation
PHY-015 metadata test; NUM-011 reproduces 575.31 ± 0.01 for the DE440-class model.

