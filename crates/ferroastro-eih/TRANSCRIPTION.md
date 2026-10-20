<!--
SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
SPDX-License-Identifier: Apache-2.0
-->
# EIH transcription record (NUM-020, open item OI-33)

**Status: first transcription done (agent, WP19, 2026-10-20). Independent second check: NOT DONE.**

PHY-014 (rung EIH-FULL, WP22) must not rely on `ferroastro-eih` until a human reviewer signs the
second check below.

## Sources
**Primary.** Park, R. S., Folkner, W. M., Williams, J. G., & Boggs, D. H. 2021, *The JPL Planetary
and Lunar Ephemerides DE440 and DE441*, AJ 161, 105, doi:10.3847/1538-3881/abd414. The equations
used are:
- Eq. 27 (§3.1 "Point-mass Acceleration", p. 5), attributed there to Will & Nordtvedt 1972 and
  Moyer 2003;
- Eqs. 1–2 (§2.2 "Solar System Barycenter", p. 2).

The first transcription was read from page images of the typeset PDF, rendered at 300 dpi. The
PDF's text layer garbles the equations, so it was not used.

**Also cited by NUM-020, but not read for the first transcription.** Folkner, W. M., Williams, J.
G., Boggs, D. H., Park, R. S., & Kuchynka, P. 2014, IPN Progress Report 42-196, Eq. 27 (DE430).
It is not in the local corpus. Park et al. 2021 say they repeat the DE430 material "so that this
paper can be self-contained". **The second check should compare against both PDFs.**

**Clean room (DEP-008).** No REBOUND, REBOUNDx (`gr_full.c`), ASSIST, Swiftest or Mercury6 source
was opened.

## Eq. 27 as typeset
Vectors are bold in the paper. Here r_ij = |r_i − r_j|, and the sums run over all bodies.

```
a_i,pm−pm = Σ_{j≠i} GM_j (r_j − r_i)/r_ij³ · { 1 − 2(β+γ)/c² Σ_{k≠i} GM_k/r_ik − (2β−1)/c² Σ_{k≠j} GM_k/r_jk
                                              + γ (v_i/c)² + (1+γ) (v_j/c)² − 2(1+γ)/c² v_i·v_j
                                              − 3/(2c²) [ (r_i − r_j)·v_j / r_ij ]² + 1/(2c²) (r_j − r_i)·a_j }
          + 1/c² Σ_{j≠i} GM_j/r_ij³ { [r_i − r_j] · [ (2+2γ) v_i − (1+2γ) v_j ] } (v_i − v_j)
          + (3+4γ)/(2c²) Σ_{j≠i} GM_j a_j / r_ij
```

## Term-by-term table
**Where the terms live.**
- **Code:** `crates/ferroastro-eih/src/eih.rs`, `Eih::correction_with`. The code adds only the c⁻²
  part; the Newtonian "1" term is in `ferroastro_forces::NewtonianMutual`.
- **Sympy:** `tools/eih_limit.py`, `eq27_correction` and `Coefficients`, written independently of
  the Rust code.
- **Symbols:** `U_i = Σ_{k≠i} GM_k/r_ik`; S1 is the sum that multiplies (r_j − r_i).

| # | typeset term | sign and coefficient | Rust (`eih.rs`) | sympy (`eih_limit.py`) | 2nd check |
|---|---|---|---|---|---|
| T0 | GM_j (r_j − r_i)/r_ij³ · 1 | +1 | `NewtonianMutual` (forces) | not in the correction | ☐ |
| T1 | −2(β+γ)/c² Σ_{k≠i} GM_k/r_ik, inside {} | −2(β+γ) · U_i | `k_ui * p.u[i]` in `b` | `ui=-2*(BETA+GAMMA)` × `_potential(i)` | ☐ |
| T2 | −(2β−1)/c² Σ_{k≠j} GM_k/r_jk, inside {} | −(2β−1) · U_j | `k_uj * p.u[j]` | `uj=-(2*BETA-1)` × `_potential(j)` | ☐ |
| T3 | +γ (v_i/c)², inside {} | +γ v_i² | `gamma * vi2` | `vi2=GAMMA` | ☐ |
| T4 | +(1+γ)(v_j/c)², inside {} | +(1+γ) v_j² | `k_vj2 * dot(vj, vj)` | `vj2=1+GAMMA` | ☐ |
| T5 | −2(1+γ)/c² v_i·v_j, inside {} | −2(1+γ) | `k_vivj * dot(vi, vj)` | `vivj=-2*(1+GAMMA)` | ☐ |
| T6 | −3/(2c²) [(r_i − r_j)·v_j / r_ij]², inside {} | −3/2; the projection uses **v_j** and **(r_i − r_j)** | `- three_halves * proj * proj`, with `proj = dot(d_ij, vj)/rij` | `proj=-3/2`, `(_dot(d_ij, bj.v)/rij)**2` | ☐ |
| T7 | +1/(2c²) (r_j − r_i)·a_j, inside {} | +1/2; **(r_j − r_i)**, not (r_i − r_j) | `R::HALF * dot(d_ji, aj)` | `aj=1/2`, `_dot(d_ji, bj.a)` | ☐ |
| T8 | {} multiplies GM_j (r_j − r_i)/r_ij³ | — | `s1 = gj*inv_r3*b`, times `d_ji[c]` | `bj.gm*d_ji[c]/rij**3*brace` | ☐ |
| T9 | +1/c² Σ GM_j/r_ij³ {[r_i − r_j]·[(2+2γ)v_i − (1+2γ)v_j]} (v_i − v_j) | +(2+2γ), −(1+2γ); dot with **(r_i − r_j)**; times **(v_i − v_j)** | `s2 = gj*inv_r3*dot(d_ij, w)`, `w = k_s2i*vi − k_s2j*vj`, times `(vi[c]-vj[c])` | `s2i=2+2*GAMMA`, `s2j=1+2*GAMMA` | ☐ |
| T10 | +(3+4γ)/(2c²) Σ GM_j a_j / r_ij | +(3+4γ)/2 | `k_s3 * gj / rij`, times `aj[c]` | `s3=(3+4*GAMMA)/2` | ☐ |
| T11 | overall 1/c² on T1–T10 | — | `s.map(\|x\| x * inv_c2)` | the c⁻² coefficient (ε = 1/c²) | ☐ |
| T12 | "summations are over all bodies" | — | the **Major** bodies of the model; Minor and Ring bodies stay Newtonian (NEEDS HUMAN REVIEW for WP22: in DE440 the 343 asteroids are integrated, and whether they enter the EIH sums is not stated) | all bodies in the list | ☐ |
| T13 | a_j on the right-hand side | Eq. 27 does not say | the Newtonian point-mass acceleration by the Majors (default; NUM-018), or one fixed-point step (`RhsAcceleration::Iterated`) | symbolic `a` | ☐ |

**Eqs. 1–2 (relativistic barycentre)**, in `crates/ferroastro-eih/src/barycentre.rs`:

| # | typeset | Rust | 2nd check |
|---|---|---|---|
| B1 | r_ssbc = (Σ_i μ*_i r_i) / (Σ_i μ*_i), "over all bodies with finite mass" | `relativistic_barycentre` | ☐ |
| B2 | μ*_i = GM_i (1 + v_i²/(2c²) − 1/(2c²) Σ_{j≠i} GM_j/r_ij), v_i barycentric | `mu_star`: `gm * (1 + (v2 − u)/(2c²))`; bodies with GM = 0 are skipped in u | ☐ |

## Evidence from the first transcription (WP19)
**Symbolic (C1).** `tools/eih_limit.py` takes a massless body i and a static source j
(v_j = a_j = 0). Its Eq. 27 correction equals the R3 Schwarzschild correction
`μ/(c²r³)[(2(β+γ)μ/r − γv²) r + 2(1+γ)(r·v) v]` for general β and γ, and the source receives no
correction. A negative control (T3's sign flipped) is detected. This checks T1, T3, T8, T9 and
T11 in the limit. **It cannot see T2, T4–T7 or T10**: they vanish when v_j = a_j = 0 and the
partner is massless.

**Symbolic, two bodies at β = γ = 1** (`check_lagrangian` in `tools/eih_limit.py`). This check
was added after mutation probe E3 (T6's coefficient −3/2 → −1) survived every other check. It
compares Eq. 27 for two bodies with the order-reduced Euler–Lagrange equations of the EIH
Lagrangian (Landau & Lifshitz, *The Classical Theory of Fields*, Eq. 106.17), a source
independent of the DE papers.
- They agree to 60 significant digits at three random configurations with general masses,
  positions and velocities, for **every** term T1–T10.
- The Lagrangian's energy is conserved at O(c⁻²) under Eq. 27.
- A negative control (T6 at −1) is detected.

β and γ enter only T1–T5 and T9–T10, linearly. The β, γ dependence is checked by the R5 → R3
limit and by the PPN ratio test, not by the Lagrangian.

**Numerical, PR tier** (`crates/ferroastro-eih/tests/eih_checks.rs`, `crates/ferroastro-ladder/tests/phy005_eih.rs`):

| check | result |
|---|---|
| R5 → R3 limit | 1.7e-16 relative |
| Sun + Mercury rate | 42.980114 ″/cy, R3 + 2.9e-5 |
| PPN ratio (2+2γ−β)/3 | to 4e-10 |
| RHS iteration | O(c⁻⁴) |
| **Relativistic barycentre conservation** | 3.2e-10 km over 10 yr, vs 2.0e-5 km for the Newtonian barycentre |
| Equal-mass binary: periastron advance vs 6πGM/(c²p) | −1.5e-5 relative |
| Equal-mass binary: 1PN Lagrangian energy | conserved to 5.3e-10 of \|E_N\|, scaling as c⁻⁴ |

Eight coefficient and sign mutants of `eih.rs` (IMPLEMENTATION-NOTES WP19) are all killed.

The conservation check is the main test of T2 and T4–T10 together. It involves every term with
v_j and a_j. A sign or coefficient error in any of them would break the conservation of the 1PN
centre of mass at the c⁻² level. **It is a consistency check, not a transcription check**: an
error that keeps the equations Lagrangian-consistent would pass.

**Real data (C7).** EIH + J2 against de440.bsp over 10 yr: Mercury 1.44 km barycentric (55 m
heliocentric), against 3409 km for Newtonian + J2.

## Second check (to be completed by a human reviewer; NUM-020 acceptance)
Read Eq. 27 and Eqs. 1–2 in **both** typeset PDFs (DE430 IPN PR 42-196 and DE440 AJ 161, 105).
Compare each row of the tables above with the PDF, with `eih.rs` and with `eih_limit.py`, then
tick the row. Record any discrepancy as an issue linked to OI-33.

- Reviewer:
- Date:
- PDFs and pages read:
- Rows ticked:
- Discrepancies found:
- Signature (sign-off of NUM-020 / closes OI-33 for the EIH equation):
