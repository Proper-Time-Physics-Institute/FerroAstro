// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// Static check of the viewer's explanatory text (glossary.js) against the page that uses it.
// No npm packages and no wasm: node >= 18 built-ins only.
//
//   node crates/ferroastro-viewer/web/check-glossary.mjs
//
// Checks:
//  * every glossary entry has a term, a known group, and non-empty short, long and mercury texts
//    (and a non-empty typical text when it has one); short texts stay tooltip-sized;
//  * every "see" id and every [[id]] / [[id|text]] link inside an entry names an existing entry;
//  * every data-term / data-tip in index.html names an existing entry, and every info button
//    (class="tip") carries a data-tip;
//  * every glossary id that viewer.js puts into generated markup (termButton, tipHtml, termHtml,
//    `term: "..."` fields, dataset.tip / dataset.term assignments) names an existing entry;
//  * physics numbers quoted in glossary.js agree with tests/vectors (read with a minimal TOML
//    reader, no dependencies): every row of VECTOR_QUOTES must appear in the text and equal its
//    vector rounded to the printed digits, and any other number of four or more significant digits
//    that lies within 1e-4 (relative) of a vector value without rounding to one is drift.
//    Numbers the engine computes rather than reads from a vector (the conformal-gravity Gauss
//    rates) are checked against the wasm engine by smoke.mjs.
// It prints the coverage counts and exits 1 on any failure.

import { readFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const here = dirname(fileURLToPath(import.meta.url));
const read = (f) => readFileSync(join(here, f), "utf8");

const ctx = {};
vm.createContext(ctx);
vm.runInContext(read("glossary.js"), ctx, { filename: "glossary.js" });
const G = ctx.FA_GLOSSARY;

let failures = 0;
const fail = (msg) => {
  failures++;
  console.error("FAIL " + msg);
};

if (!G || !G.entries || !G.groups) {
  console.error("FAIL glossary.js does not define FA_GLOSSARY.{groups, entries}");
  process.exit(1);
}

const E = G.entries;
const ids = Object.keys(E);
const groupIds = new Set(G.groups.map((g) => g.id));
const LINK = /\[\[([^\]|]+)(?:\|([^\]]*))?\]\]/g;
const SHORT_MAX = 340; // characters: one or two sentences in a tooltip

const nonEmpty = (v) => typeof v === "string" && v.trim().length > 0;

for (const id of ids) {
  const e = E[id];
  if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(id)) fail(`entry id "${id}" is not kebab-case`);
  if (!nonEmpty(e.term)) fail(`${id}: empty term`);
  if (!groupIds.has(e.group)) fail(`${id}: unknown group "${e.group}"`);
  for (const k of ["short", "long", "mercury"]) if (!nonEmpty(e[k])) fail(`${id}: empty ${k}`);
  if ("typical" in e && !nonEmpty(e.typical)) fail(`${id}: typical is present but empty`);
  if ("sym" in e && !nonEmpty(e.sym)) fail(`${id}: sym is present but empty`);
  if (nonEmpty(e.short) && e.short.length > SHORT_MAX) fail(`${id}: short text is ${e.short.length} characters (max ${SHORT_MAX})`);
  for (const k of ["short", "long", "mercury", "typical"]) {
    if (!nonEmpty(e[k])) continue;
    for (const m of e[k].matchAll(LINK)) {
      if (!E[m[1]]) fail(`${id}.${k}: link [[${m[1]}]] names no entry`);
      if (m[2] !== undefined && !nonEmpty(m[2])) fail(`${id}.${k}: link [[${m[1]}|]] has empty text`);
    }
    if (/\[\[|\]\]/.test(e[k].replace(LINK, ""))) fail(`${id}.${k}: unbalanced [[ ]] markup`);
  }
  if (e.see !== undefined) {
    if (!Array.isArray(e.see)) fail(`${id}: see is not an array`);
    else
      for (const s of e.see) {
        if (!E[s]) fail(`${id}: see "${s}" names no entry`);
        if (s === id) fail(`${id}: see lists itself`);
      }
  }
}

// ---------------------------------------------------------------- index.html
const html = read("index.html");
const used = new Map(); // id -> number of references (page + generated)
const use = (id, where) => {
  if (!E[id]) fail(`${where}: "${id}" names no glossary entry`);
  used.set(id, (used.get(id) || 0) + 1);
};
let htmlTerm = 0;
let htmlTip = 0;
for (const m of html.matchAll(/data-(term|tip)="([^"]*)"/g)) {
  if (m[1] === "term") htmlTerm++;
  else htmlTip++;
  use(m[2], `index.html data-${m[1]}`);
}
for (const m of html.matchAll(/<button\b[^>]*\bclass="tip"[^>]*>/g)) {
  if (!/\bdata-tip="/.test(m[0])) fail(`index.html: info button without data-tip: ${m[0]}`);
}
for (const m of html.matchAll(/data-glossary-open="([^"]+)"/g)) use(m[1], "index.html data-glossary-open");
if (!/<dialog\b[^>]*id="glossary"/.test(html)) fail("index.html: no <dialog id=\"glossary\">");
for (const f of ["glossary.js", "glossary-ui.js"]) {
  if (!html.includes(`<script src="${f}"></script>`)) fail(`index.html does not load ${f}`);
}
if (html.indexOf('src="glossary.js"') > html.indexOf('src="glossary-ui.js"')) fail("index.html must load glossary.js before glossary-ui.js");

// ---------------------------------------------------------------- viewer.js (generated markup)
const js = read("viewer.js");
let jsRefs = 0;
const jsUse = (id, where) => {
  jsRefs++;
  use(id, where);
};
for (const m of js.matchAll(/\b(termButton|tipHtml|termHtml)\(\s*"([^"]+)"/g)) jsUse(m[2], `viewer.js ${m[1]}`);
// A ternary first argument: termButton(cond ? "a" : "b", ...).
for (const m of js.matchAll(/\b(?:termButton|tipHtml|termHtml)\(\s*[^,"()]+\?\s*"([^"]+)"\s*:\s*"([^"]+)"/g)) {
  jsUse(m[1], "viewer.js ternary term");
  jsUse(m[2], "viewer.js ternary term");
}
for (const m of js.matchAll(/[{,]\s*term:\s*"([^"]*)"/g)) jsUse(m[1], "viewer.js term field");
for (const m of js.matchAll(/dataset\.(?:tip|term)\s*=\s*([^;]+);/g)) {
  for (const lit of m[1].matchAll(/"([^"]+)"/g)) jsUse(lit[1], "viewer.js dataset assignment");
}
const effectTermFn = /function effectTerm\(key\) \{([\s\S]*?)\n {2}\}/.exec(js);
if (!effectTermFn) fail("viewer.js: effectTerm() not found");
else for (const m of effectTermFn[1].matchAll(/return "([^"]+)"/g)) jsUse(m[1], "viewer.js effectTerm");
// Every effect toggle needs its glossary id.
const toggleBlock = /const TOGGLES = \[([\s\S]*?)\];/.exec(js);
if (toggleBlock) {
  const rows = toggleBlock[1].split("\n").filter((l) => /\{ bit:/.test(l));
  for (const r of rows) if (!/\bterm:\s*"/.test(r)) fail(`viewer.js TOGGLES row without term: ${r.trim()}`);
  // ABI v2: bits 0-12 (bit 12 is the conformal-gravity comparison model, ADR-0016).
  if (rows.length !== 13) fail(`viewer.js TOGGLES has ${rows.length} rows, expected 13 (bits 0-12)`);
  // A comparison model must say so in its toggle label (ADR-0016, PHY-020).
  for (const r of rows) if (/\bcomparison:\s*true\b/.test(r) && !/label:\s*"[^"]*\(comparison model\)"/.test(r)) fail(`viewer.js comparison-model TOGGLES row without "(comparison model)" in its label: ${r.trim()}`);
}

// ---------------------------------------------------------------- numbers against tests/vectors
// A minimal reader of the ADR-0004 vector files: each [[vector]] table's id and a numeric value
// (a bare number or a quoted decimal string); other keys and non-numeric values are ignored.
const vecDir = join(here, "..", "..", "..", "tests", "vectors");
const VEC = new Map();
for (const f of readdirSync(vecDir).filter((n) => n.endsWith(".toml"))) {
  const text = readFileSync(join(vecDir, f), "utf8");
  for (const block of text.split(/^\[\[vector\]\][ \t]*$/m).slice(1)) {
    const id = /^id\s*=\s*"([^"]+)"/m.exec(block);
    const v = /^value\s*=\s*"?([-+]?[0-9.]+(?:[eE][-+]?\d+)?)"?[ \t]*(?:#.*)?$/m.exec(block);
    if (id && v && Number.isFinite(Number(v[1]))) VEC.set(id[1], { value: Number(v[1]), file: f });
  }
}
if (VEC.size < 100) fail(`tests/vectors: read only ${VEC.size} numeric vectors (expected hundreds)`);

// The glossary's whole text, with the true minus sign and ×10⁻ⁿ superscripts made machine-readable.
const SUP = { "⁰": "0", "¹": "1", "²": "2", "³": "3", "⁴": "4", "⁵": "5", "⁶": "6", "⁷": "7", "⁸": "8", "⁹": "9", "⁻": "-" };
const plain = (t) => t.replace(/×10([⁻⁰¹²³⁴⁵⁶⁷⁸⁹]+)/g, (_, e) => "e" + [...e].map((c) => SUP[c]).join("")).replace(/−/g, "-");
const gText = ids.map((id) => ["term", "short", "long", "mercury", "typical"].map((k) => E[id][k] || "").join("\n")).join("\n");
const gPlain = plain(gText);

// Printed `lit` (as it appears in glossary.js) equals vector `id` × `scale` rounded to the printed
// digits: half a unit in its last digit.
const printedTol = (s) => {
  const m = /^[-+]?(\d*)(?:\.(\d+))?(?:e([-+]?\d+))?$/.exec(s);
  if (!m) return NaN;
  return 0.5 * 10 ** (-(m[2] || "").length + Number(m[3] || 0));
};
// [quoted text in glossary.js, its number, vector id, scale from the vector's unit]
const VECTOR_QUOTES = [
  ["42.980114", "42.980114", "M5-GR-SUN-MERCURY-RATE", 1],
  ["575.309161", "575.309161", "M5-EIH-FULL-RATE", 1],
  ["575.309161 ± 0.0015", "0.0015", "M5-EIH-FULL-RATE-JK-SIGMA", 1],
  ["J2 +0.02845", "0.02845", "M5-J2-PREMOVE-RATE", 1],
  ["LT −0.00201", "-0.00201", "M5-LT-PREMOVE-RATE", 1],
  ["575.3093 ± 0.005", "575.3093", "PHY014-EIH-FULL-RATE", 1],
  ["Park17's 575.3100", "575.3100", "PARK17-TOTAL-RATE", 1],
  ["analytic 42.9805", "42.9805", "PHY004-GR-RATE", 1],
  ["Park17's 42.9799", "42.9799", "PARK17-GE-RATE", 1],
  ["277.4176", "277.4176", "PARK17-T3-VENUS", 1],
  ["153.9899", "153.9899", "PARK17-T3-JUPITER", 1],
  ["90.8881", "90.8881", "PARK17-T3-EARTH-MOON", 1],
  ["0.0411 ″/cy", "0.0411", "PARK17-T3-X-JUPITER-SATURN", 1],
  ["Park's J2 = 2.25×10⁻⁷", "2.25e-7", "PARK17-J2SUN", 1],
  ["J2 0.0279 at DE440's J2", "0.0279", "PHY010-J2-DE440-ANALYTIC-RATE", 1],
  ["0.0 ± 0.0031 ″/cy", "0.0031", "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE-INTERVAL", 1],
  ["0.0 ± 0.0031 ″/cy", "0.0", "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE", 1],
  ["0.0 ± 3.1 mas/cy", "3.1", "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE-INTERVAL", 1000],
];
let quoted = 0;
for (const [lit, num, id, scale] of VECTOR_QUOTES) {
  const v = VEC.get(id);
  if (!v) {
    fail(`VECTOR_QUOTES: vector ${id} not found in tests/vectors`);
    continue;
  }
  if (!gText.includes(lit)) {
    fail(`glossary.js no longer quotes "${lit}" (vector ${id}); update the text or VECTOR_QUOTES`);
    continue;
  }
  const x = Number(num);
  const tol = printedTol(num);
  if (!(Math.abs(x - v.value * scale) <= tol * (1 + 1e-9))) {
    fail(`glossary.js quotes ${num} for ${id} (${v.file}), but the vector is ${v.value}${scale !== 1 ? " (×" + scale + ")" : ""}`);
  }
  quoted++;
}
// Quoted measurements that are deliberately not vector values: [number, reason].
const NOT_VECTORS = new Map([
  ["42.9796", "WP29 G1's measured GR-potential rung with WHFast (named in viewer.toml's source text, not a vector value)"],
]);
let scanned = 0;
const vecVals = [...VEC.entries()];
for (const m of gPlain.matchAll(/(?<![\w.])([-+]?\d+\.\d+(?:e[-+]?\d+)?)(?![\w.]*\d)/g)) {
  const n = m[1];
  const digits = n.replace(/e.*$/, "").replace(/^[-+]?[0.]*/, "").replace(".", "").length;
  if (digits < 4 || NOT_VECTORS.has(n.replace(/^\+/, ""))) continue;
  scanned++;
  const x = Number(n);
  const tol = printedTol(n);
  if (vecVals.some(([, v]) => Math.abs(v.value - x) <= tol * (1 + 1e-9))) continue;
  const near = vecVals.find(([, v]) => v.value !== 0 && Math.abs(v.value - x) <= 1e-4 * Math.abs(v.value));
  if (near) fail(`glossary.js quotes ${n}, within 1e-4 of ${near[0]} = ${near[1].value} (${near[1].file}) but not equal to it at the printed digits: drift?`);
}

// ---------------------------------------------------------------- report
const referenced = ids.filter((id) => used.has(id));
const glossaryOnly = ids.filter((id) => !used.has(id));
const perGroup = G.groups.map((g) => g.title + " " + ids.filter((id) => E[id].group === g.id).length).join(", ");
console.log(`glossary entries: ${ids.length} (${perGroup})`);
console.log(`index.html: ${htmlTip} data-tip + ${htmlTerm} data-term hooks; viewer.js: ${jsRefs} generated hooks`);
console.log(`entries with a tooltip on the page: ${referenced.length}/${ids.length}`);
console.log(`numbers against tests/vectors: ${quoted} quoted values checked, ${scanned} other numbers of 4+ digits scanned (${VEC.size} vectors)`);
console.log(`entries reached only through the glossary and its links: ${glossaryOnly.length}${glossaryOnly.length ? " (" + glossaryOnly.join(", ") + ")" : ""}`);
if (failures) {
  console.error(`${failures} failure(s)`);
  process.exit(1);
}
console.log("glossary check passed");
