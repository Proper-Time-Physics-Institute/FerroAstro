<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# NUM-012 cross-estimator spread: single-body rungs of the Mercury ladder

NUM-012's acceptance asks the docs to report how far the precession estimators disagree on the Mercury
ladder. This page reports it for five single-body rungs (M2); the [Scope](#scope) section lists what is not
covered.

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

Rates in ″/cy, 1000 orbits per rung, 44074 samples every 2 d for the Park2017 columns. Every row has the
initial-condition hash `6596cdfc5a9ca58d`.

Measured on 2026-11-22 with the code of `main` at `e86984d`, by running the test and copying its printed
table:

```sh
cargo test -p ferroastro-ladder --test num012_estimator_spread -- --nocapture
```

The run was local (x86-64 Linux, rustc 1.95.0, `header.440` with SHA-256 `0f4636b6…6ac3359`). The same test
passed in CI at the same commit (`ci` run 45030787159, rustc 1.99.0, the repository's pin), but the CI log
does not print the table of a passing test, so the digits below are those of the local run.

| Rung | Passages | LRL | Park2017 linear (jack-knife σ) | Park2017 quadratic | Analytic | Spread |
|---|---|---|---|---|---|---|
| SR-a | 7.163412435 | 7.163416197 | 7.163412432 (5.2e-9) | 7.163412424 | 7.163413185 | 3.77e-6 |
| SR-b/harmonic | 50.143885680 | 50.143898151 | 50.143885680 (4.3e-9) | 50.143885697 | 50.143892140 | 1.25e-5 |
| GR-SCHW | 42.980468783 | 42.980486451 | 42.980468779 (5.1e-9) | 42.980468759 | 42.980478977 | 1.77e-5 |
| J2 (Park17 J2) | 0.028601478 | 0.028601464 | 0.028601469 (5.3e-9) | 0.028601500 | 0.028601479 | 3.60e-8 |
| LT | −0.002018350 | −0.002018348 | −0.002018349 (5.4e-9) | −0.002018328 | −0.002018349 | 2.24e-8 |

The largest spread is 1.77×10⁻⁵ ″/cy (GR-SCHW), under a fifth of the bound.

The "Analytic" column is not an estimator and does not enter the spread. It is the analytic per-orbit
advance of the integrated dynamics (a test particle, μ = GM☉) divided by the Keplerian period of the initial
orbit computed with μ = GM☉ + GM☿. This is the rate convention of PHY-004, which every rung report prints
(`RATE_CONVENTION` in `crates/ferroastro-ladder/src/single_body.rs`; OI-77, closed 2026-10-21).

## Reading the table

- Passages and the two Park2017 fits agree within about 3×10⁻⁸ ″/cy on every rung (the widest gap is
  3.1×10⁻⁸, between the linear and the quadratic fit for J2). Passages and the linear fit agree within
  1×10⁻⁸.
- The LRL estimator is the outlier on the 1PN rungs: +1.77×10⁻⁵ ″/cy against passages for GR-SCHW
  (4.1×10⁻⁷ relative), +1.25×10⁻⁵ for SR-b/harmonic and +3.8×10⁻⁶ for SR-a. The Newtonian eccentricity
  vector carries 1PN periodic terms, and they do not average out when the samples are taken at the
  integrator's own, non-uniform steps. A 1PN-corrected LRL vector is not implemented; its form is an open
  item (OI-40).
- On the J2 and LT rungs the LRL estimator is not the outlier. There the spread is set by the Park2017
  quadratic fit, which lies 3.1×10⁻⁸ (J2) and 2.1×10⁻⁸ ″/cy (LT) from the linear fit, four to six times the
  linear fit's jack-knife σ.
- The analytic rate for GR-SCHW is 1.02×10⁻⁵ ″/cy above the passages rate (2.4×10⁻⁷ relative). This is
  not an estimator error. It has two parts:
  - 6.6×10⁻⁶ ″/cy (1.5×10⁻⁷ relative): the passages rate divides by the anomalistic period of the integrated
    orbit and the analytic rate by a Keplerian period. This attribution is the one recorded in
    `IMPLEMENTATION-NOTES.md` (WP4 and WP6); it was not re-derived for this page.
  - 3.57×10⁻⁶ ″/cy (8.30×10⁻⁸ relative, which is GM☿/(2 GM☉)): the Keplerian period of the analytic rate
    uses μ = GM☉ + GM☿, while the integrated test particle has μ = GM☉. This part was checked for this
    page: the Analytic column of every 1PN rung is 1 + 8.30×10⁻⁸ times the value recorded on 2026-10-15,
    and commit `9fd3b40` (2026-10-15, review F5), which introduced the convention, records the same
    +3.567×10⁻⁶ ″/cy for GR-SCHW.

## Earlier table (2026-10-15), superseded

[`IMPLEMENTATION-NOTES.md`](../strategy/IMPLEMENTATION-NOTES.md) (WP6, section "NUM-012 cross-estimator
spread") holds the first measurement. It is kept there as a record and differs from the table above in two
ways:

- Its Analytic column was computed before commit `9fd3b40`, with the Keplerian period from GM☉ alone
  (GR-SCHW 42.980475410, SR-a 7.163412591, SR-b/harmonic 50.143887978). Its text therefore gives 6.6×10⁻⁶
  ″/cy for analytic minus passages on GR-SCHW, where the current code gives 1.02×10⁻⁵.
- Its estimator columns differ from the current ones by up to 8×10⁻⁸ ″/cy (the LRL value for GR-SCHW), and
  its J2 and LT spreads were 1.6×10⁻⁸ and 7.9×10⁻⁹ ″/cy, against 3.60×10⁻⁸ and 2.24×10⁻⁸ now. The cause was
  not isolated. The integrator's default predictor changed between the two measurements (commit `f6f6e27`,
  2026-11-08), and no other change to the estimators or to the single-body force terms was looked for
  beyond the commit log.

The conclusions are the same in both: the largest spread is the LRL offset on GR-SCHW, about 1.8×10⁻⁵ ″/cy,
and every spread is below the 10⁻⁴ ″/cy bound.

## Scope

The table covers five of the eight single-body rungs: SR-a, SR-b/harmonic, GR-SCHW, J2 and LT. The test does
not run the other three (SR-b/areal, J2-DE440 and J2-INCL), so no spread is reported for them. The osculating
Ω + ω ("dog-leg") longitude of the Park2017 estimator, which NUM-012's statement names, is not a column of
the table; in the initial orbit plane the osculating ϖ is the LRL column.

The N-body rungs (M3 onward) use the Park2017 estimator (NUM-011, authoritative under OI-68's provisional
resolution) and are not covered here.
