// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Shared helpers of the viewer tests: run a configuration through the rlib API (the code the
//! `fav_*` exports wrap) and fit it; the viewer vectors.

#![allow(dead_code, reason = "each test binary uses a subset")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers: a failed unwrap is a test failure"
)]

use ferroastro_vectors::VectorSet;
use ferroastro_viewer::config::Param;
use ferroastro_viewer::{FAV_DONE, FAV_OK, Viewer};

/// True if System mode can run (the crate was built with `embed-header`); otherwise prints why
/// `test` is skipped. System-mode initial conditions come from the JPL header.440, which only the
/// `embed-header` feature includes (DESIGN §5, #22).
pub(crate) fn system_mode_or_skip(test: &str) -> bool {
    if ferroastro_viewer::ic::EMBED_HEADER {
        return true;
    }
    eprintln!(
        "SKIP {test}: built without embed-header, so System mode has no header.440 initial \
         conditions; run `cargo test -p ferroastro-viewer --features embed-header` with \
         FA_DATA_DIR set (or after `tools/fetch_jpl.py de440-header`)"
    );
    false
}

/// True if the Rung lab can run (the crate was built with `embed-header`); otherwise prints why
/// `test` is skipped. Its DE440 constants come from the embedded header.440 too (#22: the
/// committed constants table is gone; DESIGN §5).
pub(crate) fn rung_lab_or_skip(test: &str) -> bool {
    if ferroastro_viewer::ic::EMBED_HEADER {
        return true;
    }
    eprintln!(
        "SKIP {test}: built without embed-header, so the Rung lab has no DE440 constants; run \
         `cargo test -p ferroastro-viewer --features embed-header` with FA_DATA_DIR set (or after \
         `tools/fetch_jpl.py de440-header`)"
    );
    false
}

/// The workspace test vectors.
pub(crate) fn vectors() -> VectorSet {
    VectorSet::load_workspace().expect("tests/vectors")
}

/// `ENGINE = IAS15`.
pub(crate) const IAS15: (Param, f64) = (Param::Engine, 2.0);
/// `ENGINE = WHFAST`.
pub(crate) const WHFAST: (Param, f64) = (Param::Engine, 1.0);
/// `MODE = Rung lab`.
pub(crate) const LAB: (Param, f64) = (Param::Mode, 1.0);

/// A configured viewer.
pub(crate) fn configured(mask: u32, params: &[(Param, f64)], years: u32) -> Viewer {
    let mut v = Viewer::new();
    assert_eq!(v.set_toggles(mask), FAV_OK, "{}", v.error());
    assert_eq!(
        v.set_param(Param::SpanYears as u32, f64::from(years)),
        FAV_OK,
        "{}",
        v.error()
    );
    for (p, x) in params {
        assert_eq!(v.set_param(*p as u32, *x), FAV_OK, "{}", v.error());
    }
    assert_eq!(v.configure(), FAV_OK, "{}", v.error());
    v
}

/// Run a configured viewer to the end of its span in `chunk`-year calls, draining as a page
/// would; returns the number of calls.
pub(crate) fn run_to_end(v: &mut Viewer, chunk: f64) -> usize {
    let mut calls = 0;
    loop {
        let s = v.run(chunk);
        calls += 1;
        assert!(s == FAV_OK || s == FAV_DONE, "status {s}: {}", v.error());
        if s == FAV_DONE {
            return calls;
        }
        assert!(calls < 1_000_000, "no progress"); // lint: allow-literal(a loop guard, not a physical value)
    }
}

/// The fit record of a whole run of `mask` with `params` over `years`.
pub(crate) fn fit_of(mask: u32, params: &[(Param, f64)], years: u32) -> Vec<f64> {
    let mut v = configured(mask, params, years);
    run_to_end(&mut v, 5.0);
    assert_eq!(v.fit(f64::NAN, f64::NAN), FAV_OK, "{}", v.error());
    println!("mask {mask:#x} {params:?} {years} yr: {}", v.meta_json());
    v.fit_record().to_vec()
}

/// The fitted rate (″/cy) of a whole run.
pub(crate) fn rate(mask: u32, params: &[(Param, f64)], years: u32) -> f64 {
    fit_of(mask, params, years).first().copied().unwrap()
}

/// Check `measured` against vector `id`, printing the deviation.
pub(crate) fn check(vs: &VectorSet, id: &str, measured: f64) {
    let v = vs.get(id).unwrap();
    match v.check(measured) {
        Ok(d) => println!(
            "{id}: measured {measured} expected {} |diff| {d}",
            v.value().unwrap()
        ),
        Err(e) => panic!("{e}"),
    }
}
