// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// Node smoke test of the viewer's wasm module through the same JS glue the page uses (fav.js).
// docs/viewer/DESIGN.md section 9. No npm packages: node >= 18 built-ins only.
//
//   node crates/ferroastro-viewer/web/smoke.mjs [path/to/ferroastro_viewer.wasm]
//   node crates/ferroastro-viewer/web/smoke.mjs --expect-no-header [path/to/ferroastro_viewer.wasm]
//
// The module must be built with `--features embed-header` (web/build.sh), which includes the
// fetched JPL header.440 for the System-mode initial conditions (DESIGN section 5, #22). With
// --expect-no-header it instead checks a module built WITHOUT the feature: System-mode
// fav_configure returns E_UNSUPPORTED, the text the page shows (FAV.describeError, as worker.js
// posts it to viewer.js's error banner) names FA-E0303 and "built without embed-header", and the
// Rung lab is refused the same way (its DE440 constants come from the header too since #22/#53).
//
// Checks: zero imports; ABI version 1; error paths (E_STATE, E_ARG + FA-E message); Kepler-only
// (mask 0, WHFast) gives a near-zero rate; chunked and single-call runs give bitwise-identical
// passages (CI-010); GR only (IAS15) is within a short-span bound of PHY004-GR-RATE, loaded by id
// from tests/vectors (AGENTS.md rule 1); the metadata JSON has every documented key.
// Verifies: API-004 (proposed in DESIGN.md section 9)

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
vm.runInThisContext(readFileSync(join(here, "fav.js"), "utf8"), { filename: "fav.js" });
const FAV = globalThis.FAV;
const P = FAV.PARAM;

// Smoke bounds (not validation tolerances; the validated ones live in tests/vectors, DESIGN.md section 8).
const KEPLER_SPAN_YEARS = 6; // 20 passages minimum for a fit (section 6.3) is about 5 years
const KEPLER_BOUND = 0.05; // arcsec/cy over 6 yr; E1's 1e-3 applies to 100 yr
const GR_SPAN_YEARS = 10;
const GR_BOUND = 1.0; // arcsec/cy over 10 yr (section 9)

let failures = 0;
function check(cond, what, detail) {
  console.log((cond ? "ok   " : "FAIL ") + what + (detail !== undefined ? ": " + detail : ""));
  if (!cond) failures++;
}

function loadVector(file, id) {
  const text = readFileSync(join(here, "..", "..", "..", "tests", "vectors", file), "utf8");
  const at = text.indexOf('id = "' + id + '"');
  if (at < 0) throw new Error(id + " not found in " + file);
  const m = /\nvalue\s*=\s*"?([-+0-9.eE]+)"?/.exec(text.slice(at));
  if (!m) throw new Error("no value for " + id);
  return Number(m[1]);
}

function runChunked(eng, years, chunk) {
  const passages = [];
  let bodies = 0;
  let elements = 0;
  let last = null;
  let status = FAV.STATUS.OK;
  for (let t = 0; t < years - 1e-12 && status !== FAV.STATUS.DONE; t += chunk) {
    status = eng.run(Math.min(chunk, years - t));
    const c = eng.takeChunk();
    passages.push(...c.passages);
    bodies += c.bodies.length;
    elements += c.elements.length;
    if (c.bodies.length) last = c.bodies.slice(c.bodies.length - FAV.STRIDE.bodies);
  }
  return { passages: Float64Array.from(passages), bodies, elements, last };
}

const args = process.argv.slice(2);
const expectNoHeader = args.includes("--expect-no-header");
const wasmPath = args.find((a) => !a.startsWith("--")) || join(here, "ferroastro_viewer.wasm");
const module = await WebAssembly.compile(readFileSync(wasmPath));
check(WebAssembly.Module.imports(module).length === 0, "module has no imports", WebAssembly.Module.imports(module).length);
const eng = await FAV.instantiate(module);
check(eng.abi === FAV.ABI_VERSION, "fav_abi_version() === 1", eng.abi);

// Error paths.
eng.reset();
let st = eng.x.fav_run(1.0) | 0;
check(st === FAV.STATUS.E_STATE, "fav_run before fav_configure returns E_STATE", st);
st = eng.x.fav_set_toggles(1 << 12) | 0;
const msg = eng.errorMessage();
check(st === FAV.STATUS.E_ARG, "reserved toggle bit returns E_ARG", st);
check(/^FA-E\d{4}/.test(msg), "error message starts with FA-Exxxx", JSON.stringify(msg.slice(0, 80)));
st = eng.x.fav_set_param(999, 1) | 0;
check(st === FAV.STATUS.E_ARG, "unknown param id returns E_ARG", st);
check(Number.isNaN(eng.getParam(999)), "fav_get_param(unknown) is NaN");

// header.440 (#22): is it embedded? A build without embed-header refuses System mode.
let noHeader = null;
try {
  eng.configure(0, { [P.SPAN_YEARS]: 1 });
} catch (err) {
  noHeader = err;
}
const shown = noHeader ? FAV.describeError(noHeader) : "";
if (expectNoHeader) {
  check(noHeader !== null && noHeader.status === FAV.STATUS.E_UNSUPPORTED, "built without embed-header: System-mode fav_configure returns E_UNSUPPORTED",
    noHeader ? noHeader.status : "it configured");
  check(/^FAV_E_UNSUPPORTED from fav_configure: FA-E0303: .*built without embed-header/.test(shown),
    "the page's error text names FA-E0303 and 'built without embed-header'", JSON.stringify(shown.slice(0, 110)));
  let labErr = null;
  try {
    eng.configure(0, { [P.MODE]: FAV.MODE.RUNG_LAB, [P.SPAN_YEARS]: 1 });
  } catch (err) {
    labErr = err;
  }
  check(labErr !== null && labErr.status === FAV.STATUS.E_UNSUPPORTED, "built without embed-header: Rung-lab fav_configure returns E_UNSUPPORTED",
    labErr ? labErr.status : "it configured");
  check(labErr !== null && /^FAV_E_UNSUPPORTED from fav_configure: FA-E0303: .*built without embed-header/.test(FAV.describeError(labErr)),
    "the Rung-lab error text names FA-E0303 and 'built without embed-header'", labErr ? JSON.stringify(FAV.describeError(labErr).slice(0, 110)) : "");
  console.log(failures === 0 ? "smoke (no embed-header): all checks passed" : "smoke (no embed-header): " + failures + " check(s) failed");
  process.exit(failures === 0 ? 0 : 1);
}
if (noHeader) {
  check(false, "System mode configures (module built with --features embed-header)", shown);
  process.exit(1);
}
eng.reset();
check(eng.getParam(P.SPAN_YEARS) === 1000 && eng.getParam(P.WH_STEP_DAYS) === 1, "fav_reset restores defaults");

// Kepler only, WHFast.
const kParams = { [P.ENGINE]: FAV.ENGINE_PARAM.WHFAST, [P.SPAN_YEARS]: 100 };
let cfg = eng.configure(0, kParams);
check(cfg.engine === FAV.ENGINE_RESOLVED.WHFAST, "mask 0 with ENGINE=WHFAST resolves to WHFast", cfg.engine);
check(cfg.epochJd === 2440400.5, "System-mode epoch is JDEPOC 2440400.5", cfg.epochJd);
const keys = ["abi", "mode", "engine", "nominal", "toggles", "params", "model", "gauge", "frame", "display_frame",
  "epoch_jd_tdb", "time_scale", "ic_source", "ic_hash", "constants", "scenario_digest", "estimator", "fit", "j2_nominal"];
const missing = keys.filter((k) => !cfg.meta || !(k in cfg.meta));
check(missing.length === 0, "metadata JSON has every documented key", missing.join(", ") || "all present");
check(cfg.scenario.length > 0, "fav_scenario returns the ScenarioSpec TOML", cfg.scenario.length + " bytes");

let t0 = performance.now();
const chunked = runChunked(eng, KEPLER_SPAN_YEARS, 0.25);
const kMs = performance.now() - t0;
check(chunked.bodies % FAV.STRIDE.bodies === 0 && chunked.bodies > 0, "bodies buffer is a whole number of 25-f64 records", chunked.bodies);
check(chunked.elements % FAV.STRIDE.elements === 0 && chunked.elements > 0, "elements buffer is a whole number of 6-f64 records", chunked.elements);
check(chunked.last && Number.isFinite(chunked.last[1]) && Number.isNaN(chunked.last[4]), "Mercury slot set, Venus slot NaN when off");
// The engine advances in whole integrator steps, so the reached time may exceed the request by up to
// one step (WHFast h = 1 d here).
const stepYears = eng.getParam(P.WH_STEP_DAYS) / 365.25;
const dt = eng.timeYears() - KEPLER_SPAN_YEARS;
check(dt >= -1e-9 && dt <= stepYears + 1e-9, "fav_time_years within one step of the requested span", eng.timeYears());
let f = eng.fit(NaN, NaN);
check(f.status === FAV.STATUS.OK && f.fit.length === FAV.STRIDE.fit, "fav_fit over all passages", f.status + " " + (f.message || ""));
if (f.fit) {
  const r = f.fit[FAV.FIT.RATE];
  check(Math.abs(r) <= KEPLER_BOUND, "Kepler-only rate within " + KEPLER_BOUND + " arcsec/cy over " + KEPLER_SPAN_YEARS + " yr",
    r + " +- " + f.fit[FAV.FIT.JACKKNIFE_SIGMA] + " (" + f.fit[FAV.FIT.N_PASSAGES] + " passages)");
}
console.log("info WHFast Sun+Mercury: " + (KEPLER_SPAN_YEARS / (kMs / 1000)).toFixed(1) + " sim yr/s");

// Chunking invariance (CI-010): same configuration as one call.
eng.configure(0, kParams);
const single = runChunked(eng, KEPLER_SPAN_YEARS, KEPLER_SPAN_YEARS);
let same = single.passages.length === chunked.passages.length && single.passages.length > 0;
for (let i = 0; same && i < single.passages.length; i++) {
  same = Object.is(single.passages[i], chunked.passages[i]);
}
check(same, "chunked and single-call passages are bitwise identical", single.passages.length / FAV.STRIDE.passages + " passages");

// GR only, IAS15.
const grRate = loadVector("phy004-gr-analytic.toml", "PHY004-GR-RATE");
cfg = eng.configure(1 << FAV.BIT.GR_1PN, { [P.ENGINE]: FAV.ENGINE_PARAM.IAS15, [P.SPAN_YEARS]: 100 });
check(cfg.engine === FAV.ENGINE_RESOLVED.IAS15, "GR only with ENGINE=IAS15 resolves to IAS15", cfg.engine);
t0 = performance.now();
runChunked(eng, GR_SPAN_YEARS, 1.0);
const gMs = performance.now() - t0;
f = eng.fit(NaN, NaN);
check(f.status === FAV.STATUS.OK, "fav_fit for GR only", f.status + " " + (f.message || ""));
if (f.fit) {
  const r = f.fit[FAV.FIT.RATE];
  check(Math.abs(r - grRate) <= GR_BOUND, "GR-only rate within " + GR_BOUND + " of PHY004-GR-RATE " + grRate + " over " + GR_SPAN_YEARS + " yr",
    r + " +- " + f.fit[FAV.FIT.JACKKNIFE_SIGMA]);
  check(f.meta && f.meta.fit && typeof f.meta.fit.model === "string", "metadata carries the fit model after fav_fit", f.meta && f.meta.fit ? f.meta.fit.model : "null");
}
console.log("info IAS15 Sun+Mercury+GR: " + (GR_SPAN_YEARS / (gMs / 1000)).toFixed(1) + " sim yr/s");

// A forced WHFast with an IAS15-only effect is refused with FA-E1201.
eng.reset();
eng.setToggles(1 << FAV.BIT.LENSE_THIRRING);
eng.setParam(P.ENGINE, FAV.ENGINE_PARAM.WHFAST);
st = eng.x.fav_configure() | 0;
check(st === FAV.STATUS.E_UNSUPPORTED && /FA-E1201/.test(eng.errorMessage()), "WHFast + Lense-Thirring returns E_UNSUPPORTED with FA-E1201", st);

console.log(failures === 0 ? "smoke: all checks passed" : "smoke: " + failures + " check(s) failed");
process.exit(failures === 0 ? 0 : 1);
