<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->
# FerroAstro orbit viewer: design

Status: design, not yet implemented (branch `feat/orbit-viewer` off `m5-performance`).
Purpose (principal's request): *see the actual orbits and toggle the perturbation effects on and off
while probing Mercury's perihelion.*

This document is the contract between the engine builder (`crates/ferroastro-viewer`, Rust) and the
UI builder (`crates/ferroastro-viewer/web/`, plain HTML/JS/CSS). Both MUST follow §4 (the C ABI)
exactly. The design adds no dependencies, Rust or JS (§10).

---

## 1. Shape of the solution

```
 browser (static dir, offline)                    wasm32-unknown-unknown, no imports
 ┌─────────────────────────────────────┐         ┌──────────────────────────────────────────┐
 │ index.html + viewer.js (main thread)│ post-   │ worker.js: WebAssembly.instantiate(      │
 │  canvas 2D orbit view, ϖ(t) chart,  │ Message │   ferroastro_viewer.wasm, {})            │
 │  contribution table, controls       │◀──────▶ │   fav_* extern "C" calls, copies buffers  │
 └─────────────────────────────────────┘         │   out of linear memory, posts them        │
         one "live" worker + N "leg" workers     └──────────────────────────────────────────┘
                                                        │ (Rust, same code on native)
                                                        ▼
        ferroastro-viewer (L12) ── propagate (L8) ── forces/eih/symplectic/integrators
                               ── diffrun (L9) Estimator (Park2017, one per table)
                               ── ladder (L10) single-body rung builders (Rung lab)
                               ── precession (L5), analytic (L4), frames (L3), constants (L2)
```

* All physics comes from existing crates. The viewer builds a `ferroastro_scenario::ScenarioSpec`
  from the toggles and runs it with `ferroastro_propagate::{run_until, resume_until}` (the same
  path the CLI's `ferroastro scenario run` and the diffrun harness take). No force model, integrator
  or estimator is written in the viewer crate or in JS. The JS draws numbers it is given.
* Two modes:
  * **System mode** (default): N-body, Sun + Mercury + the planets that are toggled on, header.440
    initial conditions (§5). Toggles: planets, solar J2, GR 1PN, Lense–Thirring, asteroids.
  * **Rung lab**: Mercury as a test particle about a fixed Sun (the M2 single-body rungs), the only
    place where the SR rungs of ADR-0012 are offered (§3.3).

## 2. The crate `crates/ferroastro-viewer`

### 2.1 Package

* `crate-type = ["cdylib", "rlib"]`. The rlib is what native tests, clippy and docs use; the cdylib
  is the `.wasm` (and a harmless `.so` on native).
* Builds for `wasm32-unknown-unknown` and native on the pinned 1.95.0 toolchain, `std`, f64 only.
* Dependencies (all existing workspace crates or workspace deps; **no new third-party crate**):
  `ferroastro-propagate`, `ferroastro-diffrun`, `ferroastro-ladder`, `ferroastro-precession`,
  `ferroastro-analytic`, `ferroastro-frames`, `ferroastro-constants`, `ferroastro-scenario`,
  `ferroastro-system`, `ferroastro-forces`, `ferroastro-core`, `ferroastro-real`, `serde_json`.
  Dev: `ferroastro-vectors`, `ferroastro-testdata` (test-support, allowed by the layering rules).
  `ferroastro-propagate` is used **without** its `parallel` feature (no rayon on wasm).
* `[package] publish = false` (it embeds files from outside the crate directory, §5).
* Layering: add `"ferroastro-viewer": 12` to `LAYERS` in `tools/check_layers.py` (surfaces layer,
  next to `ferroastro-cli` / planned `ferroastro-wasm`). Every dependency above is in a strictly
  lower layer, so no `EXCEPTIONS` entry is needed. Update the matching pytest if it enumerates
  layers.

### 2.2 The one unsafe-attribute deviation (NEEDS HUMAN REVIEW)

A wasm export needs `#[unsafe(no_mangle)]` (edition 2024). Verified on the host with rustc 1.95.0:
under `#![forbid(unsafe_code)]` this fails with `error: declaration of a no_mangle function`, and a
`forbid` cannot be relaxed by an inner `allow`. Without the attribute nothing is exported, and
wasm-bindgen is neither installed nor allowed (no new deps). So the crate cannot use
`[lints] workspace = true` verbatim. Design:

1. `crates/ferroastro-viewer/Cargo.toml` carries its own `[lints.rust]`, `[lints.rustdoc]` and
   `[lints.clippy]` tables, a copy of `[workspace.lints]` with exactly one change:
   `unsafe_code = "deny"` instead of `"forbid"`.
2. Only `src/ffi.rs` has `#![allow(unsafe_code)]`. It contains only `#[unsafe(no_mangle)] pub extern
   "C" fn` shims: no `unsafe {}` blocks, no `unsafe fn`, no `unsafe impl`, no raw-pointer
   dereference. Every other module starts with `#![forbid(unsafe_code)]`.
3. A PR-tier test `ffi_contains_no_unsafe_code` does `include_str!("ffi.rs")` and asserts that the
   only `unsafe` tokens are `#[unsafe(no_mangle)]` attributes.
4. `tools/lint_canary.py`'s config check gets a documented exception for `ferroastro-viewer`. It
   requires the crate's lint tables to equal the root `[workspace.lints]` except for
   `unsafe_code = "deny"`, so drift in either direction fails CI. Add a pytest for the exception.
5. AGENTS.md rule 5 ("No `unsafe`") and STD-003 need a maintainer's sign-off for this exception.
   A rejected alternative is a separate shim crate: it needs the same exception, in one more crate.

Pointers cross the ABI only as **return values** (`*const f64`, `*const u8`). Making a raw pointer
from a slice is safe Rust. JS never writes into wasm memory. Every input is a scalar argument, so
the crate exports no allocator.

### 2.3 Modules

| module | content |
|---|---|
| `config` | `ViewerConfig` (toggles bitmask, parameters §4.3), validation, `to_scenario(&self, leg) -> ScenarioSpec` |
| `ic` | the build-time embedded header.440 (feature `embed-header`) and small-body states (§5), `DataSources` assembly |
| `engine` | `Engine`: holds the spec, the propagate `Checkpoint`, the Mercury relative track, the output buffers; `configure`, `run_years`, `drain` |
| `observe` | osculating elements and ϖ (§6), display-frame transform |
| `fit` | the live rate fit and the per-table estimator, both via `ferroastro_diffrun::Estimator` / `ferroastro_precession::park2017` |
| `lab` | Rung lab: test-particle model from `ferroastro_ladder::single_body` |
| `meta` | PHY-015 metadata JSON (frame, epoch, span, fit model, constants version, IC hash, model string) |
| `ffi` | the C ABI (only module allowed the unsafe attribute) |

The state lives in `thread_local! { static ENGINE: RefCell<Option<Engine>> }`. wasm is
single-threaded and each Worker has its own instance. On native, tests use `Engine` directly (the
rlib API), and the `ffi` functions are thin wrappers over it, so tests and the browser exercise the
same code.

### 2.4 Things the implementer must not trip on

* `std::time::Instant::now()` **panics on wasm32-unknown-unknown**. `ferroastro-ladder`'s
  `single_body.rs:554` times its run with it. The Rung lab therefore uses the ladder only to *build*
  the `TestParticle` model and its initial state. It drives `ferroastro_integrators::Ias15` itself,
  or the ladder gets an untimed entry point. Either way, no `Instant` on the wasm path. The node
  smoke test (§9) catches a regression as a trap.
* `ferroastro-diffrun`'s on-disk store (`std::fs`) must not be used. Use `Estimator` and `Track`
  only.
* No `unwrap`/`expect`/indexing panics on the FFI path: wasm32 aborts on a panic (`panic=abort`
  trap) and the worker dies. Every error becomes a negative status plus a message (§4.1).
* `ferroastro_propagate::run_until` refuses test-particle scenarios (`not_for_test_particles`). The
  Rung lab does not use it (§3.3).

## 3. Toggles, engines and what each switch maps to

### 3.1 System-mode toggles (bitmask `u32`, §4.3)

| bit | toggle | scenario mapping (`ScenarioSpec`) | engines |
|---|---|---|---|
| 0 | Venus | `"Venus"` in `bodies` | WH, IAS15 |
| 1 | Earth–Moon | `"EMB"` in `bodies` (one body; WH needs EMB, not Earth + Moon) | WH, IAS15 |
| 2 | Mars | `"Mars"` | WH, IAS15 |
| 3 | Jupiter | `"Jupiter"` | WH, IAS15 |
| 4 | Saturn | `"Saturn"` | WH, IAS15 |
| 5 | Uranus | `"Uranus"` | WH, IAS15 |
| 6 | Neptune | `"Neptune"` | WH, IAS15 |
| 7 | Solar J2 | `forces.sun_j2 = true` (`SolarJ2WithReaction`, DE440 `J2SUN` (read from the embedded header.440, §5), `ASUN`, WGCCRE pole; J2 overridable via `SolarOverrides.j2`) | WH, IAS15 |
| 8 | GR 1PN (Sun) | IAS15: `forces.gr_1pn_central = true`, `forces.ppn = {β, γ}` (`ferroastro_eih::HeliocentricSchwarzschild`, PPN isotropic/harmonic gauge, Sun-only, no reaction). WH: `forces.gr_potential = true` (`GrPotential`, Nobili–Roxburgh; reproduces the 1PN secular advance, **β = γ = 1 only**) | WH (β=γ=1), IAS15 |
| 9 | Lense–Thirring (Sun spin) | `forces.lense_thirring = true` (`SolarLenseThirring`, DE440 Eqs. 37–39, `G·S = GMS·MOISUN·ASUN²·OMGSUN`, γ from the PPN parameter) | IAS15 only |
| 10 | Big asteroids | `forces.asteroids = N` (first N numbered header.440 asteroids, Minor tier, reaction on; default N = 16), states from the in-repo REF-003 file (§5) | IAS15 only |
| 11 | 1PN among all bodies (EIH), advanced | `forces.gr_eih = true` with `ppn` (replaces bit 8; mutually exclusive) | IAS15 only (~1.3× slower) |
| 12–31 | reserved, must be 0 | `FAV_E_ARG` | |

**Kepler-only baseline** = mask `0`: Sun + Mercury, Newtonian. ϖ̇ must be 0 to integrator error
(PHY-001). The UI has a one-click "Kepler only" preset (mask 0) and a "Full model" preset
(bits 0–9).

Removing a planet from `bodies` is the **P-remove** operation of ADR-0014. For Newtonian point
masses it is identical to Park's "GM set to zero". `ferroastro-propagate` keeps every remaining
body's initial record bitwise identical across configurations (NUM-013). The engine records the IC
hash of every run so the table can prove it.

### 3.2 Engine choice (parameter `ENGINE`)

* `AUTO` (default): **WHFast** (`integrator = whfast`, `step_days = 1.0`, `wh_corrector = 11`)
  when the toggle set allows it: no LT, no asteroids, no EIH, and GR only with β = γ = 1.
  Otherwise **IAS15** (`integrator = ias15`, f64, ε = 1e-9, the ladder/PHY-012 default).
  `ferroastro_propagate::refuse_unsupported` is the authority. `AUTO` is resolved by trying WH and
  falling back to IAS15 on `FA-E1201`, never by a hand-maintained copy of the rules.
* `WHFAST` / `IAS15` force one. A forced combination that propagate refuses returns
  `FAV_E_UNSUPPORTED`, with propagate's FA-E1201 text, which names the switch.
* The **contribution table uses one engine for every leg** (CI-009: one integrator and one
  estimator per table): IAS15 if any leg needs it, otherwise WH. The engine is printed in the table
  header.

Responsiveness and accuracy (to be measured by the validator; reported, not gated, because the
host is loaded):

| engine | cost basis (repo evidence) | expected wasm throughput, Sun + 8 planets | accuracy note |
|---|---|---|---|
| WHFast h = 1 d (≈88 steps per Mercury orbit) | 3.96 µs/step for Sun + Mercury native (WP29 W4); O(N²) kick | ≈ 100 yr in 1–3 s (target ≥ 50 yr/s) | NUM-004: energy error ≤ 1e-8 over 1e6 yr; GR-potential rung 42.9796 ″/cy vs 42.98 (WP29 G1). Symplectic, so no secular ϖ drift from the integrator; h-dependent short-period phase error (Mercury's mean motion differs from 1PN by O(GM/ac²) with the GR potential, Saha & Tremaine 1994) |
| IAS15 ε = 1e-9 | ≈ 18.5 s per 1000 yr native, 317 k steps (WP18 table (a)) | ≈ 100 yr in 3–5 s (target ≥ 15 yr/s) | round-off-limited (Brouwer), 52 m from REBOUND after 1000 yr; handles the velocity-dependent 1PN, LT and asteroid terms |

The `IAS15_EPSILON` parameter (allowed 1e-12 … 1e-6) trades accuracy for speed. The UI labels any
ε > 1e-9 as "fast, reduced accuracy".

### 3.3 Rung lab (test-particle mode) and the SR rungs

ADR-0012's SR-a and SR-b are **test-particle definitions**: Mercury in the field of a fixed Sun. They
have no meaning with the planets on, because H = γmc² − μm/r has only the Sun's potential. So they
are offered only in Rung lab, never in System mode, and the UI says why. Rung lab:

* Model: the `TestParticle` of `ferroastro_ladder::single_body::Model`, chosen by parameter `RUNG`.
  Values: `0` Newton only, `1` SR-a (`SrKepler`, gauge "SR, no PN gauge", 7.1634 ″/cy),
  `2` SR-b harmonic (`PnLagrangian::sr_b_harmonic`, 50.144 ″/cy, label "PPN isotropic/harmonic"),
  `3` SR-b areal (`PnLagrangian::sr_b_areal`, 35.817 ″/cy, label "Schwarzschild areal,
  pedagogical"), `4` GR PPN (`SchwarzschildPpn` with β, γ). Bit 7 (J2) and bit 9 (LT) add the
  single-body `SolarJ2` / `LenseThirring` terms. The planet bits must be 0 (`FAV_E_ARG` otherwise).
* ICs: the ladder's embedded `MERCURY_SINGLE_BODY` scenario (`crates/ferroastro-ladder/data/
  mercury-single-body.toml`): Standish & Williams 1992 J2000 mean elements, J2000 mean ecliptic,
  μ = GM☉ (DE440), fixed Sun.
* Run: IAS15 driven by the viewer (see §2.4), the same chunked `fav_run` interface, the same
  buffers. Passages from `ferroastro_precession::passages`; rate from `fit_per_orbit` (the ladder's
  estimator), labelled as such.
* SR-b is always shown with its gauge label and a note that the SR/GR split is not observable
  (ADR-0012).

## 4. The C ABI (contract for both builders) — ABI version 1

### 4.1 Conventions

* All exports are `extern "C"`, prefixed `fav_`, and take and return only `i32`, `u32` and `f64`.
  Pointer results are `*const f64` / `*const u8`. On wasm32 a pointer is a `u32` byte offset into
  the exported `memory`.
* **Status** (`i32`): `0` `FAV_OK`; `1` `FAV_DONE` (span reached, nothing more to run); negative
  values are errors: `-1` `FAV_E_STATE` (call out of order, e.g. `fav_run` before
  `fav_configure`), `-2` `FAV_E_ARG` (bad id, value out of range, reserved bit set), `-3`
  `FAV_E_UNSUPPORTED` (propagate FA-E1201), `-4` `FAV_E_RUN` (propagation or model error), `-5`
  `FAV_E_FIT` (estimator: too few passages or too short a span; not fatal, running may continue).
* After any negative status, `fav_error_ptr()` / `fav_error_len()` give a UTF-8 message that starts
  with the underlying `FA-Exxxx` code when there is one. The message stays valid until the next
  `fav_*` call.
* **Buffer lifetime:** a pointer returned by `fav_*_ptr` is valid until the next `fav_*` call
  *other than* the matching `_len` call. Wasm memory can grow during any call, which detaches JS
  views, so JS must build a fresh `Float64Array(memory.buffer, ptr, len)` and copy it (`.slice()`)
  immediately. All f64 buffers are 8-byte aligned. Byte order is little-endian (wasm).
* "Drain" buffers hold the records produced since the previous `fav_drain()`. `fav_drain()`
  empties them; the engine does not wait for it (if JS never drains, buffers grow).
* Time: **TDB**. `t` in buffers is **Julian years (365.25 d) since the run epoch** (`fav_epoch_jd`).
  Angles in buffers are in **degrees**; rates in **arcsec per Julian century** (″/cy).
  Lengths in **au** (DE440 `AU` = 149 597 870.700 km). Internally km, km/s, s (forces crate).
* NaN marks "no value" (e.g. the slots of bodies that are toggled off).

### 4.2 Functions

| signature | purpose |
|---|---|
| `fav_abi_version() -> u32` | `1`. JS refuses any other value. |
| `fav_reset() -> i32` | drop the engine; toggles and parameters return to the defaults of §4.3 (System mode, mask = bits 0–9, AUTO engine) |
| `fav_set_toggles(mask: u32) -> i32` | set the toggle mask (§3.1); takes effect at the next `fav_configure` |
| `fav_set_param(id: u32, value: f64) -> i32` | set a parameter (§4.3); range-checked now; takes effect at the next `fav_configure` |
| `fav_get_param(id: u32) -> f64` | current value (NaN for an unknown id) |
| `fav_configure() -> i32` | validate, resolve the engine, build the `ScenarioSpec`, assemble ICs, start a run at t = 0. Discards any previous run and its buffers. |
| `fav_run(years: f64) -> i32` | advance by `years` (0 < years ≤ 50) and append to the drain buffers. Returns `FAV_DONE` at `SPAN_YEARS`. JS picks `years` so a call takes ≲ 30 ms (§7.4). |
| `fav_time_years() -> f64` | TDB Julian years since the epoch reached so far |
| `fav_epoch_jd() -> f64` | run epoch, TDB Julian date (2440400.5 in System mode; 2451545.0 in Rung lab) |
| `fav_engine() -> u32` | resolved engine: `1` WHFast, `2` IAS15, `3` IAS15 test particle (Rung lab) |
| `fav_drain() -> i32` | empty the drain buffers (call after copying them) |
| `fav_bodies_ptr() -> *const f64`, `fav_bodies_len() -> u32` | positions buffer (§4.4 A), length in f64 |
| `fav_elements_ptr() -> *const f64`, `fav_elements_len() -> u32` | Mercury osculating-element series (§4.4 B) |
| `fav_passages_ptr() -> *const f64`, `fav_passages_len() -> u32` | perihelion passages since the last drain (§4.4 C) |
| `fav_fit(t0_years: f64, t1_years: f64) -> i32` | fit ϖ̇ over the passages with t in [t0, t1] (pass NaN, NaN for all passages so far). Fills the fit buffer. |
| `fav_fit_ptr() -> *const f64`, `fav_fit_len() -> u32` | fit result (§4.4 D) |
| `fav_meta_ptr() -> *const u8`, `fav_meta_len() -> u32` | UTF-8 JSON metadata of the current run and last fit (§4.5) |
| `fav_scenario_ptr() -> *const u8`, `fav_scenario_len() -> u32` | the run's `ScenarioSpec` as TOML (System mode), for "reproduce with the CLI" |
| `fav_error_ptr() -> *const u8`, `fav_error_len() -> u32` | last error message |

The engine never calls into JS; the module has **no imports** (checked in CI, §9).

### 4.3 Parameter ids and defaults

| id | name | default | range / values | used by |
|---|---|---|---|---|
| 0 | `MODE` | 0 | 0 System, 1 Rung lab | all |
| 1 | `ENGINE` | 0 | 0 AUTO, 1 WHFAST, 2 IAS15 | System |
| 2 | `PPN_BETA` | 1.0 | finite, \|β−1\| ≤ 100 | GR 1PN, EIH, Rung lab GR |
| 3 | `PPN_GAMMA` | 1.0 | finite, \|γ−1\| ≤ 100 | GR 1PN, EIH, LT, Rung lab GR |
| 4 | `WH_STEP_DAYS` | 1.0 | 0.125 … 4 | WHFast |
| 5 | `IAS15_EPSILON` | 1e-9 | 1e-12 … 1e-6 | IAS15 |
| 6 | `SPAN_YEARS` | 1000 | 1 … 20000 | all |
| 7 | `DISPLAY_CADENCE_DAYS` | 4.0 | 0.5 … 50; ≥ the WH step | bodies/elements buffers |
| 8 | `ASTEROID_COUNT` | 16 | integer 1 … 343 | bit 10 |
| 9 | `RUNG` | 4 | 0 … 4 (§3.3) | Rung lab |
| 10 | `J2_OVERRIDE` | NaN (= DE440 `J2SUN`) | NaN or 0 … 1e-3 | bit 7 (via `SolarOverrides.j2`) |
| 11 | `SPIN_SCALE` | 1.0 | 0 … 1e6 (scales DE440 G·S via `SolarOverrides.spin_gm`) | bit 9 |
| 12 | `FIT_PERIODIC` | 1 | 0 linear only; 1 add the Park17 Table 2 terms that the span resolves (§6.3) | fits |

Any value other than the default for ids 10/11, or ε > 1e-9, marks the run "non-nominal" in the
metadata and in the UI ("exaggerated physics: J2 × k"). That is how a physical exaggeration is
labelled. It is separate from the display exaggeration of §7.2.

### 4.4 Buffer layouts (all `f64`, row-major, little-endian)

**A. Bodies** (`fav_bodies_*`): one record per display sample, stride **25** f64:

```
[ t_years,
  Mercury x, y, z,  Venus x,y,z,  EMB x,y,z,  Mars x,y,z,
  Jupiter x,y,z,  Saturn x,y,z,  Uranus x,y,z,  Neptune x,y,z ]
```

Heliocentric (body − Sun) positions in **au**, rotated to the **J2000 ecliptic** (`ECLIPJ2000`, via
`ferroastro_frames::icrf_to_ecliptic_j2000`, ε₀ = 84381.448″) so that "top-down" means looking down
the ecliptic pole. Slots of bodies that are off are NaN. Samples every `DISPLAY_CADENCE_DAYS`. In
Rung lab only the Mercury slot is filled; its frame is the ladder's J2000 mean ecliptic.

**B. Mercury elements** (`fav_elements_*`): one record per display sample, stride **6**:

```
[ t_years, varpi_deg, e, a_au, i_deg, varpi_ecl_deg ]
```

* `varpi_deg`: osculating longitude of perihelion Ω + ω in the **Park17 mean-Mercury-orbit frame**
  (ICRF pole α = 280.9876°, δ = 61.4481°; `ReferencePlane::park2017_mean_orbit`). Continuous across
  the 0/360 wrap: the engine keeps the series continuous across chunks.
* `e`, `a_au`: osculating heliocentric eccentricity and semi-major axis with μ = GM☉ + GM☿.
* `i_deg`: inclination to the Park17 plane.
* `varpi_ecl_deg`: Ω + ω on the J2000 ecliptic, also unwrapped (for comparison only; ADR-0011's
  canonical frame is the Park17 plane).

**C. Passages** (`fav_passages_*`): one record per perihelion passage, stride **6**:

```
[ t_years, varpi_proj_arcsec, x_au, y_au, z_au, direction_error_arcsec ]
```

* `varpi_proj_arcsec`: the **ADR-0011 observable**, the longitude of the passage position projected
  on the Park17 mean-orbit plane (`LongitudeDefinition::Projected`), unwrapped, in arcsec.
* x, y, z: passage position, heliocentric, J2000 ecliptic, au (for drawing the markers).
* Passages are located by the Park2017 estimator in Mercury's sampled relative states (Lagrange
  order 8, `PassageSource::SampledStates`, internal sample cadence = the WH step or 1 d for IAS15).
  This is the same estimator path for both engines (CI-009). A passage is reported once its
  interpolation window is complete, so passages lag the integration by about 4 internal samples.

**D. Fit** (`fav_fit_*`): stride **10**, one record:

```
[ rate_arcsec_per_cy, formal_sigma, jackknife_sigma, q_arcsec_per_cy2,
  rms_residual_arcsec, n_passages, t_first_years, t_last_years, n_periodic_terms, b_arcsec ]
```

### 4.5 Metadata JSON (`fav_meta_*`) — PHY-015

One object, keys fixed (UI builders may rely on them):

```json
{ "abi": 1, "mode": "system" | "rung-lab", "engine": "whfast h=1 d" | "ias15 eps=1e-9" | ...,
  "nominal": true, "toggles": 1023, "params": {"beta":1,"gamma":1, ...},
  "model": "<propagate RunOutput.model string: every force term, its gauge and parameters>",
  "gauge": "PPN isotropic/harmonic" | "Newtonian" | ...,
  "frame": "Park17 mean Mercury orbit plane, ICRF pole RA 280.9876 deg Dec 61.4481 deg",
  "display_frame": "ECLIPJ2000 (R_x(84381.448 arcsec) of ICRF), heliocentric, au",
  "epoch_jd_tdb": 2440400.5, "time_scale": "TDB",
  "ic_source": "<RunOutput.ic_source: embedded header.440 provenance + interpretation>",
  "ic_hash": "<RunOutput.ic_hash>", "constants": "<constants_version()>",
  "scenario_digest": "<ScenarioSpec::digest>",
  "estimator": "<diffrun Estimator::id(SampledStates)>",
  "fit": { "model": "<FitModel::describe>", "span_years": [t0, t1], "passages": n } | null,
  "j2_nominal": <DE440 J2SUN as the engine loaded it> | null }
```

`j2_nominal` (added with #22) lets the page scale its `J2_OVERRIDE` reference values without
copying a header.440 value into JS.

## 5. Initial conditions (embedded at build time, no network or file access at run time)

**System mode: DE440 `header.440` GROUP 1041 initial conditions at `JDEPOC` = 2440400.5 TDB
(1969-06-28 00:00 TDB), ICRF axes, solar-system barycentric** (CENTER = 0, read under
`ferroastro_system::HEADER_STATE_INTERPRETATION`, open item OI-23). This is the IC source of every
Mercury rung in the repository (`initial_conditions = { source = "de440-header" }`, e.g.
`ci/scenarios/sun-8planets-newton-j2-1000yr.toml`, the PHY-012/013 nightlies with shared IC hash
`6d875b67…`).

`header.440` itself is not in the repository, and no value from it may be committed either
(ADR-0009; the principal's decision on FerroAstro #22, implemented for `ferroastro-constants` by
PR #53). The viewer therefore follows #22's design: **an opt-in `embed-header` feature includes the
fetched, hash-pinned file at build time**, and nothing derived from it lives in the tree.

* `crates/ferroastro-viewer/build.rs` (the pattern of #53's `ferroastro-constants/build.rs`): with
  the feature on, it locates `header.440` in the FerroAstro data cache (`$FA_DATA_DIR`, else
  `$CARGO_TARGET_DIR/fa-data`, else `<workspace>/target/fa-data`, the lookup of
  `ferroastro-testdata` and `tools/fetch_jpl.py`), checks its SHA-256 against the `de440-header`
  pin of `tests/data/MANIFEST.toml`, and copies the **whole file** into `OUT_DIR`. A missing file
  fails the build with `FA-E0303` (fix: `(cd tools && uv run python fetch_jpl.py de440-header)` or
  set `FA_DATA_DIR`); a changed one with `FA-E0304`. Without the feature the script does nothing.
* `ic.rs` includes it with `include_bytes!(concat!(env!("OUT_DIR"), "/header.440"))`
  (`ic::HEADER_440`), parses it once with `ferroastro_constants::header440::Header::parse`, and passes
  it through the unchanged `DataSources::with_header` path, with a provenance line (URL, SHA-256,
  bytes, pin date, "embedded at build time") that appears in `RunOutput.ic_source` and the
  metadata. No new parser, and the ICs are exactly the repository's.
* **Without the feature** the crate compiles, passes clippy and its tests with no data present (CI's
  `cargo clippy --workspace` and the no-data wasm32 build), and System-mode `fav_configure` refuses
  cleanly with `FAV_E_UNSUPPORTED` and the `FA-E0303` message `ic::WITHOUT_EMBED_HEADER` ("…built
  without embed-header…", with the fix). The page shows it in its error banner. The Rung lab
  (SW92 elements, §5 below) needs no header and still runs.
* The wasm is built with `--features embed-header` (`web/build.sh`, `tools/viewer/build.sh`, CI's
  wasm32 job after fetching the pinned file). The `.wasm` therefore contains the header and stays a
  git-ignored build artefact (`web/.gitignore`); it is for local use, never published or committed.
* The page never carries a header value: the nominal solar J2 it needs to scale `J2_OVERRIDE`
  references comes from the metadata key `j2_nominal` (§4.5).
* Tests: PR tier: with the feature, the header parses, `JDEPOC` equals the DE440 initial epoch, and
  the System-mode tests run; without it they print `SKIP … built without embed-header` and pass,
  and `without_embed_header_*` tests check the refusal. **Data tier** (`#[ignore]`, `--profile data`,
  needs `FA_DATA_DIR` and the feature): the embedded bytes equal the cached pinned file, and
  `ferroastro_propagate::initial_conditions` from them equals the result from that file bitwise
  for every body record.
* Superseded: an earlier revision committed a generated verbatim excerpt
  (`data/header440-excerpt.440`, from an example `gen_header_excerpt`). Both are deleted; the root
  `.gitignore` ignores `*.440` and `header440-excerpt*` as a backstop.
* **Migration (once #53 is on `main` and merged into `m5-performance`):** replace the local
  `build.rs` copy and `Header::parse` with #22's API: enable `ferroastro-constants/embed-header`
  from this crate's `embed-header` feature and take the constants and header from
  `De440::embedded()` (or `De440::from_header_bytes(ic::HEADER_440)`), which re-checks the SHA-256
  at run time; `ic::de440()` then also stops reading the committed constants table that #53
  removes, so the Rung lab will need the feature too.

**Asteroids (bit 10): offline, from the repo.** States: `tests/goldens/sb441-n373s-jdepoc-states.csv`
(REF-003 sampled states of every `sb441-n373s.bsp` target at JDEPOC, Sun-centred ICRF, provisional
under OI-24). It is embedded with `include_str!("../../../tests/goldens/sb441-n373s-jdepoc-states.csv")`
and parsed with `ferroastro_system::SmallBodyStates`. GMs: the `MAnnnn` tokens of the embedded header. Data
path: `DataSources::with_small_bodies`.

**Rung lab:** `ferroastro_ladder::scenario::MERCURY_SINGLE_BODY` (already `include_str!`-embedded):
SW92 J2000 mean elements, J2000 mean ecliptic and equinox, epoch J2000.0 (JD 2451545.0 TDB),
fixed Sun, DE440 GM☉ and c.

Resolved (#22): no header.440 token is committed; the earlier excerpt proposal is withdrawn.

## 6. Observables and fits (ADR-0011, ADR-0014, PHY-015)

### 6.1 Osculating ϖ, e, a (display series, §4.4 B)

From Mercury − Sun (r, v), μ = GM☉ + GM☿ (DE440):

* e and a: `ferroastro_analytic::kepler::Elements::from_vectors(r, v, μ)`.
* ϖ on the Park17 plane: `ReferencePlane::park2017_mean_orbit()?.dog_leg_angle(e_vec, h × e_vec)`
  with `e_vec = ferroastro_analytic::vector::eccentricity_vector(r, v, μ)` and `h = r × v`.
  `dog_leg_angle(x, y)` returns Ω + (angle from the node to x in the plane of x × y). Passing the
  eccentricity vector gives Ω + ω = ϖ exactly. Unit test: at a located perihelion passage it equals
  `dog_leg_angle(r, v)` to ≤ 1e-9 rad.
* ϖ_ecl: the same with `ReferencePlane::from_pole` at the ECLIPJ2000 pole.

The osculating ϖ oscillates by tens of arcsec at the planetary synodic periods. The UI plots it
lightly; the passage-based series (C) is what gets fitted.

### 6.2 Live rate (`fav_fit`)

`ferroastro_precession::park2017::fit_longitudes(t_s, longitudes, model, blocks)` on the projected
passage longitudes (C), with the same `Park2017` settings as the diffrun estimator of the PHY-012
nightly: plane = Park17 mean orbit, `Projected`, Lagrange order 8, jack-knife blocks = min(10,
n/20). Fit model from §6.3. The rate is reported **with its metadata or not at all** (PHY-015). The
UI shows ±jack-knife σ.

### 6.3 Fit model vs span

Park17's full model (B + ϖ̇t + Qt² + Table 2 periodic terms) is defined for a 2000-yr window. For
the viewer's forward runs from 1969:

* `FIT_PERIODIC = 1`: include the `park17_table2_frequencies` terms whose period ≤ span/2 (a term
  that does not complete two cycles is degenerate with the polynomial). Include Q only when
  span ≥ 500 yr. The included labels go into the `fit_model` string.
* `FIT_PERIODIC = 0`: B + ϖ̇t only (useful for showing why short-span rates wobble).
* Minimum: 20 passages (≈ 5 yr); below that `fav_fit` returns `FAV_E_FIT`.

### 6.4 Per-effect contribution table (run with − run without, ADR-0014 P-remove)

For the current toggle set S (the "full" configuration) and every effect X ∈ S:

`contribution(X) = ϖ̇(S) − ϖ̇(S \ X)` (**P-remove**, the asserted protocol of ADR-0014).

* Legs: S plus one leg per X in S, all with the same span, the same engine (§3.2), identical ICs
  (the table shows each leg's IC hash over the shared bodies; a mismatch voids the table, NUM-013),
  and **one estimator** (§6.2 on the same window). Legs are independent `fav_configure` /
  `fav_run` sequences, so the page runs them in parallel in N leg workers
  (N = `navigator.hardwareConcurrency − 1`, at least 1) while the live view keeps running.
* Columns: effect | ϖ̇(S) − ϖ̇(S\X) (″/cy) ± √(σ²+σ²) jack-knife | reference value (§8, with its
  source and frame) | difference | leg IC hash (short) | engine.
* Footer rows: the sum of contributions, ϖ̇(S), ϖ̇(Kepler-only) (its integrator floor), and the
  residual ϖ̇(S) − Σ − ϖ̇(Kepler-only) labelled "interaction (cross) terms; not an error" (Park's
  Jupiter+Saturn cross row is 0.0411 ″/cy).
* An optional "also P-add" button computes ϖ̇({☉,☿} ∪ X) − ϖ̇({☉,☿}) per X, shown as an
  **informational** column labelled "P-add (information only, ADR-0014)".
* Each row is labelled with its protocol. The table header carries the PHY-015 metadata (frame,
  epoch, span, fit model, constants version).
* The page can export the table as CSV, with the metadata as `#` comment lines.

## 7. UI (`crates/ferroastro-viewer/web/`)

Files: `index.html`, `viewer.css`, `viewer.js` (main thread), `worker.js` (wasm host), `fav.js` (the
ABI constants and a ~100-line wrapper: instantiate, call, copy buffers, decode UTF-8, all shared by
the workers). The UI also includes `smoke.mjs` (§9). Each file starts with an SPDX header (`<!-- -->`
for HTML, `//` for JS, `/* */` for CSS). There are no external URLs, fonts or CDNs. It works from
`python3 -m http.server` in that directory after copying `ferroastro_viewer.wasm` next to
`index.html` (§9). Workers are classic workers (`new Worker("worker.js")`) using `fetch` +
`WebAssembly.instantiate` (`instantiateStreaming` with a fallback when the server sends a wrong MIME
type).

### 7.1 Layout

* **Left: orbit canvas** (canvas 2D, `devicePixelRatio`-aware), top-down on the J2000 ecliptic.
  * Zoom presets: "Inner system" (±1.7 au) and "Full system" (±32 au). Mouse-wheel / pinch zoom
    about the cursor, drag to pan, double-click to reset. A scale bar is shown in au.
  * Sun at the origin. Each enabled body is a dot plus a fading trail (last K samples, K chosen so
    a trail covers about one orbit). Disabled bodies are greyed in the legend and absent from the
    canvas.
  * **Mercury**: full trail of the last ~2 orbits, its **osculating ellipse** (from §4.4 B: a, e,
    ϖ_ecl; drawn in the ecliptic projection) and the **perihelion markers**: one tick per
    passage (C) from the last M passages, colour-graded by age, plus a line Sun→latest perihelion.
    The advance shows as the markers rotating.
  * **Display exaggeration**: slider ×1 … ×10⁵ (log), default ×1. It rotates the drawn Mercury
    ellipse and perihelion markers about the Sun by (k − 1)·(ϖ(t) − ϖ(t₀)). It never changes the
    physics or any number in the chart or table. While k ≠ 1 a persistent badge on the canvas
    reads: **"Display exaggeration ×k: perihelion rotation drawn k times larger than computed"**.
    The badge is part of any screenshot.
* **Right top: ϖ(t) chart** (canvas 2D). The x-axis is years since epoch, also labelled in calendar
  years. The y-axis is ϖ − ϖ(t₀) in arcsec, Park17 frame. It draws the osculating series (thin,
  light), the passage longitudes (dots) and the fitted line from `fav_fit`. A large readout shows
  **"ϖ̇ = 575.31 ± 0.02 ″/cy"** with the span, the number of passages and a "fit model" tooltip.
  The axis auto-scales; the ϖ_ecl series can be toggled for comparison. Small multiples of e(t)
  and a(t) sit under the chart (sparklines, same x-axis).
* **Right middle: toggles panel**: one checkbox per bit of §3.1 with the effect's expected size
  next to it (§8), β/γ numeric inputs (enabled when GR or EIH is on), the engine selector showing
  the resolved engine, presets ("Kepler only", "Planets only", "Full model", "GR only"), and the
  mode switch to Rung lab (whose RUNG selector shows the SR gauge labels). Changing a toggle
  restarts the live run, after asking if more than 10 s of integration would be discarded.
* **Right bottom: contribution table** (§6.4) with "Compute contributions over [span] years" and a
  progress bar per leg.
* **Transport bar** (bottom): play/pause, restart, **time speed** (simulated years per wall-second:
  0.1, 1, 10, 100, max), current date (TDB, calendar), integrated span, steps/s, and the engine.
* **Frame caveat panel** (collapsible, "Why not 5600″/cy?") states:
  * the simulation is inertial (ICRF);
  * the observed ≈ 5600 ″/cy (Clemence 1947: 5599.74 ± 0.41, equinox of date) includes ≈ 5025 ″/cy
    of general precession of the equinoxes (Clemence's value; IAU 2006 p_A = 5028.796195 ″/cy,
    which PHY-017 marks UNVERIFIED), which no inertial-frame simulation shows;
  * frame-dependent totals: 575.31 ″/cy in the Park17 mean-orbit frame and 577.72 ″/cy in the fixed
    J2000 ecliptic (ADR-0011);
  * adding 575.31 and 5028.80 does not reproduce the equinox-referred value, because they refer to
    different planes (PHY-017 notes).

  If the panel offers a "construct the equinox-referred value" line, it shows the construction
  explicitly, cites p_A and makes no comparison tighter than ±0.41 ″/cy (PHY-017).

### 7.2 Worker protocol (main ↔ worker messages)

```
main → worker: {op:"init", wasmUrl}                → {op:"ready", abi}
               {op:"configure", mask, params:{id:value}} → {op:"configured", meta} | {op:"error", status, message}
               {op:"run", years}                   → {op:"chunk", bodies:Float64Array, elements, passages, t, status}
               {op:"fit", t0, t1}                  → {op:"fit", fit:Float64Array, meta}
               {op:"leg", mask, params, spanYears} → progress {op:"progress", t} … then {op:"legDone", fit, meta}
```

Typed arrays are posted as transferables. The live worker runs `fav_run` repeatedly while playing.

### 7.3 Time speed and responsiveness

* The worker sizes `years` per `fav_run` call adaptively so each call takes ~20–30 ms (it measures
  `performance.now()` around calls; it starts at 0.05 yr for WH and 0.02 yr for IAS15). The wasm
  side has no clock.
* Speed control: the worker sleeps between chunks (via `setTimeout`) to hold the requested
  simulated-years-per-second. At "max" it runs back to back. The main thread renders at
  `requestAnimationFrame` from the latest data; it never blocks on the worker.
* The main thread keeps only rolling windows for drawing (trails, last M passages). The full
  passage list and the elements series decimated to ≤ 20 000 points are kept for the chart.
* The live fit is refreshed at most every 0.5 s of wall time.

## 8. Expected numbers for validation

These are expectations, not new goldens. Where a value is used as a test oracle, the test loads it
by `id` from `tests/vectors/*.toml` (AGENTS.md rule 1). The viewer-specific tolerances below go
into a new `tests/vectors/viewer.toml`, each with source, citation and rationale. Values marked
"approx. literature" depend on the frame, epoch, span and estimator. Rates are in the Park17
mean-orbit frame unless stated.

| # | quantity | expectation | formula / source | tolerance (honest) |
|---|---|---|---|---|
| E1 | Kepler-only (mask 0) | ϖ̇ = 0 | two-body Newtonian (PHY-001) | \|ϖ̇\| ≤ 1e-3 ″/cy over 100 yr, either engine |
| E2 | GR 1PN, Schwarzschild, β=γ=1 | Δϖ = 6πGM☉/(c²a(1−e²)) per orbit = 5.0187e-7 rad; × 415.20 orbits/cy = **42.98 ″/cy** | PHY-004 (`PHY004-GR-RATE` 42.9805), Park17 GE 42.9799 | Rung lab GR (IAS15, 100 yr): ±0.01; System WH `gr_potential` (Sun+☿, 100 yr): ±0.002 (WP29 G1: 42.9796); P-remove GR in the full system over 200 yr: ±0.05 |
| E3 | PPN scaling | ϖ̇_GR × (2 + 2γ − β)/3 | PHY-006 | relative 1e-3 for β, γ ∈ [0, 2] (Rung lab and IAS15 System) |
| E4 | Solar J2 | planar estimate Δϖ = 3πJ₂R☉²/(a²(1−e²)²) per orbit → **0.0279 ″/cy** at the DE440 J₂ (PHY010-J2-DE440-ANALYTIC-RATE 0.02792; the header value is not reproduced, #22); 0.0286 at Park's 2.25e-7; with the real solar-pole geometry FerroAstro measures 0.02845 (J2-REAL, WP29) — "≈ 0.03 ″/cy" | PHY-010 | P-remove over ≥ 100 yr: ±0.002 ″/cy |
| E5 | Lense–Thirring (Sun) | **−0.0020 ″/cy** (Park17 −0.0020; FerroAstro LT-REAL −0.002015) | PHY-011 | P-remove over ≥ 200 yr: ±0.0005; over shorter spans it is below resolution, so the UI greys the number when \|value\| < 3σ |
| E6 | Planetary (Newtonian), P-remove, approx. literature (Park17 Table 3, 2000-yr window centred on J2000, Park frame) | Venus 277.42, Earth–Moon 90.89, Jupiter 153.99, Saturn 7.32, Mars 2.48, Uranus 0.14, Neptune 0.04; **sum ≈ 532.3 ″/cy** (N-SUM 532.30) | PHY-012/013; often quoted as ≈ 277 / 90 / 153 / 7 / 2.5, total ≈ 532 | Depends on the window. Forward 1000-yr window with Q + periodic fit: ±0.05 per planet, ±0.1 on the sum (the WP17 forward window moved Venus by ~0.007 ″/cy from the centred window). 100–200-yr window: **provisional** ±2 (Venus, Earth–Moon, Jupiter), ±0.5 (Saturn), ±0.2 (Mars), ±0.05 (Uranus, Neptune), ±5 on the sum; the validator reports measured deviations and must not tighten them without evidence |
| E7 | Full model total (bits 0–9, IAS15) | ≈ 532.30 + 42.98 + 0.028 − 0.002 ≈ **575.31 ″/cy** (Park17 575.3100; PHY-014 575.3093 for DE440 J2 with EIH + asteroids + KBOs) | ADR-0011, PHY-014 | ±0.1 ″/cy over a forward 1000-yr window (model differences: Sun-only 1PN instead of EIH, no KBOs/ring; window); ±5 over 100 yr. In the fixed J2000 ecliptic the same physics gives ≈ 577.7 ″/cy |
| E8 | Asteroids (bit 10) | ≈ 0.001 ″/cy for the first 16 numbered (Park17's 343: 0.0012, PHY-019) | PHY-019 | below the viewer's resolution for spans < ~1000 yr; check only the sign and order of magnitude (0 … 0.002) |
| E9 | SR rungs (Rung lab) | SR-a **7.1634** (GR/6); SR-b harmonic **50.144** (7/6 GR); SR-b areal **35.817** (5/6 GR) ″/cy | ADR-0012, PHY-007/008 | ±0.01 ″/cy, 1000 orbits |
| E10 | Frame caveat | observed ≈ 5600 ″/cy (5599.74 ± 0.41, Clemence 1947, equinox of date) = inertial-frame rate + ≈ 5025 ″/cy general precession (IAU 2006 p_A 5028.796 ″/cy, UNVERIFIED) | ADR-0011, PHY-017 | never a test; displayed only as the explicit construction |
| E11 | Engine agreement | WH vs IAS15 on the same WH-compatible mask (planets + J2 + GR with β=γ=1) over 100 yr | — | rates differ by ≤ 0.05 ″/cy (GR potential vs 1PN mean-motion difference plus WH phase error); report the measured value |
| E12 | Same code path | a viewer leg vs `ferroastro_diffrun` / `ferroastro_propagate::run` on the identical `ScenarioSpec` (exported via `fav_scenario`) | CI-009 | passage longitudes bitwise equal; rate equal to ≤ 1e-12 relative |
| E13 | Chunking invariance | running 100 yr as 1 call vs 400 calls of 0.25 yr | CI-010 (checkpoint/resume is bitwise) | passages bitwise identical |
| E14 | Osculating ϖ identity | `dog_leg_angle(e_vec, h×e_vec)` equals `dog_leg_angle(r, v)` at each passage | §6.1 | ≤ 1e-9 rad |

## 9. Build, CI and REUSE

* Native: `cargo test -p ferroastro-viewer` (PR tier: E1, E2-lab, E3, E4/E5 short, E9, E12, E13,
  E14, the ABI round-trip through the `ffi` functions called as plain Rust, error paths and every
  status code). Long checks (E6 1000-yr, E7) go under `#[ignore]` in the nightly profile. The
  embedded-header equality test goes in the data profile; System-mode tests need
  `--features embed-header` and skip with a message without it (§5). Tests carry `/// Verifies: PHY-0xx`.
  Propose a new requirement **API-004 "Interactive orbit viewer"** in `docs/requirements/API.md`
  (statement: the viewer is built only from FerroAstro crates, labels every rate with PHY-015
  metadata, uses P-remove for contributions and builds for wasm32 with no imports), so that
  `trace.py --check` has something to trace the ABI tests to.
* wasm: `cargo build -p ferroastro-viewer --release --target wasm32-unknown-unknown --features embed-header`
  (with the pinned header.440 fetched by `tools/fetch_jpl.py de440-header` or in `$FA_DATA_DIR`, §5) →
  `target/wasm32-unknown-unknown/release/ferroastro_viewer.wasm`. A small committed script
  `crates/ferroastro-viewer/web/build.sh` (SPDX header) runs that and copies the file into `web/`.
  The `.wasm` is a build artefact: add it to `.gitignore`, never commit it.
* CI (`.github/workflows/ci.yml`): add `-p ferroastro-precession -p ferroastro-symplectic -p
  ferroastro-ladder -p ferroastro-viewer` to the wasm32 job's build. Add a release build of the
  viewer and `node crates/ferroastro-viewer/web/smoke.mjs <wasm>`. The smoke test asserts
  `WebAssembly.Module.imports(m).length === 0` and `fav_abi_version() === 1`, configures mask 0
  (WH), runs 2 yr in 0.25-yr chunks, checks |ϖ̇| small, configures mask = GR only with
  `ENGINE=IAS15`, runs 10 yr and checks that the rate is within ±1 ″/cy of 42.98 (short span), and
  checks that an invalid toggle bit returns −2 with a message. Node is preinstalled on GitHub
  runners and needs no npm install. #22 (§5): the job first builds the viewer without
  `embed-header` (no data) and runs `smoke.mjs --expect-no-header`, which checks the
  `FAV_E_UNSUPPORTED` / `FA-E0303` "built without embed-header" refusal text the page shows; it then
  fetches the pinned header.440 and builds and smoke-tests the module with `--features embed-header`.
  The rust job runs `cargo nextest run -p ferroastro-viewer --features embed-header` for the
  System-mode tests.
* Lint: the viewer's own `[lints]` table (§2.2), `lint_canary.py` exception and pytest,
  `check_layers.py` entry and pytest. `cargo fmt`, `clippy -D warnings`, `cargo doc -D warnings`
  (every pub item documented; the ABI tables of §4 go in the crate docs), `cargo deny` (no new
  crates). REUSE: SPDX header on every new file (`.rs`, `.toml`, `.md`, `.html`, `.js`, `.mjs`,
  `.css`, `.sh`). No header.440 data is in the tree (§5).

## 10. Constraints recap and non-goals

* No new dependencies (Rust or JS), no wasm-bindgen, no npm, no CDN, no network at run time.
* No physics in JS, and none re-implemented in the viewer crate. The viewer composes
  `ferroastro-propagate` scenarios, `ferroastro-ladder` rung models and the `ferroastro-precession`
  / `ferroastro-diffrun` estimators.
* Not in scope: KBOs and the KBO ring, the Moon as a separate body (needs IAS15 geocentric
  variables; EMB is used), backward integration to reproduce Park's centred 2000-yr window
  (forward-only from JDEPOC; the table states its window), double-double precision, and issue #42
  (the LT/asteroid along-track question affects mean motion at δn/n ≈ 4e-12, not ϖ̇ at the viewer's
  resolution; the LT row's tooltip links #42 for context).

## 11. Open items for the maintainers (NEEDS HUMAN REVIEW)

1. The `unsafe_code = "deny"` lint exception for the ABI shims (§2.2; AGENTS.md rule 5, STD-003).
2. ~~Committing the verbatim header.440 excerpt~~ resolved by #22: the header is included at build
   time by the `embed-header` feature and never committed (§5); migrate to #53's API once merged.
3. The provisional short-window tolerances of E6/E7. The validator measures and reports them; they
   are not to be used as gates until a maintainer accepts calibrated values.
4. The Rung lab's untimed path into `ferroastro-ladder` (§2.4) may need a small public API addition
   in the ladder (a model builder without `Instant`).
