// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The C ABI (DESIGN §4) called as plain Rust: version, every status code, buffer strides and
//! lifetimes, metadata keys; and the check that `src/ffi.rs` holds no unsafe code beyond the
//! `no_mangle` attribute (DESIGN §2.2).

mod common;

use ferroastro_viewer::config::{Param, bits};
use ferroastro_viewer::ffi::*;
use ferroastro_viewer::{
    ABI_VERSION, BODIES_STRIDE, ELEMENTS_STRIDE, ENGINE_IAS15_TEST_PARTICLE, ENGINE_WHFAST,
    FAV_DONE, FAV_E_ARG, FAV_E_FIT, FAV_E_RUN, FAV_E_STATE, FAV_E_UNSUPPORTED, FAV_OK, FIT_STRIDE,
    PASSAGES_STRIDE, Viewer, ic,
};

fn id(p: Param) -> u32 {
    p as u32
}

/// A negative status leaves a message (reading it through the pointer would need `unsafe`; the
/// rlib tests below check its text).
fn has_error(len: u32) {
    assert!(len > 0, "a negative status leaves a message");
    assert!(!fav_error_ptr().is_null());
}

/// Verifies: API-004, API-002
#[test]
fn abi_version_status_codes_and_buffers() {
    assert_eq!(fav_abi_version(), ABI_VERSION);
    assert_eq!(fav_run(1.0), FAV_E_STATE);
    has_error(fav_error_len());
    assert_eq!(fav_fit(f64::NAN, f64::NAN), FAV_E_STATE);
    // ABI v2: bit 12 (conformal gravity) is defined; bit 13 is the first reserved bit.
    assert_eq!(fav_set_toggles(1 << 13), FAV_E_ARG);
    has_error(fav_error_len());
    assert_eq!(fav_set_param(99, 0.0), FAV_E_ARG);
    assert!(fav_get_param(99).is_nan());
    assert_eq!(fav_set_param(id(Param::SpanYears), 0.5), FAV_E_ARG);
    assert_eq!(fav_set_param(id(Param::Ias15Epsilon), f64::NAN), FAV_E_ARG);
    if !common::system_mode_or_skip("abi_version_status_codes_and_buffers (System-mode part)") {
        return;
    }
    // Lense-Thirring is IAS15 only: forcing WHFast is refused with propagate's FA-E1201.
    assert_eq!(fav_set_toggles(bits::LENSE_THIRRING), FAV_OK);
    assert_eq!(fav_set_param(id(Param::Engine), 1.0), FAV_OK);
    assert_eq!(fav_configure(), FAV_E_UNSUPPORTED);
    // GR 1PN and EIH together are mutually exclusive.
    assert_eq!(fav_set_param(id(Param::Engine), 0.0), FAV_OK);
    assert_eq!(fav_set_toggles(bits::GR_1PN | bits::EIH), FAV_OK);
    assert_eq!(fav_configure(), FAV_E_ARG);
    // Kepler only, 2 yr, AUTO -> WHFast.
    assert_eq!(fav_set_toggles(0), FAV_OK);
    assert_eq!(fav_set_param(id(Param::SpanYears), 2.0), FAV_OK);
    assert_eq!(fav_configure(), FAV_OK);
    assert_eq!(fav_engine(), ENGINE_WHFAST);
    let jd = ic::header().unwrap().value("JDEPOC").unwrap();
    assert_eq!(fav_epoch_jd().to_bits(), jd.to_bits());
    assert_eq!(fav_run(0.0), FAV_E_ARG);
    assert_eq!(fav_run(f64::from(51_u8)), FAV_E_ARG);
    assert_eq!(fav_run(0.25), FAV_OK);
    assert_eq!(fav_fit(f64::NAN, f64::NAN), FAV_E_FIT, "too few passages");
    assert_eq!(fav_fit_len(), 0);
    let mut status = FAV_OK;
    let mut calls = 1;
    while status == FAV_OK {
        status = fav_run(0.25);
        calls += 1;
    }
    assert_eq!(status, FAV_DONE);
    assert_eq!(calls, 8);
    assert_eq!(fav_run(0.25), FAV_DONE);
    for (len, stride) in [
        (fav_bodies_len(), BODIES_STRIDE),
        (fav_elements_len(), ELEMENTS_STRIDE),
        (fav_passages_len(), PASSAGES_STRIDE),
    ] {
        let len = usize::try_from(len).unwrap();
        assert!(len > 0 && len % stride == 0, "{len} % {stride}");
    }
    assert!(fav_bodies_ptr().is_aligned() && fav_elements_ptr().is_aligned());
    assert_eq!(fav_drain(), FAV_OK);
    assert_eq!(
        fav_bodies_len() + fav_elements_len() + fav_passages_len(),
        0
    );
    // Too short a span to fit the rate with 20 passages: E_FIT is not fatal.
    assert_eq!(fav_fit(0.0, 1.0), FAV_E_FIT);
    assert_eq!(fav_fit(1.0, 0.0), FAV_E_ARG);
    assert!(fav_meta_len() > 0 && fav_scenario_len() > 0);
    assert_eq!(fav_reset(), FAV_OK);
    let default_span = ferroastro_viewer::config::DEFAULTS[Param::SpanYears as usize];
    assert_eq!(
        fav_get_param(id(Param::SpanYears)).to_bits(),
        default_span.to_bits()
    );
    assert_eq!(fav_engine(), 0);
}

/// Verifies: API-004, API-002
///
/// DESIGN §5 (#22): a build without `embed-header` has no header.440 initial conditions; System
/// mode refuses cleanly with `FAV_E_UNSUPPORTED` and an `FA-E0303` message, and so does the Rung
/// lab (its DE440 constants come from the header too since #53).
#[test]
fn without_embed_header_system_mode_is_refused() {
    if ic::EMBED_HEADER {
        eprintln!("SKIP without_embed_header_system_mode_is_refused: built with embed-header");
        return;
    }
    let mut v = Viewer::new();
    assert_eq!(v.set_toggles(bits::GR_1PN), FAV_OK);
    assert_eq!(v.configure(), FAV_E_UNSUPPORTED);
    assert!(v.error().starts_with("FA-E0303"), "{}", v.error());
    assert!(
        v.error().contains("built without embed-header"),
        "{}",
        v.error()
    );
    assert_eq!(v.engine(), 0);
    assert_eq!(v.run(1.0), FAV_E_STATE);
    // The Rung lab (planets off) is refused the same way.
    let mut lab = Viewer::new();
    assert_eq!(lab.set_param(id(Param::Mode), 1.0), FAV_OK);
    assert_eq!(lab.set_toggles(0), FAV_OK);
    assert_eq!(lab.configure(), FAV_E_UNSUPPORTED);
    assert!(lab.error().starts_with("FA-E0303"), "{}", lab.error());
    assert!(
        lab.error().contains("built without embed-header"),
        "{}",
        lab.error()
    );
    // The same through the exports.
    assert_eq!(fav_set_toggles(0), FAV_OK);
    assert_eq!(fav_configure(), FAV_E_UNSUPPORTED);
    has_error(fav_error_len());
}

/// Verifies: API-004, API-002
#[test]
fn rung_lab_status_codes() {
    let mut v = Viewer::new();
    assert_eq!(v.set_param(id(Param::Mode), 1.0), FAV_OK);
    assert_eq!(v.configure(), FAV_E_ARG, "planets are on by default");
    assert!(v.error().starts_with("FA-E0002"), "{}", v.error());
    if !common::rung_lab_or_skip("rung_lab_status_codes (run part)") {
        return;
    }
    assert_eq!(v.set_toggles(0), FAV_OK);
    assert_eq!(v.set_param(id(Param::SpanYears), 1.0), FAV_OK);
    assert_eq!(v.configure(), FAV_OK, "{}", v.error());
    assert_eq!(v.engine(), ENGINE_IAS15_TEST_PARTICLE);
    assert_eq!(v.run(1.0), FAV_DONE);
    assert!(v.bodies().len() % BODIES_STRIDE == 0 && !v.bodies().is_empty());
    // SR-a (special-relativistic) and Lense-Thirring (PPN) declare different gauges.
    assert_eq!(v.set_toggles(bits::LENSE_THIRRING), FAV_OK);
    assert_eq!(v.set_param(id(Param::Rung), 1.0), FAV_OK);
    assert_eq!(v.configure(), FAV_E_RUN, "{}", v.error());
    assert!(v.error().starts_with("FA-E05"), "{}", v.error());
    assert_eq!(v.run(1.0), FAV_E_STATE);
}

/// Verifies: API-004, PHY-015
#[test]
fn metadata_has_the_design_keys() {
    if !common::system_mode_or_skip("metadata_has_the_design_keys") {
        return;
    }
    let mut v = Viewer::new();
    assert_eq!(v.set_toggles(bits::GR_1PN), FAV_OK);
    assert_eq!(v.set_param(id(Param::SpanYears), 10.0), FAV_OK);
    assert_eq!(v.configure(), FAV_OK, "{}", v.error());
    assert_eq!(v.run(10.0), FAV_DONE);
    assert_eq!(v.fit(f64::NAN, f64::NAN), FAV_OK, "{}", v.error());
    assert_eq!(v.fit_record().len(), FIT_STRIDE);
    let meta: serde_json::Value = serde_json::from_slice(v.meta_bytes()).unwrap();
    for key in [
        "abi",
        "mode",
        "engine",
        "nominal",
        "toggles",
        "params",
        "model",
        "gauge",
        "frame",
        "display_frame",
        "epoch_jd_tdb",
        "time_scale",
        "ic_source",
        "ic_hash",
        "constants",
        "scenario_digest",
        "estimator",
        "fit",
        "conformal",
    ] {
        assert!(meta.get(key).is_some(), "metadata lacks {key}: {meta}");
    }
    assert_eq!(meta["mode"], "system");
    assert_eq!(meta["nominal"], true);
    assert!(
        meta["ic_source"]
            .as_str()
            .unwrap()
            .contains("embedded at build time (feature embed-header")
    );
    assert!(meta["j2_nominal"].as_f64().is_some_and(|j2| j2 > 0.0));
    assert!(meta["fit"]["model"].as_str().unwrap().contains("varpi_dot"));
    let toml = String::from_utf8(v.scenario_bytes().to_vec()).unwrap();
    assert!(toml.contains("gr_potential = true"), "{toml}");
    // A physical exaggeration marks the run non-nominal.
    assert_eq!(v.set_param(id(Param::J2Override), 1e-5), FAV_OK); // lint: allow-literal(an exaggerated J2 probe)
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert_eq!(meta["nominal"], false);
}

/// Verifies: API-004
///
/// DESIGN §2.2: `src/ffi.rs` is the only module allowed the `unsafe_code` lint, and only for the
/// `#[unsafe(no_mangle)]` attribute of each export: no unsafe block, function or impl.
#[test]
fn ffi_contains_no_unsafe_code() {
    let src = include_str!("../src/ffi.rs");
    let mut exports = 0;
    for (n, line) in src.lines().enumerate() {
        let code = line.split("//").next().unwrap_or("");
        let rest = code
            .replace("#[unsafe(no_mangle)]", "")
            .replace("#![allow(unsafe_code)]", "");
        exports += code.matches("#[unsafe(no_mangle)]").count();
        assert!(
            !rest.contains("unsafe"),
            "src/ffi.rs:{}: unsafe code beyond the no_mangle attribute: {line}",
            n + 1
        );
    }
    assert!(exports >= 25, "{exports} exports");
    for (name, src) in [
        ("lib.rs", include_str!("../src/lib.rs")),
        ("config.rs", include_str!("../src/config.rs")),
        ("engine.rs", include_str!("../src/engine.rs")),
        ("fit.rs", include_str!("../src/fit.rs")),
        ("ic.rs", include_str!("../src/ic.rs")),
        ("lab.rs", include_str!("../src/lab.rs")),
        ("meta.rs", include_str!("../src/meta.rs")),
        ("observe.rs", include_str!("../src/observe.rs")),
        ("reference.rs", include_str!("../src/reference.rs")),
    ] {
        let code: String = src
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect();
        assert!(
            name == "lib.rs" || code.contains("#![forbid(unsafe_code)]"),
            "{name} must forbid unsafe code"
        );
        assert!(
            !code
                .replace("#![forbid(unsafe_code)]", "")
                .contains("unsafe"),
            "{name} contains unsafe"
        );
    }
}
