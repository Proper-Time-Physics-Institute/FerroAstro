// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// viewer.js: main thread of the FerroAstro orbit viewer (docs/viewer/DESIGN.md section 7).
// It owns the DOM and the two canvases and talks to Web Workers (worker.js) that host the wasm engine.
// It computes no orbits, forces or fits: every number drawn comes from the engine through the C ABI.
// The only arithmetic here is drawing geometry (view transforms, the conic of the osculating ellipse
// from the engine's a, e, varpi), display rotation for the labelled exaggeration, calendar labels,
// and the ADR-0014 differences between leg rates in the contribution table.

/* global FAV */
"use strict";

(function () {
  const P = FAV.PARAM;
  const B = FAV.BIT;
  const ARCSEC_PER_RAD = 206264.80624709636;
  const DEG = Math.PI / 180;
  const WASM_URL = "ferroastro_viewer.wasm";
  const MAX_ELEM_POINTS = 20000;
  const FIT_INTERVAL_MS = 500;
  const MARKERS = 48;

  // ------------------------------------------------------------------------------------------------
  // Static descriptions (display text and literature reference values, never used for physics)
  // ------------------------------------------------------------------------------------------------

  // Orbital periods in years: only to size the drawn trails (about one orbit each).
  const BODIES = [
    { name: "Mercury", css: "--mercury", period: 0.2408, trailOrbits: 2 },
    { name: "Venus", css: "--venus", period: 0.6152, trailOrbits: 1 },
    { name: "Earth–Moon", css: "--emb", period: 1.0, trailOrbits: 1 },
    { name: "Mars", css: "--mars", period: 1.8808, trailOrbits: 1 },
    { name: "Jupiter", css: "--jupiter", period: 11.862, trailOrbits: 1 },
    { name: "Saturn", css: "--saturn", period: 29.457, trailOrbits: 1 },
    { name: "Uranus", css: "--uranus", period: 84.02, trailOrbits: 1 },
    { name: "Neptune", css: "--neptune", period: 164.8, trailOrbits: 1 },
  ];
  // Toggle bit -> body slot in the bodies buffer.
  const BIT_BODY = { 0: 1, 1: 2, 2: 3, 3: 4, 4: 5, 5: 6, 6: 7 };

  // One colour token per effect, shared by the toggles, the chart wedges, the budget strip and the
  // table rows. Planets reuse their body colour from the orbit view; GR 1PN and EIH share one (they
  // are mutually exclusive and describe the same physics).
  const TOGGLES = [
    { bit: B.VENUS, label: "Venus", short: "Venus", css: "--venus", exp: "≈277 ″/cy" },
    { bit: B.EMB, label: "Earth–Moon (EMB)", short: "EMB", css: "--emb", exp: "≈91 ″/cy" },
    { bit: B.MARS, label: "Mars", short: "Mars", css: "--mars", exp: "≈2.5 ″/cy" },
    { bit: B.JUPITER, label: "Jupiter", short: "Jupiter", css: "--jupiter", exp: "≈154 ″/cy" },
    { bit: B.SATURN, label: "Saturn", short: "Saturn", css: "--saturn", exp: "≈7.3 ″/cy" },
    { bit: B.URANUS, label: "Uranus", short: "Uranus", css: "--uranus", exp: "≈0.14 ″/cy" },
    { bit: B.NEPTUNE, label: "Neptune", short: "Neptune", css: "--neptune", exp: "≈0.04 ″/cy" },
    { bit: B.SUN_J2, label: "Solar J2", short: "J2", css: "--fx-j2", exp: "≈0.03 ″/cy" },
    { bit: B.GR_1PN, label: "GR 1PN (Sun)", short: "GR", css: "--fx-gr", exp: "≈42.98 ″/cy" },
    { bit: B.LENSE_THIRRING, label: "Lense–Thirring", short: "LT", css: "--fx-lt", exp: "≈−0.002 ″/cy", note: "IAS15 only" },
    { bit: B.ASTEROIDS, label: "Big asteroids", short: "Asteroids", css: "--fx-ast", exp: "≈0.001 ″/cy", note: "IAS15 only" },
    { bit: B.EIH, label: "EIH 1PN, all bodies (adv.)", short: "EIH", css: "--fx-gr", exp: "≈42.98 ″/cy", note: "IAS15 only; replaces GR 1PN" },
  ];

  // Reference values for the table (approx. literature; frame/window caveats in `src`).
  function referenceFor(effect, cfg) {
    const p = cfg.params;
    switch (effect) {
      case "b0":
        return { v: 277.4176, src: "Park17 Table 3 (PARK17-T3-VENUS), 2000-yr window centred on J2000" };
      case "b1":
        return { v: 90.8881, src: "Park17 Table 3 (PARK17-T3-EARTH-MOON)" };
      case "b2":
        return { v: 2.4814, src: "Park17 Table 3 (PARK17-T3-MARS)" };
      case "b3":
        return { v: 153.9899, src: "Park17 Table 3 (PARK17-T3-JUPITER)" };
      case "b4":
        return { v: 7.3227, src: "Park17 Table 3 (PARK17-T3-SATURN)" };
      case "b5":
        return { v: 0.1425, src: "Park17 Table 3 (PARK17-T3-URANUS)" };
      case "b6":
        return { v: 0.0424, src: "Park17 Table 3 (PARK17-T3-NEPTUNE)" };
      case "b7": {
        const j2 = p[P.J2_OVERRIDE];
        // J2(DE440) comes from the engine's metadata (j2_nominal), never from a copied header value (#22).
        const j2n = nominalJ2();
        const scaled = Number.isFinite(j2);
        const k = scaled ? (Number.isFinite(j2n) ? j2 / j2n : NaN) : 1;
        return {
          v: 0.02845 * k,
          src:
            "FerroAstro J2-REAL 0.02845 (real solar pole, DE440 J2); planar analytic 0.02792 (PHY010-J2-DE440-ANALYTIC-RATE)" +
            (scaled ? (Number.isFinite(k) ? "; scaled ×J2/J2(DE440)" : "; J2(DE440) not yet reported by the engine") : ""),
        };
      }
      case "b8":
      case "b11": {
        const f = (2 + 2 * p[P.PPN_GAMMA] - p[P.PPN_BETA]) / 3;
        return {
          v: 42.9805 * f,
          src:
            "PHY004-GR-RATE 42.9805" + (f !== 1 ? " × (2+2γ−β)/3 (PHY-006)" : "") +
            (effect === "b11" ? "; EIH adds the planets' 1PN terms on top of the Sun's" : ""),
        };
      }
      case "b9": {
        const k = p[P.SPIN_SCALE];
        return { v: -0.002 * k, src: "Park17 (PARK17-LT-RATE); FerroAstro LT-REAL −0.002015" + (k !== 1 ? "; scaled ×spin" : "") };
      }
      case "b10":
        return { v: 0.0012, src: "Park17 343 asteroids (PARK17-T3-ASTEROIDS); first 16 ≈ 0.001 (PHY-019)" };
      case "rung": {
        const r = p[P.RUNG];
        const v = [0, 7.1634, 50.144, 35.817, 42.9805][r];
        const ids = ["", "PHY007-SRA-NUM-RATE", "PHY008-SRB-HARMONIC-RATE", "PHY008-SRB-AREAL-RATE", "PHY004-GR-RATE"];
        let vv = v;
        let s = ids[r] + " (ADR-0012)";
        if (r === 4) {
          const f = (2 + 2 * p[P.PPN_GAMMA] - p[P.PPN_BETA]) / 3;
          vv = v * f;
          if (f !== 1) s += " × (2+2γ−β)/3";
        }
        return { v: vv, src: s };
      }
      case "kepler":
        return { v: 0, src: "two-body Newtonian (PHY-001)" };
      default:
        return null;
    }
  }

  const RUNG_NAMES = ["Newton", "SR-a (SR, no PN gauge)", "SR-b harmonic", "SR-b areal (pedagogical)", "GR PPN"];

  // ------------------------------------------------------------------------------------------------
  // DOM
  // ------------------------------------------------------------------------------------------------
  const $ = (id) => document.getElementById(id);
  const el = {
    status: $("status"),
    error: $("error"),
    badgeMode: $("badge-mode"),
    badgeEngine: $("badge-engine"),
    badgeNominal: $("badge-nominal"),
    orbit: $("orbit"),
    exag: $("exag"),
    exagOut: $("exag-out"),
    exagBadge: $("exag-badge"),
    legend: $("orbit-legend"),
    showEllipse: $("show-ellipse"),
    showMarkers: $("show-markers"),
    showTrails: $("show-trails"),
    zoomInner: $("zoom-inner"),
    zoomFull: $("zoom-full"),
    zoomMercury: $("zoom-mercury"),
    chart: $("chart"),
    sparkE: $("spark-e"),
    sparkA: $("spark-a"),
    sparkEVal: $("spark-e-val"),
    sparkAVal: $("spark-a-val"),
    rate: $("rate-readout"),
    rateSub: $("rate-sub"),
    showOsc: $("show-osc"),
    showEcl: $("show-ecl"),
    detrend: $("detrend"),
    fitWindow: $("fit-window"),
    toggles: $("toggles"),
    modeNote: $("mode-note"),
    rungFs: $("rung-fs"),
    rung: $("rung"),
    beta: $("p-beta"),
    gamma: $("p-gamma"),
    engine: $("p-engine"),
    engineResolved: $("engine-resolved"),
    whstep: $("p-whstep"),
    eps: $("p-eps"),
    span: $("p-span"),
    cadence: $("p-cadence"),
    ast: $("p-ast"),
    fitper: $("p-fitper"),
    j2: $("p-j2"),
    spin: $("p-spin"),
    copyScenario: $("copy-scenario"),
    metaLine: $("meta-line"),
    tSpan: $("t-span"),
    tPadd: $("t-padd"),
    tRun: $("t-run"),
    tCancel: $("t-cancel"),
    tCsv: $("t-csv"),
    tMeta: $("t-meta"),
    tBody: $("contrib-body"),
    tFoot: $("contrib-foot"),
    play: $("play"),
    restart: $("restart"),
    speed: $("speed"),
    tDate: $("t-date"),
    tSpanDone: $("t-span-done"),
    tRate: $("t-rate"),
    tEngine: $("t-engine"),
    tProgress: $("t-progress"),
    advance: $("advance"),
    rampT0: $("ramp-t0"),
    rampT1: $("ramp-t1"),
    fitDetails: $("fit-details"),
    showFan: $("show-fan"),
    budget: $("budget"),
    budgetBar: $("budget-bar"),
    budgetKey: $("budget-key"),
  };

  // ------------------------------------------------------------------------------------------------
  // State
  // ------------------------------------------------------------------------------------------------
  const state = {
    module: null,
    live: null,
    liveReady: false,
    awaitingConfigured: false,
    configured: false,
    playing: false,
    wantPlay: false,
    done: false,
    sysMask: FAV.DEFAULT_MASK,
    labMask: 0,
    applied: null, // {mask, params}
    engine: 0,
    epochJd: 2440400.5,
    meta: null,
    j2Nominal: NaN, // DE440 solar J2 from the engine's metadata (j2_nominal), see nominalJ2()
    scenario: "",
    tNow: 0,
    integratedMs: 0,
    rateSmoothed: NaN,
    lastChunkWall: 0,
    fit: null,
    fitMeta: null,
    fitMessage: "",
    fitInFlight: false,
    fitAskedAtPassages: -1,
    lastFitWall: 0,
  };

  // Rolling display data.
  let trails = [];
  let current = [];
  let elems = null;
  let pass = null;
  let lastElem = null;
  let firstElem = null; // the osculating elements at the start of the run: the ghosted reference ellipse

  function resetData() {
    trails = BODIES.map(() => ({ t: [], x: [], y: [], lastT: -Infinity }));
    current = BODIES.map(() => null);
    elems = { t: [], w: [], e: [], a: [], inc: [], wecl: [], stride: 1, counter: 0 };
    pass = { t: [], w: [], x: [], y: [], z: [], err: [] };
    lastElem = null;
    firstElem = null;
    state.tNow = 0;
    state.integratedMs = 0;
    state.rateSmoothed = NaN;
    state.fit = null;
    state.fitMeta = null;
    state.fitMessage = "";
    state.fitInFlight = false;
    state.fitAskedAtPassages = -1;
    state.done = false;
    dirty.orbit = dirty.chart = true;
  }

  const dirty = { orbit: true, chart: true };

  // ------------------------------------------------------------------------------------------------
  // Helpers
  // ------------------------------------------------------------------------------------------------
  function setStatus(s) {
    el.status.textContent = s;
  }
  function showError(s) {
    if (!s) {
      el.error.hidden = true;
      el.error.textContent = "";
      return;
    }
    el.error.hidden = false;
    el.error.textContent = s;
  }

  let colors = {};
  function readColors() {
    const cs = getComputedStyle(document.documentElement);
    const names = [
      "--bg", "--panel", "--panel-2", "--plate", "--fg", "--muted", "--border", "--accent", "--grid", "--axis", "--sun",
      "--osc", "--fitline", "--ecl", "--time-0", "--time-mid", "--time-1", "--warn-bg", "--warn-fg",
    ].concat(BODIES.map((b) => b.css), TOGGLES.map((t) => t.css));
    for (const n of names) colors[n] = cs.getPropertyValue(n).trim() || "#888888";
    fonts.sans = cs.getPropertyValue("--sans").trim() || "system-ui, sans-serif";
    fonts.serif = cs.getPropertyValue("--serif").trim() || "Georgia, serif";
    dirty.orbit = dirty.chart = true;
  }
  const fonts = { sans: "system-ui, sans-serif", serif: "Georgia, serif" };
  function font(px, weight, family) {
    const dpr = window.devicePixelRatio || 1;
    return (weight ? weight + " " : "") + px * dpr + "px " + (family || fonts.sans);
  }

  // The time ramp shared by the orbit markers and the chart dots: f = 0 at the first passage, 1 at the
  // latest. Three stops, linear in sRGB between them.
  function rampColor(f) {
    const g = Math.min(1, Math.max(0, Number.isFinite(f) ? f : 1));
    return g < 0.5 ? mix(colors["--time-0"], colors["--time-mid"], g * 2) : mix(colors["--time-mid"], colors["--time-1"], (g - 0.5) * 2);
  }
  function passFraction(t) {
    const n = pass.t.length;
    if (n < 2) return 1;
    const span = pass.t[n - 1] - pass.t[0];
    return span > 0 ? (t - pass.t[0]) / span : 1;
  }

  // Fills a ".parts" container with one span per non-empty part (separated by CSS rules).
  function setParts(node, parts) {
    node.textContent = "";
    for (const p of parts) {
      if (!p) continue;
      const s = document.createElement("span");
      s.textContent = p;
      node.appendChild(s);
    }
  }

  function effectCss(key) {
    if (key === "rung") return "--fx-gr";
    const t = TOGGLES.find((x) => "b" + x.bit === key);
    return t ? t.css : "--muted";
  }
  function effectShort(key) {
    if (key === "rung") return "Rung";
    const t = TOGGLES.find((x) => "b" + x.bit === key);
    return t ? t.short : key;
  }

  function hexToRgb(h) {
    const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(h);
    if (!m) return [128, 128, 128];
    return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
  }
  function mix(a, b, f) {
    const x = hexToRgb(a);
    const y = hexToRgb(b);
    return "rgb(" + x.map((v, i) => Math.round(v + (y[i] - v) * f)).join(",") + ")";
  }
  function withAlpha(h, a) {
    const [r, g, b] = hexToRgb(h);
    return "rgba(" + r + "," + g + "," + b + "," + a + ")";
  }

  function decimalsFor(sigma) {
    if (!Number.isFinite(sigma) || sigma <= 0) return 3;
    return Math.min(6, Math.max(1, Math.ceil(-Math.log10(sigma)) + 1));
  }
  function fmtPm(v, s) {
    if (!Number.isFinite(v)) return "–";
    if (Number.isFinite(s) && s > 0 && s < 1e-5) {
      // Below the fixed-point range: keep the sigma visible instead of printing 0.000000.
      const vt = Math.abs(v) < 1e-4 && v !== 0 ? v.toExponential(2) : v.toFixed(6);
      return vt + " ± " + s.toExponential(1);
    }
    const d = decimalsFor(s);
    return v.toFixed(d) + (Number.isFinite(s) ? " ± " + s.toFixed(d) : "");
  }
  function refDigits(v, d) {
    const small = v !== 0 && Number.isFinite(v) ? Math.ceil(-Math.log10(Math.abs(v))) + 1 : 0;
    return Math.min(6, Math.max(2, Math.min(5, d), small));
  }
  function fmtNum(v, d) {
    return Number.isFinite(v) ? v.toFixed(d) : "–";
  }
  // The nominal DE440 solar J2 as the engine reports it in its metadata (j2_nominal); NaN until a
  // metadata object has arrived. The page never carries a header.440 value itself (#22).
  function nominalJ2() {
    return state.j2Nominal;
  }
  function noteMeta(m) {
    if (m && typeof m.j2_nominal === "number" && Number.isFinite(m.j2_nominal)) state.j2Nominal = m.j2_nominal;
  }

  function fmtK(k) {
    if (k < 10) return k.toFixed(k < 1.995 && k !== 1 ? 2 : k === 1 ? 0 : 1).replace(/\.0+$/, "");
    if (k < 1000) return Math.round(k).toString();
    const e = Math.floor(Math.log10(k));
    const m = k / Math.pow(10, e);
    return (Math.abs(m - 1) < 0.05 ? "" : m.toFixed(1) + "×") + "10" + supNum(e);
  }
  function supNum(n) {
    const s = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    return String(n)
      .split("")
      .map((c) => (c === "-" ? "⁻" : s[+c]))
      .join("");
  }

  // Julian date -> calendar date string (Meeus, Astronomical Algorithms ch. 7). Labels only.
  function jdToCalendar(jd) {
    const z = Math.floor(jd + 0.5);
    const f = jd + 0.5 - z;
    let a = z;
    if (z >= 2299161) {
      const al = Math.floor((z - 1867216.25) / 36524.25);
      a = z + 1 + al - Math.floor(al / 4);
    }
    const b = a + 1524;
    const c = Math.floor((b - 122.1) / 365.25);
    const d = Math.floor(365.25 * c);
    const e = Math.floor((b - d) / 30.6001);
    const day = Math.floor(b - d - Math.floor(30.6001 * e) + f);
    const month = e < 14 ? e - 1 : e - 13;
    const year = month > 2 ? c - 4716 : c - 4715;
    const pad = (n) => String(n).padStart(2, "0");
    return (year < 0 ? "−" + String(-year).padStart(4, "0") : String(year).padStart(4, "0")) + "-" + pad(month) + "-" + pad(day);
  }
  function epochYear() {
    return 2000 + (state.epochJd - 2451545.0) / 365.25;
  }

  function niceTicks(lo, hi, n) {
    const span = hi - lo;
    if (!(span > 0) || !Number.isFinite(span)) return [lo];
    const raw = span / Math.max(1, n);
    const p = Math.pow(10, Math.floor(Math.log10(raw)));
    const m = raw / p;
    const step = (m < 1.5 ? 1 : m < 3.5 ? 2 : m < 7.5 ? 5 : 10) * p;
    const out = [];
    for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) out.push(Math.abs(v) < step * 1e-9 ? 0 : v);
    return out;
  }
  function tickLabel(v, ticks) {
    const step = ticks.length > 1 ? Math.abs(ticks[1] - ticks[0]) : 1;
    const d = Math.max(0, Math.min(6, -Math.floor(Math.log10(step) + 1e-9)));
    return v.toFixed(d);
  }

  function interp(ts, vs, t) {
    const n = ts.length;
    if (n === 0) return NaN;
    if (t <= ts[0]) return vs[0];
    if (t >= ts[n - 1]) return vs[n - 1];
    let lo = 0;
    let hi = n - 1;
    while (hi - lo > 1) {
      const mid = (lo + hi) >> 1;
      if (ts[mid] <= t) lo = mid;
      else hi = mid;
    }
    const f = (t - ts[lo]) / (ts[hi] - ts[lo] || 1);
    return vs[lo] + f * (vs[hi] - vs[lo]);
  }

  // ------------------------------------------------------------------------------------------------
  // Controls: build, read, write
  // ------------------------------------------------------------------------------------------------
  const toggleInputs = {};
  function buildToggles() {
    for (const t of TOGGLES) {
      const li = document.createElement("li");
      const id = "tg-" + t.bit;
      const sw = '<span class="swatch" aria-hidden="true" style="background:var(' + t.css + ')"></span>';
      const noteId = id + "-note";
      li.innerHTML =
        '<label class="check" for="' + id + '"><input type="checkbox" id="' + id + '"' + (t.note ? ' aria-describedby="' + noteId + '"' : "") + ">" +
        sw + "<span>" + t.label + "</span></label>" +
        '<span class="exp">' + t.exp + (t.note ? '<span class="req" id="' + noteId + '">' + t.note + "</span>" : "") + "</span>";
      el.toggles.appendChild(li);
      const input = li.querySelector("input");
      toggleInputs[t.bit] = { input, li };
      input.addEventListener("change", () => onToggle(t.bit, input.checked));
    }
  }

  function mode() {
    return document.querySelector('input[name="mode"]:checked').value === "1" ? 1 : 0;
  }
  function labAllowed(bit) {
    return bit === B.SUN_J2 || bit === B.LENSE_THIRRING;
  }

  function onToggle(bit, on) {
    let m = mode() === 1 ? state.labMask : state.sysMask;
    if (on) m |= 1 << bit;
    else m &= ~(1 << bit);
    // EIH (bit 11) replaces the Sun-only 1PN (bit 8): mutually exclusive.
    if (on && bit === B.EIH) m &= ~(1 << B.GR_1PN);
    if (on && bit === B.GR_1PN) m &= ~(1 << B.EIH);
    m >>>= 0;
    if (mode() === 1) state.labMask = m;
    else state.sysMask = m;
    syncToggleUI();
    requestReconfigure();
  }

  function currentMask() {
    return (mode() === 1 ? state.labMask : state.sysMask) >>> 0;
  }

  function syncToggleUI() {
    const lab = mode() === 1;
    const m = currentMask();
    for (const t of TOGGLES) {
      const { input, li } = toggleInputs[t.bit];
      const allowed = !lab || labAllowed(t.bit);
      input.disabled = !allowed;
      input.checked = allowed && (m & (1 << t.bit)) !== 0;
      li.classList.toggle("disabled", !allowed);
    }
    el.modeNote.hidden = !lab;
    el.rungFs.hidden = !lab;
    el.engine.disabled = lab;
    el.whstep.disabled = lab;
    el.ast.disabled = lab || (m & (1 << B.ASTEROIDS)) === 0;
    const rung = Number(el.rung.value);
    const ppn = lab ? rung === 4 || (m & (1 << B.LENSE_THIRRING)) !== 0
      : (m & ((1 << B.GR_1PN) | (1 << B.EIH) | (1 << B.LENSE_THIRRING))) !== 0;
    el.beta.disabled = !ppn;
    el.gamma.disabled = !ppn;
    el.j2.disabled = (m & (1 << B.SUN_J2)) === 0;
    el.spin.disabled = (m & (1 << B.LENSE_THIRRING)) === 0;
    document.querySelector('[data-preset="planets"]').disabled = lab;
    el.badgeMode.textContent = lab ? "Rung lab: " + RUNG_NAMES[rung] : "System mode";
    buildLegend();
  }

  function numberField(input, lo, hi, opts) {
    opts = opts || {};
    const raw = input.value.trim();
    if (raw === "" && opts.emptyNaN) {
      input.removeAttribute("aria-invalid");
      return NaN;
    }
    const v = Number(raw);
    let ok = raw !== "" && Number.isFinite(v) && v >= lo && v <= hi;
    if (ok && opts.integer) ok = Number.isInteger(v);
    if (!ok) {
      input.setAttribute("aria-invalid", "true");
      throw new Error((input.labels && input.labels[0] ? input.labels[0].textContent : input.id) + ": enter a value in " + lo + " … " + hi + (opts.integer ? " (integer)" : ""));
    }
    input.removeAttribute("aria-invalid");
    return v;
  }

  /** Reads the controls into {mask, params}; throws with a readable message when a field is invalid. */
  function readConfig() {
    const lab = mode() === 1;
    const params = {};
    params[P.MODE] = lab ? 1 : 0;
    params[P.ENGINE] = lab ? 0 : Number(el.engine.value);
    params[P.PPN_BETA] = numberField(el.beta, -99, 101);
    params[P.PPN_GAMMA] = numberField(el.gamma, -99, 101);
    params[P.WH_STEP_DAYS] = numberField(el.whstep, 0.125, 4);
    params[P.IAS15_EPSILON] = Number(el.eps.value);
    params[P.SPAN_YEARS] = numberField(el.span, 1, 20000);
    params[P.DISPLAY_CADENCE_DAYS] = numberField(el.cadence, 0.5, 50);
    params[P.ASTEROID_COUNT] = numberField(el.ast, 1, 343, { integer: true });
    params[P.RUNG] = Number(el.rung.value);
    params[P.J2_OVERRIDE] = numberField(el.j2, 0, 1e-3, { emptyNaN: true });
    params[P.SPIN_SCALE] = numberField(el.spin, 0, 1e6);
    params[P.FIT_PERIODIC] = Number(el.fitper.value);
    return { mask: currentMask(), params };
  }

  function writeConfig(cfg) {
    const p = cfg.params;
    const lab = p[P.MODE] === 1;
    document.querySelector('input[name="mode"][value="' + (lab ? 1 : 0) + '"]').checked = true;
    if (lab) state.labMask = cfg.mask;
    else state.sysMask = cfg.mask;
    el.engine.value = String(p[P.ENGINE]);
    el.beta.value = p[P.PPN_BETA];
    el.gamma.value = p[P.PPN_GAMMA];
    el.whstep.value = p[P.WH_STEP_DAYS];
    el.eps.value = String(p[P.IAS15_EPSILON]);
    el.span.value = p[P.SPAN_YEARS];
    el.cadence.value = p[P.DISPLAY_CADENCE_DAYS];
    el.ast.value = p[P.ASTEROID_COUNT];
    el.rung.value = String(p[P.RUNG]);
    el.j2.value = Number.isFinite(p[P.J2_OVERRIDE]) ? p[P.J2_OVERRIDE] : "";
    el.spin.value = p[P.SPIN_SCALE];
    el.fitper.value = String(p[P.FIT_PERIODIC]);
    for (const inp of document.querySelectorAll("[aria-invalid]")) inp.removeAttribute("aria-invalid");
    syncToggleUI();
  }

  function nonNominalText(cfg) {
    const p = cfg.params;
    const parts = [];
    if (Number.isFinite(p[P.J2_OVERRIDE])) {
      const j2n = nominalJ2();
      parts.push("J2 = " + p[P.J2_OVERRIDE].toExponential(3) + (Number.isFinite(j2n) ? " (×" + fmtK(p[P.J2_OVERRIDE] / j2n) + " DE440)" : ""));
    }
    if (p[P.SPIN_SCALE] !== 1) parts.push("solar spin ×" + fmtK(p[P.SPIN_SCALE]));
    if (p[P.IAS15_EPSILON] > 1e-9) parts.push("IAS15 ε = " + p[P.IAS15_EPSILON] + " (fast, reduced accuracy)");
    return parts.join("; ");
  }

  function applyPreset(name) {
    const lab = mode() === 1;
    if (lab) {
      if (name === "kepler") {
        state.labMask = 0;
        el.rung.value = "0";
      } else if (name === "gr") {
        state.labMask = 0;
        el.rung.value = "4";
      } else if (name === "full") {
        state.labMask = (1 << B.SUN_J2) | (1 << B.LENSE_THIRRING);
        el.rung.value = "4";
      }
    } else {
      state.sysMask = { kepler: 0, planets: FAV.PLANET_MASK, gr: 1 << B.GR_1PN, full: FAV.DEFAULT_MASK }[name];
    }
    syncToggleUI();
    requestReconfigure();
  }

  // ------------------------------------------------------------------------------------------------
  // Live worker
  // ------------------------------------------------------------------------------------------------
  function sameConfig(a, b) {
    return !!a && !!b && JSON.stringify(cfgKey(a)) === JSON.stringify(cfgKey(b));
  }
  function cfgKey(c) {
    const p = {};
    for (let i = 0; i < FAV.N_PARAMS; i++) p[i] = Number.isNaN(c.params[i]) ? "NaN" : c.params[i];
    return { m: c.mask, p };
  }

  function requestReconfigure(force) {
    let cfg;
    try {
      cfg = readConfig();
    } catch (err) {
      showError(err.message);
      return;
    }
    if (!force && sameConfig(cfg, state.applied)) return;
    if (!force && state.integratedMs > 10000 && state.applied) {
      const secs = Math.round(state.integratedMs / 1000);
      if (!window.confirm("This discards " + secs + " s of integration (" + state.tNow.toFixed(1) + " simulated years). Restart with the new settings?")) {
        writeConfig(state.applied);
        return;
      }
    }
    showError("");
    state.applied = cfg;
    configureLive();
  }

  function configureLive() {
    if (!state.liveReady || !state.applied) return;
    state.wantPlay = state.playing || state.wantPlay;
    state.playing = false;
    state.configured = false;
    state.awaitingConfigured = true;
    resetData();
    updateReadout();
    updateTransport();
    const nn = nonNominalText(state.applied);
    el.badgeNominal.hidden = !nn;
    el.badgeNominal.textContent = nn ? "Exaggerated physics: " + nn : "";
    setStatus("Configuring…");
    writeHash(state.applied);
    state.live.postMessage({ op: "configure", mask: state.applied.mask, params: state.applied.params });
    invalidateTable();
  }

  function speedValue() {
    return el.speed.value === "max" ? Infinity : Number(el.speed.value);
  }

  function setPlaying(on) {
    if (!state.configured || state.done) on = false;
    state.playing = on;
    state.wantPlay = on;
    if (state.live) state.live.postMessage(on ? { op: "play", speed: speedValue() } : { op: "pause" });
    updateTransport();
  }

  function onLiveMessage(ev) {
    const m = ev.data;
    switch (m.op) {
      case "ready":
        state.liveReady = true;
        if (m.recovered) {
          if (onEngineLost(m.reason || "wasm trap")) requestReconfigure(true);
          break;
        }
        setStatus("Engine ready (ABI v" + m.abi + ").");
        requestReconfigure(true);
        break;
      case "configured":
        state.awaitingConfigured = false;
        state.configured = true;
        state.engine = m.engine;
        state.epochJd = m.epochJd;
        state.meta = m.meta;
        noteMeta(m.meta);
        state.scenario = m.scenario || "";
        state.lastChunkWall = performance.now();
        onConfiguredMeta();
        setStatus("Running " + describeRun() + ".");
        el.play.disabled = false;
        el.restart.disabled = false;
        if (state.wantPlay) setPlaying(true);
        else updateTransport();
        break;
      case "chunk":
        if (state.awaitingConfigured) return;
        ingestChunk(m);
        break;
      case "fit":
        if (state.awaitingConfigured) return;
        state.fitInFlight = false;
        if (m.status === FAV.STATUS.E_FIT) {
          state.fit = null;
          state.fitMessage = m.message;
        } else {
          state.fit = m.fit;
          state.fitMeta = m.meta;
          noteMeta(m.meta);
          state.fitMessage = "";
        }
        updateReadout();
        dirty.chart = true;
        break;
      case "error":
        if (m.context === "configure") {
          state.awaitingConfigured = false;
          state.configured = false;
          state.playing = false;
          el.play.disabled = true;
          // A wasm built without the embed-header feature has no DE440 header.440 initial
          // conditions (#22): System mode refuses with FA-E0303 and says how to rebuild.
          setStatus(/built without embed-header/.test(m.message || "") ? "System mode unavailable: this engine was built without embed-header." : "Configuration refused.");
          el.engineResolved.textContent = "";
        }
        if (m.context === "fit") {
          state.fitInFlight = false;
          state.fitMessage = m.message;
          updateReadout();
          return;
        }
        if (m.context === "run") {
          state.playing = false;
          state.wantPlay = false;
          updateTransport();
        }
        showError(m.message);
        break;
      default:
        break;
    }
  }

  function describeRun() {
    const lab = state.applied.params[P.MODE] === 1;
    return (lab ? "Rung lab" : "System mode") + " with " + (FAV.ENGINE_NAME[state.engine] || "engine " + state.engine);
  }

  function onConfiguredMeta() {
    const meta = state.meta || {};
    const eng = meta.engine || FAV.ENGINE_NAME[state.engine] || "";
    el.badgeEngine.textContent = "engine: " + eng;
    el.tEngine.textContent = eng;
    el.engineResolved.textContent = state.applied.params[P.ENGINE] === 0 && state.applied.params[P.MODE] === 0 ? "resolved: " + (FAV.ENGINE_NAME[state.engine] || "?") : "";
    if (meta.nominal === false && el.badgeNominal.hidden) {
      el.badgeNominal.hidden = false;
      el.badgeNominal.textContent = "Non-nominal run";
    }
    const bits = [];
    if (meta.gauge) bits.push("gauge: " + meta.gauge);
    if (meta.ic_hash) bits.push("IC " + String(meta.ic_hash).slice(0, 10));
    if (meta.constants) bits.push("constants " + meta.constants);
    setParts(el.metaLine, bits);
    el.metaLine.title = meta.model ? "Model: " + meta.model + (meta.ic_source ? "\nIC source: " + meta.ic_source : "") : "";
    el.copyScenario.disabled = !state.scenario;
  }

  // ------------------------------------------------------------------------------------------------
  // Data ingest
  // ------------------------------------------------------------------------------------------------
  function ingestChunk(m) {
    const now = performance.now();
    const wall = (now - state.lastChunkWall) / 1000;
    state.lastChunkWall = now;
    if (wall > 0 && m.t > state.tNow) {
      const r = (m.t - state.tNow) / wall;
      state.rateSmoothed = Number.isFinite(state.rateSmoothed) ? 0.8 * state.rateSmoothed + 0.2 * r : r;
    }
    state.tNow = m.t;
    state.integratedMs += m.ms || 0;

    const SB = FAV.STRIDE.bodies;
    const bd = m.bodies;
    for (let r = 0; r + SB <= bd.length; r += SB) {
      const t = bd[r];
      for (let i = 0; i < BODIES.length; i++) {
        const x = bd[r + 1 + 3 * i];
        const y = bd[r + 2 + 3 * i];
        const z = bd[r + 3 + 3 * i];
        if (!Number.isFinite(x) || !Number.isFinite(y)) {
          current[i] = null;
          continue;
        }
        current[i] = { t, x, y, z };
        const tr = trails[i];
        const spacing = i === 0 ? 0 : BODIES[i].period / 360;
        if (t - tr.lastT >= spacing) {
          tr.t.push(t);
          tr.x.push(x);
          tr.y.push(y);
          tr.lastT = t;
        }
        const win = BODIES[i].period * BODIES[i].trailOrbits;
        while (tr.t.length > 2 && t - tr.t[0] > win) {
          tr.t.shift();
          tr.x.shift();
          tr.y.shift();
        }
      }
    }

    const SE = FAV.STRIDE.elements;
    const ed = m.elements;
    for (let r = 0; r + SE <= ed.length; r += SE) {
      lastElem = { t: ed[r], w: ed[r + 1], e: ed[r + 2], a: ed[r + 3], inc: ed[r + 4], wecl: ed[r + 5] };
      if (!firstElem && Number.isFinite(lastElem.a)) firstElem = lastElem;
      if (elems.counter++ % elems.stride !== 0) continue;
      elems.t.push(ed[r]);
      elems.w.push(ed[r + 1]);
      elems.e.push(ed[r + 2]);
      elems.a.push(ed[r + 3]);
      elems.inc.push(ed[r + 4]);
      elems.wecl.push(ed[r + 5]);
      if (elems.t.length > MAX_ELEM_POINTS) decimateElems();
    }

    const SP = FAV.STRIDE.passages;
    const pd = m.passages;
    const newPass = pd.length >= SP;
    for (let r = 0; r + SP <= pd.length; r += SP) {
      pass.t.push(pd[r]);
      pass.w.push(pd[r + 1]);
      pass.x.push(pd[r + 2]);
      pass.y.push(pd[r + 3]);
      pass.z.push(pd[r + 4]);
      pass.err.push(pd[r + 5]);
    }

    if (m.status === FAV.STATUS.DONE) {
      state.done = true;
      state.playing = false;
      state.wantPlay = false;
      setStatus("Span reached (" + state.tNow.toFixed(1) + " yr). Restart or change the span to continue.");
      requestFit(true);
    }
    dirty.orbit = dirty.chart = true;
    if (newPass) updateAdvance();
    updateTransport();
    maybeFit();
  }

  function decimateElems() {
    for (const k of ["t", "w", "e", "a", "inc", "wecl"]) {
      const src = elems[k];
      const out = [];
      for (let i = 0; i < src.length; i += 2) out.push(src[i]);
      elems[k] = out;
    }
    elems.stride *= 2;
  }

  function fitWindow() {
    const v = el.fitWindow.value;
    if (v === "all") return [NaN, NaN];
    const n = Number(v);
    return [Math.max(0, state.tNow - n), state.tNow];
  }

  function maybeFit() {
    const now = performance.now();
    if (state.fitInFlight || now - state.lastFitWall < FIT_INTERVAL_MS) return;
    if (pass.t.length === state.fitAskedAtPassages) return;
    requestFit(false);
  }

  function requestFit(force) {
    if (!state.configured || !state.live) return;
    if (state.fitInFlight && !force) return;
    if (pass.t.length < 2) return;
    const [t0, t1] = fitWindow();
    state.fitInFlight = true;
    state.lastFitWall = performance.now();
    state.fitAskedAtPassages = pass.t.length;
    state.live.postMessage({ op: "fit", t0, t1 });
  }

  function updateReadout() {
    const f = state.fit;
    const F = FAV.FIT;
    if (!f || f.length < FAV.STRIDE.fit) {
      el.rate.textContent = "ϖ̇ = – ″/cy";
      el.rate.title = "";
      el.rateSub.textContent = state.fitMessage
        ? state.fitMessage + (pass.t.length ? " (" + pass.t.length + " passages so far)" : "")
        : "Needs at least 20 perihelion passages (about 5 years).";
      el.fitDetails.textContent = "No fit yet.";
      el.chart.setAttribute("aria-label", "Perihelion longitude chart; no fit yet.");
      updateAdvance();
      return;
    }
    const sigma = Number.isFinite(f[F.JACKKNIFE_SIGMA]) ? f[F.JACKKNIFE_SIGMA] : f[F.FORMAL_SIGMA];
    const txt = "ϖ̇ = " + fmtPm(f[F.RATE], sigma) + " ″/cy";
    el.rate.textContent = txt;
    const meta = state.fitMeta || {};
    const fm = meta.fit && meta.fit.model ? meta.fit.model : "";
    const y0 = epochYear();
    setParts(el.rateSub, [
      "± jack-knife σ",
      f[F.N_PASSAGES] + " passages",
      fmtNum(f[F.T_FIRST], 1) + "–" + fmtNum(f[F.T_LAST], 1) + " yr (" + Math.floor(y0 + f[F.T_FIRST]) + "–" + Math.floor(y0 + f[F.T_LAST]) + ")",
      "Park17 plane",
      state.applied && nonNominalText(state.applied) ? "non-nominal" : "",
    ]);
    el.rate.title =
      "Fit model: " + (fm || "–") +
      "\nformal σ " + fmtNum(f[F.FORMAL_SIGMA], 6) + ", jack-knife σ " + fmtNum(f[F.JACKKNIFE_SIGMA], 6) + " ″/cy" +
      "\nQ = " + fmtNum(f[F.Q], 6) + " ″/cy², periodic terms " + f[F.N_PERIODIC] +
      "\nrms residual " + fmtNum(f[F.RMS], 4) + " ″" +
      (meta.estimator ? "\nestimator: " + meta.estimator : "") +
      (meta.frame ? "\nframe: " + meta.frame : "") +
      (meta.engine ? "\nengine: " + meta.engine : "") +
      (meta.constants ? "\nconstants: " + meta.constants : "");
    el.fitDetails.textContent = el.rate.title;
    el.chart.setAttribute("aria-label", "Perihelion longitude chart. " + txt + ", " + f[F.N_PASSAGES] + " passages.");
    updateAdvance();
  }

  // The advance in words under the orbit plate (and in the plate's accessible name). Engine numbers
  // only: the latest projected passage longitude minus the first, both from the passage buffer.
  let lastAdvanceText = "";
  function updateAdvance() {
    const n = pass.t.length;
    let txt;
    if (n < 2) {
      txt = n === 1 ? "First perihelion passage recorded; the advance shows from the second." : "The perihelion markers appear after Mercury's first pass.";
      el.advance.textContent = txt;
      el.rampT0.textContent = "first passage";
      el.rampT1.textContent = "latest";
    } else {
      const d = pass.w[n - 1] - pass.w[0];
      const y0 = epochYear();
      const deg = d / 3600;
      const k = exaggeration();
      const yrs = pass.t[n - 1] - pass.t[0];
      el.advance.textContent = "";
      const lead = document.createTextNode("Since " + Math.floor(y0 + pass.t[0]) + " the perihelion has turned ");
      const num = document.createElement("span");
      num.className = "num";
      num.textContent = (Math.abs(d) >= 100 ? d.toFixed(0) : d.toFixed(2)) + "″" + (Math.abs(deg) >= 0.01 ? " (" + deg.toFixed(3) + "°)" : "");
      const tail = document.createTextNode(
        " in " + yrs.toFixed(yrs < 10 ? 2 : 1) + " years, over " + n + " passages" + (k !== 1 ? "; drawn ×" + fmtK(k) + " on the plate." : "."),
      );
      el.advance.append(lead, num, tail);
      txt = el.advance.textContent;
      el.rampT0.textContent = String(Math.floor(y0 + pass.t[0]));
      el.rampT1.textContent = String(Math.floor(y0 + pass.t[n - 1]));
    }
    if (txt !== lastAdvanceText) {
      lastAdvanceText = txt;
      el.orbit.setAttribute("aria-label", "Orbit view, top-down on the J2000 ecliptic. " + txt + " Focus and use plus, minus and arrow keys to zoom and pan, 0 to reset.");
    }
  }

  // ------------------------------------------------------------------------------------------------
  // Transport
  // ------------------------------------------------------------------------------------------------
  function updateTransport() {
    el.play.textContent = state.done ? "Done" : state.playing ? "Pause" : "Play";
    el.play.setAttribute("aria-pressed", state.playing ? "true" : "false");
    el.play.disabled = !state.configured || state.done;
    const span = state.applied ? state.applied.params[P.SPAN_YEARS] : 1;
    el.tDate.textContent = state.configured ? jdToCalendar(state.epochJd + state.tNow * 365.25) : "–";
    el.tSpanDone.textContent = state.configured ? state.tNow.toFixed(2) + " / " + span + " yr" : "–";
    el.tRate.textContent = state.playing && Number.isFinite(state.rateSmoothed) ? state.rateSmoothed.toPrecision(3) + " sim yr/s" : "–";
    el.tProgress.value = Math.min(1, state.tNow / span);
  }

  // ------------------------------------------------------------------------------------------------
  // Canvas sizing
  // ------------------------------------------------------------------------------------------------
  function fitCanvas(c) {
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.round(c.clientWidth * dpr));
    const h = Math.max(1, Math.round(c.clientHeight * dpr));
    if (c.width !== w || c.height !== h) {
      c.width = w;
      c.height = h;
      return true;
    }
    return false;
  }

  // ------------------------------------------------------------------------------------------------
  // Orbit view
  // ------------------------------------------------------------------------------------------------
  const PRESETS = { inner: 1.7, full: 32, mercury: 0.55 };
  const view = { cx: 0, cy: 0, half: PRESETS.inner, preset: "inner" };

  function setPreset(name) {
    view.cx = 0;
    view.cy = 0;
    view.half = PRESETS[name];
    view.preset = name;
    el.zoomInner.setAttribute("aria-pressed", name === "inner");
    el.zoomFull.setAttribute("aria-pressed", name === "full");
    el.zoomMercury.setAttribute("aria-pressed", name === "mercury");
    dirty.orbit = true;
  }
  function clearPreset() {
    view.preset = "";
    for (const b of [el.zoomInner, el.zoomFull, el.zoomMercury]) b.setAttribute("aria-pressed", "false");
  }

  function orbitScale() {
    const c = el.orbit;
    return Math.min(c.width, c.height) / 2 / view.half;
  }
  function toPx(x, y) {
    const c = el.orbit;
    const s = orbitScale();
    return [c.width / 2 + (x - view.cx) * s, c.height / 2 - (y - view.cy) * s];
  }
  function toWorld(px, py) {
    const c = el.orbit;
    const s = orbitScale();
    return [view.cx + (px - c.width / 2) / s, view.cy - (py - c.height / 2) / s];
  }
  function zoomAbout(px, py, factor) {
    const [wx, wy] = toWorld(px, py);
    view.half = Math.min(200, Math.max(0.02, view.half / factor));
    const [wx2, wy2] = toWorld(px, py);
    view.cx += wx - wx2;
    view.cy += wy - wy2;
    clearPreset();
    dirty.orbit = true;
  }

  function exaggeration() {
    return Math.pow(10, Number(el.exag.value));
  }

  // Projected passage longitude (arcsec) relative to the first passage, as a function of t, for the
  // display rotation of the drawn ellipse: linear between passages, extended with the fitted rate.
  function deltaVarpiArcsec(t) {
    const n = pass.t.length;
    if (n === 0) return 0;
    const w0 = pass.w[0];
    if (t <= pass.t[n - 1]) return interp(pass.t, pass.w, t) - w0;
    const f = state.fit;
    const slope = f && Number.isFinite(f[FAV.FIT.RATE]) ? f[FAV.FIT.RATE] / 100 : 0;
    return pass.w[n - 1] - w0 + slope * (t - pass.t[n - 1]);
  }

  function rotate(x, y, ang) {
    const c = Math.cos(ang);
    const s = Math.sin(ang);
    return [x * c - y * s, x * s + y * c];
  }

  function buildLegend() {
    const m = currentMask();
    const lab = mode() === 1;
    let html = "";
    BODIES.forEach((b, i) => {
      const on = i === 0 || (!lab && (m & (1 << (i - 1))) !== 0);
      html += '<span class="item' + (on ? "" : " off") + '"><span class="dot" aria-hidden="true" style="background:var(' + b.css + ')"></span>' + b.name +
        (on ? "" : '<span class="sr-only"> (off)</span>') + "</span>";
    });
    html += '<span class="item"><span class="ramp-dot"></span>perihelion passages, by time</span>';
    el.legend.innerHTML = html;
  }

  // Indices of the passages drawn as markers: every passage when there are few, otherwise MARKERS
  // passages spread evenly over the whole run (always the first and the latest), so the markers fan
  // out by the full advance rather than the last few orbits.
  function markerIndices() {
    const n = pass.t.length;
    if (n <= MARKERS) return pass.t.map((_, i) => i);
    const out = [];
    let prev = -1;
    for (let j = 0; j < MARKERS; j++) {
      const i = Math.round((j * (n - 1)) / (MARKERS - 1));
      if (i !== prev) out.push(i);
      prev = i;
    }
    return out;
  }

  // Draws the conic of an osculating ellipse (a, e, varpi_ecl; inclination neglected) rotated by
  // `extra` radians for the labelled display exaggeration.
  function ellipsePath(ctx, el0, extra) {
    const wv = el0.wecl * DEG + extra;
    const p = el0.a * (1 - el0.e * el0.e);
    ctx.beginPath();
    for (let j = 0; j <= 240; j++) {
      const nu = (j / 240) * 2 * Math.PI;
      const r = p / (1 + el0.e * Math.cos(nu));
      const [px, py] = toPx(r * Math.cos(nu + wv), r * Math.sin(nu + wv));
      if (j === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    }
    return wv;
  }
  function validElem(e0) {
    return !!e0 && Number.isFinite(e0.a) && Number.isFinite(e0.e) && Number.isFinite(e0.wecl) && e0.e < 1 && e0.a > 0;
  }

  function drawOrbit() {
    const c = el.orbit;
    fitCanvas(c);
    const ctx = c.getContext("2d");
    const W = c.width;
    const H = c.height;
    const dpr = window.devicePixelRatio || 1;
    const s = orbitScale();
    ctx.clearRect(0, 0, W, H);
    ctx.fillStyle = colors["--plate"];
    ctx.fillRect(0, 0, W, H);

    // Reference circles every "nice" au, labelled, and the J2000 equinox direction.
    ctx.lineWidth = 1 * dpr;
    ctx.strokeStyle = colors["--grid"];
    const ticks = niceTicks(0, view.half * 1.5, 4).filter((v) => v > 0);
    const [ox, oy] = toPx(0, 0);
    ctx.font = font(12);
    ctx.fillStyle = colors["--muted"];
    ctx.textAlign = "left";
    for (const r of ticks) {
      ctx.beginPath();
      ctx.arc(ox, oy, r * s, 0, 2 * Math.PI);
      ctx.stroke();
      const lx = ox + r * s * Math.SQRT1_2 + 3 * dpr;
      const ly = oy - r * s * Math.SQRT1_2 - 3 * dpr;
      if (lx > 0 && lx < W - 30 * dpr && ly > 14 * dpr && ly < H) ctx.fillText(tickLabel(r, ticks) + " au", lx, ly);
    }
    ctx.setLineDash([4 * dpr, 4 * dpr]);
    ctx.beginPath();
    ctx.moveTo(ox, oy);
    ctx.lineTo(W, oy);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = colors["--muted"];
    ctx.textAlign = "right";
    ctx.fillText("\u2648\uFE0E equinox J2000", W - 6 * dpr, oy - 5 * dpr);

    // Trails.
    if (el.showTrails.checked) {
      for (let i = BODIES.length - 1; i >= 0; i--) {
        if (!current[i]) continue;
        const tr = trails[i];
        const n = tr.t.length;
        if (n < 2) continue;
        const col = colors[BODIES[i].css];
        const groups = 8;
        const per = Math.ceil(n / groups);
        ctx.lineWidth = (i === 0 ? 1.4 : 1.2) * dpr;
        for (let g = 0; g < groups; g++) {
          const a = g * per;
          const b = Math.min(n - 1, (g + 1) * per);
          if (b <= a) continue;
          ctx.strokeStyle = withAlpha(col, (i === 0 ? 0.06 : 0.1) + ((i === 0 ? 0.45 : 0.7) * (g + 1)) / groups);
          // Quadratic segments through the sample midpoints: a smooth trail even at coarse cadence.
          ctx.beginPath();
          let [px, py] = toPx(tr.x[a], tr.y[a]);
          ctx.moveTo(px, py);
          for (let k = a + 1; k <= b; k++) {
            const [qx, qy] = toPx(tr.x[k], tr.y[k]);
            if (k < b) ctx.quadraticCurveTo(qx, qy, (qx + toPx(tr.x[k + 1], tr.y[k + 1])[0]) / 2, (qy + toPx(tr.x[k + 1], tr.y[k + 1])[1]) / 2);
            else ctx.lineTo(qx, qy);
            [px, py] = [qx, qy];
          }
          if (g === groups - 1 || b === n - 1) {
            [px, py] = toPx(current[i].x, current[i].y);
            ctx.lineTo(px, py);
          }
          ctx.stroke();
        }
      }
    }

    const k = exaggeration();
    const w0 = pass.t.length ? pass.w[0] : 0;

    // Mercury's osculating ellipse now (solid) over the ghost of the ellipse at the start of the run
    // (dashed, faint): the gap between their perihelia is the advance.
    if (el.showEllipse.checked && validElem(lastElem)) {
      if (validElem(firstElem) && firstElem !== lastElem) {
        const extra0 = ((k - 1) * deltaVarpiArcsec(firstElem.t)) / ARCSEC_PER_RAD;
        ctx.strokeStyle = withAlpha(colors["--mercury"], 0.28);
        ctx.lineWidth = 1 * dpr;
        ctx.setLineDash([2 * dpr, 4 * dpr]);
        ellipsePath(ctx, firstElem, extra0);
        ctx.stroke();
        ctx.setLineDash([]);
      }
      const extra = ((k - 1) * deltaVarpiArcsec(lastElem.t)) / ARCSEC_PER_RAD;
      ctx.strokeStyle = withAlpha(colors["--mercury"], 0.8);
      ctx.lineWidth = 1.4 * dpr;
      const wv = ellipsePath(ctx, lastElem, extra);
      ctx.stroke();
      // Perihelion point of the current osculating ellipse.
      const q = lastElem.a * (1 - lastElem.e);
      const [qx, qy] = toPx(q * Math.cos(wv), q * Math.sin(wv));
      ctx.fillStyle = colors["--mercury"];
      ctx.beginPath();
      ctx.arc(qx, qy, 2.5 * dpr, 0, 2 * Math.PI);
      ctx.fill();
    }

    // Perihelion markers: one radial tick per drawn passage, coloured by time on the shared ramp,
    // then the Sun→first and Sun→latest lines and the arc between them labelled with the advance.
    if (el.showMarkers.checked && pass.t.length) {
      const n = pass.t.length;
      const idx = markerIndices();
      const drawn = (j) => {
        const extra = ((k - 1) * (pass.w[j] - w0)) / ARCSEC_PER_RAD;
        return rotate(pass.x[j], pass.y[j], extra);
      };
      for (const j of idx) {
        const [x, y] = drawn(j);
        const r = Math.hypot(x, y);
        if (!(r > 0)) continue;
        const ux = x / r;
        const uy = y / r;
        const [px, py] = toPx(x, y);
        const last = j === n - 1;
        const len = (last ? 14 : 8) * dpr;
        ctx.strokeStyle = rampColor(passFraction(pass.t[j]));
        ctx.lineWidth = (last ? 3 : 2) * dpr;
        ctx.lineCap = "round";
        ctx.beginPath();
        ctx.moveTo(px - ux * len * 0.5, py + uy * len * 0.5);
        ctx.lineTo(px + ux * len, py - uy * len);
        ctx.stroke();
        ctx.lineCap = "butt";
      }
      if (n >= 2) {
        const [x0, y0] = drawn(0);
        const [x1, y1] = drawn(n - 1);
        const a0 = Math.atan2(y0, x0);
        let da = Math.atan2(y1, x1) - a0;
        while (da > Math.PI) da -= 2 * Math.PI;
        while (da <= -Math.PI) da += 2 * Math.PI;
        const [p0x, p0y] = toPx(x0, y0);
        const [p1x, p1y] = toPx(x1, y1);
        ctx.lineWidth = 1 * dpr;
        ctx.setLineDash([2 * dpr, 3 * dpr]);
        ctx.strokeStyle = rampColor(0);
        ctx.beginPath();
        ctx.moveTo(ox, oy);
        ctx.lineTo(p0x, p0y);
        ctx.stroke();
        ctx.strokeStyle = rampColor(1);
        ctx.beginPath();
        ctx.moveTo(ox, oy);
        ctx.lineTo(p1x, p1y);
        ctx.stroke();
        ctx.setLineDash([]);
        // Arc at 60 % of the perihelion distance; canvas angles run clockwise, world angles counter.
        const rArc = 0.6 * Math.hypot(p1x - ox, p1y - oy);
        if (rArc > 12 * dpr && Math.abs(da) * rArc > 1.5 * dpr) {
          ctx.strokeStyle = colors["--fg"];
          ctx.lineWidth = 1.5 * dpr;
          ctx.beginPath();
          ctx.arc(ox, oy, rArc, -a0, -(a0 + da), da > 0);
          ctx.stroke();
        }
        if (rArc > 12 * dpr) {
          const d = pass.w[n - 1] - pass.w[0];
          const am = a0 + da / 2;
          const lr = rArc + 10 * dpr;
          const lx = ox + Math.cos(am) * lr;
          const ly = oy - Math.sin(am) * lr;
          const label = "Δϖ " + (Math.abs(d) >= 100 ? d.toFixed(0) : d.toFixed(1)) + "″" + (k !== 1 ? " (drawn ×" + fmtK(k) + ")" : "");
          ctx.font = font(12.5, "600", fonts.serif);
          ctx.textAlign = Math.cos(am) >= 0 ? "left" : "right";
          ctx.lineWidth = 3 * dpr;
          ctx.strokeStyle = colors["--plate"];
          ctx.strokeText(label, lx, ly + 4 * dpr);
          ctx.fillStyle = colors["--fg"];
          ctx.fillText(label, lx, ly + 4 * dpr);
        }
      }
    }

    // Sun and bodies.
    ctx.fillStyle = colors["--sun"];
    ctx.beginPath();
    ctx.arc(ox, oy, 6 * dpr, 0, 2 * Math.PI);
    ctx.fill();
    ctx.font = font(12);
    ctx.textAlign = "left";
    for (let i = BODIES.length - 1; i >= 0; i--) {
      const b = current[i];
      if (!b) continue;
      const [px, py] = toPx(b.x, b.y);
      if (i === 0) {
        // Mercury: an ink disc with a plate-coloured halo, so it reads over the ellipse.
        ctx.fillStyle = colors["--plate"];
        ctx.beginPath();
        ctx.arc(px, py, 6 * dpr, 0, 2 * Math.PI);
        ctx.fill();
      }
      ctx.fillStyle = colors[BODIES[i].css];
      ctx.beginPath();
      ctx.arc(px, py, (i === 0 ? 4.5 : 3.5) * dpr, 0, 2 * Math.PI);
      ctx.fill();
      if (px > -50 && px < W + 50 && py > -50 && py < H + 50) {
        ctx.fillStyle = colors["--fg"];
        ctx.font = i === 0 ? font(12, "600") : font(12);
        ctx.fillText(BODIES[i].name, px + 7 * dpr, py - 6 * dpr);
      }
    }

    // Scale bar.
    const target = (W * 0.22) / s;
    let nice = niceTicks(0, target, 1).filter((v) => v > 0)[0];
    if (!nice) {
      // Round down to 1, 2 or 5 times a power of ten so the label stays short.
      const p = Math.pow(10, Math.floor(Math.log10(target)));
      const m = target / p;
      nice = (m >= 5 ? 5 : m >= 2 ? 2 : 1) * p;
    }
    const bx = 12 * dpr;
    const by = H - 14 * dpr;
    ctx.strokeStyle = colors["--fg"];
    ctx.lineWidth = 2 * dpr;
    ctx.beginPath();
    ctx.moveTo(bx, by - 4 * dpr);
    ctx.lineTo(bx, by);
    ctx.lineTo(bx + nice * s, by);
    ctx.lineTo(bx + nice * s, by - 4 * dpr);
    ctx.stroke();
    ctx.fillStyle = colors["--fg"];
    ctx.textAlign = "left";
    ctx.font = font(12);
    ctx.fillText(nice + " au", bx, by - 7 * dpr);

    // The exaggeration badge is drawn into the canvas so it is part of any screenshot.
    if (k !== 1) {
      let txt = "Display exaggeration ×" + fmtK(k) + ": perihelion rotation drawn " + fmtK(k) + " times larger than computed";
      ctx.font = font(12.5, "600");
      if (ctx.measureText(txt).width > W - 32 * dpr) txt = "Display ×" + fmtK(k) + ": rotation drawn " + fmtK(k) + "× larger than computed";
      const tw = ctx.measureText(txt).width;
      ctx.fillStyle = colors["--warn-bg"];
      ctx.fillRect(8 * dpr, 8 * dpr, Math.min(W - 16 * dpr, tw + 16 * dpr), 26 * dpr);
      ctx.strokeStyle = colors["--warn-fg"];
      ctx.lineWidth = 1 * dpr;
      ctx.strokeRect(8.5 * dpr, 8.5 * dpr, Math.min(W - 16 * dpr, tw + 16 * dpr) - dpr, 25 * dpr);
      ctx.fillStyle = colors["--warn-fg"];
      ctx.textAlign = "left";
      ctx.fillText(txt, 16 * dpr, 25.5 * dpr, W - 32 * dpr);
    }
  }

  // Pointer interaction: drag to pan, wheel and pinch to zoom, double-click to reset.
  function installOrbitInput() {
    const c = el.orbit;
    const pointers = new Map();
    let pinchDist = 0;
    const dpr = () => window.devicePixelRatio || 1;
    const pos = (e) => {
      const r = c.getBoundingClientRect();
      return [(e.clientX - r.left) * dpr(), (e.clientY - r.top) * dpr()];
    };
    c.addEventListener("pointerdown", (e) => {
      c.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, pos(e));
      c.classList.add("dragging");
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinchDist = Math.hypot(a[0] - b[0], a[1] - b[1]);
      }
    });
    c.addEventListener("pointermove", (e) => {
      if (!pointers.has(e.pointerId)) return;
      const prev = pointers.get(e.pointerId);
      const p = pos(e);
      pointers.set(e.pointerId, p);
      if (pointers.size === 1) {
        const s = orbitScale();
        view.cx -= (p[0] - prev[0]) / s;
        view.cy += (p[1] - prev[1]) / s;
        clearPreset();
        dirty.orbit = true;
      } else if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const d = Math.hypot(a[0] - b[0], a[1] - b[1]);
        if (pinchDist > 0 && d > 0) zoomAbout((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, d / pinchDist);
        pinchDist = d;
      }
    });
    const end = (e) => {
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinchDist = 0;
      if (pointers.size === 0) c.classList.remove("dragging");
    };
    c.addEventListener("pointerup", end);
    c.addEventListener("pointercancel", end);
    c.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const [px, py] = pos(e);
        const dy = e.deltaMode === 1 ? e.deltaY * 16 : e.deltaY;
        zoomAbout(px, py, Math.exp(-dy * 0.0015));
      },
      { passive: false },
    );
    c.addEventListener("dblclick", () => setPreset(view.preset || "inner"));
    c.addEventListener("keydown", (e) => {
      const W = c.width;
      const H = c.height;
      const step = view.half * 0.1;
      let handled = true;
      if (e.key === "+" || e.key === "=") zoomAbout(W / 2, H / 2, 1.25);
      else if (e.key === "-" || e.key === "_") zoomAbout(W / 2, H / 2, 0.8);
      else if (e.key === "ArrowLeft") view.cx -= step;
      else if (e.key === "ArrowRight") view.cx += step;
      else if (e.key === "ArrowUp") view.cy += step;
      else if (e.key === "ArrowDown") view.cy -= step;
      else if (e.key === "0") setPreset("inner");
      else handled = false;
      if (handled) {
        if (e.key.startsWith("Arrow")) clearPreset();
        dirty.orbit = true;
        e.preventDefault();
      }
    });
  }

  // ------------------------------------------------------------------------------------------------
  // varpi(t) chart and sparklines
  // ------------------------------------------------------------------------------------------------
  function drawChart() {
    const c = el.chart;
    fitCanvas(c);
    const ctx = c.getContext("2d");
    const W = c.width;
    const H = c.height;
    const dpr = window.devicePixelRatio || 1;
    ctx.clearRect(0, 0, W, H);
    const L = 58 * dpr;
    const R = 10 * dpr;
    const T = 32 * dpr;
    const Bm = 18 * dpr;
    const pw = W - L - R;
    const ph = H - T - Bm;
    ctx.font = font(12);

    const xmax = Math.max(state.tNow, 0.25);
    const f = state.fit;
    const F = FAV.FIT;
    const rate = f && Number.isFinite(f[F.RATE]) ? f[F.RATE] : NaN;
    const slope = el.detrend.checked && Number.isFinite(rate) ? rate / 100 : 0; // arcsec per year

    // Each series is plotted relative to its own value at tRef (the first passage, else the first sample).
    const tRef = pass.t.length ? pass.t[0] : elems.t.length ? elems.t[0] : 0;
    const series = [];
    if (el.showOsc.checked && elems.t.length > 1) {
      const ref = interp(elems.t, elems.w, tRef) * 3600;
      series.push({ kind: "line", color: colors["--osc"], width: 1, t: elems.t, y: elems.w.map((v, i) => v * 3600 - ref - slope * (elems.t[i] - tRef)) });
    }
    if (el.showEcl.checked && elems.t.length > 1) {
      const ref = interp(elems.t, elems.wecl, tRef) * 3600;
      series.push({ kind: "line", color: colors["--ecl"], width: 1.4, dash: [5, 4], t: elems.t, y: elems.wecl.map((v, i) => v * 3600 - ref - slope * (elems.t[i] - tRef)) });
    }
    let passY = null;
    if (pass.t.length) {
      const w0 = pass.w[0];
      passY = pass.w.map((v, i) => v - w0 - slope * (pass.t[i] - tRef));
      series.push({ kind: "dots", color: colors["--passage"], t: pass.t, y: passY });
    }
    let fitLine = null;
    if (f && passY && Number.isFinite(rate)) {
      const t1 = f[F.T_FIRST];
      const t2 = f[F.T_LAST];
      let st = 0;
      let sy = 0;
      let n = 0;
      for (let i = 0; i < pass.t.length; i++) {
        if (pass.t[i] >= t1 - 1e-9 && pass.t[i] <= t2 + 1e-9) {
          st += pass.t[i];
          sy += passY[i];
          n++;
        }
      }
      if (n > 0) {
        const tc = st / n;
        const yc = sy / n;
        const m = rate / 100 - slope;
        fitLine = { t: [t1, t2], y: [yc + m * (t1 - tc), yc + m * (t2 - tc)] };
      }
    }

    let ymin = Infinity;
    let ymax = -Infinity;
    for (const s of series) for (const v of s.y) {
      if (v < ymin) ymin = v;
      if (v > ymax) ymax = v;
    }
    if (fitLine) for (const v of fitLine.y) {
      ymin = Math.min(ymin, v);
      ymax = Math.max(ymax, v);
    }
    if (!Number.isFinite(ymin)) {
      ymin = -1;
      ymax = 1;
    }
    if (ymax - ymin < 1e-6) {
      ymin -= 1;
      ymax += 1;
    }
    const padY = (ymax - ymin) * 0.06;
    ymin -= padY;
    ymax += padY;
    const X = (t) => L + (t / xmax) * pw;
    const Y = (v) => T + ph - ((v - ymin) / (ymax - ymin)) * ph;

    // Grid and axes.
    ctx.strokeStyle = colors["--grid"];
    ctx.fillStyle = colors["--muted"];
    ctx.lineWidth = 1;
    const yt = niceTicks(ymin, ymax, 5);
    ctx.textAlign = "right";
    for (const v of yt) {
      const y = Y(v);
      ctx.beginPath();
      ctx.moveTo(L, y);
      ctx.lineTo(L + pw, y);
      ctx.stroke();
      ctx.fillText(tickLabel(v, yt), L - 5 * dpr, y + 3.5 * dpr);
    }
    const xt = niceTicks(0, xmax, 6);
    ctx.textAlign = "center";
    const y0 = epochYear();
    const xstep = xt.length > 1 ? xt[1] - xt[0] : 1;
    const calDec = Math.max(0, Math.min(3, Math.ceil(-Math.log10(xstep) - 1e-9)));
    for (const v of xt) {
      const x = X(v);
      ctx.beginPath();
      ctx.moveTo(x, T);
      ctx.lineTo(x, T + ph);
      ctx.stroke();
      ctx.fillText(tickLabel(v, xt), x, T + ph + 14 * dpr);
      ctx.fillText((y0 + v).toFixed(calDec), x, T - 6 * dpr);
    }
    ctx.textAlign = "left";
    ctx.fillText((el.detrend.checked ? "ϖ − ϖ₀ − ϖ̇t (″)" : "ϖ − ϖ₀ (″)"), 4 * dpr, 12 * dpr);

    ctx.save();
    ctx.beginPath();
    ctx.rect(L, T, pw, ph);
    ctx.clip();
    const fan = el.showFan.checked && !el.detrend.checked && pass.t.length ? fanWedges() : null;
    if (fan) {
      // Per-effect wedges: cumulative P-remove slopes from the Kepler floor, largest effect at the
      // bottom, each in its effect colour. Indicative: the rates come from the table's own span.
      const t0 = tRef;
      let cum = fan.floor / 100;
      const labels = [];
      for (const w of fan.wedges) {
        const lo = cum;
        const hi = cum + w.v / 100;
        cum = hi;
        ctx.fillStyle = withAlpha(colors[w.css], 0.2);
        ctx.beginPath();
        ctx.moveTo(X(t0), Y(0));
        ctx.lineTo(X(xmax), Y(lo * (xmax - t0)));
        ctx.lineTo(X(xmax), Y(hi * (xmax - t0)));
        ctx.closePath();
        ctx.fill();
        ctx.strokeStyle = withAlpha(colors[w.css], 0.6);
        ctx.lineWidth = 1 * dpr;
        ctx.beginPath();
        ctx.moveTo(X(t0), Y(0));
        ctx.lineTo(X(xmax), Y(hi * (xmax - t0)));
        ctx.stroke();
        const yLo = Y(lo * (xmax - t0));
        const yHi = Y(hi * (xmax - t0));
        if (yLo - yHi > 13 * dpr) labels.push({ text: w.short, y: (yLo + yHi) / 2, css: w.css });
      }
      ctx.font = font(12, "600");
      ctx.textAlign = "right";
      for (const lb of labels) {
        ctx.lineWidth = 3 * dpr;
        ctx.strokeStyle = colors["--panel"];
        ctx.strokeText(lb.text, L + pw - 4 * dpr, lb.y + 4 * dpr);
        ctx.fillStyle = colors[lb.css];
        ctx.fillText(lb.text, L + pw - 4 * dpr, lb.y + 4 * dpr);
      }
    }
    for (const s of series) {
      if (s.kind === "line") {
        ctx.strokeStyle = s.color;
        ctx.lineWidth = s.width * dpr;
        ctx.setLineDash(s.dash ? s.dash.map((d) => d * dpr) : []);
        ctx.beginPath();
        for (let i = 0; i < s.t.length; i++) {
          const x = X(s.t[i]);
          const y = Y(s.y[i]);
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        }
        ctx.stroke();
        ctx.setLineDash([]);
      } else {
        // Passage dots on the shared time ramp, in bands of equal colour to keep the draw cheap.
        const r = (s.t.length > 3000 ? 1.1 : 2) * dpr;
        const bands = 24;
        const n = s.t.length;
        for (let b = 0; b < bands; b++) {
          const i0 = Math.floor((b * n) / bands);
          const i1 = Math.floor(((b + 1) * n) / bands);
          if (i1 <= i0) continue;
          ctx.fillStyle = rampColor(passFraction(s.t[Math.floor((i0 + i1 - 1) / 2)]));
          for (let i = i0; i < i1; i++) ctx.fillRect(X(s.t[i]) - r, Y(s.y[i]) - r, 2 * r, 2 * r);
        }
      }
    }
    if (fitLine) {
      // A plate-coloured casing under the fit line keeps it legible over dense dots.
      for (const [col, wdt] of [[colors["--panel"], 4.5], [colors["--fitline"], 2.25]]) {
        ctx.strokeStyle = col;
        ctx.lineWidth = wdt * dpr;
        ctx.beginPath();
        ctx.moveTo(X(fitLine.t[0]), Y(fitLine.y[0]));
        ctx.lineTo(X(fitLine.t[1]), Y(fitLine.y[1]));
        ctx.stroke();
      }
      if (Number.isFinite(rate)) {
        const lbl = "fit ϖ̇ = " + rate.toFixed(2) + " ″/cy" + (el.detrend.checked ? " (subtracted)" : "");
        ctx.font = font(12, "600");
        ctx.textAlign = "left";
        const lx = L + 6 * dpr;
        const ly = T + 14 * dpr;
        ctx.lineWidth = 3 * dpr;
        ctx.strokeStyle = colors["--panel"];
        ctx.strokeText(lbl, lx, ly);
        ctx.fillStyle = colors["--fitline"];
        ctx.fillText(lbl, lx, ly);
      }
    }
    ctx.restore();

    drawSpark(el.sparkE, elems.t, elems.e, xmax, el.sparkEVal, (v) => v.toFixed(7));
    drawSpark(el.sparkA, elems.t, elems.a, xmax, el.sparkAVal, (v) => v.toFixed(8));
  }

  function drawSpark(c, ts, vs, xmax, out, fmt) {
    fitCanvas(c);
    const ctx = c.getContext("2d");
    const W = c.width;
    const H = c.height;
    ctx.clearRect(0, 0, W, H);
    if (ts.length < 2) {
      out.textContent = "";
      return;
    }
    let lo = Infinity;
    let hi = -Infinity;
    for (const v of vs) {
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    if (hi - lo < 1e-15) {
      lo -= 1e-9;
      hi += 1e-9;
    }
    ctx.strokeStyle = colors["--accent"];
    ctx.lineWidth = 1 * (window.devicePixelRatio || 1);
    ctx.beginPath();
    for (let i = 0; i < ts.length; i++) {
      const x = (ts[i] / xmax) * W;
      const y = H - 2 - ((vs[i] - lo) / (hi - lo)) * (H - 4);
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    out.textContent = lastElem ? fmt(vs[vs.length - 1]) : "";
    c.title = "range " + fmt(lo) + " … " + fmt(hi);
  }

  // ------------------------------------------------------------------------------------------------
  // Contribution table (ADR-0014 P-remove, computed from parallel leg workers)
  // ------------------------------------------------------------------------------------------------
  const table = { legs: [], rows: [], workers: [], queue: [], running: false, cfg: null, span: 0, meta: null, done: 0 };

  function effectLabel(key, cfg) {
    if (key.startsWith("b")) {
      const t = TOGGLES.find((x) => "b" + x.bit === key);
      return t ? t.label : key;
    }
    if (key === "rung") return "Relativistic central term: " + RUNG_NAMES[cfg.params[P.RUNG]];
    return key;
  }

  function legKey(mask, params) {
    return JSON.stringify(cfgKey({ mask, params }));
  }

  function planTable(cfg, span, padd) {
    const lab = cfg.params[P.MODE] === 1;
    const base = Object.assign({}, cfg.params);
    if (!lab) base[P.ENGINE] = state.engine === FAV.ENGINE_RESOLVED.WHFAST ? FAV.ENGINE_PARAM.WHFAST : FAV.ENGINE_PARAM.IAS15;
    base[P.SPAN_YEARS] = span;
    const legs = new Map();
    const addLeg = (name, mask, over) => {
      const params = Object.assign({}, base, over || {});
      const key = legKey(mask >>> 0, params);
      if (!legs.has(key)) legs.set(key, { key, name, mask: mask >>> 0, params, t: 0, fit: null, meta: null, error: "", done: false });
      return key;
    };
    const S = addLeg("S", cfg.mask);
    const kepOver = lab ? { [P.RUNG]: 0 } : {};
    const K = addLeg("Kepler only", 0, kepOver);
    const rows = [];
    for (let bit = 0; bit < FAV.N_BITS; bit++) {
      if ((cfg.mask & (1 << bit)) === 0) continue;
      const without = addLeg("S∖" + bit, cfg.mask & ~(1 << bit));
      const add = padd ? addLeg("+" + bit, 1 << bit, kepOver) : null;
      rows.push({ key: "b" + bit, without, add });
    }
    if (lab && cfg.params[P.RUNG] !== 0) {
      const without = addLeg("S∖rung", cfg.mask, { [P.RUNG]: 0 });
      const add = padd ? addLeg("+rung", 0, { [P.RUNG]: cfg.params[P.RUNG] }) : null;
      rows.push({ key: "rung", without, add });
    }
    return { legs, rows, S, K };
  }

  function invalidateTable() {
    if (table.running) return;
    if (table.rows.length) el.tMeta.textContent = "Settings changed since this table was computed; it describes the previous configuration.";
  }

  function stopTable() {
    for (const w of table.workers) w.terminate();
    table.workers = [];
    table.queue = [];
    table.running = false;
    el.tRun.disabled = false;
    el.tCancel.hidden = true;
  }

  function startTable() {
    if (!state.applied || !state.configured) {
      showError("Configure a valid live run first; the table uses its settings and resolved engine.");
      return;
    }
    let span;
    try {
      span = numberField(el.tSpan, 10, 20000);
    } catch (err) {
      showError(err.message);
      return;
    }
    stopTable();
    const cfg = state.applied;
    const plan = planTable(cfg, span, el.tPadd.checked);
    table.cfg = cfg;
    table.span = span;
    table.plan = plan;
    table.legs = [...plan.legs.values()];
    table.rows = plan.rows;
    table.meta = null;
    table.done = 0;
    table.running = true;
    table.queue = table.legs.slice();
    el.tRun.disabled = true;
    el.tCancel.hidden = false;
    el.tCsv.disabled = true;
    document.querySelectorAll(".padd-col").forEach((n) => (n.hidden = !el.tPadd.checked));
    renderTable();
    const n = Math.max(1, Math.min(table.legs.length, (navigator.hardwareConcurrency || 2) - 1, 8));
    setStatus("Computing " + table.legs.length + " legs of " + span + " yr in " + n + " worker" + (n > 1 ? "s" : "") + "…");
    for (let i = 0; i < n; i++) spawnLegWorker();
  }

  function spawnLegWorker() {
    let w;
    try {
      w = new Worker("worker.js");
    } catch (err) {
      showError("Cannot start a worker: " + err.message);
      stopTable();
      return;
    }
    table.workers.push(w);
    let leg = null;
    const next = () => {
      leg = table.queue.shift() || null;
      if (!leg) {
        w.terminate();
        table.workers = table.workers.filter((x) => x !== w);
        if (table.workers.length === 0) finishTable();
        return;
      }
      w.postMessage({ op: "leg", id: leg.key, mask: leg.mask, params: leg.params, spanYears: table.span });
    };
    w.onmessage = (ev) => {
      const m = ev.data;
      if (m.op === "ready") {
        if (m.recovered && leg) {
          leg.done = true;
          leg.error = m.reason || "wasm trap";
          table.done++;
          renderTable();
        }
        next();
      }
      else if (m.op === "progress" && leg) {
        leg.t = m.t;
        updateLegProgress(leg);
      } else if (m.op === "legConfigured" && leg) {
        leg.meta = m.meta;
        noteMeta(m.meta);
        leg.engine = m.engine;
      } else if (m.op === "legDone" && leg) {
        leg.done = true;
        leg.t = m.t;
        leg.fit = m.fit;
        leg.meta = m.meta || leg.meta;
        leg.engine = m.engine;
        if (m.status === FAV.STATUS.E_FIT) leg.error = m.message || "too few passages";
        if (leg.key === table.plan.S) table.meta = leg.meta;
        table.done++;
        renderTable();
        next();
      } else if (m.op === "error") {
        if (leg) {
          leg.done = true;
          leg.error = m.message;
          table.done++;
          renderTable();
          next();
        } else {
          showError("Leg worker: " + m.message);
        }
      }
    };
    w.onerror = (e) => {
      if (leg) {
        leg.done = true;
        leg.error = "worker crashed: " + (e.message || "unknown error");
        table.done++;
        renderTable();
      }
      e.preventDefault();
      w.terminate();
      table.workers = table.workers.filter((x) => x !== w);
      if (table.queue.length) spawnLegWorker();
      else if (table.workers.length === 0) finishTable();
    };
    w.postMessage({ op: "init", module: state.module });
  }

  function finishTable() {
    table.running = false;
    el.tRun.disabled = false;
    el.tCancel.hidden = true;
    el.tCsv.disabled = false;
    renderTable();
    setStatus("Contribution table done (" + table.legs.length + " legs).");
  }

  function legRate(key) {
    const leg = table.plan.legs.get(key);
    if (!leg || !leg.fit || leg.fit.length < FAV.STRIDE.fit) return null;
    const f = leg.fit;
    const s = Number.isFinite(f[FAV.FIT.JACKKNIFE_SIGMA]) ? f[FAV.FIT.JACKKNIFE_SIGMA] : f[FAV.FIT.FORMAL_SIGMA];
    return { v: f[FAV.FIT.RATE], s, leg };
  }

  function legCell(key) {
    const leg = table.plan.legs.get(key);
    if (!leg) return "";
    if (leg.error) return '<span class="err" title="' + escapeHtml(leg.error) + '">error</span>';
    if (!leg.done) {
      return '<progress data-leg="' + escapeHtml(key) + '" max="1" value="' + Math.min(1, leg.t / table.span) + '" aria-label="leg progress"></progress>';
    }
    const h = leg.meta && leg.meta.ic_hash ? String(leg.meta.ic_hash).slice(0, 8) : "–";
    const e = FAV.ENGINE_NAME[leg.engine] || "";
    return '<span title="IC hash ' + escapeHtml(leg.meta && leg.meta.ic_hash ? leg.meta.ic_hash : "") + "; mask " + leg.mask + '"><span class="hash">' + h + "</span> " + e + "</span>";
  }

  function updateLegProgress(leg) {
    const n = document.querySelectorAll('progress[data-leg="' + CSS.escape(leg.key) + '"]');
    n.forEach((p) => (p.value = Math.min(1, leg.t / table.span)));
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
  }

  function computeRows() {
    const S = legRate(table.plan.S);
    const K = legRate(table.plan.K);
    const kAdd = K;
    const out = [];
    for (const r of table.rows) {
      const W = legRate(r.without);
      let v = NaN;
      let s = NaN;
      if (S && W) {
        v = S.v - W.v;
        s = Math.hypot(S.s, W.s);
      }
      let addV = NaN;
      let addS = NaN;
      if (r.add) {
        const A = legRate(r.add);
        if (A && kAdd) {
          addV = A.v - kAdd.v;
          addS = Math.hypot(A.s, kAdd.s);
        }
      }
      const ref = referenceFor(r.key, table.cfg);
      out.push({ r, label: effectLabel(r.key, table.cfg), v, s, ref, addV, addS });
    }
    return { S, K, out };
  }

  function renderTable() {
    if (!table.plan) return;
    const { S, K, out } = computeRows();
    const padd = !!table.rows.find((r) => r.add);
    let html = "";
    if (out.length === 0) html = '<tr><td colspan="6" class="muted">No effects are switched on: S is the Kepler-only baseline.</td></tr>';
    for (const row of out) {
      const unresolved = Number.isFinite(row.v) && Number.isFinite(row.s) && Math.abs(row.v) < 3 * row.s;
      const diff = row.ref && Number.isFinite(row.v) ? row.v - row.ref.v : NaN;
      const d = decimalsFor(row.s);
      html +=
        '<tr class="' + (unresolved ? "unresolved" : "") + '"' + (unresolved ? ' title="Below resolution for this span: |value| < 3σ"' : "") + ">" +
        '<td class="effect"><span class="lbl"><span class="swatch" aria-hidden="true" style="background:var(' + effectCss(row.r.key) + ')"></span>' +
        escapeHtml(row.label) + '</span><span class="ref-src">P-remove</span></td>' +
        '<td class="num val">' + fmtPm(row.v, row.s) + "</td>" +
        '<td class="num">' + (row.ref ? '<span title="' + escapeHtml(row.ref.src) + '">' + fmtNum(row.ref.v, refDigits(row.ref.v, d)) + '<span class="ref-src">' + escapeHtml(row.ref.src.split(/[(;]/)[0].trim()) + "</span></span>" : "–") + "</td>" +
        '<td class="num">' + (Number.isFinite(diff) ? (diff >= 0 ? "+" : "") + diff.toFixed(Math.max(2, d)) : "–") + "</td>" +
        (padd ? '<td class="num padd-col">' + (row.r.add ? fmtPm(row.addV, row.addS) + legCellInline(row.r.add) : "–") + "</td>" : '<td class="padd-col" hidden></td>') +
        '<td class="leg">' + legCell(row.r.without) + "</td></tr>";
    }
    el.tBody.innerHTML = html;

    let sum = 0;
    let ok = out.length > 0;
    for (const row of out) {
      if (Number.isFinite(row.v)) sum += row.v;
      else ok = false;
    }
    const sumTxt = ok ? sum.toFixed(4) : "–";
    const sTxt = S ? fmtPm(S.v, S.s) : "–";
    const kTxt = K ? fmtPm(K.v, K.s) : "–";
    const cross = ok && S && K ? S.v - sum - K.v : NaN;
    const pc = padd ? '<td class="padd-col"></td>' : '<td class="padd-col" hidden></td>';
    el.tFoot.innerHTML =
      "<tr><td>Σ contributions</td><td class=\"num\">" + sumTxt + "</td><td></td><td></td>" + pc + "<td></td></tr>" +
      "<tr><td>ϖ̇(S), all selected effects</td><td class=\"num\">" + sTxt + "</td><td></td><td></td>" + pc + '<td class="leg">' + legCell(table.plan.S) + "</td></tr>" +
      "<tr><td>ϖ̇(Kepler only), integrator/estimator floor</td><td class=\"num\">" + kTxt + '</td><td class="num">0</td><td></td>' + pc + '<td class="leg">' + legCell(table.plan.K) + "</td></tr>" +
      '<tr class="cross"><td>ϖ̇(S) − Σ − ϖ̇(Kepler): interaction (cross) terms; not an error</td><td class="num">' +
      (Number.isFinite(cross) ? cross.toFixed(4) : "–") + '</td><td class="num" title="Park17 Table 3 Jupiter+Saturn cross row (PARK17-T3-X-JUPITER-SATURN), for scale">0.0411<span class="ref-src">Park17 J+S cross</span></td><td></td>' + pc + "<td></td></tr>";

    const meta = table.meta || {};
    const parts = [
      "span " + table.span + " yr forward from JD " + (meta.epoch_jd_tdb || state.epochJd) + " " + (meta.time_scale || "TDB"),
      "engine " + (meta.engine || FAV.ENGINE_NAME[state.engine] || "?") + " (one engine for every leg)",
      meta.frame ? "frame: " + meta.frame : "",
      meta.fit && meta.fit.model ? "fit: " + meta.fit.model : "",
      meta.estimator ? "estimator: " + meta.estimator : "",
      meta.constants ? "constants " + meta.constants : "",
      table.running ? table.done + "/" + table.legs.length + " legs done" : "",
      "Literature references use Park's 2000-yr window centred on J2000; a forward window from 1969 differs (DESIGN.md §8 E6).",
    ];
    const kept = parts.filter(Boolean);
    // The engine's model/estimator descriptions are long: show a clipped line, keep the full text in
    // the tooltip and in the CSV export.
    setParts(el.tMeta, kept.map((x) => (x.length > 110 ? x.slice(0, 107) + "…" : x)));
    el.tMeta.title = kept.join("\n");
    renderBudget(out);
    dirty.chart = true;
  }

  // Wedges for the chart: the finished positive P-remove rows of a table computed for the current
  // configuration, largest first, and the Kepler-only floor they stack on. Null otherwise.
  function fanWedges() {
    if (!table.plan || !sameConfig(table.cfg, state.applied)) return null;
    const { K, out } = computeRows();
    const wedges = out
      .filter((row) => Number.isFinite(row.v) && row.v > 0)
      .map((row) => ({ v: row.v, css: effectCss(row.r.key), short: effectShort(row.r.key) }))
      .sort((a, b) => b.v - a.v);
    if (!wedges.length) return null;
    return { floor: K && Number.isFinite(K.v) ? K.v : 0, wedges };
  }

  // The budget strip above the table: positive contributions to scale, in the effect colours.
  function renderBudget(out) {
    const pos = out.filter((row) => Number.isFinite(row.v) && row.v > 0).sort((a, b) => b.v - a.v);
    const total = pos.reduce((acc, row) => acc + row.v, 0);
    if (!pos.length || !(total > 0)) {
      el.budget.hidden = true;
      return;
    }
    el.budget.hidden = false;
    el.budgetBar.textContent = "";
    el.budgetKey.textContent = "";
    for (const row of pos) {
      const css = effectCss(row.r.key);
      const seg = document.createElement("span");
      seg.style.background = "var(" + css + ")";
      seg.style.flexGrow = String(row.v / total);
      seg.style.flexBasis = "0";
      seg.title = row.label + ": " + fmtPm(row.v, row.s) + " ″/cy";
      el.budgetBar.appendChild(seg);
      const li = document.createElement("li");
      const sw = document.createElement("span");
      sw.className = "swatch";
      sw.setAttribute("aria-hidden", "true");
      sw.style.background = "var(" + css + ")";
      const name = document.createElement("span");
      name.textContent = effectShort(row.r.key);
      const v = document.createElement("span");
      v.className = "v";
      v.textContent = row.v >= 1 ? row.v.toFixed(1) : row.v.toPrecision(2);
      li.append(sw, name, v);
      el.budgetKey.appendChild(li);
    }
    const neg = out.filter((row) => Number.isFinite(row.v) && row.v <= 0);
    if (neg.length) {
      const li = document.createElement("li");
      li.className = "muted";
      li.textContent = "Not drawn (≤ 0): " + neg.map((row) => effectShort(row.r.key)).join(", ");
      el.budgetKey.appendChild(li);
    }
  }

  function legCellInline(key) {
    const leg = table.plan.legs.get(key);
    if (!leg || leg.done) return leg && leg.error ? ' <span class="err" title="' + escapeHtml(leg.error) + '">error</span>' : "";
    return ' <progress data-leg="' + escapeHtml(key) + '" max="1" value="' + Math.min(1, leg.t / table.span) + '" aria-label="P-add leg progress"></progress>';
  }

  function exportCsv() {
    if (!table.plan) return;
    const { S, K, out } = computeRows();
    const meta = table.meta || {};
    const q = (s) => '"' + String(s).replace(/"/g, '""') + '"';
    const lines = [
      "# FerroAstro orbit viewer: per-effect perihelion-rate contributions (ADR-0014 P-remove)",
      "# exported " + new Date().toISOString(),
      "# span_years " + table.span,
      "# toggles " + table.cfg.mask + " params " + JSON.stringify(cfgKey(table.cfg).p),
    ];
    for (const k of ["engine", "frame", "epoch_jd_tdb", "time_scale", "gauge", "model", "estimator", "constants", "ic_source", "ic_hash", "scenario_digest"]) {
      if (meta[k] !== undefined) lines.push("# " + k + " " + String(meta[k]).replace(/\n/g, " "));
    }
    if (meta.fit) lines.push("# fit " + JSON.stringify(meta.fit));
    lines.push("effect,protocol,rate_arcsec_per_cy,sigma_jackknife,reference,reference_source,difference,padd_info_only,padd_sigma,leg_ic_hash");
    for (const row of out) {
      const leg = table.plan.legs.get(row.r.without);
      lines.push(
        [
          q(row.label), "P-remove", row.v, row.s, row.ref ? row.ref.v : "", q(row.ref ? row.ref.src : ""),
          row.ref && Number.isFinite(row.v) ? row.v - row.ref.v : "", Number.isFinite(row.addV) ? row.addV : "",
          Number.isFinite(row.addS) ? row.addS : "", q(leg && leg.meta ? leg.meta.ic_hash || "" : ""),
        ].join(","),
      );
    }
    let sum = 0;
    for (const row of out) sum += row.v;
    lines.push(["sum", "", sum, "", "", "", "", "", "", ""].join(","));
    if (S) lines.push(["rate(S)", "", S.v, S.s, "", "", "", "", "", q(S.leg.meta ? S.leg.meta.ic_hash || "" : "")].join(","));
    if (K) lines.push(["rate(Kepler only)", "", K.v, K.s, 0, "", "", "", "", q(K.leg.meta ? K.leg.meta.ic_hash || "" : "")].join(","));
    if (S && K) lines.push([q("interaction (cross) terms; not an error"), "", S.v - sum - K.v, "", "", "", "", "", "", ""].join(","));
    download("ferroastro-contributions.csv", lines.join("\n") + "\n", "text/csv");
  }

  function download(name, text, type) {
    const url = URL.createObjectURL(new Blob([text], { type }));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    document.body.appendChild(a);
    a.click();
    a.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }

  // ------------------------------------------------------------------------------------------------
  // Render loop
  // ------------------------------------------------------------------------------------------------
  let lastChartDraw = 0;
  function frame(now) {
    if (dirty.orbit) {
      dirty.orbit = false;
      drawOrbit();
    }
    // The chart can hold tens of thousands of points: redraw at most ~8 times a second.
    if (dirty.chart && now - lastChartDraw > 120) {
      dirty.chart = false;
      lastChartDraw = now;
      drawChart();
    }
    if (state.playing && !state.fitInFlight) maybeFit();
    requestAnimationFrame(frame);
  }

  // ------------------------------------------------------------------------------------------------
  // Wiring
  // ------------------------------------------------------------------------------------------------
  function wire() {
    buildToggles();
    document.querySelectorAll('input[name="mode"]').forEach((r) =>
      r.addEventListener("change", () => {
        syncToggleUI();
        requestReconfigure();
      }),
    );
    el.rung.addEventListener("change", () => {
      syncToggleUI();
      requestReconfigure();
    });
    for (const inp of [el.beta, el.gamma, el.engine, el.whstep, el.eps, el.span, el.cadence, el.ast, el.fitper, el.j2, el.spin]) {
      inp.addEventListener("change", () => requestReconfigure());
    }
    document.querySelectorAll("[data-preset]").forEach((b) => b.addEventListener("click", () => applyPreset(b.dataset.preset)));

    el.exag.addEventListener("input", () => {
      const k = exaggeration();
      el.exagOut.textContent = "×" + fmtK(k);
      el.exagBadge.textContent = k === 1 ? "" : "Display exaggeration ×" + fmtK(k) + ": perihelion rotation drawn " + fmtK(k) + " times larger than computed";
      el.exag.setAttribute("aria-valuetext", "times " + fmtK(k));
      updateAdvance();
      dirty.orbit = true;
    });
    for (const c of [el.showEllipse, el.showMarkers, el.showTrails]) c.addEventListener("change", () => (dirty.orbit = true));
    for (const c of [el.showOsc, el.showEcl, el.detrend, el.showFan]) c.addEventListener("change", () => (dirty.chart = true));
    el.fitWindow.addEventListener("change", () => requestFit(true));
    el.zoomInner.addEventListener("click", () => setPreset("inner"));
    el.zoomFull.addEventListener("click", () => setPreset("full"));
    el.zoomMercury.addEventListener("click", () => setPreset("mercury"));
    installOrbitInput();

    el.play.addEventListener("click", () => setPlaying(!state.playing));
    el.restart.addEventListener("click", () => {
      state.wantPlay = state.playing || state.wantPlay;
      if (!state.live && state.module) {
        crashes = [];
        startLiveWorker();
        return;
      }
      requestReconfigure(true);
    });
    el.speed.addEventListener("change", () => {
      if (state.live) state.live.postMessage({ op: "speed", speed: speedValue() });
    });

    el.tRun.addEventListener("click", startTable);
    el.tCancel.addEventListener("click", () => {
      stopTable();
      renderTable();
      setStatus("Contribution table cancelled.");
    });
    el.tCsv.addEventListener("click", exportCsv);
    el.copyScenario.addEventListener("click", async () => {
      if (!state.scenario) return;
      try {
        await navigator.clipboard.writeText(state.scenario);
        setStatus("Scenario TOML copied. Reproduce with the CLI: ferroastro scenario run <file>.");
      } catch (e) {
        download("viewer-scenario.toml", state.scenario, "application/toml");
        setStatus("Clipboard unavailable; the scenario TOML was downloaded instead.");
      }
    });

    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    if (mq.addEventListener) mq.addEventListener("change", readColors);
    document.querySelectorAll("[data-theme-set]").forEach((b) => b.addEventListener("click", () => setTheme(b.dataset.themeSet, true)));
    const ro = new ResizeObserver(() => {
      dirty.orbit = dirty.chart = true;
    });
    ro.observe(el.orbit);
    ro.observe(el.chart);

    // Keep the orbit panel sticky only while it fits above the fixed transport bar (viewer.css).
    const orbitPanel = document.querySelector(".orbit-panel");
    const transport = document.querySelector(".transport");
    if (orbitPanel && transport) {
      const fitSticky = () => {
        const fixed = getComputedStyle(transport).position === "fixed";
        const barH = fixed ? transport.getBoundingClientRect().height : 0;
        document.documentElement.style.setProperty("--transport-h", barH + "px");
        const top = parseFloat(getComputedStyle(document.documentElement).fontSize) * 0.5 || 8;
        const fits = orbitPanel.getBoundingClientRect().height + top + 8 <= window.innerHeight - barH;
        orbitPanel.classList.toggle("fits", fits);
      };
      new ResizeObserver(fitSticky).observe(orbitPanel);
      new ResizeObserver(fitSticky).observe(transport);
      window.addEventListener("resize", fitSticky);
      fitSticky();
      // scroll-padding-bottom (viewer.css) is not applied when the browser judges a newly focused
      // control already "visible" in the viewport, so a control just above the viewport bottom can
      // still sit under the bar. Nudge it clear (WCAG 2.4.11). Tall focus targets such as the
      // skip-link section are left alone.
      document.addEventListener("focusin", (ev) => {
        const t = ev.target;
        if (!(t instanceof Element) || transport.contains(t)) return;
        if (getComputedStyle(transport).position !== "fixed") return;
        const r = t.getBoundingClientRect();
        const barTop = transport.getBoundingClientRect().top;
        if (r.height < barTop / 2 && r.bottom > barTop - 4) window.scrollBy(0, r.bottom - barTop + 12);
      });
    }
  }

  // Colour theme: "auto" follows the OS; "light"/"dark" force it. A per-viewer convenience only.
  const THEME_KEY = "ferroastro-viewer-theme";
  function setTheme(name, remember) {
    const t = name === "light" || name === "dark" ? name : "auto";
    if (t === "auto") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
    document.querySelectorAll("[data-theme-set]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.themeSet === t)));
    if (remember) {
      try {
        localStorage.setItem(THEME_KEY, t);
      } catch (e) {
        /* storage unavailable: the choice lasts for this page view */
      }
    }
    readColors();
    // Browser chrome follows the page: a forced theme overrides both media-scoped theme-color metas.
    document.querySelectorAll('meta[name="theme-color"]').forEach((m) => {
      if (!m.dataset.orig) m.dataset.orig = m.getAttribute("content");
      m.setAttribute("content", t === "auto" ? m.dataset.orig : colors["--bg"]);
    });
  }

  // The applied configuration lives in the URL fragment (#m=<mask>&p=<params>), so a view can be
  // bookmarked or shared. It uses replaceState, so applying a run adds no history entries; the page
  // reads the fragment at load (a pasted fragment applies on reload). readConfig still validates
  // every value.
  function writeHash(cfg) {
    const vals = [];
    for (let i = 0; i < FAV.N_PARAMS; i++) vals.push(Number.isNaN(cfg.params[i]) ? "NaN" : String(cfg.params[i]));
    const h = "#m=" + (cfg.mask >>> 0) + "&p=" + vals.join(",");
    if (location.hash !== h) {
      try {
        history.replaceState(null, "", h);
      } catch (e) {
        /* sandboxed frames may refuse; the page works without it */
      }
    }
  }
  function readHash() {
    const m = /^#m=(\d+)&p=([-+0-9.eEaN,]+)$/.exec(location.hash);
    if (!m) return null;
    const vals = m[2].split(",");
    if (vals.length !== FAV.N_PARAMS) return null;
    const params = {};
    for (let i = 0; i < vals.length; i++) {
      const v = vals[i] === "NaN" ? NaN : Number(vals[i]);
      if (vals[i] !== "NaN" && !Number.isFinite(v)) return null;
      params[i] = v;
    }
    const mask = Number(m[1]) >>> 0;
    if ((mask & FAV.RESERVED_MASK) !== 0 || (params[P.MODE] !== 0 && params[P.MODE] !== 1)) return null;
    return { mask, params };
  }

  async function boot() {
    let savedTheme = "auto";
    try {
      savedTheme = localStorage.getItem(THEME_KEY) || "auto";
    } catch (e) {
      /* storage unavailable */
    }
    setTheme(savedTheme, false);
    readColors();
    resetData();
    wire();
    syncToggleUI();
    const fromHash = readHash();
    if (fromHash) {
      try {
        writeConfig(fromHash);
      } catch (e) {
        /* an unusable fragment falls back to the defaults */
      }
    }
    updateTransport();
    updateAdvance();
    requestAnimationFrame(frame);

    if (location.protocol === "file:") {
      showError("Open this page through a local web server, not as a file: browsers do not run Web Workers or fetch wasm from file:// URLs.\n  cd crates/ferroastro-viewer/web && python3 -m http.server 8000\nthen open http://localhost:8000/");
      setStatus("Not started.");
      return;
    }
    try {
      const resp = await fetch(WASM_URL);
      if (!resp.ok) throw new Error(WASM_URL + " not found (HTTP " + resp.status + "). Build it with crates/ferroastro-viewer/web/build.sh, which copies it next to index.html.");
      state.module = await FAV.compileResponse(resp);
      const imports = WebAssembly.Module.imports(state.module);
      if (imports.length) throw new Error(WASM_URL + " has imports; ABI v1 requires none.");
    } catch (err) {
      showError(String(err.message || err));
      setStatus("Engine not loaded.");
      return;
    }
    state.wantPlay = !window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    startLiveWorker();
  }

  // A wasm trap (panic=abort) ends the instance. worker.js then re-instantiates and reports
  // {op:"ready", recovered:true}; a worker-level crash fires onerror. Both go through here, and
  // repeated failures stop the automatic restart so a trapping configuration cannot loop.
  let crashes = [];
  function onEngineLost(what) {
    const now = performance.now();
    crashes = crashes.filter((t) => now - t < 30000).concat([now]);
    state.playing = false;
    state.configured = false;
    state.wantPlay = false;
    state.awaitingConfigured = false;
    updateTransport();
    if (crashes.length > 2) {
      showError("The engine stopped repeatedly (" + what + "). Change the settings, then press Restart.");
      return false;
    }
    showError("The engine stopped (" + what + ") and was restarted with the same settings, paused.");
    return true;
  }

  function startLiveWorker() {
    state.live = new Worker("worker.js");
    state.live.onmessage = onLiveMessage;
    state.live.onerror = (e) => {
      e.preventDefault();
      state.liveReady = false;
      state.live.terminate();
      const again = onEngineLost(e.message || "worker error");
      if (again) startLiveWorker();
      else state.live = null;
    };
    state.live.postMessage({ op: "init", module: state.module });
  }

  boot();
})();
