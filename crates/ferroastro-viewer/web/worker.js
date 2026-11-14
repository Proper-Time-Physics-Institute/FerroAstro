// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// worker.js: hosts one ferroastro_viewer.wasm instance (classic Web Worker). The page runs one "live"
// worker and N "leg" workers (docs/viewer/DESIGN.md section 7.2). All physics happens in the wasm
// engine; this file only sizes the fav_run chunks, copies buffers and posts them.
//
// Protocol (main -> worker):
//   {op:"init", module | wasmUrl}               -> {op:"ready", abi}
//   {op:"configure", mask, params}              -> {op:"configured", engine, epochJd, meta, scenario}
//   {op:"run", years}                           -> {op:"chunk", ...}           (one call)
//   {op:"play", speed} / {op:"pause"} / {op:"speed", speed}   (live loop; speed = sim yr per wall s,
//                                                 Infinity = back to back)
//   {op:"fit", t0, t1, id}                      -> {op:"fit", id, status, fit, meta, message}
//   {op:"leg", id, mask, params, spanYears}     -> {op:"progress", id, t} ... {op:"legDone", id, ...}
//   {op:"cancel"}                               stops a play loop or a leg
// Errors come back as {op:"error", status, message, context, id}.

/* global FAV */
"use strict";

importScripts("fav.js");

const TARGET_MS = 25; // wall time per fav_run call (section 7.3)
const MAX_YEARS = 50; // contract limit per fav_run call
const MIN_YEARS = 1e-4;
const FRAME_S = 1 / 30; // at finite speeds a chunk covers at most one 30 Hz frame of sim time

let eng = null;
let engineKind = 0;
let chunkYears = 0.02;
let playing = false;
let speed = 1;
let loopTimer = 0;
let generation = 0; // bumps on configure/cancel so stale loops stop

function post(msg, transfer) {
  self.postMessage(msg, transfer || []);
}

function postError(err, context, id) {
  if (typeof WebAssembly !== "undefined" && err instanceof WebAssembly.RuntimeError) {
    // A trap (panic=abort, unreachable) leaves the instance unusable: start a fresh one.
    recover(String(err.message || err), context);
    return;
  }
  post({
    op: "error",
    status: err && typeof err.status === "number" ? err.status : FAV.STATUS.E_RUN,
    message: FAV.describeError(err),
    raw: err && err.message ? err.message : String(err),
    context: context || "",
    id: id,
  });
}

function startChunk(kind) {
  // Section 7.3: start at 0.05 yr for WH and 0.02 yr for IAS15, then adapt.
  return kind === FAV.ENGINE_RESOLVED.WHFAST ? 0.05 : 0.02;
}

function adapt(years, ms) {
  const f = ms > 0.5 ? TARGET_MS / ms : 2;
  const next = years * Math.min(2, Math.max(0.5, f));
  return Math.min(MAX_YEARS, Math.max(MIN_YEARS, next));
}

/** One fav_run of `years`, copy + drain, timed. */
function runOnce(years) {
  const t0 = performance.now();
  const status = eng.run(years);
  const ms = performance.now() - t0;
  const chunk = eng.takeChunk();
  return { status, ms, chunk, t: eng.timeYears() };
}

function postChunk(r, years) {
  post(
    {
      op: "chunk",
      bodies: r.chunk.bodies,
      elements: r.chunk.elements,
      passages: r.chunk.passages,
      t: r.t,
      status: r.status,
      ms: r.ms,
      years: years,
    },
    [r.chunk.bodies.buffer, r.chunk.elements.buffer, r.chunk.passages.buffer],
  );
}

function stopLoop() {
  playing = false;
  if (loopTimer) clearTimeout(loopTimer);
  loopTimer = 0;
}

function liveStep(gen) {
  loopTimer = 0;
  if (!playing || gen !== generation || !eng) return;
  let years = chunkYears;
  if (Number.isFinite(speed)) years = Math.min(years, Math.max(MIN_YEARS, speed * FRAME_S));
  let r;
  try {
    r = runOnce(years);
  } catch (err) {
    stopLoop();
    postError(err, "run");
    return;
  }
  // Adapt the compute-bound chunk size only from calls that used it.
  if (years >= chunkYears * 0.99) chunkYears = adapt(years, r.ms);
  else if (r.ms > TARGET_MS * 2) chunkYears = adapt(chunkYears, r.ms * (chunkYears / years));
  postChunk(r, years);
  if (r.status === FAV.STATUS.DONE) {
    stopLoop();
    return;
  }
  let wait = 0;
  if (Number.isFinite(speed) && speed > 0) wait = Math.max(0, (years / speed) * 1000 - r.ms);
  loopTimer = setTimeout(() => liveStep(gen), wait);
}

function runLeg(msg) {
  const id = msg.id;
  const gen = generation;
  const params = Object.assign({}, msg.params || {});
  params[FAV.PARAM.SPAN_YEARS] = msg.spanYears;
  // Every other parameter is passed through unchanged, so all legs share one scenario recipe
  // (engine, step, cadence, estimator); the drained display buffers are simply discarded.
  let cfg;
  try {
    cfg = eng.configure(msg.mask >>> 0, params);
  } catch (err) {
    postError(err, "configure", id);
    return;
  }
  engineKind = cfg.engine;
  let years = startChunk(cfg.engine);
  let nPassages = 0;
  let lastProgress = 0;
  const legT0 = performance.now();
  post({ op: "legConfigured", id, engine: cfg.engine, meta: cfg.meta, scenario: cfg.scenario });

  const step = () => {
    if (gen !== generation) return;
    const sliceEnd = performance.now() + 100;
    let status = FAV.STATUS.OK;
    try {
      // Several calls per macrotask; yield every ~100 ms so cancel messages get through.
      do {
        const r = runOnce(years);
        nPassages += r.chunk.passages.length / FAV.STRIDE.passages;
        years = adapt(years, r.ms);
        status = r.status;
      } while (status !== FAV.STATUS.DONE && performance.now() < sliceEnd);
    } catch (err) {
      postError(err, "run", id);
      return;
    }
    const now = performance.now();
    if (now - lastProgress > 200 || status === FAV.STATUS.DONE) {
      lastProgress = now;
      post({ op: "progress", id, t: eng.timeYears(), passages: nPassages });
    }
    if (status !== FAV.STATUS.DONE) {
      setTimeout(step, 0);
      return;
    }
    let f;
    try {
      f = eng.fit(NaN, NaN);
    } catch (err) {
      postError(err, "fit", id);
      return;
    }
    const msgOut = {
      op: "legDone",
      id,
      status: f.status,
      fit: f.fit,
      meta: f.meta || eng.meta(),
      message: f.message,
      engine: cfg.engine,
      passages: nPassages,
      t: eng.timeYears(),
      wallMs: performance.now() - legT0,
    };
    post(msgOut, f.fit ? [f.fit.buffer] : []);
  };
  setTimeout(step, 0);
}

let wasmModule = null;

async function init(msg) {
  let source = msg.module;
  if (!source) source = await FAV.compileResponse(await fetch(msg.wasmUrl || "ferroastro_viewer.wasm"));
  wasmModule = source;
  eng = await FAV.instantiate(source);
  post({ op: "ready", abi: eng.abi });
}

function recover(reason, context) {
  stopLoop();
  generation++;
  eng = null;
  FAV.instantiate(wasmModule)
    .then((e) => {
      eng = e;
      post({ op: "ready", abi: e.abi, recovered: true, reason: "wasm trap in " + context + ": " + reason });
    })
    .catch((err) => post({ op: "error", status: FAV.STATUS.E_RUN, message: String(err), context: "init" }));
}

self.onmessage = (ev) => {
  const msg = ev.data || {};
  if (msg.op === "init") {
    init(msg).catch((err) => postError(err, "init"));
    return;
  }
  if (!eng) {
    postError(new Error("worker not initialised"), msg.op, msg.id);
    return;
  }
  switch (msg.op) {
    case "configure": {
      stopLoop();
      generation++;
      try {
        const cfg = eng.configure(msg.mask >>> 0, msg.params || {});
        engineKind = cfg.engine;
        chunkYears = startChunk(cfg.engine);
        post({ op: "configured", engine: cfg.engine, epochJd: cfg.epochJd, meta: cfg.meta, scenario: cfg.scenario });
      } catch (err) {
        postError(err, "configure");
      }
      break;
    }
    case "run": {
      try {
        const years = Math.min(MAX_YEARS, Math.max(MIN_YEARS, Number(msg.years) || chunkYears));
        postChunk(runOnce(years), years);
      } catch (err) {
        postError(err, "run");
      }
      break;
    }
    case "play": {
      if (typeof msg.speed === "number") speed = msg.speed;
      if (!playing) {
        playing = true;
        const gen = generation;
        loopTimer = setTimeout(() => liveStep(gen), 0);
      }
      break;
    }
    case "speed":
      if (typeof msg.speed === "number") speed = msg.speed;
      break;
    case "pause":
      stopLoop();
      break;
    case "fit": {
      try {
        const t0 = msg.t0 === null || msg.t0 === undefined ? NaN : msg.t0;
        const t1 = msg.t1 === null || msg.t1 === undefined ? NaN : msg.t1;
        const f = eng.fit(t0, t1);
        post(
          { op: "fit", id: msg.id, status: f.status, fit: f.fit, meta: f.meta, message: f.message, engine: engineKind },
          f.fit ? [f.fit.buffer] : [],
        );
      } catch (err) {
        postError(err, "fit", msg.id);
      }
      break;
    }
    case "leg":
      stopLoop();
      generation++;
      runLeg(msg);
      break;
    case "cancel":
      stopLoop();
      generation++;
      break;
    default:
      postError(new Error("unknown op " + msg.op), "dispatch", msg.id);
  }
};
