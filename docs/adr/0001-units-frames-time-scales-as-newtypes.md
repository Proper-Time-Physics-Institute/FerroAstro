---
status: accepted
date: 2026-10-14
decision-makers: maintainers (decision recorded in Journal #6)
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# Units, frames and time scales as home-grown newtypes

## Context and Problem Statement
Mixing up frames (GCRS/ICRF/ecliptic), centres (heliocentric/barycentric), time scales (TT/TDB/TCB) or units is the most common astrodynamics bug, and agents make it too. How should the type system prevent it?

## Considered Options
* `uom` crate: mature, but compiles slowly, gives opaque errors, and its support for double-double storage is UNVERIFIED.
* Home-grown newtypes generic over `Real`.
* Typed boundary with raw arrays inside the hot loop.

## Decision Outcome
Chosen: **home-grown newtypes** (`Epoch<Tdb>`, `State<Icrf, Barycentric>`, `Length`, `GravParam`, …), generic over `Real`. Inside the force kernels, plain `Real` SoA arrays are allowed behind a typed API.

### Consequences
* Good: readable compile errors for agents, fast builds, and double-double works.
* Bad: we maintain our own dimension set, kept deliberately small.

### Confirmation
`trybuild` compile-fail tests prove that each cross-frame or cross-scale mistake fails to compile (API-001).

