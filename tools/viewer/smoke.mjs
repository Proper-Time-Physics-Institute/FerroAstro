// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// Engine-level node smoke test of ferroastro_viewer.wasm through the raw ABI v1 exports, with no
// JS glue (docs/viewer/DESIGN.md sections 4 and 9; the page-level test through fav.js is
// crates/ferroastro-viewer/web/smoke.mjs). Node >= 18 built-ins only; no npm packages.
//
//   node tools/viewer/smoke.mjs [path/to/ferroastro_viewer.wasm]
//
// The module must be built with `--features embed-header` (tools/viewer/build.sh), which includes
// the fetched JPL header.440 for the System-mode initial conditions (DESIGN section 5, #22); a
// module built without it fails the first check below with its FA-E0303 message.
//
// Checks: zero imports; every ABI export present; fav_abi_version() === 1; E_STATE / E_ARG /
// E_UNSUPPORTED / E_FIT with FA-E messages; Kepler only (mask 0, WHFast) runs in 0.25-yr chunks to
// FAV_DONE with whole buffer records and a near-zero rate; GR only with IAS15 and the Rung-lab GR
// rung give rates within a short-span bound of PHY004-GR-RATE (loaded by id from tests/vectors,
// AGENTS.md rule 1). Exit code 0 when every check passes.
// Verifies: API-004

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const wasmPath =
  process.argv[2] || join(root, "crates", "ferroastro-viewer", "web", "ferroastro_viewer.wasm");

// ABI v1 constants (crates/ferroastro-viewer/src/lib.rs).
const OK = 0, DONE = 1, E_STATE = -1, E_ARG = -2, E_UNSUPPORTED = -3, E_FIT = -5;
const P = { MODE: 0, ENGINE: 1, SPAN_YEARS: 6 };
const BIT = { GR_1PN: 1 << 8, LENSE_THIRRING: 1 << 9 };
const STRIDE = { bodies: 25, elements: 6, passages: 6, fit: 10 };
const EXPORTS = [
  "fav_abi_version", "fav_reset", "fav_set_toggles", "fav_set_param", "fav_get_param",
  "fav_configure", "fav_run", "fav_time_years", "fav_epoch_jd", "fav_engine", "fav_drain",
  "fav_bodies_ptr", "fav_bodies_len", "fav_elements_ptr", "fav_elements_len", "fav_passages_ptr",
  "fav_passages_len", "fav_fit", "fav_fit_ptr", "fav_fit_len", "fav_meta_ptr", "fav_meta_len",
  "fav_scenario_ptr", "fav_scenario_len", "fav_error_ptr", "fav_error_len",
];
// Smoke bounds, not validation tolerances (those are in tests/vectors/viewer.toml).
const KEPLER_BOUND = 0.05; // arcsec/cy over 6 yr
const GR_BOUND = 1.0; // arcsec/cy over 10 yr (DESIGN section 9)

let failures = 0;
function check(cond, what, detail) {
  console.log((cond ? "ok   " : "FAIL ") + what + (detail === undefined ? "" : ": " + detail));
  if (!cond) failures++;
}

function vector(file, id) {
  const text = readFileSync(join(root, "tests", "vectors", file), "utf8");
  const at = text.indexOf(`id = "${id}"`);
  const m = at < 0 ? null : /\nvalue = "?([-+0-9.eE]+)"?/.exec(text.slice(at));
  if (!m) throw new Error(`${id} not found in ${file}`);
  return Number(m[1]);
}

const module = new WebAssembly.Module(readFileSync(wasmPath));
const imports = WebAssembly.Module.imports(module);
check(imports.length === 0, "module has no imports", imports.length);
const x = new WebAssembly.Instance(module, {}).exports;
const missing = EXPORTS.filter((n) => typeof x[n] !== "function");
check(missing.length === 0 && x.memory instanceof WebAssembly.Memory, "every ABI v1 export", missing.join(", ") || "all present");
check(x.fav_abi_version() === 1, "fav_abi_version() === 1", x.fav_abi_version());

// A pointer is valid until the next fav_* call other than its _len partner: fresh view, copy now.
function f64s(name) {
  const ptr = x[`fav_${name}_ptr`]() >>> 0;
  const len = x[`fav_${name}_len`]() >>> 0;
  return new Float64Array(x.memory.buffer, ptr, len).slice();
}
function text(name) {
  const ptr = x[`fav_${name}_ptr`]() >>> 0;
  const len = x[`fav_${name}_len`]() >>> 0;
  return new TextDecoder().decode(new Uint8Array(x.memory.buffer, ptr, len).slice());
}

function configure(mask, params) {
  x.fav_reset();
  if (x.fav_set_toggles(mask) !== OK) throw new Error(text("error"));
  for (const [id, v] of Object.entries(params)) {
    if (x.fav_set_param(Number(id), v) !== OK) throw new Error(text("error"));
  }
  return x.fav_configure();
}

function runAll(chunk) {
  const got = { bodies: 0, elements: 0, passages: 0 };
  let status = OK;
  let calls = 0;
  const t0 = performance.now();
  while (status === OK) {
    status = x.fav_run(chunk);
    calls++;
    for (const k of Object.keys(got)) {
      const b = f64s(k);
      if (b.length % STRIDE[k] !== 0) throw new Error(`${k} buffer length ${b.length}`);
      got[k] += b.length / STRIDE[k];
    }
    x.fav_drain();
  }
  return { status, calls, got, seconds: (performance.now() - t0) / 1000 };
}

// header.440 (#22): a build without embed-header refuses System mode (E_UNSUPPORTED, FA-E0303).
if (configure(0, { [P.SPAN_YEARS]: 6 }) === E_UNSUPPORTED && /built without embed-header/.test(text("error"))) {
  check(false, "System mode configures (rebuild with --features embed-header)", text("error").slice(0, 120));
  process.exit(1);
}
x.fav_reset();

// Error paths.
check(x.fav_run(1) === E_STATE, "fav_run before fav_configure is E_STATE");
check(x.fav_set_toggles(1 << 12) === E_ARG, "reserved toggle bit is E_ARG");
check(/^FA-E\d{4}/.test(text("error")), "error message starts with FA-Exxxx", text("error").slice(0, 60));
check(x.fav_set_param(99, 0) === E_ARG && Number.isNaN(x.fav_get_param(99)), "unknown parameter id is E_ARG / NaN");
check(configure(BIT.LENSE_THIRRING, { [P.ENGINE]: 1 }) === E_UNSUPPORTED && text("error").startsWith("FA-E1201"),
  "WHFast + Lense-Thirring is E_UNSUPPORTED (FA-E1201)", text("error").slice(0, 60));

// Kepler only, WHFast, 6 yr in 0.25-yr chunks.
check(configure(0, { [P.SPAN_YEARS]: 6 }) === OK, "configure mask 0", x.fav_engine());
check(x.fav_engine() === 1 && x.fav_epoch_jd() === vector("de440-constants.toml", "DE440-JDEPOC"),
  "WHFast at JDEPOC", `${x.fav_engine()} ${x.fav_epoch_jd()}`);
x.fav_run(0.25);
check(x.fav_fit(NaN, NaN) === E_FIT, "fav_fit with too few passages is E_FIT", text("error").slice(0, 50));
let r = runAll(0.25);
check(r.status === DONE && r.calls === 23, "Kepler only runs to FAV_DONE", `${r.calls + 1} calls, ${JSON.stringify(r.got)}`);
check(x.fav_run(0.25) === DONE, "fav_run after the span is FAV_DONE");
check(x.fav_fit(NaN, NaN) === OK, "fav_fit Kepler only", text("error"));
let fit = f64s("fit");
check(fit.length === STRIDE.fit && Math.abs(fit[0]) < KEPLER_BOUND, "Kepler-only rate near zero", `${fit[0]} arcsec/cy, ${fit[5]} passages`);
const meta = JSON.parse(text("meta"));
check(meta.abi === 1 && meta.toggles === 0 && typeof meta.ic_hash === "string" && meta.fit !== null, "metadata JSON", meta.engine);

// GR only, IAS15, 10 yr; the Rung-lab GR rung, 10 yr.
const gr = vector("phy004-gr-analytic.toml", "PHY004-GR-RATE");
for (const [label, mask, params] of [
  ["System GR 1PN, IAS15", BIT.GR_1PN, { [P.ENGINE]: 2, [P.SPAN_YEARS]: 10 }],
  ["Rung lab GR PPN", 0, { [P.MODE]: 1, [P.SPAN_YEARS]: 10 }],
]) {
  check(configure(mask, params) === OK, `configure ${label}`, text("error"));
  r = runAll(1);
  check(r.status === DONE && x.fav_fit(NaN, NaN) === OK, `${label}: run and fit`, text("error"));
  fit = f64s("fit");
  check(Math.abs(fit[0] - gr) <= GR_BOUND, `${label}: rate within ${GR_BOUND} of PHY004-GR-RATE ${gr}`, fit[0]);
  console.log(`info ${label}: ${(10 / r.seconds).toFixed(1)} simulated yr/s (engine ${x.fav_engine()})`);
}

console.log(failures === 0 ? "smoke: all checks passed" : `smoke: ${failures} check(s) failed`);
process.exit(failures === 0 ? 0 : 1);
