// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// fav.js: constants of the ferroastro-viewer C ABI v2 (docs/viewer/DESIGN.md section 4) and a small
// wrapper around the wasm exports. It is shared by worker.js (classic worker, importScripts) and
// smoke.mjs (node, loaded with vm). It does no physics: it calls the fav_* exports, copies buffers out
// of linear memory and decodes UTF-8.
//
// Buffer rule (section 4.1): a pointer is valid only until the next fav_* call other than its _len
// partner, and memory may grow during any call. So every read here is ptr(), len(), then a fresh view
// on memory.buffer that is .slice()d immediately. JS never writes into wasm memory.

(function (root) {
  "use strict";

  const FAV = {};

  FAV.ABI_VERSION = 2;

  FAV.STATUS = Object.freeze({
    OK: 0,
    DONE: 1,
    E_STATE: -1,
    E_ARG: -2,
    E_UNSUPPORTED: -3,
    E_RUN: -4,
    E_FIT: -5,
  });

  FAV.STATUS_NAME = Object.freeze({
    0: "FAV_OK",
    1: "FAV_DONE",
    "-1": "FAV_E_STATE",
    "-2": "FAV_E_ARG",
    "-3": "FAV_E_UNSUPPORTED",
    "-4": "FAV_E_RUN",
    "-5": "FAV_E_FIT",
  });

  // Toggle mask bits (section 3.1). Bit 12 (ABI v2) is the conformal-gravity comparison model,
  // outside the GR ladder; bits 13-31 are reserved and must be 0.
  FAV.BIT = Object.freeze({
    VENUS: 0,
    EMB: 1,
    MARS: 2,
    JUPITER: 3,
    SATURN: 4,
    URANUS: 5,
    NEPTUNE: 6,
    SUN_J2: 7,
    GR_1PN: 8,
    LENSE_THIRRING: 9,
    ASTEROIDS: 10,
    EIH: 11,
    CONFORMAL: 12,
  });
  FAV.N_BITS = 13;
  FAV.DEFAULT_MASK = 0x3ff; // bits 0-9
  FAV.PLANET_MASK = 0x7f; // bits 0-6
  FAV.RESERVED_MASK = 0xffffe000;

  // Parameter ids (section 4.3).
  FAV.PARAM = Object.freeze({
    MODE: 0,
    ENGINE: 1,
    PPN_BETA: 2,
    PPN_GAMMA: 3,
    WH_STEP_DAYS: 4,
    IAS15_EPSILON: 5,
    SPAN_YEARS: 6,
    DISPLAY_CADENCE_DAYS: 7,
    ASTEROID_COUNT: 8,
    RUNG: 9,
    J2_OVERRIDE: 10,
    SPIN_SCALE: 11,
    FIT_PERIODIC: 12,
    CG_GAMMA0: 13,
    CG_GAMMA_STAR: 14,
    CG_KAPPA: 15,
    CG_GAMMA0_CENTRE: 16,
  });
  FAV.N_PARAMS = 17;
  // ABI v1 had 13 parameters: a v1 URL fragment is read with ids 13-16 at their defaults.
  FAV.N_PARAMS_V1 = 13;

  // Defaults as documented; the engine is the authority (fav_reset restores them).
  FAV.PARAM_DEFAULTS = Object.freeze({
    0: 0,
    1: 0,
    2: 1,
    3: 1,
    4: 1.0,
    5: 1e-9,
    6: 1000,
    7: 4,
    8: 16,
    9: 4,
    10: NaN,
    11: 1,
    12: 1,
    // Conformal gravity (bit 12): the published galactic-fit values, UNVERIFIED (OI-79), display
    // defaults only; the engine is the authority.
    13: 3.06e-30,
    14: 5.42e-41,
    15: 9.54e-54,
    16: 0,
  });
  FAV.CG_CENTRE = Object.freeze({ UNIFORM: 0, SUN: 1 });

  FAV.MODE = Object.freeze({ SYSTEM: 0, RUNG_LAB: 1 });
  FAV.ENGINE_PARAM = Object.freeze({ AUTO: 0, WHFAST: 1, IAS15: 2 });
  FAV.ENGINE_RESOLVED = Object.freeze({ WHFAST: 1, IAS15: 2, IAS15_TEST_PARTICLE: 3 });
  FAV.ENGINE_NAME = Object.freeze({ 1: "WHFast", 2: "IAS15", 3: "IAS15 (test particle)" });

  FAV.STRIDE = Object.freeze({ bodies: 25, elements: 6, passages: 6, fit: 10 });

  // Slot order of the bodies buffer (section 4.4 A), after the leading t.
  FAV.BODY_NAMES = Object.freeze([
    "Mercury",
    "Venus",
    "EMB",
    "Mars",
    "Jupiter",
    "Saturn",
    "Uranus",
    "Neptune",
  ]);

  // Field offsets inside one record.
  FAV.ELEM = Object.freeze({ T: 0, VARPI_DEG: 1, E: 2, A_AU: 3, I_DEG: 4, VARPI_ECL_DEG: 5 });
  FAV.PASS = Object.freeze({ T: 0, VARPI_ARCSEC: 1, X: 2, Y: 3, Z: 4, DIR_ERR_ARCSEC: 5 });
  FAV.FIT = Object.freeze({
    RATE: 0,
    FORMAL_SIGMA: 1,
    JACKKNIFE_SIGMA: 2,
    Q: 3,
    RMS: 4,
    N_PASSAGES: 5,
    T_FIRST: 6,
    T_LAST: 7,
    N_PERIODIC: 8,
    B: 9,
  });

  FAV.REQUIRED_EXPORTS = Object.freeze([
    "memory",
    "fav_abi_version",
    "fav_reset",
    "fav_set_toggles",
    "fav_set_param",
    "fav_get_param",
    "fav_configure",
    "fav_run",
    "fav_time_years",
    "fav_epoch_jd",
    "fav_engine",
    "fav_drain",
    "fav_bodies_ptr",
    "fav_bodies_len",
    "fav_elements_ptr",
    "fav_elements_len",
    "fav_passages_ptr",
    "fav_passages_len",
    "fav_fit",
    "fav_fit_ptr",
    "fav_fit_len",
    "fav_meta_ptr",
    "fav_meta_len",
    "fav_scenario_ptr",
    "fav_scenario_len",
    "fav_error_ptr",
    "fav_error_len",
  ]);

  /** An ABI error: a negative status plus the engine's message (which starts with FA-Exxxx). */
  class FavError extends Error {
    constructor(status, message, call) {
      super(message || FAV.STATUS_NAME[status] || "status " + status);
      this.name = "FavError";
      this.status = status;
      this.statusName = FAV.STATUS_NAME[status] || "status " + status;
      this.call = call || "";
    }
  }
  FAV.FavError = FavError;

  const decoder = typeof TextDecoder !== "undefined" ? new TextDecoder("utf-8") : null;

  /** Wraps the exports object of an instantiated module. */
  class Engine {
    constructor(exports) {
      const missing = FAV.REQUIRED_EXPORTS.filter((n) => !(n in exports));
      if (missing.length > 0) {
        throw new Error("ferroastro_viewer.wasm lacks exports: " + missing.join(", "));
      }
      this.x = exports;
      const abi = this.x.fav_abi_version() >>> 0;
      if (abi !== FAV.ABI_VERSION) {
        throw new Error("unsupported ABI version " + abi + " (this page speaks " + FAV.ABI_VERSION + ")");
      }
      this.abi = abi;
    }

    // ---- low-level readers -------------------------------------------------------------------

    /** Copies an f64 buffer named `name` (bodies, elements, passages, fit). */
    f64(name) {
      const ptr = this.x["fav_" + name + "_ptr"]() >>> 0;
      const len = this.x["fav_" + name + "_len"]() >>> 0;
      if (len === 0) return new Float64Array(0);
      // Fresh view after both calls (memory may have grown), then copy at once.
      return new Float64Array(this.x.memory.buffer, ptr, len).slice();
    }

    /** Copies and decodes a UTF-8 buffer named `name` (meta, scenario, error). */
    text(name) {
      const ptr = this.x["fav_" + name + "_ptr"]() >>> 0;
      const len = this.x["fav_" + name + "_len"]() >>> 0;
      if (len === 0) return "";
      const bytes = new Uint8Array(this.x.memory.buffer, ptr, len).slice();
      return decoder ? decoder.decode(bytes) : String.fromCharCode.apply(null, bytes);
    }

    errorMessage() {
      return this.text("error");
    }

    /** Throws FavError for a negative status (reading the message first); returns the status. */
    check(status, call) {
      status = status | 0;
      if (status < 0) throw new FavError(status, this.errorMessage(), call);
      return status;
    }

    // ---- calls ---------------------------------------------------------------------------------

    reset() {
      return this.check(this.x.fav_reset(), "fav_reset");
    }
    setToggles(mask) {
      return this.check(this.x.fav_set_toggles(mask >>> 0), "fav_set_toggles");
    }
    setParam(id, value) {
      return this.check(this.x.fav_set_param(id >>> 0, Number(value)), "fav_set_param(" + id + ")");
    }
    getParam(id) {
      return this.x.fav_get_param(id >>> 0);
    }
    timeYears() {
      return this.x.fav_time_years();
    }
    epochJd() {
      return this.x.fav_epoch_jd();
    }
    engine() {
      return this.x.fav_engine() >>> 0;
    }

    /**
     * reset, set the mask and every given parameter, configure. `params` maps id -> value.
     * Returns {engine, epochJd, meta, scenario}. Throws FavError.
     */
    configure(mask, params) {
      this.reset();
      this.setToggles(mask);
      const ids = Object.keys(params || {})
        .map(Number)
        .sort((a, b) => a - b);
      for (const id of ids) this.setParam(id, params[id]);
      this.check(this.x.fav_configure(), "fav_configure");
      return {
        engine: this.engine(),
        epochJd: this.epochJd(),
        meta: this.meta(),
        scenario: this.text("scenario"),
      };
    }

    /** One fav_run call: returns FAV_OK or FAV_DONE, throws on error. */
    run(years) {
      return this.check(this.x.fav_run(years), "fav_run");
    }

    /** Copies the three drain buffers, then calls fav_drain. */
    takeChunk() {
      const bodies = this.f64("bodies");
      const elements = this.f64("elements");
      const passages = this.f64("passages");
      this.check(this.x.fav_drain(), "fav_drain");
      return { bodies, elements, passages };
    }

    /**
     * fav_fit over [t0, t1] (NaN, NaN = all passages so far). Returns {status, fit, meta, message}.
     * FAV_E_FIT is not fatal and is returned, not thrown; other errors throw.
     */
    fit(t0, t1) {
      const st = this.x.fav_fit(t0, t1) | 0;
      if (st === FAV.STATUS.E_FIT) {
        return { status: st, fit: null, meta: null, message: this.errorMessage() };
      }
      this.check(st, "fav_fit");
      return { status: st, fit: this.f64("fit"), meta: this.meta(), message: "" };
    }

    meta() {
      const s = this.text("meta");
      if (!s) return null;
      try {
        return JSON.parse(s);
      } catch (e) {
        return { unparsed: s };
      }
    }

    scenario() {
      return this.text("scenario");
    }
  }
  FAV.Engine = Engine;

  /** Wraps an exports object (for tests that supply their own). */
  FAV.wrap = function (exports) {
    return new Engine(exports);
  };

  /**
   * Instantiates the module. `source` is a WebAssembly.Module, an ArrayBuffer/typed array, or a
   * Response. Refuses a module that has imports (the contract has none).
   */
  FAV.instantiate = async function (source) {
    let module;
    if (source instanceof WebAssembly.Module) {
      module = source;
    } else if (typeof Response !== "undefined" && source instanceof Response) {
      module = await FAV.compileResponse(source);
    } else {
      module = await WebAssembly.compile(source);
    }
    const imports = WebAssembly.Module.imports(module);
    if (imports.length !== 0) {
      throw new Error(
        "ferroastro_viewer.wasm has " + imports.length + " imports; ABI v1 requires none (" +
          imports.map((i) => i.module + "." + i.name).join(", ") + ")",
      );
    }
    const instance = await WebAssembly.instantiate(module, {});
    return new Engine(instance.exports);
  };

  /** compileStreaming, falling back to arrayBuffer when the server sends a wrong MIME type. */
  FAV.compileResponse = async function (response) {
    if (!response.ok) {
      throw new Error("HTTP " + response.status + " fetching " + (response.url || "the wasm module"));
    }
    if (typeof WebAssembly.compileStreaming === "function") {
      try {
        return await WebAssembly.compileStreaming(response.clone());
      } catch (e) {
        // Usually "Incorrect response MIME type"; fall through.
      }
    }
    return WebAssembly.compile(await response.arrayBuffer());
  };

  /** Formats a FavError or Error for display. */
  FAV.describeError = function (err) {
    if (err && err.name === "FavError") {
      return err.statusName + (err.call ? " from " + err.call : "") + ": " + err.message;
    }
    return String((err && err.message) || err);
  };

  root.FAV = FAV;
})(typeof self !== "undefined" ? self : globalThis);
