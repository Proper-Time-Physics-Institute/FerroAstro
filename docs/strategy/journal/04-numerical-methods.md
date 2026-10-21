## 📓 Journal #4 — Numerical methods & measuring precession

> **Lead review notes:**
> - Re-checked R2 independently. From $d(\gamma\mathbf v)/dt=\mathbf F/m$ with a Newtonian $\mathbf F$, we get $\mathbf a=-\mu[\mathbf r-(\mathbf r\cdot\mathbf v)\mathbf v/c^2]/(\gamma r^3)$, which matches. R3 matches the standard IERS-form 1PN term. The R5 structure matches the familiar DE-series EIH form, but NUM-020 (a second check against the typeset PDF) still stands.
> - Agrees with Journal #1: SR-a = GR/6 = 7.163″/cy, J2 0.0286, LT −0.0020.
> - **Architectural consequences adopted for the plan:**
>   - IAS15 is the default integrator. ABM13 with the mixed double-double scheme is the "ephemeris mode" for DE440 reproduction.
>   - The Moon is integrated geocentrically.
>   - Bodies are tiered: Major (EIH), Minor (Newtonian from the Majors), and Ring.
>   - The Park 2017 estimator is the canonical observable.
> - The 80-bit x87 path is dropped, since it is unavailable in stable Rust. That fits DEP-007.

**Scope.** This entry covers which integrators to use, when f64 is not enough, how to extract a perihelion-precession rate (″/cy) from a trajectory in a way that matches the published observable, the exact equations of motion for each physics rung, and where the CPU time goes for N ≈ 10–350 bodies.

**Evidence labels.** A claim marked **[V]** was checked against the primary source during this review (PDF text or abstract page). **UNVERIFIED** means the claim comes from memory, a secondary source, or my own estimate, and must be checked before anyone quotes it. A claim marked *(derived here)* is my own arithmetic or reasoning. Those are not literature results.

---

### TL;DR recommendations

1. **Default integrator: an IAS15-class adaptive 15th-order Gauss–Radau integrator with compensated summation.** It handles velocity-dependent 1PN/EIH forces natively through its predictor–corrector loop. Rein & Spiegel also report that its energy error follows Brouwer's law in f64 **[V]**.
2. **Ephemeris-fidelity integrator (the DE440 path): a fixed-step Adams–Bashforth–Moulton (ABM) PECE(C) method of order ~12–13, with the Moon integrated geocentrically.** This is the JPL / EPM / INPOP heritage. It needs about 2 force evaluations per step, against roughly 15 for IAS15. A step of 1/16 d with order 13 is the published EPM-based reference configuration **[V]**.
3. **Symmetric reference integrator: Gauss–Legendre implicit RK (IRKGL16)**, using the Hairer–McLachlan–Razakarivony round-off fix. It serves as an independent cross-check, and it is the only family on this list that stays symplectic and symmetric for an arbitrary Hamiltonian.
4. **Fast rung only: Wisdom–Holman / WHFast.** Use it for Newtonian runs plus a velocity-independent GR potential, or the Saha–Tremaine pseudo-velocity 1PN scheme. Do **not** use it for full EIH unless an implicit or mixed scheme is built.
5. **Precision: f64 plus compensated summation by default. Add an opt-in "mixed double-double" mode (Subbotin–Kodukov–Pavlov 2023 scheme) for DE-reproduction runs.** In that mode the state and integrator accumulators are double-double and the forces are f64, at roughly 3× the cost of f64 **[V]**. Reserve full double-double or f128 for validation.
6. **Precession observable: follow Park et al. (2017) exactly** **[V]**:
   - Find heliocentric perihelion passages over a 2000-yr run.
   - Project them onto a fixed (ICRF-referenced) mean-Mercury-orbit frame.
   - Fit $B + \dot\varpi t + Q t^2 + \sum_i (S_i \sin\nu_i t + C_i\cos\nu_i t)$.

   For each physics rung, also use **differential runs** (rung on minus rung off, same ICs) and compare against the analytic per-orbit formulas in §4.

---

### 1. Integrators

#### 1.1 What the production ephemerides use

| Ephemeris | Integrator | Arithmetic | Source |
|---|---|---|---|
| JPL DE (DE102 onward) | DIVA: variable-step, variable-order Adams (Krogh) | Pre-DE405: UNIVAC 72-bit "double" (60-bit fraction). DE405: 128-bit quad on DEC/VAX Alpha. DE410: plain IEEE double, which caused 1–2 m numerical noise. | Subbotin et al. 2023 §4.1 **[V]**, citing Standish et al. 1992 and the DE410 IOM |
| JPL DE (DE414 onward) | QIVA = DIVA in emulated quad precision, **mixed**: only the Newtonian acceleration in quad, the rest in double. About 30× the cost of double. "Probably" still used for DE430/440. | mixed quad/double | Subbotin et al. 2023 **[V]** (their wording is "probably"). DE440 paper (Park et al. 2021) does not name the integrator **[V, by text search]** |
| EPM (since EPM2004) | Adams-family multistep (EPM-based study: ABM PECEC, order 13, h = 1/16 d, Dormand–Prince 8 start-up at h/8) | 80-bit x87 extended | Subbotin et al. 2023 **[V]**. Whether production EPM uses exactly this order and step is UNVERIFIED |
| INPOP (since INPOP06) | "Adams–Cowell in extended precision" | 80-bit extended | Fienga IHES 2014 slides; Subbotin et al. 2023 **[V]** |
| INPOP08 → La2010 fit | INPOP's Adams integrator extended to 1 Myr in 80-bit (about 4 CPU-months, 2010-era workstation) | 80-bit | Laskar et al. 2011 (La2010), A&A, doi:10.1051/0004-6361/201116836. Text snippet only; full PDF not read, so treat as UNVERIFIED in detail |
| La2004 | Integrator details not confirmed in this review | — | Laskar et al. 2004, A&A 428, 261, doi:10.1051/0004-6361:20041335. UNVERIFIED |
| DE430/DE431 asteroids | 343 asteroids "integrated iteratively with the positions of the Sun, Moon, and planets" (not fully coupled) | — | Folkner et al. 2014, IPN PR 42-196 **[V]** |

**Takeaways.**
- Every major ephemeris uses a fixed- or variable-step **Adams/Cowell multistep** method with **>f64 accumulation**.
- The minimum standard has moved to "extended precision, at least for the Newtonian part".
- DE441 covers −13,200 to +17,191. The planetary orbits in DE440 and DE441 differ by <1 m over the 1100-yr DE440 span **[V]**.

#### 1.2 Candidate integrators

Cost is given in force evaluations ("evals") per step.

| Method | Order / type | Evals per step | Step control | Velocity-dependent forces (1PN/EIH) | Long-term error (f64) | Fit for FerroAstro |
|---|---|---|---|---|---|---|
| **IAS15** (Rein & Spiegel 2015) | 15, Gauss–Radau (Everhart-type), predictor–corrector | ~7 × (iterations). Usually ~2 iterations, capped at 12 **[V]**, so ~15 typical (derived here) | adaptive, scale-free criterion | **Native.** The predictor–corrector loop iterates implicit dependence on $v$ **[V]** | Energy follows **Brouwer's law**: $\propto n^{1/2}$ for energy, $\propto n^{3/2}$ for phase. Uses compensated summation **[V]** | **Primary** for science runs and all rungs |
| Everhart RADAU15 (1985) | 15, Gauss–Radau | ~7 × fixed 6 iterations (Everhart's suggestion, per Rein & Spiegel) | adaptive (dimensional tolerance) | yes | Linear energy drift in the MERCURY implementation **[V, per Rein & Spiegel]** | Superseded by IAS15 |
| **ABM / Adams–Cowell PECE(C)** (DIVA/QIVA, EPM, INPOP) | 10–14, linear multistep | **2** (PECE) or 3 (PECEC) | fixed (EPM study) or variable (DIVA) | yes. $a_j$ on the RHS is replaced by its Newtonian value, an $O(c^{-4})$ effect **[V]** | Truncation can sit well below round-off. Round-off then dominates, which is why everyone uses extended precision | **Primary** for DE440 reproduction (cheapest per step) |
| Gauss–Jackson (Störmer–Cowell summed form) | ~8–12, second-order multistep | 1–2 | fixed (variable versions exist) | yes, via PECE on $\dot r$ | Similar to ABM. Summed form reduces round-off | Alternative to ABM. Ref: Berry & Healy 2004, J. Astronaut. Sci. 52, 331 (UNVERIFIED page range) |
| Symmetric multistep (Quinlan & Tremaine 1990, AJ 100, 1694) | 12, symmetric Störmer | 1 | fixed | no (position-only forces) | No secular truncation drift. Weak instabilities possible | Newtonian-only reference |
| **Wisdom–Holman / WHFast** (WH 1991, AJ 102, 1528; Rein & Tamayo 2015, arXiv:1506.01084) | 2 (+ symplectic correctors, up to 11th order) | 1 | fixed | **Breaks symplecticity** (see §1.3) | Bounded truncation energy error. Rein & Spiegel report **linear** round-off energy growth for standard WH/MVS implementations **[V]** | **Fast Newtonian rung** plus GR-potential rung |
| WHFast512 (Javaheri, Rein & Tamayo 2023, OJAp 6) | as WHFast, AVX-512 | 1 | fixed | no | as WHFast | Up to 4.7× over WHFast. 8 planets × 5 Gyr in <1.4 d **[V, abstract]**. Design pattern for SIMD |
| **Gauss–Legendre IRK** (IRKGL16; FCIRK16: Antoñana et al. 2022, CeMDA, doi:10.1007/s10569-022-10081-9) | 16 (8 stages), implicit, symmetric, symplectic | 8 × fixed-point iterations | fixed | Yes, with caveats (§1.3) | Brouwer's law reachable with the round-off-aware implementation (Hairer, McLachlan & Razakarivony 2008, BIT 48, 231, doi:10.1007/s10543-008-0170-3 **[V]**). Mixed precision used in FCIRK16 **[V, abstract]** | **Reference / cross-check** |
| Implicit midpoint | 2, symplectic, symmetric | 1 × iterations | fixed | yes | low order, so too slow for this accuracy | Teaching / tests only |
| Bulirsch–Stoer (Gragg extrapolation) | variable, high | many (~10–100 for a step) | adaptive | yes | Rein & Spiegel find **linear** energy growth (MERCURY implementation) **[V]** | Not recommended as default. Useful as an independent check |
| **Taylor series** (TIDES: Abad et al. 2012, ACM TOMS 39(1), doi:10.1145/2382585.2382590 **[V]**; heyoka: Biscani & Izzo 2021, MNRAS 504, 2614, doi:10.1093/mnras/stab1032 **[V]**; TaylorIntegration.jl / PlanetaryEphemeris.jl, which reimplements the DE430 model (repo README **[V]**) | 20–30+, explicit, automatic differentiation | one "jet" (~cost of several evals; UNVERIFIED factor) | adaptive (order and step) | yes (any smooth RHS) | heyoka reports Brouwer's law for energy in long-term planetary runs **[V, abstract]**. TIDES offers quad and multi-precision **[V]** | **Stretch goal.** Needs an AD expression system. Natural fit for variational equations / jet transport (Pérez-Hernández & Benet 2022, Commun. Earth Environ. 3, 10, Apophis) |

#### 1.3 Velocity-dependent 1PN forces and symplectic methods

- **Why WH breaks.** The 1PN Hamiltonian mixes coordinates and momenta, so it does not split into "Kepler + position-only interaction" **[V]**. Saha & Tremaine 1994 (AJ 108, 1962, doi:10.1086/117210) give the $O(c^{-2})$ Sun-dominant Hamiltonian:

  $$H_{PN}=\frac{1}{c^2}\sum_i\left(\frac{\mu_i^2\,\tilde m_i}{2\tilde r_i^2}-\frac{\tilde p_i^4}{8\tilde m_i^3}-\frac{3\mu_i\,\tilde p_i^2}{2\tilde m_i\,\tilde r_i}\right)$$

  This is the standard isotropic-gauge single-centre 1PN Hamiltonian. Its three-term structure matches what was extracted from their Eq. 26, but the coefficients must be cross-checked against the typeset paper (UNVERIFIED).

- **Ad hoc fixes and their flaw.** The usual fix is a velocity-independent potential $U_{PN}$ (Nobili & Roxburgh 1986; Sussman & Wisdom 1992) or the orbit average $\bar H_{PN}$. Saha & Tremaine note this reproduces the precession but gives the **mean motion an error of order the PN effect itself** **[V]**. REBOUNDx `gr_potential` has the same limitation **[V, REBOUNDx docs]**.
- **Saha & Tremaine remedy** **[V]**. They rearrange $H_{PN}$ into three individually integrable pieces:
  - $\propto H_{\rm Kep}^2$: implemented by **rescaling the Kepler-drift time** by $\tau_i' = \tau_i(1 - 3\mu_i/(2c^2 a_i))$-type factors.
  - $\propto 1/r^2$: absorbed into the interaction kick.
  - $\propto p^4$: extra leapfrog sub-drifts.

  The integration then runs in **pseudo-velocities** $\tilde v = \tilde p/\tilde m \neq \dot x$, converted on input and output. Cost is <5% overhead. It covers **only the dominant Sun-planet 1PN terms**, not full EIH.
- **REBOUNDx experience.** Tamayo, Rein, Shi & Hernandez 2020 (MNRAS, doi:10.1093/mnras/stz2870) report that earlier operator-splitting approaches give "qualitatively wrong answers for conservative but velocity-dependent forces like post-Newtonian corrections" **[V, abstract]**. REBOUNDx provides three options **[V, docs]**:
  - `gr_potential` (Nobili–Roxburgh)
  - `gr` (Anderson et al. 1975, single dominant mass)
  - `gr_full` (EIH after Newhall et al. 1983)
- **Implicit symplectic option.** Write the EIH 1PN *Lagrangian* with β, γ (Estabrook 1971, JPL IOM, cited by DE430 **[V]**; Will & Nordtvedt 1972). Legendre-transform it to $H(q,p)$ to $O(c^{-2})$. Then apply Gauss–Legendre IRK or implicit midpoint in canonical variables. The result is symplectic for any $H$.
- **Caveat** *(derived here)*. Gauss collocation applied to the *velocity-form* ODE $\ddot x = f(x,\dot x)$ in $(x,v)$ is **symmetric/reversible but not canonically symplectic**, because $(x,v)$ are not canonical once $p \neq m v$. Symmetry alone usually gives good long-term behaviour for reversible problems, but that is not a guarantee.
- **Pragmatic position (Rein & Spiegel 2015 argument).** If truncation error is pushed below f64 round-off, the result is accurate to machine precision regardless of symplecticity. IAS15 does this and accepts velocity-dependent forces directly **[V]**. FerroAstro therefore does not *need* a symplectic 1PN integrator for the flagship. A symplectic variant is a research item, not a dependency.

#### 1.4 Error growth summary

- **Brouwer's law** (Brouwer 1937, AJ 46, 149 **[V, citation]**). With unbiased rounding, conserved quantities random-walk as $\propto n^{1/2}$. Angle-like quantities (orbital phase / mean longitude) grow as $\propto n^{3/2}$ **[V, Rein & Spiegel 2015]**.
- **Quantitative anchor** (Subbotin, Kodukov & Pavlov 2023, CeMDA, doi:10.1007/s10569-023-10139-2, Table 1 **[V]**). Setup: 40-yr one-way EIH integration, ABM13 PECEC, h = 1/16 d.

  | Arithmetic | Moon | Mercury | Mars | Wall time |
  |---|---|---|---|---|
  | f64 | 0.28 m | 2.47 m | 0.7 m | 81 s |
  | 80-bit x87 | 0.1 mm | 7 mm | 1 mm | 323 s (4×) |
  | mixed double-double | 1.2 mm | 8.4 mm | 0.44 mm | 246 s (3×) |
  | full double-double | 3e-14 m | 3e-16 m | 3e-16 m | 32 min (24×) |

- **Extrapolation to FerroAstro spans** *(derived here, UNVERIFIED)*. Assume pure $t^{3/2}$ scaling of the f64 Mercury along-track error. Then 2.47 m at 40 yr becomes ~0.3 km at 10³ yr and ~3×10² km at 10⁵ yr. That is irrelevant for a ″/cy precession rate but fatal for "reproduce DE441 to metres".
- **Chaos.** The inner solar system's Lyapunov time is ~5 Myr (Laskar 1989, Nature 338, 237, doi:10.1038/338237a0; UNVERIFIED exact value). It does not limit 10³–10⁵-yr runs.

---

### 2. Extended precision

**Where f64 round-off actually bites** *(derived here)*:

1. **Adding a small step increment to a large coordinate.** Over one step the change in $r$ relative to $r$ is ~10⁻² to 10⁻³, so each step loses about 2–3 digits of the increment.
   - Fix: **compensated (Kahan/Møller) summation of the state update**, as IAS15 does **[V]**, or a double-double state as in Subbotin's mixed scheme **[V]**.
2. **Catastrophic cancellation in Earth–Moon differences.** $|r_M - r_E|$ is ~400× smaller than the barycentric vectors **[V]**.
   - Fix: integrate the Moon **geocentrically**, as all three ephemeris families (DE, EPM, INPOP) do **[V]**.
   - Fix: Taylor-expand the tidal (differential) acceleration from third bodies. Four terms of $(1-x)^{-3/2}$ suffice in f64 **[V]**.
3. **Multistep accumulators.** Store the predictor/corrector sums and $\gamma_j$ coefficients in double-double, and the backward differences $\nabla^j f$ in f64 **[V]**.
4. **Force evaluation itself.** In Subbotin's mixed scheme, $r_{ij}$, $r_{ij}$ magnitudes, all relativistic terms, $GM$ and $1/c^2$ stay in **f64**, and the scheme still reaches the 80-bit accuracy level **[V]**.

**Precision options and costs:**

| Option | Cost vs f64 | Portability | Notes |
|---|---|---|---|
| f64 + compensated state update | ~1.1–1.3× (UNVERIFIED estimate) | everywhere | Default |
| Mixed DD (state + accumulators in DD, forces f64) | **~3×** **[V]** | everywhere (needs FMA for a fast TwoProd) | **Opt-in "ephemeris" mode** |
| 80-bit x87 `long double` | 4× (no SIMD) **[V]** | x86 only, not ARM **[V]** | **Not available in stable Rust.** Do not pursue |
| Full double-double | 24× **[V]** | everywhere | Validation runs only |
| Emulated binary128 (f128) | ≥ QIVA-like 30× for mixed use **[V]**. Full use likely much worse (UNVERIFIED) | Rust `f128` is nightly-only as of my knowledge (UNVERIFIED current status) | Golden-reference runs only |

**Requirement.** The precision mode must be a **type parameter** (generic over a `Real` trait with f64, DD and f128 backends), not a runtime switch. Monomorphised f64 code then pays nothing.

---

### 3. Measuring precession from a trajectory

#### 3.1 How Park et al. (2017) define the observable

Reference: Park, Folkner, Konopliv, Williams, Smith & Zuber 2017, AJ 153, 121, doi:10.3847/1538-3881/aa5be2. The definition below was read from the IOP article page **[V]**.

1. Integrate the ephemeris for **2000 yr (1000-01-01 to 3000-01-01)**.
2. Define a **mean Mercury orbit frame**, fixed and referenced to ICRF:
   - $\hat z$ is the mean orbit pole, at RA = 280.9876°, Dec = 61.4481°.
   - $\hat y = \hat z \times \hat x_{\rm ICRF}$.
   - $\hat x$ completes the triad.
   - The solar equator is inclined ~3.4° to this plane.
3. Find the **times of perihelion**, project the position vector at each onto the mean-orbit plane, and measure its angle from the initial perihelion direction.
4. Fit $B + \dot\varpi\,t + Q t^2 + \sum_i (S_i\sin\nu_i t + C_i\cos\nu_i t)$, with epoch 2000-01-01 and **14 periodic terms with amplitude ≥ 0.5″**. The result is $\dot\varpi = 575.3100''$/cy with $Q \approx -0.0448''$/cy².
5. They use $\dot\varpi = \dot\omega + \dot\Omega\cos i$.

**Contribution table (Park 2017, Table 3)**, in ″/Julian century **[V]**:

| Contribution | Rate (″/cy) |
|---|---|
| Venus | 277.4176 |
| Earth/Moon | 90.8881 |
| Mars | 2.4814 |
| Jupiter | 153.9899 |
| Saturn | 7.3227 |
| Uranus | 0.1425 |
| Neptune | 0.0424 |
| Asteroids | 0.0012 |
| Mercury self term | 0.0050 |
| Pairwise interaction terms | up to 0.0411 (Jupiter+Saturn) and −0.0209 (Venus+Earth/Moon) |
| Solar J₂ | 0.0286 ± 0.0011 |
| GR gravitoelectric | **42.9799 ± 0.0009** |
| Lense–Thirring | −0.0020 ± 0.0002 |
| **Total** | **575.3100 ± 0.0015** (RSS of the listed uncertainties) |

Other Park 2017 results: $(\beta - 1) = (-2.7 \pm 3.9)\times10^{-5}$ and $J_{2\odot} = (2.25 \pm 0.09)\times10^{-7}$ (with Cassini γ).

**Implications.**
- The headline number is **inertial (ICRF-fixed)**. It does **not** include general precession, which is ~5029″/cy in longitude (IAU 2006 $p_A$ = 5028.796195″/cy; UNVERIFIED in this review).
- Classical "≈5600″/cy" figures are in the equinox-of-date frame.
- The **interaction terms** (non-additivity at the 10⁻² ″/cy level) mean a per-body decomposition depends on how it is defined. Park's exact switch-on/switch-off protocol is UNVERIFIED.
- **Ambiguity is real.** Purely Newtonian "planetary" values in the literature range 526.7–532.4″/cy, depending on the perihelion definition and fitting span (Pogossian 2022, CeMDA, doi:10.1007/s10569-022-10085-5, arXiv:2112.07301 **[V]**). FerroAstro must document its definition.

#### 3.2 Methods compared

| Method | What it measures | Pros | Cons |
|---|---|---|---|
| Osculating heliocentric elements (ϖ = Ω + ω) | instantaneous Kepler fit | cheap, every step | Large short-period terms (Venus ~ several ″). Under 1PN, osculating elements computed with the Newtonian formula from harmonic-coordinate states carry $O(GM/c^2p) \sim 2.7\times10^{-8}$ rad (~5 mas) periodic oscillations *(derived here)*. ϖ is a "dog-leg" angle |
| Laplace–Runge–Lenz vector $\mathbf A = \mathbf v\times\mathbf h - \mu\hat{\mathbf r}$ | direction of the instantaneous ellipse's pericentre | smooth, vector-valued, no Ω/ω split. Pogossian finds it only weakly step-size dependent **[V]** | Not conserved at 1PN even for pure Schwarzschild (periodic part). A 1PN-corrected LRL vector would remove that (UNVERIFIED form) |
| **Perihelion-passage timing** (Park) | geometric minima of $r_{\rm helio}(t)$ | matches the published observable | Needs dense output plus root-finding of $\mathbf r\cdot\mathbf v = 0$. One sample per orbit (415/cy) |
| Mean-ellipse fit per orbit (Pogossian) | best-fit ellipse each period | robust to noise | Definition-dependent. Values differ from the others at the ~0.1–1″/cy level **[V, spread quoted]** |
| **Differential runs** (rung on − rung off, same ICs) | isolated contribution | Cancels most periodic terms. Directly comparable to analytic formulas | Requires a convention for ICs. GR changes mean motion, which feeds back into planetary perturbations at second order (cf. Park's interaction terms) |

**Fitting recommendation** *(derived here)*:
- Use linear least squares on the Park basis, with the periodic frequencies $\nu_i$ taken from known combinations of planetary mean motions (Venus, Earth, Jupiter, Venus–Earth synodic, and so on) or found from an FFT of the residuals.
- Include $Q t^2$.
- Report the slope uncertainty both formally and from jack-knife over sub-spans.
- An unmodelled sinusoid of amplitude $A$ and period $P$ biases the fitted slope by roughly $\sim A\,P/T^2$ (order-of-magnitude, UNVERIFIED constant). That is why a long span (≥1–2 kyr) and the quadratic term matter.

**Coordinate and gauge points:**
- DE uses the isotropic PPN metric, which is identical to harmonic at 1PN for the EOM. The *standard Schwarzschild* form gives different EOM (Saha & Tremaine 1994, footnote **[V]**). FerroAstro EOMs must be in PPN-isotropic/harmonic coordinates.
- The secular precession per orbit is gauge-invariant at 1PN, but $a$, $n$ and osculating elements are not.
- Use **heliocentric** elements and perihelia, not barycentric. The Sun moves ~1 R☉ around the SSB **[V, DE440 Fig. 2 discussion]**.
- Time argument is TDB. The TCB/TDB rate difference is $L_B \approx 1.55\times10^{-8}$, which is negligible for ″/cy.

#### 3.3 Achievable precision (estimates, derived here, UNVERIFIED until NUM tests run)

- **Truncation.** IAS15 is below f64 eps per step. ABM13 at 1/16 d has truncation well below round-off, based on Subbotin's sub-mm double-double results.
- **Round-off.** It affects phase as $t^{3/2}$, but the **perihelion direction** depends on the eccentricity vector, which random-walks as $\sim \varepsilon\sqrt{n}/e$. For $n \sim 10^7$ steps that is ~10⁻¹² rad, negligible versus 10⁻⁴″/cy ≈ 5×10⁻¹⁰ rad/cy.
- **Fit/definition systematics dominate.** Expected reproducibility:
  - Differential GR rate (rung on − off) over 2 kyr: about **10⁻⁴–10⁻⁵ ″/cy**.
  - Total $\dot\varpi$ reproduced with the Park protocol: about **10⁻³ ″/cy**, comparable to Park's ±0.0015.
  - Agreement with Park's 575.3100 needs the same model (DE-class bodies, J₂, LT) and the same frame.

---

### 4. Equations of motion per rung, with analytic tests

Notation: $\mu_j = GM_j$, $\mathbf r_{ij} = \mathbf r_j - \mathbf r_i$, $r_{ij} = |\mathbf r_{ij}|$, $p = a(1-e^2)$, $n = 2\pi/P$. Mercury reference values used in the numbers below: $a = 0.38709893$ au, $e = 0.20563069$, $P = 87.9691$ d (415.20 orbits/cy), $\mu_\odot = 1.32712440041\times10^{20}$ m³ s⁻². All ″/cy numbers in this section were computed here *(derived here)*.

**R0. Newtonian N-body**

$$\ddot{\mathbf r}_i=\sum_{j\neq i}\mu_j\,\frac{\mathbf r_j-\mathbf r_i}{r_{ij}^3}$$

Test: two-body conservation of energy, angular momentum and LRL vector to Brouwer's law. Planetary secular rates are checked against Park Table 3.

**R1. Solar J₂.** Here $z = \mathbf r\cdot\hat{\mathbf k}_\odot$, with $\mathbf r$ heliocentric. The reaction on the Sun is $-\tfrac{m}{M_\odot}\mathbf a$ (DE440 Eq. 29 **[V]**).

$$\mathbf a_{J_2}=-\frac{3\,\mu_\odot J_{2\odot}R_\odot^2}{2\,r^5}\left[\left(1-\frac{5z^2}{r^2}\right)\mathbf r+2z\,\hat{\mathbf k}_\odot\right]$$

Analytic test, for an orbit in the solar equatorial plane:

$$\Delta\varpi_{J_2}=\frac{3\pi J_2R_\odot^2}{p^2}\ \text{per orbit}$$

This gives 0.0280″/cy for $J_2 = 2.2\times10^{-7}$, $R_\odot = 6.96\times10^5$ km, and 0.0286″/cy for $J_2 = 2.25\times10^{-7}$, matching Park. General inclination: $\dot\omega = \tfrac{3nJ_2R^2}{4p^2}(5\cos^2 i - 1)$ and $\dot\Omega = -\tfrac{3nJ_2R^2}{2p^2}\cos i$ (standard; UNVERIFIED citation). Reference values: DE430 $J_{2\odot} = 2.1106088532726840\times10^{-7}$, estimated without LT. *(Corrected 2026-10-14: the original text said "~7% too high". A J₂ fitted without the negative LT term is biased **low**, consistent with INPOP 2.01→2.218 and with DE430 < DE440. The "7%" in Park 2021 refers to LT being about 7% of the oblateness precession. To be confirmed against the Park 2021 text, OI-50.)*

**R2. Special-relativistic Kepler (Sommerfeld-type).** Newtonian force acting on relativistic momentum, $\frac{d}{dt}(\gamma\mathbf v) = -\mu\mathbf r/r^3$, which is equivalent to

$$\mathbf a=-\frac{\mu}{\gamma r^3}\left[\mathbf r-\frac{(\mathbf r\cdot\mathbf v)\,\mathbf v}{c^2}\right],\qquad \gamma=(1-v^2/c^2)^{-1/2}$$

Here $\mathbf v$ is the coordinate velocity. Exact per-orbit advance, with $h = \gamma|\mathbf r\times\mathbf v|$:

$$\Delta\varpi_{SR}=2\pi\left[\left(1-\frac{\mu^2}{c^2h^2}\right)^{-1/2}-1\right]\approx\frac{\pi\mu^2}{c^2h^2}=\frac16\cdot\frac{6\pi\mu}{c^2p}$$

- Mercury: **7.163″/cy**, prograde. The exact-to-1PN ratio is 0.1666667 *(derived here)*.
- Caveat: the SR answer is **model-dependent**. Adding a "relativistic potential energy" gives 1/3 of GR instead of 1/6 (Lemmon & Mondragon, arXiv:1012.5438, versions differ **[V]**).
- FerroAstro must name the Hamiltonian, $H = \gamma mc^2 - \mu m/r$, and test against the exact formula above. That formula is the classical Sommerfeld fine-structure analogue; the exact attribution to Sommerfeld 1916 is UNVERIFIED.

**R3. Schwarzschild test particle, 1PN, PPN harmonic/isotropic gauge.** This has the same form as the IERS Conventions 2010 Ch. 10 satellite term (equation number UNVERIFIED; check against IERS TN36).

$$\mathbf a=-\frac{\mu}{r^3}\mathbf r+\frac{\mu}{c^2r^3}\left[\left(2(\beta+\gamma)\frac{\mu}{r}-\gamma v^2\right)\mathbf r+2(1+\gamma)(\mathbf r\cdot\mathbf v)\,\mathbf v\right]$$

Analytic test:

$$\Delta\varpi_{GR}=\frac{2+2\gamma-\beta}{3}\cdot\frac{6\pi\mu}{c^2a(1-e^2)}\ \text{per orbit}$$

- Mercury ($\beta = \gamma = 1$): 5.0187×10⁻⁷ rad/orbit = **42.981″/cy**. Park's 42.9799 comes from full-model fitting. *(Corrected 2026-10-14, red-team review, Journal #9: 42.98048 rounds to **42.980**, not 42.981. The element set used here (a = 0.38709893 au, e = 0.20563069) changes the value by only 2×10⁻⁶ ″/cy, so it does not explain the difference; it was a rounding error.)*
- Optional exact geodesic cross-check, in Schwarzschild coordinates with proper-time parametrisation: $u'' + u = \mu/h^2 + 3\mu u^2/c^2$. The 2PN correction is ~$(\mu/c^2p)\times$GR ≈ 10⁻⁶″/cy, negligible.

**R3b. Lense–Thirring (solar spin $\mathbf S$).** IERS-style form, with $\mathbf J$ the angular momentum per unit mass (UNVERIFIED typography vs DE440 Eq., which uses $(1+\gamma)G/(2c^2r^3)[\ldots]$ with total $\mathbf J$ **[V, partial]**):

$$\mathbf a_{LT}=(1+\gamma)\frac{\mu}{c^2r^3}\left[\frac{3}{r^2}(\mathbf r\times\mathbf v)(\mathbf r\cdot\mathbf J)+\mathbf v\times\mathbf J\right]$$

Analytic test (orbit in the equatorial plane, γ = 1):

$$\dot\varpi_{LT}=-\frac{4GS}{c^2a^3(1-e^2)^{3/2}}$$

For $S = 1.9\times10^{41}$ kg m² s⁻¹ this gives **−0.0020″/cy**, matching Park.

**R4. 1PN Sun-only N-body.** R0 for all bodies, plus the R3 correction for each planet using the heliocentric $\mathbf r$, $\mathbf v$ and $\mu_\odot$. Test: the Mercury differential (R4 − R0) rate is ≈ 42.98″/cy, with planet-interaction residuals at the ≲10⁻² ″/cy level (cf. Park Table 3).

**R5. Full EIH 1PN N-body with PPN β, γ.** This is DE430 Eq. 27 (Folkner et al. 2014, IPN PR 42-196 **[V]**), repeated in DE440 §3.1 (Park et al. 2021, AJ 161, 105, doi:10.3847/1538-3881/abd414), and follows Moyer 2003 (Wiley/JPL DESCANSO; doi:10.1002/0471728470) and Will & Nordtvedt 1972.

$$
\begin{aligned}
\ddot{\mathbf r}_i=\sum_{j\neq i}\frac{\mu_j(\mathbf r_j-\mathbf r_i)}{r_{ij}^3}\Bigg\{&1-\frac{2(\beta+\gamma)}{c^2}\sum_{k\neq i}\frac{\mu_k}{r_{ik}}-\frac{2\beta-1}{c^2}\sum_{k\neq j}\frac{\mu_k}{r_{jk}}+\gamma\frac{v_i^2}{c^2}+(1+\gamma)\frac{v_j^2}{c^2}-\frac{2(1+\gamma)}{c^2}\,\mathbf v_i\cdot\mathbf v_j\\
&-\frac{3}{2c^2}\left[\frac{(\mathbf r_i-\mathbf r_j)\cdot\mathbf v_j}{r_{ij}}\right]^2+\frac{1}{2c^2}(\mathbf r_j-\mathbf r_i)\cdot\ddot{\mathbf r}_j\Bigg\}\\
+\frac{1}{c^2}\sum_{j\neq i}\frac{\mu_j}{r_{ij}^3}&\Big\{(\mathbf r_i-\mathbf r_j)\cdot\big[(2+2\gamma)\mathbf v_i-(1+2\gamma)\mathbf v_j\big]\Big\}(\mathbf v_i-\mathbf v_j)
+\frac{3+4\gamma}{2c^2}\sum_{j\neq i}\frac{\mu_j\,\ddot{\mathbf r}_j}{r_{ij}}
\end{aligned}
$$

- $\ddot{\mathbf r}_j$ on the RHS is replaced by the **Newtonian** acceleration, which is accurate to $O(c^{-2})$ **[V]**.
- The transcription above was checked term-by-term against the extracted DE430 PDF text. A **second independent check against the typeset PDF is required** (NUM-020).
- Companion definitions:
  - SSB with relativistic masses: $\mu_i^* = \mu_i\left(1 + \frac{v_i^2}{2c^2} - \frac{1}{2c^2}\sum_{j\neq i}\frac{\mu_j}{r_{ij}}\right)$, $\mathbf r_{\rm SSB} = \sum\mu_i^*\mathbf r_i/\sum\mu_i^*$ (DE440 Eqs. 1–2 **[V]**).
  - DE440 adds 30 KBOs plus a 36-point-mass ring at 44 au, and LT for all bodies except the Sun **[V]**. *(Clarified 2026-10-14, red-team review, Journal #9: this means the LT acceleration from solar angular momentum, applied to every non-Sun body; it is not "LT of all bodies".)*
- **Test of R5 against R3.** In the limit of massless planets, R5 reduces to R3 for each planet in the Sun's field, in SSB coordinates with the Sun fixed. A unit test should check that limit symbolically and numerically.

---

### 5. Performance (small N ≈ 10–350)

**What dominates:**
- **Pair forces.** Newtonian cost is ~N²/2 pairs, each needing a sqrt and a divide (~20–30 flops).
  - EIH needs first the per-body potentials $\Phi_i = \sum_k \mu_k/r_{ik}$, which is O(N²) and reuses the $1/r$ values, and then the pair terms, so it stays **O(N²)**. Estimate: ~3–5× Newtonian flops per pair (UNVERIFIED).
- **Tiering is the single biggest lever** **[V, Subbotin; DE430 "iterative" asteroids]**. EPM-style: 16 "major" bodies with full EIH. The asteroids feel Newtonian forces from the majors and do not interact with each other, which costs <1 cm (planets) / <1 mm (Moon) over 40 yr **[V]**.
  - For 343 asteroids this is 16×343 + 120 ≈ 5.6×10³ interactions instead of ~6.4×10⁴ *(derived here)*.
  - Make tiers a first-class model setting: `Major` = EIH, `Minor` = Newtonian from majors only, `Ring` = fixed-mass points.
- **Integrator overhead.**
  - At N ≈ 10 the force is only ~45 pairs, so IAS15's polynomial bookkeeping (O(7N) per iteration) and the step control are comparable to the force cost.
  - ABM PECE needs about 2 evals per step against ~15 for IAS15. Even with a smaller step (1/16 d vs ~1 d for IAS15 at Mercury's 88 d period, UNVERIFIED typical), ABM is cost-competitive *(derived here)*.
- **Reference speed** **[V]**: 40 yr, EIH model, h = 1/16 d (233,760 steps) took 81 s in f64. That is ~0.35 ms/step for the EPM2021-subset model on 2023 hardware (CPU unspecified in what was read).

**Strategies:**
1. **Structure-of-arrays** layout (x[], y[], z[], vx[], …). Vectorise the inner $j$ loop with f64x4 / f64x8 (AVX2 / AVX-512 / NEON).
   - WHFast512 shows up to 4.7× from AVX-512 alone at N = 9 **[V]**.
   - In Rust: `std::simd` (nightly) or a stable crate such as `wide` / `pulp` (UNVERIFIED current API status).
2. **Exact 1/r³.** Use `sqrt` + `div`, or `rsqrt14` + 2 Newton steps (AVX-512) validated to ≤1 ulp. Never ship approximate rsqrt without refinement.
3. **Determinism.** Use a fixed summation order (no parallel reductions inside a step), explicit FMA policy (always `mul_add`), and bitwise-reproducible results across thread counts. This is needed for Brouwer's-law tests.
4. **Parallelism across runs, not within a step**, for N ≲ 300. Use for example β/γ sweeps, ensembles, and forward/backward integrations. Threads inside a step only pay off when there are thousands of pair interactions (UNVERIFIED crossover; benchmark it).
5. **Batch-lane mode.** Integrate 4–8 independent ICs in SIMD lanes, as heyoka's batch mode does (UNVERIFIED in this review). This is a good fit for parameter sweeps and variational equations.
6. **DD arithmetic.** TwoSum costs 6 flops and TwoProd via FMA costs 2 flops. Keep DD out of the force loop (Subbotin mixed scheme) so the hot loop stays pure f64 SIMD.

---

### Requirements implied

| ID | Requirement | Verification |
|---|---|---|
| NUM-001 | Provide an IAS15-class adaptive 15th-order Gauss–Radau integrator with Rein–Spiegel step control and compensated summation of the state update. It must accept velocity-dependent forces through the predictor–corrector iteration (max 12 iterations, convergence on $\delta b_6$). | Kepler/Kozai tests from Rein & Spiegel 2015. Energy error ∝ $t^{1/2}$ over ≥10⁹ steps |
| NUM-002 | Provide a fixed-step ABM PECE/PECEC integrator, order configurable 8–14 (default 13), with a high-order RK start-up (e.g. DP8 at h/8). Accumulators must be generic over `Real`. | Reproduce Subbotin et al. 2023 Table 1 trends (f64 vs mixed DD) on an equivalent model |
| NUM-003 | Provide a Gauss–Legendre IRK (s = 8, order 16) reference integrator with the Hairer–McLachlan–Razakarivony round-off-aware implementation. | Symmetry test (forward then backward returns ICs to round-off). Agreement with NUM-001 |
| NUM-004 | Provide WH/WHFast (Jacobi or democratic-heliocentric) with symplectic correctors for Newtonian + GR-potential rungs. Document that full EIH is unsupported in WH. | Newtonian solar-system 10⁶-yr energy boundedness |
| NUM-005 | Implement the Saha & Tremaine 1994 pseudo-velocity 1PN extension of WH (time rescale + $p^4$ drift + $1/r^2$ kick) as an optional rung. | Mercury precession = 42.98″/cy ± 10⁻³ *and* correct mean motion (vs R3 under NUM-001) |
| NUM-006 | All arithmetic generic over a `Real` trait with backends f64, double-double (FMA-based TwoProd) and optional f128. No x87 80-bit dependency. | Compile-time monomorphisation. f64 path benchmark unchanged |
| NUM-007 | "Ephemeris precision mode" = Subbotin mixed scheme: state + integrator sums + coefficients in DD, forces in f64. | Mercury 40-yr round-off ≤ 1 cm vs full-DD run |
| NUM-008 | Integrate the Moon geocentrically (Earth–Moon vector as state), and use the Taylor-expanded differential third-body acceleration (≥4 terms). | Lunar round-off ≤ 2 mm / 40 yr in NUM-007 mode |
| NUM-009 | Body tiering: Major (EIH, mutual), Minor (Newtonian from Majors only), Ring (fixed point masses). | DE440 asteroid set (343) cost < 2× the 16-body cost |
| NUM-010 | Dense output (continuous interpolant) at integrator order, plus event location for $\mathbf r\cdot\mathbf v = 0$ (heliocentric perihelion) to ≤10⁻⁶ s. | Kepler orbit: perihelion times to round-off |
| NUM-011 | Precession estimator "Park2017": mean-orbit frame (configurable pole; default RA 280.9876°, Dec 61.4481°, ICRF), projected perihelion angles, LSQ fit of $B + \dot\varpi t + Qt^2 + \sum$ sinusoids with user or FFT-derived frequencies. Report formal and jack-knife σ. | Reproduce 575.31 ± 0.01″/cy for a DE440-class model over 1000–3000 CE |
| NUM-012 | Alternative estimators: osculating ϖ, LRL-vector angle, per-orbit mean ellipse. Every estimator output labels its definition, frame and span. | Cross-estimator spread reported in docs |
| NUM-013 | Differential-run harness: identical ICs, rung toggled, outputs Δϖ̇ with per-body/interaction decomposition. | GR 42.98, J₂ 0.0286 (J₂ = 2.25e-7), LT −0.0020″/cy within 10⁻⁴ | *(Superseded 2026-10-15, Journal #10: 1e-4″/cy applies only to isolated single-body terms; N-body contributions use ±0.005″/cy and are blocked on OI-38. See NUM-013.)* |
| NUM-014 | Frame utilities: ICRF/ecliptic J2000/mean-orbit frames, and optional ecliptic-of-date with IAU 2006 precession. The default reported rate is **inertial (ICRF-fixed)**. | Ecliptic-of-date result − inertial ≈ $p_A$ |
| NUM-015 | Analytic test oracles: R1 J₂ (planar + inclined), R2 SR exact $2\pi[(1-\mu^2/c^2h^2)^{-1/2}-1]$, R3 $(2+2\gamma-\beta)/3 \cdot 6\pi\mu/c^2p$, R3b LT. | Single-body numerical vs analytic to ≤10⁻⁶ relative (per-orbit, averaged over 10³ orbits) |
| NUM-016 | SR rung documents its Hamiltonian ($H = \gamma mc^2 - \mu m/r$) and states that other SR couplings give different fractions (1/3). | Doc review |
| NUM-017 | All 1PN EOMs in PPN isotropic/harmonic gauge, TDB time argument, $GM$ in TDB-compatible units. No standard-Schwarzschild-coordinate EOM except the explicitly labelled exact-geodesic oracle. | R5 → R3 limit test |
| NUM-018 | EIH implementation: Newtonian $\ddot{\mathbf r}_j$ on the RHS (default), with an optional one-step fixed-point iteration to quantify the $O(c^{-4})$ difference. Relativistic SSB ($\mu^*$) for the initial-condition barycentring. | Difference ≤ expected $O(c^{-4})$. SSB drift ≤ μm-level in DD mode (Subbotin reports 2.6 μm / 40 yr) |
| NUM-019 | Solar J₂ with the proper reaction on the Sun (DE440 Eq. 29) and configurable pole. LT for all non-Sun bodies with γ-dependence. *(Clarified 2026-10-14: LT acceleration from solar angular momentum, applied to every non-Sun body.)* | Momentum conservation test. Park Table 3 values |
| NUM-020 | Independent second transcription and check of DE430 Eq. 27 / DE440 §3.1 against the typeset PDFs before release. Every "UNVERIFIED" item in this journal is tracked as an issue. | Review sign-off |
| NUM-021 | Force kernels in SoA layout with SIMD (f64x4/x8), exact 1/r³ (≤1 ulp), fixed reduction order, explicit FMA policy, bitwise-reproducible across thread counts. | Bitwise reproducibility CI test. Speedup ≥ 2.5× vs scalar at N = 16 (target, UNVERIFIED feasibility) |
| NUM-022 | Run-level parallelism (rayon) for ensembles and β/γ sweeps, plus optional SIMD batch-lane integration of independent ICs. | Linear scaling to core count for ≥ cores runs |
| NUM-023 | Round-off diagnostics: energy / angular momentum / SSB drift vs $t$, with a Brouwer-law slope fit (expect ½ and 3⁄2), and forward–backward reversibility error. | Slopes within ±0.1 of Brouwer values in NUM-001 f64 runs |
| NUM-024 | Benchmark suite: 40-yr EIH 16-major + 343-minor run in f64 and mixed-DD. Publish wall times vs Subbotin's 81 s / 246 s (hardware-normalised). Target: within 1× of those times on comparable hardware. | Criterion benchmarks in CI |
| NUM-025 (stretch) | Taylor-series integrator with an AD expression graph (heyoka/TaylorIntegration.jl class), supporting variational equations. | Agreement with NUM-001 to round-off. Apophis 2029 encounter test |

---

### References (primary; [V] = checked in this review)

- Park, R. S., Folkner, W. M., Williams, J. G., Boggs, D. H. 2021, *The JPL Planetary and Lunar Ephemerides DE440 and DE441*, AJ 161, 105. doi:10.3847/1538-3881/abd414. [V] https://ssd.jpl.nasa.gov/doc/Park.2021.AJ.DE440.pdf
- Folkner, W. M., Williams, J. G., Boggs, D. H., Park, R. S., Kuchynka, P. 2014, *The Planetary and Lunar Ephemerides DE430 and DE431*, IPN Progress Report 42-196. [V] https://ipnpr.jpl.nasa.gov/progress_report/42-196/196C.pdf
- Park, R. S., Folkner, W. M., Konopliv, A. S., Williams, J. G., Smith, D. E., Zuber, M. T. 2017, *Precession of Mercury's Perihelion from Ranging to the MESSENGER Spacecraft*, AJ 153, 121. doi:10.3847/1538-3881/aa5be2. [V]
- Subbotin, M., Kodukov, A., Pavlov, D. 2023, *Reducing roundoff errors in numerical integration of planetary ephemeris*, CeMDA. doi:10.1007/s10569-023-10139-2. arXiv:2304.04458. [V]
- Rein, H., Spiegel, D. S. 2015, *IAS15: a fast, adaptive, high-order integrator…*, MNRAS 446, 1424. doi:10.1093/mnras/stu2164. arXiv:1409.4779. [V]
- Rein, H., Tamayo, D. 2015, *WHFast…*, MNRAS 452, 376 (volume/page UNVERIFIED). arXiv:1506.01084.
- Javaheri, P., Rein, H., Tamayo, D. 2023, *WHFast512…*, Open J. Astrophys. 6. arXiv:2307.05683. [V, abstract]
- Tamayo, D., Rein, H., Shi, P., Hernandez, D. M. 2020, *REBOUNDx…*, MNRAS 491, 2885. doi:10.1093/mnras/stz2870. arXiv:1908.05634. [V, abstract]
- Saha, P., Tremaine, S. 1994, *Long-term planetary integration with individual time steps*, AJ 108, 1962. doi:10.1086/117210. arXiv:astro-ph/9403057. [V]
- Wisdom, J., Holman, M. 1991, AJ 102, 1528 (UNVERIFIED in this review).
- Everhart, E. 1985, *An efficient integrator that uses Gauss–Radau spacings*, IAU Colloq. 83, 185 (UNVERIFIED page).
- Brouwer, D. 1937, AJ 46, 149. [V, citation]
- Hairer, E., McLachlan, R. I., Razakarivony, A. 2008, *Achieving Brouwer's law with implicit Runge–Kutta methods*, BIT 48, 231. doi:10.1007/s10543-008-0170-3. [V]
- Antoñana, M., Alberdi, E., Makazaga, J., Murua, A. 2022, *An implicit symplectic solver for high-precision long term integrations of the Solar System*, CeMDA. doi:10.1007/s10569-022-10081-9. arXiv:2204.01539. [V, abstract]
- Grazier, K. R. et al. 2005, *Achieving Brouwer's law with high-order Störmer multistep methods*, ANZIAM J. 46 (CTAC2004). [V, citation via IAS15]
- Quinlan, G. D., Tremaine, S. 1990, AJ 100, 1694 (UNVERIFIED).
- Berry, M. M., Healy, L. M. 2004, *Implementation of Gauss–Jackson integration for orbit propagation*, J. Astronaut. Sci. 52, 331 (UNVERIFIED).
- Abad, A., Barrio, R., Blesa, F., Rodríguez, M. 2012, *Algorithm 924: TIDES*, ACM TOMS 39(1). doi:10.1145/2382585.2382590. [V]
- Biscani, F., Izzo, D. 2021, *Revisiting high-order Taylor methods for astrodynamics and celestial mechanics*, MNRAS 504, 2614. doi:10.1093/mnras/stab1032. [V]
- Pérez-Hernández, J. A., Benet, L. 2022, *Non-zero Yarkovsky acceleration for near-Earth asteroid (99942) Apophis*, Commun. Earth Environ. 3, 10. doi:10.1038/s43247-021-00337-x. [V, citation]. PlanetaryEphemeris.jl: https://github.com/PerezHz/PlanetaryEphemeris.jl
- Fienga, A., Manche, H., Laskar, J., Gastineau, M. 2008, *INPOP06*, A&A 477, 315. doi:10.1051/0004-6361:20066607. [V, citation]
- Laskar, J., Fienga, A., Gastineau, M., Manche, H. 2011, *La2010*, A&A 532, A89 (volume UNVERIFIED). doi:10.1051/0004-6361/201116836.
- Laskar, J. et al. 2004, *La2004*, A&A 428, 261. doi:10.1051/0004-6361:20041335. [V, citation]
- Pogossian, S. P. 2022, *Comparative study of Mercury's perihelion advance*, CeMDA. doi:10.1007/s10569-022-10085-5. arXiv:2112.07301. [V]
- Lemmon, T. J., Mondragon, A. R., *Kepler's orbits and special relativity*, arXiv:1012.5438. [V, versions differ]
- Moyer, T. D. 2003, *Formulation for Observed and Computed Values of Deep Space Network Data Types for Navigation*, Wiley. doi:10.1002/0471728470 (DOI UNVERIFIED).
- Petit, G., Luzum, B. (eds.) 2010, *IERS Conventions (2010)*, IERS TN 36, Ch. 10 (equation numbers UNVERIFIED).
- Krogh, F. T. 1997, *An Adams guy does the Runge–Kutta*, JPL Computing Memo 554. https://trs.jpl.nasa.gov/bitstream/handle/2014/22564/97-1071.pdf [V, citation via Subbotin]
