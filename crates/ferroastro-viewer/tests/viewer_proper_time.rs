// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! ABI v3 (ADR-0016 decision D6): toggle bit 13, the Proper Time (Gill) **comparison model**
//! (PHY-021), end to end through the viewer, and its PPN (1/2, −1) preset on bit 8.
//!
//! # Oracle
//! Only the closed form the engine evaluates, `−πGM☉/(c²a(1−e²))` per orbit for Mercury's SW92
//! J2000 mean `a`, `e` ([`ferroastro_viewer::reference::proper_time_reference`], −1/6 of GR's).
//! No parameter is exaggerated: at Mercury the signal is −7.16 ″/cy, which a 100-yr run resolves.
//!
//! # Tolerance
//! `REL_SYSTEM = 2e-3`, as for the conformal-gravity System test: the run starts from the
//! header.440 osculating state of JD 2440400.5 while the reference uses the SW92 J2000 mean `a`
//! and `e`, and the 100-yr fit sees Mercury's short-period terms (DESIGN E2 measures this mismatch
//! for GR at about 5e-5 relative); the fit floor (E1, 1e-3 ″/cy) is 1.4e-4 of the signal. The
//! routes are compared with `REL_ROUTES = 1e-3`: they share initial conditions and estimator, and
//! differ physically only at O(GM/(c²p)) ≈ 2.6e-8 and by the Sun's reaction (m/M☉ ≈ 1.7e-7),
//! so the bound is the two fits' floors. The GR row's +1 and the Proper Time row's −1/6 differ by
//! 7/6, far outside either bound.
//!
//! All tests are PR-tier; none is `#[ignore]`.

mod common;

use common::{IAS15, WHFAST, configured, rate};
use ferroastro_viewer::config::{Param, ViewerConfig, bits};
use ferroastro_viewer::reference::proper_time_reference;
use ferroastro_viewer::{FAV_E_ARG, FAV_OK, Viewer};

const CENTURY: u32 = 100;
/// Relative bound against the analytic reference (module docs).
const REL_SYSTEM: f64 = 2e-3; // lint: allow-literal(tolerance justified in the module docs)
/// Relative bound between the two routes (module docs).
const REL_ROUTES: f64 = 1e-3; // lint: allow-literal(tolerance justified in the module docs)
/// The PPN preset of route 1.
const PPN: [(Param, f64); 2] = [(Param::PpnBeta, 0.5), (Param::PpnGamma, -1.0)];

/// Bit 13 is defined, off in the default mask, refused beside bits 8, 9 and 11 and in the Rung
/// lab; the metadata carries the `proper_time` object with the PPN preset, Bridge A and the
/// derivation reference, and the comparison-model note while the bit is on.
///
/// Verifies: API-004, PHY-021
#[test]
fn bit_13_is_defined_default_off_and_exclusive_with_the_gr_sector() {
    let mut v = Viewer::new();
    assert_eq!(v.config().mask() & bits::PROPER_TIME, 0, "default mask");
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert_eq!(meta["proper_time"]["enabled"], false);
    assert!(meta["comparison_model"].is_null());
    for other in [bits::GR_1PN, bits::LENSE_THIRRING, bits::EIH] {
        assert_eq!(v.set_toggles(bits::PROPER_TIME | other), FAV_OK);
        assert_eq!(v.configure(), FAV_E_ARG, "bit 13 with {other:#x}");
        assert!(v.error().contains("bit 13"), "{}", v.error());
    }
    let pt = bits::PLANETS | bits::SUN_J2 | bits::PROPER_TIME;
    assert_eq!(v.set_toggles(pt), FAV_OK);
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    let p = &meta["proper_time"];
    assert_eq!(p["label"], "comparison model");
    assert_eq!(p["enabled"], true);
    assert_eq!(p["ppn_preset"]["beta"], 0.5);
    assert_eq!(p["ppn_preset"]["gamma"], -1.0);
    assert!(p["bridge_a"].as_str().unwrap().contains("conformally flat"));
    assert_eq!(p["derivation"]["kind"], "derivation");
    assert!(
        meta["comparison_model"]
            .as_str()
            .is_some_and(|t| t.contains("Proper Time") && t.contains("not a description")),
        "{}",
        meta["comparison_model"]
    );
    assert_eq!(
        meta["nominal"], true,
        "bit 13 has no parameter to exaggerate"
    );
    assert_eq!(v.set_param(Param::Mode as u32, 1.0), FAV_OK);
    assert_eq!(v.set_toggles(bits::PROPER_TIME), FAV_OK);
    assert_eq!(v.configure(), FAV_E_ARG, "System mode only");
}

/// PHY-021 in the viewer: Sun + Mercury, 100 yr. Route 2 (bit 13, `WHFast`) and route 1 (bit 8
/// with the PPN (1/2, −1) preset, IAS15) each give a P-remove contribution equal to the engine's
/// analytic reference (−1/6 of GR's), retrograde, and agree with each other; the bit-13 run is
/// labelled "comparison model" in its gauge, model and scenario.
///
/// Verifies: PHY-021, API-004
#[test]
fn bit_13_and_the_ppn_preset_give_minus_one_sixth_of_gr() {
    if !common::system_mode_or_skip("bit_13_and_the_ppn_preset_give_minus_one_sixth_of_gr") {
        return;
    }
    let reference = proper_time_reference().unwrap();
    assert!(
        (reference.rate / reference.gr + 1.0 / 6.0).abs() <= 8.0 * f64::EPSILON, // lint: allow-literal(-1/6)
        "{reference:?}"
    );
    let gill = rate(bits::PROPER_TIME, &[WHFAST], CENTURY) - rate(0, &[WHFAST], CENTURY);
    let ppn_params = [IAS15, PPN[0], PPN[1]];
    let ppn = rate(bits::GR_1PN, &ppn_params, CENTURY) - rate(0, &[IAS15], CENTURY);
    let dev_gill = (gill / reference.rate - 1.0).abs();
    let dev_ppn = (ppn / reference.rate - 1.0).abs();
    let gap = (ppn / gill - 1.0).abs();
    println!(
        "Proper Time: bit 13 (WHFast) {gill:+.6} arcsec/cy, PPN (1/2, -1) preset (IAS15) \
         {ppn:+.6}, analytic {:+.6} (GR {:+.6}); |rel dev| {dev_gill:.2e}, {dev_ppn:.2e}; \
         route gap {gap:.2e}",
        reference.rate, reference.gr
    );
    assert!(gill < 0.0 && ppn < 0.0, "retrograde");
    assert!(dev_gill <= REL_SYSTEM, "bit 13: {dev_gill:e}");
    assert!(dev_ppn <= REL_SYSTEM, "PPN preset: {dev_ppn:e}");
    assert!(gap <= REL_ROUTES, "routes: {gap:e}");

    let mut v = configured(bits::PROPER_TIME, &[WHFAST], 1);
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert!(meta["gauge"].as_str().unwrap().contains("comparison model"));
    assert!(
        meta["model"]
            .as_str()
            .unwrap()
            .contains("Proper Time (Gill)")
    );
    let toml = String::from_utf8(v.scenario_bytes().to_vec()).unwrap();
    assert!(toml.contains("proper_time_kernel = true"), "{toml}");
    let mut cfg = ViewerConfig::default();
    cfg.set_mask(bits::PROPER_TIME).unwrap();
    assert!(
        cfg.to_scenario(ferroastro_viewer::config::EngineKind::WhFast)
            .is_ok()
    );
}
