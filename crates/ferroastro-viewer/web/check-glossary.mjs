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
//    `term: "..."` fields, dataset.tip / dataset.term assignments) names an existing entry.
// It prints the coverage counts and exits 1 on any failure.

import { readFileSync } from "node:fs";
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
  if (rows.length !== 12) fail(`viewer.js TOGGLES has ${rows.length} rows, expected 12 (bits 0-11)`);
}

// ---------------------------------------------------------------- report
const referenced = ids.filter((id) => used.has(id));
const glossaryOnly = ids.filter((id) => !used.has(id));
const perGroup = G.groups.map((g) => g.title + " " + ids.filter((id) => E[id].group === g.id).length).join(", ");
console.log(`glossary entries: ${ids.length} (${perGroup})`);
console.log(`index.html: ${htmlTip} data-tip + ${htmlTerm} data-term hooks; viewer.js: ${jsRefs} generated hooks`);
console.log(`entries with a tooltip on the page: ${referenced.length}/${ids.length}`);
console.log(`entries reached only through the glossary and its links: ${glossaryOnly.length}${glossaryOnly.length ? " (" + glossaryOnly.join(", ") + ")" : ""}`);
if (failures) {
  console.error(`${failures} failure(s)`);
  process.exit(1);
}
console.log("glossary check passed");
