## 📓 Journal #2 — Rust ecosystem & license audit

> **Lead review (spot-check):** re-queried crates.io for hifitime, nyx-space, anise, rmcp, sofars, qd, twofloat and astro-float; every license matches the table. One update: `anise` now has a `1.0.0-alpha` prerelease (2026-10-04) after 0.10.6. Track it, but keep the stable line until 1.0 ships.

**Date:** 2026-10-14 · **Scope:** crates FerroAstro could reuse instead of rebuilding the Fortran stack (linear algebra, time, ephemerides, extended precision, ODEs, SOFA/ERFA, MCP/CLI/WASM, tooling) · **Our license:** Apache-2.0

**Method:** versions, SPDX licenses, last-publish dates and download counts come from the crates.io API (queried 2026-10-14). Repo activity (last push, archived flag, stars) comes from the GitHub/Codeberg APIs. Feature claims come from each project's README or `Cargo.toml` on its default branch. Anything not checked against a primary source is marked **UNVERIFIED**. "Last release" means the date of the latest crates.io publish.

---

### Audit table

| Crate | Purpose | Version (last release) | License | Maturity / activity | Verdict |
|---|---|---|---|---|---|
| **Linear algebra** |||||
| `nalgebra` | Generic dense LA, small fixed-size vectors/matrices | 0.35.0 (2026-05) | Apache-2.0 | Very mature (~95M downloads), dimforge-maintained; `no_std`, works in WASM | **evaluate**: good for f64 frames/rotations. Generic scalars go through `simba::RealField`; whether double-double types implement it is **UNVERIFIED** |
| `faer` | High-performance dense LA (decompositions) | 0.24.4 (2026-06) | MIT | Active (Codeberg, pushed 2026-09); pure Rust. WASM build is **UNVERIFIED** | **evaluate**: only if we need large decompositions (fits, covariance). Not needed for N-body |
| `ndarray` | N-d arrays, numpy-style | 0.17.2 (2026-01) | MIT OR Apache-2.0 | Very mature (~132M) | **adopt (tooling only)**: Python/numpy interop and report data |
| `glam` | f32/f64 game-math vectors | 0.34.1 (2026-10) | MIT OR Apache-2.0 | Very mature (~161M) | **avoid for physics core** (no generic scalar). OK in WASM rendering code |
| `simba` | SIMD/scalar abstraction under nalgebra | 0.10.2 (2026-08) | Apache-2.0 | Mature | transitive only |
| `ndarray-linalg` / `nalgebra-lapack` | LAPACK bindings | 0.18.1 / 0.28.0 | MIT OR Apache-2.0 / MIT | Mature, but they need a system BLAS/LAPACK | **avoid**: breaks WASM and pure-Rust builds |
| **Time scales** |||||
| `hifitime` | Epochs; TAI/TT/ET/TDB/UTC/GPST/GST/BDT/QZSST/**TCG/TCB** (+ experimental lunar time); leap seconds | 4.3.1 (2026-08) | **MPL-2.0** | Mature (~1.3M downloads, 552★, pushed 2026-10). wasm32 target deps in `Cargo.toml`; Python feature | **adopt** (MPL caveats below). TCG and TCB confirmed in the `TimeScale` enum on master. The source notes that ET is SPICE's approximation, "slightly different from true TDB" |
| `sofars` | Pure-Rust port of IAU SOFA (time scales incl. TCG/TCB, precession/nutation, astrometry) | 0.6.1 (2026-04) | MIT **+ SOFA notice in LICENSE** | Small project (12★), active | **evaluate**: the crate is MIT, but its LICENSE file also carries the SOFA Board copyright header, so SOFA's own terms may attach (see risks) |
| `astrotime` | Time types | 0.3.0 (2026-08) | MIT OR Apache-2.0 | Small (~2k downloads) | avoid (hifitime covers it) |
| **Ephemerides / SPICE** |||||
| `anise` | Pure-Rust SPICE replacement: SPK/BSP, BPC, PCK, FK, TPC; README cites DE440 benchmarks; Python bindings | 0.10.6 (2026-10) | **MPL-2.0** | Active (281★, pushed 2026-10). Depends on `hifitime`, `nalgebra` and `sofars`. WASM support **UNVERIFIED** | **evaluate**: use as a cross-check and optional backend. It evaluates in f64, so the extended-precision path needs our own Chebyshev evaluator |
| `rust-spice` | Bindings to NAIF CSPICE | 1.0.1 (2026-09) | Apache-2.0 (wrapper) | Active, but links the C toolkit | **avoid in shipped artifacts**: C dependency, no WASM. The NAIF toolkit's own terms were **UNVERIFIED** |
| `cspice-sys` | Raw CSPICE bindings | 1.0.4 (2023-12) | **LGPL-3.0** | Stale | **avoid** |
| *(own)* DAF/SPK type-2 reader | Read JPL DE440/DE441 `.bsp` (Chebyshev, SPK type 2) | n/a | Apache-2.0 | ~hundreds of LOC (estimate) | **build-our-own**: generic over the scalar so Chebyshev evaluation runs in double-double, validated against ANISE |
| **Astrodynamics frameworks** |||||
| `nyx-space` | Full astrodynamics/OD framework | 2.6.0 (2026-09) | **AGPL-3.0-or-later** | Mature, active (499★) | **avoid**: incompatible with our licensing goals. Do not copy code from it |
| `satkit` | Satellite toolkit: frames, JPL ephemerides, SGP4, RK 9(8)…, RODAS4, Gauss-Jackson 8, gravity, PyO3 | 0.24.1 (2026-10) | MIT OR Apache-2.0 | Active (94★); README reports agreement with GMAT to a few cm over 7 days | **evaluate**: dev-dependency or validation oracle. Its Earth-satellite focus barely overlaps our heliocentric N-body work |
| `lox-space` (+ `lox-time`, `lox-ephem`, …) | Astrodynamics and mission analysis, Python bindings | 0.1.0-alpha.51 (2026-10) | MPL-2.0 | Alpha, very active (387★) | **evaluate (reference only)**: API still churning |
| `brahe` | Satellite dynamics | 1.7.0 (2026-07) | MIT | Active (103★) | reference only |
| `sgp4` | SGP4 propagator | 2.4.0 (2026-02) | MIT | Mature | out of scope (Earth satellites) |
| **Units** |||||
| `uom` | Compile-time dimensional analysis | 0.38.0 (2026-02) | Apache-2.0 OR MIT | Mature (~14M, 1.2k★); last push 2026-04 | **evaluate (API boundary only)**: whether it takes a custom storage type such as double-double is **UNVERIFIED**. Keep it out of the hot loop |
| `dimensioned` | Dimensional analysis | 0.8.0 (2022-04) | MIT/Apache-2.0 | Unmaintained (no release since 2022) | **avoid** |
| **Extended precision** |||||
| `twofloat` | Double-double (sum of two f64) | 0.8.4 (2025-07) | BSD-3-Clause | Small (21★); README says the transcendentals are "very preliminary", not full DD accuracy | **evaluate**: basic ops are fine. Do not trust its `exp`/`sin` |
| `qd` | Double-double, based on Poulson's C++ `mantis`; uses `pulp` SIMD | 0.8.0 (2025-11) | MIT | ~3.5M downloads (faer author) | **evaluate**: candidate DD backend |
| `xprec` | Fast emulated quad (double-double) | 0.2.3 (2026-10) | MIT | Tiny (9★), active | evaluate |
| `doubled` | Dekker arithmetic | 0.3.3 (2026-02) | MIT OR Apache-2.0 | Small | evaluate |
| `astro-float` | Arbitrary-precision float, pure Rust (README: designed for performance and portability) | 0.9.6 (2026-08) | MIT | Active (121★) | **adopt as test oracle**: high-precision reference for DD/f128 error budgets. Too slow for production runs (assumed, not benchmarked) |
| `dashu-float` | Arbitrary-precision float, pure Rust | 0.6.2 (2026-10) | MIT OR Apache-2.0 | Active | evaluate (alternative oracle) |
| `num-bigfloat` | Fixed ~40-digit float | 1.7.2 (2025-04) | MIT | Moderate | evaluate |
| `rug` | GMP/MPFR/MPC bindings | 1.30.0 (2026-04) | **LGPL-3.0+** (and links LGPL GMP/MPFR) | Very mature | **avoid**: LGPL, C toolchain, no wasm32-unknown-unknown. Acceptable only as an opt-in local dev oracle, never in the default graph |
| `f128` (crate) | libquadmath wrapper | 0.2.9 (2021-02) | MIT | Unmaintained; its own README says "should be avoided" in favor of native `f128` | **avoid** |
| `rustc_apfloat` | Soft-float port of LLVM APFloat | 0.2.3 (2026-10) | Apache-2.0 WITH LLVM-exception | Maintained by rust-lang | evaluate (exact IEEE binary128 reference) |
| `softfloat` | Pure-software float | 1.0.0 (2023-11) | MIT OR Apache-2.0 | Stale | avoid |
| Native `f128` (nightly) | IEEE binary128 primitive | n/a | (Rust) | Tracking issue rust-lang/rust#116909 **still open** (updated 2026-10-07). Nightly-only. Math-function and wasm32 support **UNVERIFIED** | **evaluate behind a feature flag**. Do not make it the default (stable toolchain policy) |
| **ODE / N-body integrators** |||||
| `ode_solvers` | Dopri5, Dop853, Rk4 | 0.6.2 (2026-06) | Apache-2.0 | Mature (~1.2M) | **evaluate (cross-check)**: independent DOP853 reference |
| `diffsol` | Stiff/non-stiff ODE (BDF, SDIRK, …), faer/nalgebra backends | 0.17.1 (2026-09) | MIT | Active (138★) | evaluate. Optional `suitesparse_sys` must stay off (SuiteSparse license mix) |
| `russell_ode` | ODE/DAE incl. Radau5 | 3.3.1 (2026-09) | MIT (crate) | Active, but `russell_sparse` needs system OpenBLAS + SuiteSparse (UMFPACK) / MUMPS | **avoid**: native deps (UMFPACK is GPL-licensed, per our knowledge, **UNVERIFIED** here), no WASM |
| `ivp`, `scirs2-integrate`, `peroxide` | General IVP / sci libraries | 0.6.0 / 0.6.5 / 0.43.1 | Apache-2.0 / Apache-2.0 / MIT OR Apache-2.0 | Mixed maturity | reference only |
| `librebound-sys`, `libassist-sys`, `adam_assist`, `rebound-rs` | REBOUND / ASSIST (IAS15) bindings and ports | 4.6.0 / 1.2.1 / 0.4.1 / 4.6.0-alpha.1 | **GPL-3.0** | Active (B612) | **avoid**: GPL. Do not read their code while implementing IAS15 (clean-room from the papers) |
| `thor-rs-assist` | "Fully-open ASSIST/REBOUND backend", IAS15 | 0.1.0 (2026-07) | BSD-3-Clause | Brand new (44 downloads). Whether it was derived from GPL REBOUND code is **UNVERIFIED** | **avoid until provenance is confirmed** |
| *(own)* IAS15 / Gauss-Radau 15th order + WH/WHFast-style symplectic | Flagship integrator, generic over scalar (f64 / DD / f128) | n/a | Apache-2.0 | none | **build-our-own** (no permissive Rust IAS15 exists today) |
| **SOFA / ERFA** |||||
| `erfa` / `erfa-sys` | Bindings to the ERFA C library | 0.2.1 (2022-11) | MPL-2.0 (bindings); ERFA C is BSD-3 | Stale since 2022; needs C | **avoid** |
| `erfars` | ERFA port | 0.2.0 (2025-12) | MIT OR Apache-2.0 | **Repo archived** (GitHub archived = true) | **avoid** |
| `sofars` | (see Time scales) | | | | evaluate |
| *(own)* ERFA-subset port | TT↔TDB↔TCB↔TCG, precession/nutation for frames | n/a | Apache-2.0 + BSD-3 attribution | none | **build-our-own** from **ERFA (BSD-3)**, not SOFA, tested against ERFA's test vectors |
| **Interfaces** |||||
| `rmcp` (+ `rmcp-macros`) | Official MCP Rust SDK | 3.5.1 (2026-10-05) | Apache-2.0 (crate metadata; GitHub reports NOASSERTION, so check the LICENSE file) | ~36M downloads, 4k★, very active. README: implements MCP spec `2026-07-28`, compatible with `2025-11-25` | **adopt** |
| `clap` | CLI parsing | 4.6.7 (2026-09) | MIT OR Apache-2.0 | De facto standard | **adopt** |
| `wasm-bindgen` / `wasm-bindgen-test` | JS↔WASM glue and tests | 0.2.129 / 0.3.79 (2026-09) | MIT OR Apache-2.0 | Standard (repo now under `wasm-bindgen` org) | **adopt** |
| `wasm-pack` | WASM build/packaging | 0.15.0 (2026-05) | MIT OR Apache-2.0 | Maintained | **adopt** (or plain `cargo` + `wasm-bindgen-cli`) |
| `plotters` | Static plots (SVG/PNG/canvas) | 0.3.7 (2024-09) | MIT | Very widely used. No release in ~2 years; repo last pushed 2026-04 | **evaluate**: fine for CLI report SVGs; watch maintenance |
| `plotly` (plotly.rs) | Interactive plots via Plotly.js | 0.14.1 (2026-02) | MIT | Maintained | evaluate (HTML report / web demo) |
| `serde` (+ `serde_json`) | Serialization | 1.0.229 (2026-07) | MIT OR Apache-2.0 | Standard | **adopt** |
| `pyo3` / `maturin` | Python bindings and wheel builds | 0.30.1 (2026-10-14) / 1.15.0 (2026-08) | MIT OR Apache-2.0 | Standard | **adopt** (tooling and validation notebooks) |
| **Tooling** |||||
| `cargo-deny` | License/advisory/source gating | 0.20.2 (2026-07) | MIT OR Apache-2.0 | Standard (Embark) | **adopt** |
| `cargo-nextest` | Test runner | 0.9.148 (2026-10) | Apache-2.0 OR MIT | Very active | **adopt** |
| `criterion` | Statistical benchmarks | 0.8.2 (2026-02) | Apache-2.0 OR MIT | Now under `criterion-rs` org; maintained | **adopt** |
| `divan` | Lightweight benchmarks | 0.1.21 (2025-04) | MIT OR Apache-2.0 | No release in ~18 months | evaluate |
| `proptest` | Property-based testing | 1.12.0 (2026-10) | MIT OR Apache-2.0 | Active | **adopt** (invariants: energy, angular momentum, time round-trips) |
| `approx` | Float comparisons (`assert_relative_eq!`, ulps) | 0.5.1 (2026-02) | Apache-2.0 | Standard | **adopt** |
| `float-cmp` | Float comparisons | 0.10.0 (2024-09) | MIT | Mature, quiet | optional (redundant with `approx`) |

---

### Build vs. reuse

1. **Reuse the plumbing, build the physics.** The interface and tooling layer (clap, serde, rmcp, wasm-bindgen, pyo3/maturin, cargo-deny, nextest, criterion, proptest, approx) is permissively licensed, mature and has no Fortran-stack equivalent worth porting. Adopt all of it.
2. **The integrator must be ours.** There is no permissive, maintained IAS15 / Gauss-Radau in Rust. The only real ones are REBOUND/ASSIST bindings and ports, and they are all GPL-3.0. General ODE crates (`ode_solvers`, `diffsol`) are useful as independent cross-checks, not as the flagship. IAS15 (Rein & Spiegel 2015) and a WH-style symplectic map must be written **clean-room from the papers**, generic over the scalar type.
3. **"Generic over scalar" drives most other decisions.** The multi-millennia Mercury run needs one code path for f64, double-double and (optionally) f128. Crates that hard-wire f64 belong at the edges only: `glam`, ANISE's evaluator, likely `uom`'s fast path, and `hifitime`'s f64 accessors. So:
   - **Vectors:** a small in-house `Vec3<T: Real>` / `Mat3<T>` for the hot loop, with nalgebra for f64 frame and rotation work. If `simba::RealField` can be implemented for our DD type cheaply, nalgebra can cover both (**UNVERIFIED**).
   - **Ephemerides:** our own SPK type-2 (Chebyshev) reader for DE440/DE441, evaluating in `T`. ANISE (MPL-2.0) is the validation oracle and an optional f64 backend.
   - **Extended precision:** define our own `Real` trait. Back it with one DD crate (`qd` or `twofloat`, chosen by benchmark and an accuracy audit) or our own ~500-LOC DD (estimate) with correctly rounded transcendentals. Native nightly `f128` sits behind a feature. `astro-float` (pure Rust, MIT) is the arbitrary-precision **oracle** in tests. `rug` is excluded.
4. **Time:** adopt `hifitime` for civil time, leap seconds and the TCB/TCG/TDB definitions; its scale list was checked in source. Internally, propagate in TDB seconds held as DD. Re-derive TDB↔TCB↔TCG from ERFA (BSD-3) formulas and cross-check against hifitime and sofars.
5. **SOFA/ERFA:** port the small subset we need from **ERFA** (BSD-3, created as a freely licensed derivative of SOFA). Do not depend on `erfa` (stale, needs C) or `erfars` (archived). Treat `sofars` as a test oracle until its SOFA-terms status is cleared.
6. **Frameworks:** none fits the scope. `nyx-space` is AGPL. `satkit`, `lox` and `brahe` target Earth satellites and mission analysis. They are useful as dev-dependency oracles at most.

### License risks

- **AGPL-3.0: `nyx-space`.** This is the biggest trap, because its sibling crates `hifitime` and `anise` are MPL-2.0 and have the same branding. Pulling in `nyx-space` would make the network-served MCP server and web demo subject to AGPL source-offer obligations. Hard ban. Contributors must also not copy code from it.
- **GPL-3.0: REBOUND/ASSIST bindings and ports** (`librebound-sys`, `libassist-sys`, `adam_assist`, `rebound-rs`). Hard ban, plus a clean-room policy for IAS15. `thor-rs-assist` claims BSD-3 but sits in the same lineage, so its provenance is **UNVERIFIED** and it stays excluded until clarified.
- **LGPL: `rug` (GMP/MPFR) and `cspice-sys`.** Rust's static linking makes LGPL relinking obligations awkward, and neither builds for `wasm32-unknown-unknown`. Banned from the default graph. `rug` may appear only in an out-of-tree or opt-in dev oracle that is never shipped.
- **MPL-2.0: `hifitime`, `anise`, `lox-*`, `erfa`.** These are acceptable as unmodified dependencies. MPL is file-level copyleft: if we patch an MPL file, the patched file stays MPL and its source must be available. Binary distributions, including the **WASM bundle** and PyPI wheels, must say where the MPL source can be obtained. The policy is never to vendor-and-modify MPL files and to upstream any fixes. The NOTICE/third-party file must list them.
- **SOFA license terms.** The IAU SOFA license is not an OSI license. It imposes conditions on derived works, including renaming. ERFA exists as a BSD-3 re-release precisely to avoid this. `sofars` ships the MIT text **plus** the SOFA Board copyright header. Whether SOFA's terms bind `sofars` users, and so transitively `anise` users (anise depends on sofars), is **UNVERIFIED** and needs a legal read. Mitigation: port from ERFA.
- **NAIF CSPICE terms** (via `rust-spice`): custom and not OSI. Exact terms are **UNVERIFIED**. Avoid them in shipped artifacts.
- **Native toolchain contamination.** `russell_*` (system SuiteSparse/UMFPACK, MUMPS, OpenBLAS/MKL) and `diffsol`'s optional `suitesparse_sys` can bring in GPL or non-free native libraries. They also break WASM.
- **License metadata drift.** `rmcp` is Apache-2.0 on crates.io but GitHub reports `NOASSERTION`. `sofars` is MIT on crates.io but its LICENSE file contains more text. cargo-deny has to check the actual license files, not just the crates.io SPDX field.
- **Data licenses.** JPL DE440/DE441 kernels are distributed by NASA/JPL. Their redistribution terms for bundling in the WASM demo are **UNVERIFIED**. Prefer download-on-demand with checksums, or ship a truncated, explicitly attributed segment once terms are confirmed.

### Requirements implied

- **DEP-001**: `cargo-deny` must reject GPL-*, LGPL-*, AGPL-*, SSPL, and any license not on an explicit allow-list anywhere in the dependency graph. That includes dev- and build-dependencies of published crates. CI fails on violation.
- **DEP-002**: The license allow-list is: Apache-2.0, Apache-2.0 WITH LLVM-exception, MIT, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-3.0/Unicode-DFS-2016, CC0-1.0, and MPL-2.0. MPL-2.0 is allowed only for the crates named in a reviewed exception list (initially `hifitime`, `anise`).
- **DEP-003**: `cargo-deny` must ban `nyx-space`/`nyx_space`, `rug`, `gmp-mpfr-sys`, `cspice-sys`, `librebound-sys`, `libassist-sys`, `adam_assist`, `rebound-rs`, and `thor-rs-assist` by name (until DEP-015 clears it), whatever their declared license.
- **DEP-004**: MPL-2.0 dependency files must never be vendored and modified in-tree. Any fix goes upstream, or into a separately published MPL fork.
- **DEP-005**: Every distributed artifact (crates, CLI binaries, PyPI wheels, WASM bundle, MCP server image) must ship a generated `THIRD_PARTY_LICENSES` file (e.g. via `cargo-about` or `cargo-deny list`). For MPL components it must state where their source can be obtained.
- **DEP-006**: The physics core crate (`ferroastro-core` or equivalent) must build on **stable Rust** for `wasm32-unknown-unknown` with no C/C++/Fortran toolchain and no system libraries. CI builds it for that target.
- **DEP-007**: The integrator, force model, ephemeris evaluator and time arithmetic must be generic over a crate-owned `Real` scalar trait, with implementations for `f64` and a double-double type. A nightly-only `f128` implementation sits behind a non-default feature.
- **DEP-008**: The IAS15/Gauss-Radau and symplectic integrators must be **clean-room implementations** from the published papers. Contributors must not consult GPL REBOUND/ASSIST source, and PR templates must say so.
- **DEP-009**: SOFA-equivalent routines must be ported from **ERFA (BSD-3-Clause)** with its copyright notice kept in NOTICE. Function names must not use the SOFA `iau` prefix. Each routine needs a test reproducing ERFA's reference values.
- **DEP-010**: Time-scale support must cover TAI, TT, TDB, TCB, TCG and UTC (with leap seconds). Conversions must be cross-validated against `hifitime` and an ERFA-derived reference to a stated tolerance, both at f64 and DD precision.
- **DEP-011**: JPL DE440/DE441 SPK type-2 reading must be implemented in-crate, generic over `Real`. In CI it must be cross-validated against ANISE (and optionally `satkit`) to within a documented tolerance.
- **DEP-012**: Extended-precision arithmetic must be validated against an arbitrary-precision oracle (`astro-float`, pure Rust, MIT) in property tests (`proptest`). The tests cover +, ×, ÷, sqrt, and every transcendental the force model uses, with documented ulp error bounds.
- **DEP-013**: The MCP server must use the official `rmcp` SDK, pinned to a 3.x minor, and declare the MCP spec revision it targets (currently `2026-07-28`).
- **DEP-014**: The project tooling baseline is `cargo-nextest` (tests), `criterion` (benchmarks, with tracked baselines), `proptest` (invariants), `approx` (float assertions) and `cargo-deny` (licenses, advisories, sources). These run in CI on every PR.
- **DEP-015**: Any new dependency needs a provenance check: license file read (not only the crates.io SPDX field), maintenance status (last release under 24 months or a justification), and WASM compatibility, recorded in the PR. Archived crates (e.g. `erfars`) and crates with unresolved provenance (e.g. `thor-rs-assist`) are rejected.
- **DEP-016**: Optional native-library features of otherwise-permissive crates (e.g. `diffsol`'s `suitesparse_sys`, LAPACK backends) must not be enabled anywhere in the workspace. `cargo-deny`'s `[bans]` feature checks or a CI `cargo tree -e features` audit enforce this.
- **DEP-017**: JPL ephemeris data must not be bundled in released artifacts until its redistribution terms are confirmed. The default is fetch-on-demand with pinned SHA-256 checksums.

### Open questions / UNVERIFIED items to close

- Do `qd`/`twofloat` (or our DD) implement `simba::RealField` cheaply enough to reuse nalgebra in the hot loop?
- Can `uom` take a custom storage type (DD)?
- Do `faer` and `anise` build for wasm32?
- Does nightly `f128` have math functions (`sin`, `exp`, …) on wasm32 and x86_64 targets?
- Do SOFA's terms bind `sofars` and transitively `anise`?
- NAIF CSPICE terms, and JPL DE kernel redistribution terms.
- Was `thor-rs-assist` derived from GPL code?
- Is UMFPACK's license (per our knowledge GPL) correct for the version `russell_sparse` links?
