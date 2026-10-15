---
status: proposed
date: 2026-10-14
---
<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# `Real` scalar trait and precision modes

## Context and Problem Statement
Multi-millennia runs and DE440 reproduction need more precision than f64 (Journal #4: f64 Mercury round-off is 2.47 m over 40 yr), but showcase speed needs f64. Rust has no stable 80-bit type, and `f128` is nightly-only.

## Decision Outcome
A crate-owned `Real` trait with backends **f64** (default), **double-double** (FMA-based TwoProd) and **f128** (nightly feature). Precision is a type parameter, not a runtime switch. "Ephemeris mode" = Subbotin–Kodukov–Pavlov mixed scheme (DD state and accumulators, f64 forces, about 3× f64 cost). The DD backend is chosen by benchmark and accuracy audit among `qd` (MIT), `twofloat` (BSD-3, preliminary transcendentals) or our own. `rug`/GMP is excluded (LGPL, no WASM).

### Confirmation
`proptest` against `astro-float` with documented ulp bounds (DEP-012). The f64 benchmark is unchanged after the generic refactor (NUM-006).

