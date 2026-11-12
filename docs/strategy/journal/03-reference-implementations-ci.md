## 📓 Journal #3 — Reference implementations & CI oracles

> **Lead review notes:**
> - Spot-checked `GMS` and `J2SUN` against the known DE440 header values; both match.
> - **Runner decision (2026-10-14):** Stage D and the nightly long runs move from self-hosted to **GitHub paid larger runners**. GitHub-hosted jobs, larger runners included, are capped at 6 h per job, so DE440 reproduction and multi-millennia runs must checkpoint and chain across jobs. *(Superseded 2026-10-15, Journal #10: the local machine for now. See ADR-0013.)*
> - **Big takeaway:** no external code exposes PPN β/γ, so the β/γ ≠ 1 rungs are validated *analytically* (REF-010). That is also a place where FerroAstro fills a real gap.

**Date:** 2026-10-14 · **Branch context:** `1-scientific-computing-strategy`
**Goal:** Pick the reference codes ("oracles") that CI uses to check FerroAstro (Rust, Apache-2.0) for **speed** and **accuracy** on the flagship problem: Mercury's perihelion precession. The problem is built up in stages: Newtonian N-body (Sun + planets + Moon + asteroids + solar J2), then SR/GR (1PN EIH, PPN β/γ), and finally reproducing **JPL DE440** from its published initial conditions.

> **How to read this entry.** Facts were checked on 2026-10-14 against PyPI JSON, Maven Central metadata, the GitHub API, upstream source files, the DE440 ASCII header (`header.440`, downloaded and parsed) and Park et al. (2021), which was downloaded and searched as text. Anything not checked against a primary source is marked **UNVERIFIED**.

---

### 1. Summary comparison

| Reference | Version (checked 2026-10-14) | License | How we use it | Newtonian N-body | GR / PPN | Solar J2 | Integrators | Headless Docker install |
|---|---|---|---|---|---|---|---|---|
| **Orekit** (Java) | `13.1.9` (Maven Central `<release>`, updated 2026-10-03) | Apache-2.0 | Single-body propagation; perturbers from an ephemeris | Partial: one body propagated, the others read from DE | `Relativity` (Schwarzschild, central body), `LenseThirringRelativity`, `DeSitterRelativity`; **no β/γ parameters** | `J2OnlyPerturbation` (exists in `develop`; first release **UNVERIFIED**) | All Hipparchus ODE integrators (DP853, Gragg-Bulirsch-Stoer, Adams…) | `eclipse-temurin` JDK + Maven; or `pip install orekit-jpype==13.1.9.0` |
| **Hipparchus** | `4.0.3` (Maven Central) | Apache-2.0 | Integrator library behind Orekit | — | — | — | — | Maven dependency |
| **orekit-jpype** (Python) | `13.1.9.0` (PyPI) | Apache-2.0 (PyPI classifier) | Python scripting of Orekit | as Orekit | as Orekit | as Orekit | as Orekit | `pip install` (needs a JDK) |
| **REBOUND** (C/Python) | `5.2.2` (PyPI, uploaded 2026-10-05) | **GPL-3.0** | Self-consistent N-body | Full | via REBOUNDx | via REBOUNDx | IAS15, WHFast, WHFast512, SABA, MERCURIUS, TRACE, LEAPFROG, SEI, EOS, WH kernels | `pip install rebound==5.2.2` |
| **REBOUNDx** | `5.1.0` (PyPI, 2026-08-20) | **GPL-3.0-only** | Extra forces for REBOUND | — | `gr_potential`, `gr`, `gr_full` (1PN EIH, Newhall et al. 1983); only parameter is `c`, **no β/γ** | `gravitational_harmonics` (J2, J4) | — | `pip install reboundx==5.1.0` |
| **ASSIST** (on REBOUND) | `1.2.3` (PyPI) | **GPL-3.0+** | Test particle in a DE441 force field | Perturbers from DE441 + 16 asteroids | GR included | Sun J2; Earth J2–J4 | IAS15 | `pip install assist==1.2.3` |
| **Swiftest** (Modern Fortran + Python) | `2026.7.1` (PyPI, 2026-07-17) | **GPL-3.0-or-later** (LICENSE + COPYING) | Symplectic N-body | Full | `general_relativity` flag (default **True**); Quinn et al. 1991 / Saha & Tremaine 1994 single-central-body form; no β/γ | Central-body harmonics via SHTOOLS (`c_lm`) | WHM, RMVS, HELIO, SyMBA | `pip install swiftest==2026.7.1` |
| **Mercury6** (Chambers, F77) | 6.x; manual "last modified 1 March 2001"; mirror `smirik/mercury`, last commit 2023-02-18 | Mirror is labeled GPL-3.0; **original upstream license UNVERIFIED** | Legacy cross-check | Full (central-body codes) | Manual: "This version of MERCURY does not include relativity"; must be added through `mfo_user` | Central-body J2, J4, J6 | MVS, BS (general and conservative), RA15 (RADAU), hybrid | `gfortran` + `make build` |
| **SWIFT / Swifter** | Swifter page at boulder.swri.edu returned **404** on 2026-10-14 | **UNVERIFIED** | Historical only | Full | **UNVERIFIED** | **UNVERIFIED** | WHM, RMVS, HELIO, SyMBA (now continued in Swiftest) | Not recommended |
| **JPL DE440 / DE441** | DE440 (1550–2650), DE441 (−13,200 to +17,191); files dated 2020-12-22 | JPL data; redistribution terms **UNVERIFIED**, so we fetch by URL + hash and do not vendor the files | **Ground truth** | 343 asteroids + 30 KBOs + 36-point KBO ring | PPN EIH with β = γ = 1 (header), plus solar Lense–Thirring | `J2SUN = 2.1961391516529825e-7` | Not published as a reusable code | `curl` + `sha256sum` |
| **jplephem** (Python) | `2.24` (PyPI, 2026-01-23) | MIT | Reads `.bsp` files to make golden states | — | — | — | Chebyshev evaluation | `pip install jplephem==2.24` |
| **ERFA** (C) / **pyerfa** | ERFA 2.0.1 (matches SOFA 20231011); pyerfa `2.0.1.5` | BSD-3-Clause | Time scales and frames oracle | — | — | — | — | `pip install pyerfa==2.0.1.5` or distro `liberfa` |
| **IAU SOFA** (F77/C) | Release 2023-10-11 | SOFA license (SPDX `SOFA`): free use, derived works must be marked and renamed | Time scales and frames oracle (Fortran) | — | — | — | — | Download tarball, pin sha256, `gfortran` |
| **NOVAS F3.1** (Fortran) | F3.1 (31 Mar 2011); distributed via ASCL | **UNVERIFIED** (USNO says "free"; no license text found) | Optional astrometry cross-check | — | — | — | — | `gfortran`; note USNO's `WOBBLE` sign-error fix |

---

### 2. Per-reference notes

#### 2.1 Orekit 13.1.x (Java) and Hipparchus 4.0.x

- **Version and license.** Maven Central reports `org.orekit:orekit` at `13.1.9` (lastUpdated 2026-10-03) and `org.hipparchus:hipparchus-core` at `4.0.3`. Both are Apache-2.0; the Orekit `LICENSE.txt` and file headers were checked on the CS-SI GitHub mirror. Because Orekit has the same license as FerroAstro, it is the only reference here that could even be linked in-process. We still run it in its own container for symmetry.
- **Relevant force models**, checked in `src/main/java/org/orekit/forces/gravity/` on `develop`: `NewtonianAttraction`, `ThirdBodyAttraction`, `ThirdBodyAttractionEpoch`, `SingleBodyAbsoluteAttraction`, `SingleBodyRelativeAttraction`, `HolmesFeatherstoneAttractionModel`, `J2OnlyPerturbation`, `Relativity`, `LenseThirringRelativity`, `DeSitterRelativity`, plus tides.
  - `Relativity(gm)` implements Montenbruck & Gill eq. 3.146: `a = GM/(r³c²)·[(4GM/r − v²)·r + 4(r·v)·v]`. This is the **single-central-body Schwarzschild** term with β = γ = 1 built in. It takes no β/γ inputs and has no planet–planet 1PN cross terms.
  - `LenseThirringRelativity` and `DeSitterRelativity` (both `@since 10.3`) follow IERS Conventions 2010 ch. 10 and are written for Earth satellites. Re-using them for the solar Lense–Thirring term is possible in principle but **UNVERIFIED**.
- **Full heliocentric N-body.** `NumericalPropagator` integrates **one** `SpacecraftState`. Other bodies enter as `ThirdBodyAttraction` or `SingleBodyAbsoluteAttraction`, with positions taken from an ephemeris. Orekit therefore does **not** integrate the planets self-consistently. We did not find a built-in coupled multi-body propagator; that absence is **UNVERIFIED**.
  - The `SingleBodyAbsoluteAttraction` Javadoc says it is meant for interplanetary trajectories and that several instances can be combined. An `InertialForces` term is needed if the frame is not inertial.
  - Orekit is therefore a good oracle for **"Mercury in a DE-driven field"** (Stage B below), not for the coupled N-body stages.
- **Ephemeris input.** `JPLEphemeridesLoader` reads JPL **binary** DE files matching `^[lu]nx([mp](\d{4,5}))+\.(?:4\d\d)$` or `^linux_([mp](\d{4,5}))+\.(?:4\d\d)$`, plus `inpop*.dat`. It reads **not** `.bsp` SPK files. Use the JPL `Linux/de440/linux_p1550p2650.440` file; the exact path is **UNVERIFIED**.
- **Python wrapper.** `orekit-jpype` `13.1.9.0` on PyPI (Apache-2.0, Python ≥ 3.8). It is JPype-based and cannot subclass Java classes. The legacy JCC wrapper (conda) can, but its current conda-forge status is **UNVERIFIED**.
- **Docker.** `eclipse-temurin:21-jdk` + Maven, pinning `orekit` 13.1.9 and `hipparchus-*` 4.0.3. Pin `orekit-data` to a specific git commit.
- **Mercury script sketch (Java or Python, Stage B):**
  - Frame: Sun-centred ICRF (`CelestialBodyFactory.getSun().getInertiallyOrientedFrame()`).
  - Mercury initial state from DE440.
  - Forces: `NewtonianAttraction(GM_sun)`, then `SingleBodyAbsoluteAttraction` per planet + Moon (with `InertialForces`) or `ThirdBodyAttraction`, then `J2OnlyPerturbation(GM_sun, R_sun = 696000 km, J2SUN, sunFrame)`, then `Relativity(GM_sun)`.
  - Integrator: `DormandPrince853Integrator` or `GraggBulirschStoerIntegrator` with tight tolerances.
  - Output: a `StepHandler` that dumps the state and LRL vector every N days.

#### 2.2 Fortran N-body integrators

- **Mercury6 (Chambers 1999, MNRAS 304, 793).**
  - Fortran 77. Algorithms: MVS, general BS, conservative BS, Everhart RA15 (RADAU), hybrid symplectic/BS. Supports central-body J2, J4 and J6.
  - The manual says: *"This version of MERCURY does not include relativity"*. GR must therefore be added by hand in `mfo_user`, which is our own code and makes it a weaker oracle.
  - Mirror `github.com/smirik/mercury` is labeled GPL-3.0; last commit 2023-02-18. The original Chambers license is **UNVERIFIED**.
  - Maintenance: effectively frozen.
  - Use: a Newtonian legacy cross-check with RA15/BS, for Stage A only.
- **SWIFT / Swifter (Levison & Duncan; Kaufmann).** The Swifter home page returned 404 on 2026-10-14. License and GR support are **UNVERIFIED**. Its successor is Swiftest; do not adopt Swifter.
- **Swiftest** (Wishard et al. 2023, JOSS 10.21105/joss.05409).
  - Modern Fortran core with a Python front end. `pip install swiftest` (wheels). License GPL-3.0-or-later. Actively maintained (`MintonGroup/swiftest` pushed 2026-10-09).
  - Integrators: WHM, RMVS, HELIO, SyMBA.
  - GR: `general_relativity: bool, default True`. Source `src/swiftest/swiftest_gr.f90` cites Quinn, Tremaine & Duncan 1991 eq. 5 and Saha & Tremaine 1994 eq. 28. This is a central-body-dominant 1PN term, not full EIH.
  - Central-body gravity uses SHTOOLS; whether a plain J2 input such as `j2rp2` is still supported is **UNVERIFIED**.
  - Initial conditions can be pulled from Horizons via `astroquery` (`add_solar_system_body`).
  - Use: a symplectic, Fortran-ecosystem Newtonian+GR oracle for Stage A and Stage C-lite.
- **Other Fortran (Bulirsch–Stoer, Gauss–Jackson, Everhart).** Mercury6's BS and RA15 cover the "classical high-order Fortran" niche. We found no maintained, openly licensed Fortran Gauss–Jackson planetary integrator; this is **UNVERIFIED** and treated as a gap rather than a requirement.
- **ERFA / SOFA / NOVAS (time and frames, not dynamics).**
  - SOFA: latest release 2023-10-11 (F77 and C), SOFA license. ERFA: BSD-3, version 2.0.1, matching SOFA 20231011. pyerfa `2.0.1.5`.
  - NOVAS F3.1 (2011) is distributed via ASCL. USNO lists a `WOBBLE` sign bug in the Fortran edition (about 1e-21 rad). Its license is **UNVERIFIED**.
  - Use: golden values for TT↔TDB, ICRS↔ecliptic and J2000 rotations at the FerroAstro boundary.

#### 2.3 REBOUND + REBOUNDx (+ ASSIST)

- **Versions.**
  - REBOUND `5.2.2`: PyPI, uploaded 2026-10-05; GitHub license GPL-3.0, "version 3 or any later".
  - REBOUNDx `5.1.0`: PyPI, 2026-08-20; GPL-3.0-only.
  - ASSIST `1.2.3`: GPL-3.0+.
- **Integrators.**
  - IAS15 (Rein & Spiegel 2015): 15th-order Gauss–Radau, adaptive. Its abstract claims systematic errors are "well below machine precision" and that it follows Brouwer's law over 10⁹ orbits.
  - WHFast (Rein & Tamayo 2015): its abstract says it is "significantly faster and conserves energy better than all other Wisdom-Holman integrators tested".
  - WHFast512 (Javaheri, Rein & Tamayo 2023): uses AVX512.
  - Also SABA, MERCURIUS and TRACE. TRACE is the reversible hybrid for arbitrary close encounters (Lu, Hernandez & Rein 2024; full citation **UNVERIFIED**).
- **REBOUNDx GR effects**, from the "Effects" docs:
  - `gr_potential`: Nobili & Roxburgh 1986. A single dominant mass; gets the precession right but the mean motion wrong by O(GM/ac²). It is velocity-independent, so WHFast stays symplectic.
  - `gr`: Anderson et al. 1975. A single dominant mass; gets both precession and mean motion right.
  - `gr_full`: Newhall, Standish & Williams 1983. **Full 1PN EIH from all bodies**, the same family of equations used in the DE ephemerides. Its only parameter is `c`; **no PPN β/γ knobs**, so β = γ = 1 is fixed.
  - `gravitational_harmonics`: J2/J4 with `R_eq` and an `Omega` spin axis; spins held constant.
- **ASSIST.** IAS15 integration of test particles in the DE441 field (Sun, Moon, planets + 16 asteroids from `sb441-n16.bsp`). Includes GR, Sun J2 and Earth J2–J4. Its docs claim the equations of motion match JPL's small-body integrator to machine precision term by term. Its abstract gives no orbit-level DE441 error numbers.
- **Mercury script sketch (Python):**
  - `sim = rebound.Simulation()`, units AU/day/Msun-GM; set `sim.G` so that `G·M_sun = GMS` from the DE440 header.
  - Add particles from the DE440 initial conditions.
  - `rx = reboundx.Extras(sim)`; `gr = rx.load_force("gr_full"); gr.params["c"] = 173.144632674…` (AU/day); `rx.add_force(gr)`.
  - `gh = rx.load_force("gravitational_harmonics")`; set the Sun's `J2`/`R_eq`.
  - `sim.integrator = "ias15"`; integrate, logging the Mercury–Sun LRL vector at each perihelion.
  - Run once with `gr_full` and once without, from the same initial conditions, so the GR share separates out.

#### 2.4 JPL DE440 / DE441 (ground truth)

- **Downloads** (checked live 2026-10-14):
  - SPK: `https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/de440.bsp` (119,799,808 B), `de441.bsp` (3,307,878,400 B), `de440s.bsp` (32,726,016 B). Also mirrored under `https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/`.
  - ASCII: `https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/`, containing `header.440` (22.3 K), `ascp01550.440` … `ascp02550.440` (29.2 M each) and `testpo.440` (838 K, JPL's official test-point file, a ready-made golden set).
  - Asteroids: `https://ssd.jpl.nasa.gov/ftp/eph/small_bodies/asteroids_de441/` holds `sb441-n16.bsp` (615.8 M), `sb441-n373.bsp` (14.13 G), `sb441-n373s.bsp` (936.6 M) and `SB441_IOM392R-21-005_perturbers.pdf`. That n373 = 343 asteroids + 30 KBOs is inferred from the counts and is **UNVERIFIED**.
- **Initial conditions: what is actually published.**
  - **Park et al. (2021, AJ 161:105) does *not* tabulate initial state vectors.** It says: *"key mass parameters used in DE440 are shown in Table 2, and all other relevant parameters are given in the comment blocks of the DE440 and DE441 files."*
  - The **ASCII `header.440` (GROUP 1040/1041, 645 constants)** contains the initial conditions. Values parsed by us:
    - `JDEPOC = 2440400.5` (1969-06-28 TDB), `CENTER = 0`. Barycentric is our reading and is **UNVERIFIED**.
    - Planet states `X1…ZD1` (Mercury), `X2…`, `XB…` (EMB), `X4…X9`. Units: au and au/day (inferred, consistent with Mercury `X1 = 0.36176…`).
    - Moon `XM…`, Sun `XS…`, `XC…` (meaning **UNVERIFIED**). Lunar libration `PHI, THT, PSI, OMEGA*` and core `OMGC*, PHIC, THTC, PSIC`.
    - `CLIGHT = 299792.458`, `BETA = 1.0`, `GAMMA = 1.0`, `AU = 149597870.7` km, `EMRAT = 81.30056822149722`.
    - `GMS = 2.9591220828411956e-4` au³/d², `GMSDOT = -9e-14`. `GM1 = 4.912500194889318e-11`, `GM2`, `GMB`, `GM4…GM9`.
    - `ASUN = 696000` km, `J2SUN = 2.1961391516529825e-7`, `MOISUN = 0.06884`, `OMGSUN = 0.24756443290699712`.
    - `MA0001…`: **409** mass entries = 343 asteroids + `MA8001–MA8030` (30 KBOs) + `MA8201–MA8236` (36-point KBO ring).
  - **Asteroid initial states are not in the header (masses only).** They have to be sampled from `sb441-n373.bsp` at `JDEPOC`; how JPL actually supplied them is **UNVERIFIED**.
- **DE440 dynamical model** (Park 2021 §3):
  - PPN point-mass EIH equations with β and γ.
  - Sun J2 interacting with all bodies.
  - Earth zonal harmonics up to degree 5 and the Moon's 6×6 field, each interacting with the Sun, Mercury, Venus, Mars, Jupiter and Saturn as point masses.
  - Earth tides, lunar libration and core, time-delayed tides.
  - **Solar Lense–Thirring**: Park et al. state it shifts Mercury's perihelion by "about −0.0020″/Julian century, which is about 7% of the precession caused by the solar oblateness". That puts the oblateness term at about 0.03″/cy (our arithmetic).
  - DE441 omits lunar core/mantle damping. Planet orbits differ from DE440 by under 1 m over 1550–2650.
- **jplephem 2.24 (MIT):** `SPK.open('de440.bsp')[0,1].compute_and_differentiate(jd_tdb)` returns km and km/day. This is the canonical reader for golden-file generation.
- **Horizons API** (v1.3, June 2025): `https://ssd.jpl.nasa.gov/api/horizons.api?format=json&COMMAND='199'&EPHEM_TYPE='ELEMENTS'&CENTER='500@10'&REF_PLANE='ECLIPTIC'&REF_SYSTEM='ICRF'&TP_TYPE='ABSOLUTE'&START_TIME=…&STOP_TIME=…&STEP_SIZE='1%20d'`.
  - Check `signature.version` in the response, as the docs advise.
  - Rate limits are not documented, so cache the responses.
  - Which DE Horizons uses internally right now is **UNVERIFIED**; record it from the response header.

#### 2.5 Published speed / accuracy benchmarks

| Source | Result | Caveat |
|---|---|---|
| Javaheri, Rein & Tamayo 2023 (OJAp 6; arXiv 2307.05683), WHFast512 | Up to 4.7× faster than WHFast; Solar System for 5 Gyr in about 1.4 days | AVX512 CPU; at most 8 planets |
| Dagli & Rein 2026 draft (hanno-rein.de/DagliRein2026.pdf) | 8 planets **with GR** for 5 Gyr in under 8.7 h | **Draft, not peer-reviewed** |
| Rein & Spiegel 2015 (IAS15, arXiv 1409.4779) | "superior to MVS and other popular integrators… in both speed and accuracy" | Qualitative in the abstract |
| Rein & Tamayo 2015 (WHFast, arXiv 1506.01084) | Faster than all other WH integrators tested | Codes compared not named in the abstract |
| Brown & Rein 2020 (arXiv 2012.05177) | 96 Solar-System runs, WHFast + REBOUNDx GR, step ≈ 8 d, 121 GB public dataset | Usable as an external long-term golden set |
| Abbot et al. 2023 (ApJ; arXiv 2212.14844) | WH without correctors, Sun + 8 planets, simple GR, 3.16-d step reproduces Mercury-instability statistics | Statistics, not timings |

**We found no head-to-head wall-clock comparison of Orekit vs REBOUND vs Mercury6 vs Swiftest on the same Solar-System problem.** This is a gap FerroAstro's CI can fill, and that table would itself be a publishable artifact.

---

### 3. License notes (FerroAstro is Apache-2.0)

1. **GPL references (REBOUND, REBOUNDx, ASSIST, Swiftest, the Mercury6 mirror) are fine as separate CI containers, provided we never link them or vendor their source into FerroAstro.**
   - The FSF GPL FAQ: *"pipes, sockets and command-line arguments are communication mechanisms normally used between two separate programs. So when they are used for communication, the modules normally are separate programs."* The same answer warns that exchanging complex internal data structures could make them one program, so we exchange only plain files (CSV/NetCDF/JSON state vectors).
   - Also from the FAQ: *"The output of a program is not, in general, covered by the copyright on the code of the program."* Golden outputs produced by GPL tools can therefore be committed or attached to an Apache-2.0 repository.
   - **Forbidden:**
     - a `rebound`/`reboundx` crate dependency, FFI bindings, or `build.rs` compiling their C;
     - copying `gr_full.c`, `swiftest_gr.f90` and similar into `src/`;
     - publishing a combined Docker image as a FerroAstro release artifact.
   - The CI-only Dockerfiles for GPL tools may live in the repo under `ci/oracles/` as scripts that install GPL software at build time. They are themselves our own Apache-2.0 files.
   - *This is engineering guidance, not legal advice.*
2. **Apache-2.0 → GPL-3 direction.** Apache-2.0 code can be combined into GPLv3 works, but not the other way round. FerroAstro must never absorb GPL code.
3. **Orekit / Hipparchus / orekit-jpype (Apache-2.0)** are fully compatible. Keep their `NOTICE` files in the oracle image.
4. **SOFA license.** Compatible for use, but derived/modified routines must be renamed and documented. Prefer **ERFA (BSD-3)** for anything that might ever be ported into Rust; keep SOFA Fortran as an oracle only.
5. **JPL ephemeris files.** Redistribution terms are **UNVERIFIED**. Do not commit `.bsp` or ASCII files to git. Download them in CI from the JPL URL, check a pinned SHA-256, and cache them as a GitHub Actions cache or OCI artifact.
6. **NOVAS F3.1 and the original Mercury6:** license **UNVERIFIED**. Do not vendor either; download in the oracle image only, or drop them.

---

### 4. CI architecture proposal

```
ci/
  oracles/
    jpl/        Dockerfile  # curl + sha256: de440.bsp, header.440, testpo.440, sb441-n373s.bsp; python + jplephem==2.24
    orekit/     Dockerfile  # eclipse-temurin:21-jdk, orekit 13.1.9, hipparchus 4.0.3, orekit-data@<commit>
    rebound/    Dockerfile  # python:3.12-slim + gcc, rebound==5.2.2, reboundx==5.1.0, assist==1.2.3   (GPL, isolated)
    swiftest/   Dockerfile  # python:3.12, swiftest==2026.7.1                                          (GPL, isolated)
    fortran/    Dockerfile  # gfortran; mercury6 @ <commit sha>; SOFA 20231011 tarball @ sha256       (isolated)
    erfa/       Dockerfile  # pyerfa==2.0.1.5
  scenarios/    *.toml      # one canonical scenario spec, read by every oracle runner and by FerroAstro
  golden/       MANIFEST.json + small CSVs (large files → release assets / OCI artifacts)
  compare/      compare.py  # tolerance policy, report generation
```

**Principles**

- **One image per reference, fully pinned**: base image by digest, package `==` versions, git SHAs, tarball SHA-256. Images are built in a separate workflow, pushed to GHCR with tag `<tool>-<version>-<yyyymmdd>`, and referenced **by digest** from test workflows.
- **One canonical scenario format** (`scenarios/*.toml`) holding:
  - bodies, initial-condition source (`de440-header` | `de440-bsp@JD` | `horizons@JD`);
  - GM set, force-model switches (`newton`, `sun_j2`, `gr_1pn_central`, `gr_eih`, `ppn{beta,gamma}`, `lense_thirring`, `asteroids:N`);
  - integrator, tolerance/step, span and output cadence.

  Each oracle has a thin runner that turns the TOML into its native setup. Unsupported switches make the runner **refuse** instead of silently dropping the force.
- **Golden artifacts with provenance.** Every golden file has a `MANIFEST.json` entry with:
  - `scenario_hash`, `tool`, `tool_version`, `image_digest`, `git_sha` (oracle runner);
  - `input_files` (URL + SHA-256), `host_cpu`, `wallclock_s`, `created_utc`;
  - `output_sha256`, `units`, `frame`, `time_scale`.

  Goldens are regenerated **only** by an explicit `workflow_dispatch` (or a nightly drift check). A diff to a golden requires a PR with the manifest change.
- **Comparison metrics.**
  - Position/velocity error vs time, in RTN and in light-ms for Mercury.
  - Perihelion precession rate (″/cy) from a linear fit of the LRL-vector longitude over N orbits.
  - Δ-precession between runs (GR on − GR off) from **identical initial conditions**, to isolate 1PN.
  - Energy/angular-momentum drift.
  - Wall-clock and steps/s.
- **Speed benchmarking hygiene.** Run on a fixed runner class, report the median of k runs, and record CPU model and flags (AVX512 matters for WHFast512). Compare against oracle timings **from the same nightly run on the same runner** only.

**Job tiers**

| Tier | Trigger | Content | Budget |
|---|---|---|---|
| **PR-fast** | every PR | FerroAstro vs **cached goldens only** (no oracle containers). JPL `testpo.440` points; jplephem states at ~50 epochs; 2-body/Schwarzschild analytic precession (42.98″/cy class); 100-yr Sun + 8 planets Newtonian vs REBOUND-IAS15 golden; ERFA time/frame goldens | < 10 min |
| **PR-label `oracle`** | opt-in label | Rebuild/run the affected oracle(s) on the same scenario as the PR, plus diff vs golden | < 45 min |
| **Nightly** | cron | All oracles × all scenarios. Stages A–C, 1000-yr spans; speed table; drift check of goldens (a re-run oracle must reproduce its golden bit-for-bit or within its stated tolerance) | hours OK |
| **Weekly / release** | cron + tag | Stage D: full DE440 reproduction (343 asteroids + KBOs + ring, EIH, Sun J2 + LT) over 1550–2650 vs `de440.bsp`; publish comparison report as a release asset | long-running / self-hosted |  *(Superseded: the local machine, Journal #10; see CI-007.)*

**Scenario ladder (flagship)**

- **A. Newtonian.** Sun + 8 planets (+ Moon, + asteroids). Oracles: REBOUND IAS15, Swiftest, Mercury6 RA15/BS, Orekit (Mercury only, DE-driven).
- **B. Mercury in the DE440 field.** Only Mercury is integrated. Oracles: Orekit (`Relativity` + `J2OnlyPerturbation`), ASSIST-style REBOUND setup.
- **C. Coupled 1PN.** `gr_potential`, `gr`, `gr_full` (REBOUNDx) and Swiftest GR. The main 1PN oracle is REBOUNDx `gr_full` (EIH). PPN β ≠ 1 or γ ≠ 1 have **no external oracle**: they can be checked only against the analytic precession formula (2 + 2γ − β)/3 × 42.98″/cy, and that is a requirement on us.
- **D. DE440 reproduction.** Truth is `de440.bsp`. FerroAstro alone, since no open reference implements the full DE440 model.

---

### 5. Open questions / UNVERIFIED items to close

1. In `header.440`: exact meaning of `CENTER = 0` and of the `XC…` block, and whether all states are barycentric ICRF.
2. How JPL supplies asteroid initial conditions at `JDEPOC`; what `sb441-n373.bsp` contains; the IOM 392R-21-005 PDF should be read.
3. Redistribution terms for JPL DE files.
4. Original Mercury6 license; whether Swifter is still available; NOVAS license.
5. The Orekit release that introduced `J2OnlyPerturbation`; whether `LenseThirringRelativity` can be re-pointed at the Sun.
6. Whether REBOUND/REBOUNDx publish manylinux wheels, or need a compiler in the image.
7. Which DE Horizons uses now.
8. Full citation of the TRACE paper (Lu, Hernandez & Rein 2024).

---

### Requirements implied

**Reference / oracle requirements**

- **REF-001**: CI shall use JPL **DE440** (`de440.bsp` + ASCII `header.440` + `testpo.440`) as the accuracy ground truth. Each file is fetched by URL and pinned by SHA-256, never committed to git.
- **REF-002**: FerroAstro shall load initial conditions and constants (`JDEPOC`, planet/Moon/Sun states, `GM*`, `MA*`, `J2SUN`, `ASUN`, `CLIGHT`, `AU`, `EMRAT`, `BETA`, `GAMMA`) directly from `header.440` GROUP 1040/1041. They shall not be transcribed by hand.
- **REF-003**: Asteroid/KBO initial states shall be sampled from `sb441-n373*.bsp` at `JDEPOC`, with provenance recorded. This stays **provisional until UNVERIFIED item 2 is closed**.
- **REF-004**: `jplephem==2.24` (MIT) shall be the reference reader for generating golden states from `.bsp`. FerroAstro's own SPK reader shall match it to ≤ 1 mm / 1 µm s⁻¹ (target).
- **REF-005**: REBOUND `5.2.2` + REBOUNDx `5.1.0` (`gr_full`, `gr`, `gr_potential`, `gravitational_harmonics`) shall be the primary coupled N-body and 1PN-EIH oracle.
- **REF-006**: Orekit `13.1.9` / Hipparchus `4.0.3` shall be the Apache-2.0 oracle for single-body propagation in an ephemeris-driven field (`Relativity`, `J2OnlyPerturbation`, `ThirdBodyAttraction`/`SingleBodyAbsoluteAttraction`).
- **REF-007**: Swiftest `2026.7.1` shall be the symplectic Fortran-ecosystem oracle (WHM/HELIO with `general_relativity`).
- **REF-008**: Mercury6 (pinned commit) may serve as a legacy Newtonian oracle (RA15/BS) only. GR results from it shall not be used, because GR would come from our own `mfo_user`.
- **REF-009**: ERFA/pyerfa `2.0.1.5` (SOFA 20231011) shall be the oracle for time-scale and frame transformations.
- **REF-010**: PPN β/γ ≠ 1 and solar Lense–Thirring have no external N-body oracle. They shall be validated against analytic secular-precession formulas, and against Park et al. 2021's LT value of ≈ −0.0020″/cy for Mercury.
- **REF-011**: Every oracle runner shall **refuse** a scenario that requests a force it does not implement. It must never silently omit the force.

**CI requirements**

- **CI-001**: One Docker image per reference under `ci/oracles/<tool>/`. Base images pinned by digest, packages pinned by exact version, source by git SHA or tarball SHA-256. Images are referenced by digest.
- **CI-002**: A single tool-agnostic scenario spec (`ci/scenarios/*.toml`) shall drive FerroAstro and all oracle runners.
- **CI-003**: Golden outputs shall carry a `MANIFEST.json` provenance record: tool, version, image digest, runner SHA, input SHA-256s, CPU, wall-clock, output SHA-256, units/frame/time scale.
- **CI-004**: Goldens shall be (re)generated only by explicit dispatch or by the nightly drift job. Changing a golden requires a reviewed PR.
- **CI-005**: Per-PR CI shall compare only against cached goldens, without starting oracle containers, and finish in under 10 min. Oracle re-runs on PRs are opt-in via label.
- **CI-006**: Nightly CI shall run every oracle on every scenario, regenerate the speed table on a fixed runner class, and flag golden drift.
- **CI-007**: Weekly or release CI shall run the full DE440 reproduction (Stage D) over 1550–2650 and publish an error report: Mercury position residuals and precession residual in ″/cy.
- **CI-008**: Speed results shall be reported as the median of ≥ 5 runs with CPU model/flags. They shall be compared only within the same run and runner class.
- **CI-009**: Mercury precession shall be computed with the same estimator for every tool: LRL-vector longitude, fit over a fixed number of orbits. The GR contribution shall be isolated as a GR-on minus GR-off difference from identical initial conditions.

**License requirements**

- **LIC-001**: No GPL code (REBOUND, REBOUNDx, ASSIST, Swiftest, Mercury6) shall be linked, bound via FFI, compiled by `build.rs`, or vendored into FerroAstro. Interaction is through files and process boundaries only.
- **LIC-002**: GPL oracle images shall not be published as FerroAstro release artifacts.
- **LIC-003**: JPL ephemeris files, SOFA sources and NOVAS sources shall not be committed to the repository. CI fetches them with SHA-256 verification.
- **LIC-004**: Any routine ported into Rust from a time/frame library shall come from ERFA (BSD-3), not from SOFA. Attribution goes in `NOTICE`.

---

<details><summary>Sources</summary>

- Park, Folkner, Williams & Boggs 2021, AJ 161:105, doi:10.3847/1538-3881/abd414, https://ssd.jpl.nasa.gov/doc/Park.2021.AJ.DE440.pdf
- JPL DE440/441 doc page: https://ssd.jpl.nasa.gov/doc/de440_de441.html
- DE440 ASCII directory and header: https://ssd.jpl.nasa.gov/ftp/eph/planets/ascii/de440/
- DE440 SPK: https://ssd.jpl.nasa.gov/ftp/eph/planets/bsp/ ; asteroids: https://ssd.jpl.nasa.gov/ftp/eph/small_bodies/asteroids_de441/
- Horizons API doc: https://ssd-api.jpl.nasa.gov/doc/horizons.html
- jplephem: https://pypi.org/project/jplephem/
- Orekit releases/news: https://orekit.org/news/ ; Maven Central `org.orekit:orekit`, `org.hipparchus:hipparchus-core`; source mirror https://github.com/CS-SI/Orekit
- orekit-jpype: https://pypi.org/project/orekit-jpype/ ; https://forum.orekit.org/t/orekit-python-wrapper-13-0-1-released/4591
- Orekit forum on SingleBodyAbsoluteAttraction: https://forum.orekit.org/t/single-body-attraction-vs-newtonian/788
- REBOUND: https://github.com/hannorein/rebound ; https://rebound.hanno-rein.de/integrators/
- REBOUNDx effects: https://reboundx.readthedocs.io/en/latest/effects.html
- ASSIST: https://arxiv.org/abs/2303.16246 ; https://github.com/matthewholman/assist
- Swiftest: https://github.com/MintonGroup/swiftest ; https://joss.theoj.org/papers/10.21105/joss.05409 ; https://swiftest.readthedocs.io/
- Mercury6 mirror and manual: https://github.com/smirik/mercury
- SOFA: https://iausofa.org/ ; https://spdx.org/licenses/SOFA.html ; ERFA: https://github.com/liberfa/erfa
- NOVAS: https://aa.usno.navy.mil/software/novasf_intro ; https://aa.usno.navy.mil/software/novas_faq
- GPL FAQ: https://www.gnu.org/licenses/gpl-faq.html
- WHFast512: https://arxiv.org/abs/2307.05683 ; Dagli & Rein 2026 draft: https://hanno-rein.de/DagliRein2026.pdf
- IAS15: https://arxiv.org/abs/1409.4779 ; WHFast: https://arxiv.org/abs/1506.01084
- Brown & Rein 2020: https://arxiv.org/abs/2012.05177 ; Abbot et al. 2023: https://arxiv.org/abs/2212.14844
</details>
