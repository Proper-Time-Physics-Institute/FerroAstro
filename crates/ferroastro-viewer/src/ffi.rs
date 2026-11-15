// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The C ABI, version 2 (DESIGN §4): `extern "C"` shims over one [`Viewer`] per instance (per
//! wasm module instance, i.e. per Web Worker; per thread on native).
//!
//! DESIGN §2.2 (NEEDS HUMAN REVIEW): this is the only module that allows the `unsafe_code`
//! lint, and only because a wasm export needs the `no_mangle` attribute, which edition 2024
//! spells as an unsafe attribute. The module contains no unsafe block, function or impl and
//! dereferences no raw pointer: pointers leave as return values (made from slices, which is
//! safe), every input is a scalar, and JS never writes into wasm memory. The test
//! `ffi_contains_no_unsafe_code` keeps it that way.
//!
//! A pointer is valid until the next `fav_*` call other than its `_len` partner: memory may grow
//! during any call, so JS copies a buffer out (`new Float64Array(memory.buffer, ptr, len).slice()`)
//! right away.

#![allow(unsafe_code)]

use crate::Viewer;
use std::cell::RefCell;

thread_local! {
    static VIEWER: RefCell<Viewer> = RefCell::new(Viewer::new());
}

/// Run `f` on this instance's viewer; `default` if it is unavailable (never in practice: the
/// exports do not re-enter).
fn with<T>(default: T, f: impl FnOnce(&mut Viewer) -> T) -> T {
    VIEWER
        .try_with(|cell| cell.try_borrow_mut().ok().map(|mut v| f(&mut v)))
        .ok()
        .flatten()
        .unwrap_or(default)
}

fn len_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// ABI version (2). JS refuses any other value.
#[unsafe(no_mangle)]
pub extern "C" fn fav_abi_version() -> u32 {
    crate::ABI_VERSION
}

/// Drop the run; toggles and parameters return to their defaults.
#[unsafe(no_mangle)]
pub extern "C" fn fav_reset() -> i32 {
    with(crate::FAV_E_STATE, Viewer::reset)
}

/// Set the toggle mask (effective at the next `fav_configure`).
#[unsafe(no_mangle)]
pub extern "C" fn fav_set_toggles(mask: u32) -> i32 {
    with(crate::FAV_E_STATE, |v| v.set_toggles(mask))
}

/// Set a parameter (range-checked now, effective at the next `fav_configure`).
#[unsafe(no_mangle)]
pub extern "C" fn fav_set_param(id: u32, value: f64) -> i32 {
    with(crate::FAV_E_STATE, |v| v.set_param(id, value))
}

/// A parameter's value (NaN for an unknown id).
#[unsafe(no_mangle)]
pub extern "C" fn fav_get_param(id: u32) -> f64 {
    with(f64::NAN, |v| v.get_param(id))
}

/// Validate, resolve the engine, build the scenario and ICs, start a run at t = 0.
#[unsafe(no_mangle)]
pub extern "C" fn fav_configure() -> i32 {
    with(crate::FAV_E_STATE, Viewer::configure)
}

/// Advance by `years` (0 < years ≤ 50); 1 (`FAV_DONE`) at the end of the span.
#[unsafe(no_mangle)]
pub extern "C" fn fav_run(years: f64) -> i32 {
    with(crate::FAV_E_STATE, |v| v.run(years))
}

/// TDB Julian years since the epoch reached so far.
#[unsafe(no_mangle)]
pub extern "C" fn fav_time_years() -> f64 {
    with(f64::NAN, |v| v.time_years())
}

/// Run epoch, TDB Julian date.
#[unsafe(no_mangle)]
pub extern "C" fn fav_epoch_jd() -> f64 {
    with(f64::NAN, |v| v.epoch_jd())
}

/// Resolved engine: 1 `WHFast`, 2 IAS15, 3 IAS15 test particle (0 without a run).
#[unsafe(no_mangle)]
pub extern "C" fn fav_engine() -> u32 {
    with(0, |v| v.engine())
}

/// Empty the drain buffers.
#[unsafe(no_mangle)]
pub extern "C" fn fav_drain() -> i32 {
    with(crate::FAV_E_STATE, Viewer::drain)
}

/// Bodies buffer (stride 25).
#[unsafe(no_mangle)]
pub extern "C" fn fav_bodies_ptr() -> *const f64 {
    with(std::ptr::null(), |v| v.bodies().as_ptr())
}

/// Length of the bodies buffer, in f64.
#[unsafe(no_mangle)]
pub extern "C" fn fav_bodies_len() -> u32 {
    with(0, |v| len_u32(v.bodies().len()))
}

/// Elements buffer (stride 6).
#[unsafe(no_mangle)]
pub extern "C" fn fav_elements_ptr() -> *const f64 {
    with(std::ptr::null(), |v| v.elements().as_ptr())
}

/// Length of the elements buffer, in f64.
#[unsafe(no_mangle)]
pub extern "C" fn fav_elements_len() -> u32 {
    with(0, |v| len_u32(v.elements().len()))
}

/// Passages buffer (stride 6).
#[unsafe(no_mangle)]
pub extern "C" fn fav_passages_ptr() -> *const f64 {
    with(std::ptr::null(), |v| v.passages().as_ptr())
}

/// Length of the passages buffer, in f64.
#[unsafe(no_mangle)]
pub extern "C" fn fav_passages_len() -> u32 {
    with(0, |v| len_u32(v.passages().len()))
}

/// Fit ϖ̇ over the passages with t in `[t0, t1]` years (NaN, NaN: all).
#[unsafe(no_mangle)]
pub extern "C" fn fav_fit(t0_years: f64, t1_years: f64) -> i32 {
    with(crate::FAV_E_STATE, |v| v.fit(t0_years, t1_years))
}

/// Fit record (stride 10).
#[unsafe(no_mangle)]
pub extern "C" fn fav_fit_ptr() -> *const f64 {
    with(std::ptr::null(), |v| v.fit_record().as_ptr())
}

/// Length of the fit record, in f64 (0 or 10).
#[unsafe(no_mangle)]
pub extern "C" fn fav_fit_len() -> u32 {
    with(0, |v| len_u32(v.fit_record().len()))
}

/// Metadata JSON, UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn fav_meta_ptr() -> *const u8 {
    with(std::ptr::null(), |v| v.meta_bytes().as_ptr())
}

/// Length of the metadata JSON, in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fav_meta_len() -> u32 {
    with(0, |v| len_u32(v.meta_bytes().len()))
}

/// The run's scenario as TOML, UTF-8.
#[unsafe(no_mangle)]
pub extern "C" fn fav_scenario_ptr() -> *const u8 {
    with(std::ptr::null(), |v| v.scenario_bytes().as_ptr())
}

/// Length of the scenario TOML, in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fav_scenario_len() -> u32 {
    with(0, |v| len_u32(v.scenario_bytes().len()))
}

/// Last error message, UTF-8 (starts with the `FA-Exxxx` code when there is one).
#[unsafe(no_mangle)]
pub extern "C" fn fav_error_ptr() -> *const u8 {
    with(std::ptr::null(), |v| v.error().as_ptr())
}

/// Length of the last error message, in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fav_error_len() -> u32 {
    with(0, |v| len_u32(v.error().len()))
}
