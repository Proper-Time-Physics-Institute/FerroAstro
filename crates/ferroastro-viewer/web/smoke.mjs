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
// Checks: zero imports; ABI version 2; error paths (E_STATE, E_ARG + FA-E message); Kepler-only
// (mask 0, WHFast) gives a near-zero rate; chunked and single-call runs give bitwise-identical
// passages (CI-010); GR only (IAS15) is within a short-span bound of PHY004-GR-RATE, loaded by id
// from tests/vectors (AGENTS.md rule 1); the metadata JSON has every documented key; bit 12 (the
// conformal-gravity comparison model, ABI v2) runs, a Sun-centred exaggerated gamma0 gives a
// retrograde advance matching the engine's own Gauss reference, and the default uniform centring
// gives ~0 (no oracle beyond the engine's analytic value; the fit values of OI-79 are never used);
// the metadata's comparison dashboard carries the M5 evidence and the INPOP15a bound from tests/vectors,
// with units; the bound markers' 3σ rule (FAV.boundVerdict) gives exceeds for a Sun-centred gamma0 and
// within for a uniform one, with the live σ from the engine's paired-difference jack-knife
// (fav_pair_*), which is at most the legs' σ combined as if independent; the glossary's
// conformal-gravity numbers (the Gauss rates at the fit values, which no vector holds) match the
// engine's reference.
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
// Conformal gravity (bit 12): an exaggerated gamma0 (cm^-1), about -60 arcsec/cy Sun-centred, so a
// 6-yr run resolves it; the bound is relative to the engine's Gauss reference (the native test,
// tests/viewer_conformal.rs, checks it to 2e-3 over 100 yr).
const CG_GAMMA0 = 1e-27;
const CG_REL_BOUND = 0.02;
const CG_UNIFORM_BOUND = 1e-6; // arcsec/cy: the P-remove of a uniform gamma0, Newtonian forces
const PT_REL_BOUND = 0.02; // Proper Time over 6 yr: the Kepler floor 0.05 arcsec/cy is 0.7 % of 7.16

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
check(eng.abi === FAV.ABI_VERSION && eng.abi === 3, "fav_abi_version() === 3", eng.abi);

// Error paths.
eng.reset();
let st = eng.x.fav_run(1.0) | 0;
check(st === FAV.STATUS.E_STATE, "fav_run before fav_configure returns E_STATE", st);
st = eng.x.fav_set_toggles(1 << 14) | 0;
const msg = eng.errorMessage();
check(st === FAV.STATUS.E_ARG, "reserved toggle bit 14 returns E_ARG", st);
st = eng.x.fav_set_toggles(1 << FAV.BIT.CONFORMAL) | 0;
check(st === FAV.STATUS.OK, "toggle bit 12 (conformal gravity, ABI v2) is accepted", st);
st = eng.x.fav_set_toggles(1 << FAV.BIT.PROPER_TIME) | 0;
check(st === FAV.STATUS.OK, "toggle bit 13 (Proper Time, ABI v3) is accepted", st);
check(eng.getParam(P.CG_GAMMA0) === FAV.PARAM_DEFAULTS[P.CG_GAMMA0] && eng.getParam(P.CG_GAMMA0_CENTRE) === 0,
  "parameters 13-16 exist with the documented defaults (uniform centring)", eng.getParam(P.CG_GAMMA0));
check(/^FA-E\d{4}/.test(msg), "error message starts with FA-Exxxx", JSON.stringify(msg.slice(0, 80)));
st = eng.x.fav_set_param(999, 1) | 0;
check(st === FAV.STATUS.E_ARG, "unknown param id returns E_ARG", st);
check(Number.isNaN(eng.getParam(999)), "fav_get_param(unknown) is NaN");
st = eng.x.fav_pair_push(2, 0, 0) | 0;
check(st === FAV.STATUS.E_ARG, "fav_pair_push to an unknown leg returns E_ARG", st);
check(eng.pair({ t: [], lon: [], mask: 0 }, { t: [], lon: [], mask: 0 }, 1, 2451545).status === FAV.STATUS.E_FIT,
  "fav_pair_fit without passages returns E_FIT (not fatal)");

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
  "epoch_jd_tdb", "time_scale", "ic_source", "ic_hash", "constants", "scenario_digest", "estimator", "fit", "j2_nominal", "conformal", "proper_time", "dashboard"];
const missing = keys.filter((k) => !cfg.meta || !(k in cfg.meta));
check(missing.length === 0, "metadata JSON has every documented key", missing.join(", ") || "all present");
check(cfg.scenario.length > 0, "fav_scenario returns the ScenarioSpec TOML", cfg.scenario.length + " bytes");
// The comparison dashboard (F4): the M5 evidence and the INPOP15a bound reach the page through the
// metadata unchanged from tests/vectors (the page carries none of them), and the engine's analytic
// GR estimate is PHY004-GR-RATE within that vector's own tolerance.
const dash = cfg.meta && cfg.meta.dashboard;
const m5v = loadVector("m5-evidence.toml", "M5-EIH-FULL-RATE");
check(dash && dash.m5 && dash.m5.kind === "computed-reference" && dash.m5.rows.total.value === m5v,
  "metadata dashboard carries the M5 total from tests/vectors/m5-evidence.toml (computed, not observed)", dash && dash.m5 ? dash.m5.rows.total.value : "missing");
const bw = loadVector("comparison-bounds.toml", "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE-INTERVAL");
check(dash && dash.bound && dash.bound.half_width === bw && dash.bound.applies_to.join() === "b" + FAV.BIT.CONFORMAL + ",b" + FAV.BIT.PROPER_TIME,
  "metadata dashboard carries the INPOP15a bound for the comparison models only", dash && dash.bound ? dash.bound.half_width + " " + dash.bound.applies_to : "missing");
check(dash && dash.bound && dash.bound.unit === "arcsec/cy" && ["m5", "literature"].every((k) => dash[k].unit === "arcsec/cy"),
  "every reference column states its unit (arcsec/cy)", dash && dash.bound ? dash.bound.unit : "missing");
// FAV.boundVerdict, the rule behind the bound markers (the page's 3σ convention on the interval).
// The first case is the one a 200-yr preview showed wrongly as unresolved: -0.183 +- 0.051 sits about
// 3.5σ outside the edge of +- 0.0031 (rule inputs only, not a physics reference).
{
  const b = { value: 0, half_width: bw };
  const cases = [
    [-0.183, 0.051, "exceeds", "far outside, σ wider than the bound (preview case)"],
    [-59.8, 5, "exceeds", "exaggerated gamma0, wide σ"],
    [0, 0.19, "unresolved", "uniform, 100 yr"],
    [-0.0001, 1e-4, "within", "inside with a small σ"],
    [0.004, 1e-4, "exceeds", "just outside with a small σ"],
    [0.004, 0.001, "unresolved", "just outside, 3σ reaches back in"],
    [6.67e-12, NaN, "within", "analytic, no σ"],
    [-0.183, NaN, "exceeds", "analytic Sun-centred, no σ"],
  ];
  for (const [v, sg, want, what] of cases) {
    const got = FAV.boundVerdict(v, sg, b);
    check(got && got.verdict === want, "bound verdict " + what + ": " + want, v + " +- " + sg + " -> " + (got ? got.verdict : "null"));
  }
}
const grAn = dash && dash.analytic && dash.analytic.rows.b8 ? dash.analytic.rows.b8.value : NaN;
check(Math.abs(grAn - loadVector("phy004-gr-analytic.toml", "PHY004-GR-RATE")) <= 1e-4, "engine analytic GR estimate matches PHY004-GR-RATE (+/- 1e-4)", grAn);

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

// Conformal gravity (bit 12), WHFast, Sun + Mercury: Sun-centred exaggerated gamma0 against the engine's
// Gauss reference, then the default uniform centring against Kepler only (same span and engine).
const cgSun = { [P.ENGINE]: FAV.ENGINE_PARAM.WHFAST, [P.SPAN_YEARS]: 100, [P.CG_GAMMA0]: CG_GAMMA0, [P.CG_GAMMA_STAR]: 0, [P.CG_KAPPA]: 0,
  [P.CG_GAMMA0_CENTRE]: FAV.CG_CENTRE.SUN };
cfg = eng.configure(1 << FAV.BIT.CONFORMAL, cgSun);
const cgMeta = cfg.meta && cfg.meta.conformal;
check(cgMeta && cgMeta.label === "comparison model" && cgMeta.enabled === true && cgMeta.gamma0_centre === "sun",
  "metadata labels bit 12 as a comparison model with its centring", cgMeta ? cgMeta.label + ", " + cgMeta.gamma0_centre : "no conformal object");
check(/comparison model/.test(cfg.meta.gauge || "") && /conformal = \{.*gamma0_centre = "sun"/.test(cfg.scenario),
  "gauge and scenario carry the comparison model", cfg.meta.gauge);
check(cfg.meta.nominal === false, "an exaggerated gamma0 marks the run non-nominal");
const cgRef = cgMeta && cgMeta.reference ? cgMeta.reference.total_arcsec_per_cy : NaN;
const cgEpoch = cfg.epochJd;
const sunRun = runChunked(eng, KEPLER_SPAN_YEARS, 1.0);
f = eng.fit(NaN, NaN);
const [kepRate, kepRun] = (() => {
  eng.configure(0, { [P.ENGINE]: FAV.ENGINE_PARAM.WHFAST, [P.SPAN_YEARS]: 100 });
  const run = runChunked(eng, KEPLER_SPAN_YEARS, 1.0);
  const k = eng.fit(NaN, NaN);
  return [k.fit ? k.fit[FAV.FIT.RATE] : NaN, run];
})();
// The live contribution's σ as the page forms it: the engine's jack-knife on the difference of
// the two legs (fav_pair_*), from the legs' passage rows.
const legCols = (run, mask) => {
  const t = [];
  const lon = [];
  for (let k = 0; k + FAV.STRIDE.passages <= run.passages.length; k += FAV.STRIDE.passages) {
    t.push(run.passages[k + FAV.PASS.T]);
    lon.push(run.passages[k + FAV.PASS.VARPI_ARCSEC]);
  }
  return { t: Float64Array.from(t), lon: Float64Array.from(lon), mask };
};
const pairWithKepler = (run) => {
  const r = eng.pair(legCols(run, 1 << FAV.BIT.CONFORMAL), legCols(kepRun, 0), FAV.PAIR_FLAG.FIT_PERIODIC, cgEpoch);
  return r.pair ? { s: r.pair[FAV.PAIR.PAIRED_SIGMA], indep: r.pair[FAV.PAIR.INDEPENDENT_SIGMA], d: r.pair[FAV.PAIR.DIFFERENCE] } : { s: NaN, indep: NaN, d: NaN };
};
const legSigmaOf = (run) => {
  const p = pairWithKepler(run);
  check(Number.isFinite(p.s) && p.s <= p.indep, "paired sigma is finite and at most the independent combination", p.s + " vs " + p.indep);
  return p.s;
};
const bound = dash && dash.bound;
if (f.fit) {
  const d = f.fit[FAV.FIT.RATE] - kepRate;
  const sSun = legSigmaOf(sunRun);
  const legSigma = () => sSun;
  // The bound marker (FAV.boundVerdict, the page's 3σ rule): a Sun-centred gamma0 far outside the
  // INPOP15a interval exceeds it even though its σ is wider than the bound's half-width.
  const vd = FAV.boundVerdict(d, legSigma(f.fit), bound);
  check(vd && vd.verdict === "exceeds", "Sun-centred live contribution exceeds the INPOP15a bound (3σ rule)",
    d + " +- " + legSigma(f.fit) + ": " + (vd ? vd.verdict : "no verdict"));
  check(FAV.boundVerdict(cgRef, NaN, bound).verdict === "exceeds", "Sun-centred analytic Gauss reference exceeds the bound", cgRef);
  check(d < 0 && cgRef < 0, "Sun-centred exaggerated gamma0 gives a retrograde advance", d + " arcsec/cy");
  check(Math.abs(d / cgRef - 1) <= CG_REL_BOUND, "Sun-centred gamma0 within " + CG_REL_BOUND * 100 + " % of the engine's Gauss reference " + cgRef,
    d + " over " + KEPLER_SPAN_YEARS + " yr");
} else {
  check(false, "fav_fit for Sun-centred gamma0", f.status + " " + (f.message || ""));
}
cfg = eng.configure(1 << FAV.BIT.CONFORMAL, Object.assign({}, cgSun, { [P.CG_GAMMA0_CENTRE]: FAV.CG_CENTRE.UNIFORM }));
check(cfg.meta.conformal.reference && cfg.meta.conformal.reference.gamma0_arcsec_per_cy === 0, "uniform centring: the reference's gamma0 share is 0");
const uniRun = runChunked(eng, KEPLER_SPAN_YEARS, 1.0);
f = eng.fit(NaN, NaN);
if (f.fit) {
  const d = f.fit[FAV.FIT.RATE] - kepRate;
  const sUni = legSigmaOf(uniRun);
  const legSigma = () => sUni;
  check(sUni <= CG_UNIFORM_BOUND, "uniform: the paired sigma is at the numerical noise floor (<= " + CG_UNIFORM_BOUND + " arcsec/cy)", sUni);
  check(Math.abs(d) <= CG_UNIFORM_BOUND, "uniform exaggerated gamma0 leaves the rate unchanged (|d| <= " + CG_UNIFORM_BOUND + " arcsec/cy)", d);
  const vd = FAV.boundVerdict(d, legSigma(f.fit), bound);
  check(vd && vd.verdict === "within", "uniform live contribution reads within the bound (paired sigma)",
    d + " +- " + legSigma(f.fit) + ": " + (vd ? vd.verdict : "no verdict"));
} else {
  check(false, "fav_fit for uniform gamma0", f.status + " " + (f.message || ""));
}

// The glossary quotes the engine's Gauss reference at the fit values (no vector holds it, so
// check-glossary.mjs cannot): the Sun-centred gamma0 term, the gamma* and kappa terms, and the
// ratio to the INPOP15a half-width.
{
  const gctx = {};
  vm.createContext(gctx);
  vm.runInContext(readFileSync(join(here, "glossary.js"), "utf8"), gctx, { filename: "glossary.js" });
  const ge = gctx.FA_GLOSSARY.entries;
  const gtext = Object.keys(ge).map((k) => [ge[k].short, ge[k].long, ge[k].mercury, ge[k].typical || ""].join("\n")).join("\n");
  const fitCfg = eng.configure(1 << FAV.BIT.CONFORMAL, { [P.ENGINE]: FAV.ENGINE_PARAM.WHFAST, [P.SPAN_YEARS]: 1, [P.CG_GAMMA0_CENTRE]: FAV.CG_CENTRE.SUN });
  const ref = fitCfg.meta && fitCfg.meta.conformal && fitCfg.meta.conformal.reference;
  check(fitCfg.meta && fitCfg.meta.nominal === true && typeof fitCfg.meta.comparison_model === "string",
    "a run at the fit values is nominal and still marked as a comparison model (not the real solar system)", fitCfg.meta && fitCfg.meta.comparison_model);
  if (ref) {
    const quotes = [
      ["Gauss −0.183 ″/cy", -0.183, 5e-4, ref.gamma0_arcsec_per_cy, "Sun-centred gamma0 term"],
      ["γ* term −3.2×10⁻¹² ″/cy", -3.2e-12, 5e-14, ref.linear_arcsec_per_cy - ref.gamma0_arcsec_per_cy, "gamma* term"],
      ["κ term +9.9×10⁻¹² ″/cy", 9.9e-12, 5e-14, ref.quadratic_arcsec_per_cy, "kappa term"],
      ["about 59× the [[inpop15a-bound]]", 59, 0.5, Math.abs(ref.gamma0_arcsec_per_cy) / bw, "ratio to the INPOP15a half-width"],
    ];
    for (const [lit, printed, tol, engine, what] of quotes) {
      check(gtext.includes(lit) && Math.abs(engine - printed) <= tol, "glossary quotes the engine's " + what + " as \"" + lit + "\"", engine);
    }
  } else {
    check(false, "the engine's conformal-gravity reference at the fit values", "null");
  }
}

// The Proper Time (Gill) comparison model (bit 13, ABI v3; PHY-021), WHFast, Sun + Mercury: the
// live contribution against the engine's analytic reference (-1/6 of GR's), the bound verdict
// (excluded grossly) and the glossary's quoted numbers; bit 13 beside GR 1PN is refused.
{
  const ptParams = { [P.ENGINE]: FAV.ENGINE_PARAM.WHFAST, [P.SPAN_YEARS]: 100 };
  cfg = eng.configure(1 << FAV.BIT.PROPER_TIME, ptParams);
  const pt = cfg.meta && cfg.meta.proper_time;
  check(pt && pt.label === "comparison model" && pt.enabled === true && pt.ppn_preset && pt.ppn_preset.beta === 0.5 && pt.ppn_preset.gamma === -1,
    "metadata labels bit 13 as a comparison model with its PPN (1/2, -1) preset", pt ? pt.label : "no proper_time object");
  check(/comparison model/.test(cfg.meta.gauge || "") && /proper_time_kernel = true/.test(cfg.scenario) && typeof cfg.meta.comparison_model === "string",
    "gauge, scenario and comparison-model note carry the Proper Time model", cfg.meta.gauge);
  const ref = pt && pt.reference ? pt.reference.arcsec_per_cy : NaN;
  check(pt && pt.reference && Math.abs(pt.reference.ratio_to_gr + 1 / 6) <= 1e-12, "the engine's reference is -1/6 of its GR rate", pt && pt.reference ? pt.reference.ratio_to_gr : "null");
  const ptLit = dash && dash.literature && dash.literature.rows["b" + FAV.BIT.PROPER_TIME];
  check(ptLit && ptLit.kind === "derivation" && Number.isFinite(ptLit.value), "the literature column carries the Proper Time derivation reference", ptLit ? ptLit.id : "missing");
  runChunked(eng, KEPLER_SPAN_YEARS, 1.0);
  const fpt = eng.fit(NaN, NaN);
  if (fpt.fit) {
    const d = fpt.fit[FAV.FIT.RATE] - kepRate;
    check(d < 0 && Math.abs(d / ref - 1) <= PT_REL_BOUND, "Proper Time live contribution within " + PT_REL_BOUND * 100 + " % of the engine's reference " + ref,
      d + " over " + KEPLER_SPAN_YEARS + " yr");
    check(FAV.boundVerdict(d, 0, bound).verdict === "exceeds" && FAV.boundVerdict(ref, NaN, bound).verdict === "exceeds",
      "the Proper Time contribution exceeds the INPOP15a bound", d);
  } else {
    check(false, "fav_fit for the Proper Time kernel", fpt.status + " " + (fpt.message || ""));
  }
  const gctx = {};
  vm.createContext(gctx);
  vm.runInContext(readFileSync(join(here, "glossary.js"), "utf8"), gctx, { filename: "glossary.js" });
  const ge = gctx.FA_GLOSSARY.entries;
  const gtext = Object.keys(ge).map((k) => [ge[k].short, ge[k].long, ge[k].mercury, ge[k].typical || ""].join("\n")).join("\n");
  for (const [lit, printed, tol, engine, what] of [
    ["−7.163 ″/cy", -7.163, 5e-4, ref, "Proper Time reference"],
    ["about 2300×", 2300, 50, Math.abs(ref) / bw, "Proper Time ratio to the INPOP15a half-width"],
  ]) {
    check(gtext.includes(lit) && Math.abs(engine - printed) <= tol, "glossary quotes the engine's " + what + " as \"" + lit + "\"", engine);
  }
  eng.reset();
  eng.setToggles((1 << FAV.BIT.PROPER_TIME) | (1 << FAV.BIT.GR_1PN));
  st = eng.x.fav_configure() | 0;
  check(st === FAV.STATUS.E_ARG && /bit 13/.test(eng.errorMessage()), "bit 13 beside GR 1PN (bit 8) returns E_ARG", st);
}

// A forced WHFast with an IAS15-only effect is refused with FA-E1201.
eng.reset();
eng.setToggles(1 << FAV.BIT.LENSE_THIRRING);
eng.setParam(P.ENGINE, FAV.ENGINE_PARAM.WHFAST);
st = eng.x.fav_configure() | 0;
check(st === FAV.STATUS.E_UNSUPPORTED && /FA-E1201/.test(eng.errorMessage()), "WHFast + Lense-Thirring returns E_UNSUPPORTED with FA-E1201", st);

console.log(failures === 0 ? "smoke: all checks passed" : "smoke: " + failures + " check(s) failed");
process.exit(failures === 0 ? 0 : 1);
