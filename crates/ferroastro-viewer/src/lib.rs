// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! # ferroastro-viewer
//!
//! The engine of the FerroAstro orbit viewer (CRATE-PLAN L12, surfaces; API-004;
//! `docs/viewer/DESIGN.md`): Mercury's orbit and perihelion, with the planets, solar J2, GR,
//! Lense–Thirring and asteroids switched on and off, built only from FerroAstro crates and
//! exported to the browser as a `wasm32-unknown-unknown` module with **no imports** and a small
//! `extern "C"` ABI (§4 of the design; the UI lives in `web/`).
//!
//! * **System mode** builds a [`ferroastro_scenario::ScenarioSpec`] from the toggles
//!   ([`ViewerConfig::to_scenario`]) and runs it with
//!   [`ferroastro_propagate::run_until`] / [`ferroastro_propagate::resume_until`] (the path of the
//!   CLI and the diffrun harness), from the DE440 `header.440` initial conditions ([`ic`]),
//!   which the `embed-header` feature includes at build time from the fetched, hash-pinned JPL
//!   file (never committed, #22). Without the feature the crate still builds, and System mode
//!   refuses with [`FAV_E_UNSUPPORTED`] ("built without embed-header").
//!   Perihelion passages are located by the Park2017 estimator in Mercury's sampled
//!   heliocentric states (the `SampledStates` path of [`ferroastro_diffrun::Estimator`]).
//! * **Rung lab** integrates Mercury as a test particle about a fixed Sun (the M2 single-body
//!   rungs, the only place for ADR-0012's SR rungs) with IAS15 driven here, so no
//!   `std::time::Instant` is used on wasm32.
//!
//! No force model, integrator or estimator is written in this crate: it composes them.
//!
//! ## ABI version 1
//!
//! Every export is `extern "C"`, prefixed `fav_`, and takes and returns `i32`, `u32` or `f64`;
//! pointer results are `*const f64` / `*const u8` (on wasm32, byte offsets into the exported
//! `memory`). A pointer is valid until the next `fav_*` call other than its `_len` partner.
//!
//! | status | value | meaning |
//! |---|---|---|
//! | [`FAV_OK`] | 0 | success |
//! | [`FAV_DONE`] | 1 | the span is reached |
//! | [`FAV_E_STATE`] | −1 | call out of order |
//! | [`FAV_E_ARG`] | −2 | bad id, value out of range, reserved bit |
//! | [`FAV_E_UNSUPPORTED`] | −3 | propagate refused the combination (`FA-E1201`), or System mode in a build without `embed-header` (`FA-E0303`) |
//! | [`FAV_E_RUN`] | −4 | propagation or model error |
//! | [`FAV_E_FIT`] | −5 | too few passages (not fatal) |
//!
//! Toggle bits ([`config::bits`]): 0 Venus, 1 Earth–Moon barycentre, 2 Mars, 3 Jupiter,
//! 4 Saturn, 5 Uranus, 6 Neptune, 7 solar J2, 8 GR 1PN (Sun), 9 Lense–Thirring, 10 asteroids,
//! 11 EIH among all bodies; 12–31 reserved. Parameter ids: [`config::Param`].
//!
//! Buffers (f64, row-major; `t` in TDB Julian years since [`Viewer::epoch_jd`]):
//!
//! | buffer | stride | record |
//! |---|---|---|
//! | bodies | [`BODIES_STRIDE`] | `t`, then heliocentric ECLIPJ2000 `x, y, z` (au) of Mercury, Venus, EMB, Mars, Jupiter, Saturn, Uranus, Neptune (NaN when off) |
//! | elements | [`ELEMENTS_STRIDE`] | `t, varpi_deg, e, a_au, i_deg, varpi_ecl_deg` |
//! | passages | [`PASSAGES_STRIDE`] | `t, varpi_proj_arcsec, x_au, y_au, z_au, direction_error_arcsec` |
//! | fit | [`FIT_STRIDE`] | `rate, formal_sigma, jackknife_sigma, Q, rms, n, t_first, t_last, n_periodic, B` |
//!
//! ```
//! use ferroastro_viewer::{Viewer, FAV_E_UNSUPPORTED, FAV_OK, config::Param, ic};
//! let mut v = Viewer::new();
//! assert_eq!(v.set_toggles(0), FAV_OK); // Kepler only: Sun + Mercury
//! assert_eq!(v.set_param(Param::SpanYears as u32, 1.0), FAV_OK);
//! if ic::EMBED_HEADER {
//!     assert_eq!(v.configure(), FAV_OK);
//!     assert!(v.run(0.5) >= FAV_OK);
//!     assert!(!v.bodies().is_empty());
//! } else {
//!     // Built without `embed-header`: no header.440 initial conditions.
//!     assert_eq!(v.configure(), FAV_E_UNSUPPORTED);
//!     assert!(v.error().contains("built without embed-header"));
//! }
//! ```

// No crate-level `forbid(unsafe_code)`: it could not be relaxed for the `no_mangle` attribute
// of `ffi` (DESIGN §2.2). Every other module forbids it itself; the lint table denies it.

pub mod config;
mod engine;
pub mod ffi;
mod fit;
pub mod ic;
mod lab;
mod meta;
mod observe;

pub use config::ViewerConfig;
pub use engine::{Viewer, estimator};

/// ABI version of the `fav_*` exports.
pub const ABI_VERSION: u32 = 1;
/// Success.
pub const FAV_OK: i32 = 0;
/// The span is reached; nothing more to run.
pub const FAV_DONE: i32 = 1;
/// A call out of order (e.g. `fav_run` before `fav_configure`).
pub const FAV_E_STATE: i32 = -1;
/// A bad id, an out-of-range value or a reserved toggle bit.
pub const FAV_E_ARG: i32 = -2;
/// The engine refuses the combination (propagate's `FA-E1201`), or System mode in a build
/// without the `embed-header` feature (`FA-E0303`, [`ic::WITHOUT_EMBED_HEADER`]).
pub const FAV_E_UNSUPPORTED: i32 = -3;
/// A propagation or model error.
pub const FAV_E_RUN: i32 = -4;
/// The estimator has too few passages or too short a span (not fatal).
pub const FAV_E_FIT: i32 = -5;

/// f64 per record of the bodies buffer.
pub const BODIES_STRIDE: usize = 25;
/// f64 per record of the elements buffer.
pub const ELEMENTS_STRIDE: usize = 6;
/// f64 per record of the passages buffer.
pub const PASSAGES_STRIDE: usize = 6;
/// f64 in the fit buffer.
pub const FIT_STRIDE: usize = 10;

/// Resolved engine: `WHFast` (System mode).
pub const ENGINE_WHFAST: u32 = 1;
/// Resolved engine: IAS15 (System mode).
pub const ENGINE_IAS15: u32 = 2;
/// Resolved engine: IAS15 on a test particle (Rung lab).
pub const ENGINE_IAS15_TEST_PARTICLE: u32 = 3;

/// An error with its ABI status and a message that starts with the `FA-Exxxx` code when there
/// is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerError {
    /// One of the negative `FAV_E_*` statuses.
    pub status: i32,
    /// UTF-8 message.
    pub message: String,
}

impl ViewerError {
    pub(crate) fn new(status: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
    pub(crate) fn state(message: impl Into<String>) -> Self {
        Self::new(FAV_E_STATE, message)
    }
    pub(crate) fn arg(message: impl Into<String>) -> Self {
        Self::new(FAV_E_ARG, message)
    }
    pub(crate) fn run(message: impl Into<String>) -> Self {
        Self::new(FAV_E_RUN, message)
    }
    pub(crate) fn fit(message: impl Into<String>) -> Self {
        Self::new(FAV_E_FIT, message)
    }
}

impl std::fmt::Display for ViewerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (status {})", self.message, self.status)
    }
}

impl std::error::Error for ViewerError {}

/// Seconds per Julian year (365.25 d of 86400 s).
pub(crate) const SECONDS_PER_YEAR: f64 =
    ferroastro_scenario::DAYS_PER_JULIAN_YEAR * ferroastro_core::time::SECONDS_PER_DAY;
