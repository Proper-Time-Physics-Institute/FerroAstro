<!-- SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# FerroAstro orbit viewer: build and serve locally

The orbit viewer is a static web page. It shows Mercury's orbit and the planets top-down, rotating perihelion
markers, the ϖ(t) chart with a least-squares advance rate in ″/cy, and the per-effect contribution table
(ADR-0014 P-remove). Each perturbation can be switched on and off. All physics runs in the FerroAstro engine,
compiled to WebAssembly from `crates/ferroastro-viewer`. The page is plain HTML, JS and CSS. It uses no npm, no
CDN and no network at run time. The design and the C ABI contract are in [DESIGN.md](DESIGN.md).

## Quick start

System mode starts from the DE440 `header.440` initial conditions. That JPL file is never committed, and neither
is any value from it (FerroAstro #22): the engine's `embed-header` feature includes the fetched, hash-pinned file
at build time (DESIGN.md §5). Fetch it once, or point `FA_DATA_DIR` at a directory that already holds it:

```sh
# from the repository root
(cd tools && uv run python fetch_jpl.py de440-header)   # once: into $FA_DATA_DIR (default target/fa-data), SHA-256 checked
# or: export FA_DATA_DIR=/path/to/dir/with/header.440
crates/ferroastro-viewer/web/build.sh            # cargo build --release --target wasm32-unknown-unknown --features embed-header, copy the .wasm into web/
crates/ferroastro-viewer/web/build.sh --smoke    # same, then run the node smoke test
crates/ferroastro-viewer/web/build.sh --serve    # same, then serve web/ on http://localhost:8000/
```

Or by hand:

```sh
rustup target add wasm32-unknown-unknown          # once
cargo build -p ferroastro-viewer --release --target wasm32-unknown-unknown --features embed-header
cp target/wasm32-unknown-unknown/release/ferroastro_viewer.wasm crates/ferroastro-viewer/web/
cd crates/ferroastro-viewer/web && python3 -m http.server 8000
# open http://localhost:8000/
```

`build.sh` asks `cargo metadata` for the target directory, so it also works when `CARGO_TARGET_DIR` or a
`build.target-dir` setting moves it. `ferroastro_viewer.wasm` is a build artefact that contains header.440.
`web/.gitignore` keeps it out of git, so never commit or publish it.

If header.440 is missing, the `embed-header` build stops with `FA-E0303` and the fetch command above; a changed
file stops it with `FA-E0304` (the SHA-256 is checked against `tests/data/MANIFEST.toml`). A module built
without `--features embed-header` (as CI's no-data build is) still loads, but System mode and the Rung lab refuse
with `FAV_E_UNSUPPORTED` and an `FA-E0303` message saying it was built without embed-header; the page shows that
message in its error banner. Since #22 (PR #53) the DE440 constants come from the embedded header.440 too
(`De440::from_header_bytes`), so the Rung lab needs the feature as well.

The engine takes its DE440 constants from the embedded bytes through #22's `De440::from_header_bytes`
(DESIGN.md §5). The build commands stay the same.

The page must be served over HTTP. Opening `index.html` as a `file://` URL does not work, because browsers do not
run Web Workers or fetch wasm from `file://`. The page detects this case and says so. Any static server works.
`python3 -m http.server` sends the wrong MIME type for `.wasm` on some systems. The page handles that by falling
back from `compileStreaming` to `arrayBuffer`. After the first load everything is local, so the page also works
offline.

Requirements: the pinned toolchain from `rust-toolchain.toml` with the `wasm32-unknown-unknown` target, and a
current browser (Chrome, Firefox or Safari; it uses ES2020, Web Workers, canvas 2D and `ResizeObserver`). The smoke
test needs node 18 or later and no packages.

## Files (`crates/ferroastro-viewer/web/`)

| file | role |
|---|---|
| `index.html` | page structure, controls, explainer, frame caveat |
| `viewer.css` | layout; light and dark colour tokens (the canvases read them too) |
| `viewer.js` | main thread: controls, orbit canvas, ϖ(t) chart, contribution table, transport bar |
| `worker.js` | Web Worker hosting one wasm instance: sizes `fav_run` chunks, copies buffers, runs table legs |
| `fav.js` | ABI v1 constants and a thin wrapper (instantiate, call, copy buffers, decode UTF-8); shared by the workers and the smoke test |
| `smoke.mjs` | node smoke test of the `.wasm` through `fav.js` (DESIGN.md §9) |
| `build.sh` | build and copy the `.wasm`; optional `--smoke`, `--serve` |

The main thread compiles the module once and posts the `WebAssembly.Module` to each worker. One live worker
drives the animation. The contribution table runs its legs in up to `hardwareConcurrency − 1` further workers (at
most 8), in parallel with the live run.

## Using it

* **Presets**: Kepler only (mask 0, the integrator floor, expect 0), Planets only, GR only, Full model (bits 0–9).
  Any toggle or parameter change restarts the live run. If more than 10 s of integration would be thrown away,
  the page asks first.
* **Engine**: AUTO picks WHFast when the toggles allow it, otherwise IAS15. The resolved engine is shown next to
  the selector, in the header and in the transport bar. Forcing WHFast with an IAS15-only effect is refused with
  the engine's FA-E1201 message.
* **Display exaggeration** (×1 … ×10⁵) rotates only the drawn ellipse and perihelion markers. A badge drawn into
  the canvas says so, so it shows up in any screenshot. **Physical** exaggeration (J2 override, solar-spin scale,
  and an ε looser than 1e-9) changes the physics. It is labelled "Exaggerated physics" and the run is marked
  non-nominal.
* **ϖ(t) chart**: dots are the perihelion-passage longitudes on the Park17 plane, which are what gets fitted. The
  grey line is the osculating ϖ, drawn but not fitted. The blue line is the fitted rate. "Subtract fitted rate"
  shows the residual wiggles. The readout's tooltip holds the fit model, formal and jack-knife σ, Q, the
  estimator and the constants version.
* **Contribution table**: "Compute" runs S, Kepler-only, and S∖X for every X in S, all with one engine, one span
  and one estimator. "also P-add" adds the information-only column. Rows within 3σ of zero are greyed. The footer
  carries Σ, ϖ̇(S), ϖ̇(Kepler) and the interaction (cross-term) residual. "Export CSV" writes the rows, with the
  run metadata as `#` lines.
* **Rung lab**: Mercury as a test particle about a fixed Sun, with the Newton / SR-a / SR-b / GR rungs (ADR-0012).
  Only the J2 and Lense–Thirring toggles apply. The table adds a row for the relativistic central term (the leg
  with RUNG = Newton).
* **Copy scenario TOML** copies the run's `ScenarioSpec` (from `fav_scenario`), so the same run can be reproduced
  with the CLI.
* Keyboard: every control is a native input. On the focused orbit canvas, `+`/`-` zoom, arrow keys pan and `0`
  resets.

Short spans give wobbly rates, because the periodic perturbations do not average out. Literature comparisons need
1000 years or more (DESIGN.md §8 E6/E7).

## Smoke test

```sh
node crates/ferroastro-viewer/web/smoke.mjs path/to/ferroastro_viewer.wasm                      # module built with embed-header
node crates/ferroastro-viewer/web/smoke.mjs --expect-no-header path/to/ferroastro_viewer.wasm   # module built without it
```

It checks that the module has zero imports, the ABI version, the error paths (E_STATE; E_ARG with an FA-E message;
E_UNSUPPORTED with FA-E1201), the buffer strides, the metadata keys, Kepler-only near zero over 6 yr (WHFast),
bitwise-identical passages when the run is chunked or done in one call, and GR only (IAS15) within ±1 ″/cy of
`PHY004-GR-RATE` over 10 yr. That value is read by id from `tests/vectors/phy004-gr-analytic.toml`. With
`--expect-no-header` it checks instead that System mode refuses with `FAV_E_UNSUPPORTED`, that the page's error
text names `FA-E0303` and "built without embed-header", and that the Rung lab is refused the same way.

## Contract readings made by the UI

Where the ABI text left a choice open, the UI reads it as follows:

* `fav_fit(NaN, NaN)` fits every passage since `fav_configure`, including passages already drained. The engine
  keeps its own passage list, and the drain buffers only feed the display.
* Rung lab sends only bits 7 (J2) and 9 (Lense–Thirring). GR is chosen by `RUNG`, so bit 8 is not set there.
* B's time origin is not specified, so the chart does not use B. It draws the fitted slope through the centroid of
  the fitted passages.
* The table's IC-hash column shows each leg's `meta.ic_hash`. That hash covers the leg's own body set, so equality
  across legs cannot be checked from it alone, and the table shows the hashes without voiding anything.
* The transport bar shows simulated years per wall-clock second, not steps/s, because the ABI exposes no step
  count.
* The live run's reached time can exceed the request by up to one integrator step: the engine advances in whole
  steps.
