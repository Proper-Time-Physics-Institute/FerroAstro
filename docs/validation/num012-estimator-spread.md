<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# NUM-012 cross-estimator spread: single-body rungs of the Mercury ladder

NUM-012's acceptance asks the docs to report how far the precession estimators disagree on the Mercury
ladder. This page reports it for the single-body rungs (M2).

## What is compared

Four estimators measure the same run of each rung: the same dynamics and the same initial state (the
initial-condition hash is printed with each row).

| Estimator | Definition | Code |
|---|---|---|
| Passages | Per-orbit advance fitted to the perihelion passages. It is the estimator of the rung tests. | `ferroastro-precession`, `passages` |
| LRL | Longitude of the eccentricity (Laplace–Runge–Lenz) vector in the initial orbit plane. In that plane it is the osculating ϖ. | `ferroastro-precession`, `lrl` |
| Park2017 linear | The Park et al. 2017 estimator (NUM-011) on 2-day samples in the initial orbit plane, linear fit, with its jack-knife σ. | `ferroastro-precession`, `park2017` |
| Park2017 quadratic | The same with a quadratic fit. | `ferroastro-precession`, `park2017` |

The spread of a rung is the largest of the four rates minus the smallest.

## The bound, checked on every PR

`crates/ferroastro-ladder/tests/num012_estimator_spread.rs::single_body_estimators_agree` prints the table
below and asserts that each rung's spread is at most 10⁻⁴ ″/cy (vector `NUM012-ESTIMATOR-SPREAD` in
`tests/vectors/single-body-rungs.toml`). The bound is the NUM-013(a) isolation tolerance: a larger spread
would make that comparison depend on the choice of estimator. The test runs in the PR tier.

## Recorded table

Rates in ″/cy, 1000 orbits per rung. These are the values recorded in
[`IMPLEMENTATION-NOTES.md`](../strategy/IMPLEMENTATION-NOTES.md) (WP6, section "NUM-012 cross-estimator
spread", measured 2026-10-15). They were not re-measured for this page. The default IAS15 predictor changed on
2026-11-08, so the trailing digits at current `main` may differ; the test prints the current table:

```sh
cargo test -p ferroastro-ladder --test num012_estimator_spread -- --nocapture
```

| Rung | Passages | LRL | Park2017 linear (jack-knife σ) | Park2017 quadratic | Analytic | Spread |
|---|---|---|---|---|---|---|
| SR-a | 7.163412437 | 7.163416199 | 7.163412433 (5.3e-9) | 7.163412464 | 7.163412591 | 3.8e-6 |
| SR-b/harmonic | 50.143885684 | 50.143898126 | 50.143885682 (6.3e-9) | 50.143885671 | 50.143887978 | 1.2e-5 |
| GR-SCHW | 42.980468784 | 42.980486530 | 42.980468786 (3.2e-9) | 42.980468768 | 42.980475410 | 1.8e-5 |
| J2 (Park17 J2) | 0.028601483 | 0.028601467 | 0.028601479 (4.4e-9) | 0.028601468 | 0.028601476 | 1.6e-8 |
| LT | −0.002018351 | −0.002018349 | −0.002018345 (4.2e-9) | −0.002018353 | −0.002018349 | 7.9e-9 |

The largest spread is 1.8×10⁻⁵ ″/cy (GR-SCHW), about a fifth of the bound.

## Reading the table

- Passages and the two Park2017 fits agree within about 3×10⁻⁸ ″/cy on every rung.
- The LRL estimator is the outlier on the 1PN rungs (+1.8×10⁻⁵ ″/cy for GR-SCHW, 4×10⁻⁷ relative). The
  Newtonian eccentricity vector carries 1PN periodic terms, and they do not average out when the samples are
  taken at the integrator's own, non-uniform steps. A 1PN-corrected LRL vector is not implemented; its form
  is an open item (OI-40).
- The "Analytic" column is not an estimator. The 6.6×10⁻⁶ ″/cy between it and the passages rate for GR-SCHW
  is the difference between the anomalistic and the Keplerian period, not an estimator error.

## Scope

The table covers the single-body rungs only. The N-body rungs (M3 onward) use the Park2017 estimator
(NUM-011, authoritative under OI-68's provisional resolution) and are not covered here.
