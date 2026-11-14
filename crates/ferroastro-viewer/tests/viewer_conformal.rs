// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! ABI v2 (ADR-0016 decision D5): toggle bit 12, the conformal-gravity **comparison model**
//! (PHY-020), and its parameters 13–16, end to end through the viewer.
//!
//! # Oracles (decision D7)
//! Only the closed-form Gauss secular rates, as the engine evaluates them for the current settings
//! ([`ferroastro_viewer::reference::conformal_reference`], from
//! `ferroastro_analytic::advance::conformal_{linear,quadratic}_advance`). The published fit values
//! (OI-79, the viewer's display defaults) are never asserted. The parameters are **exaggerated** so
//! that the rate is resolved in a 100-yr PR-tier run:
//! - γ0 = 1e-27 cm⁻¹ Sun-centred: about −60 ″/cy, a constant acceleration about 1e-7 of the Sun's
//!   pull at Mercury, so the neglected second-order terms are near 1e-7 relative;
//! - γ\* = −3e-28 cm⁻¹ and κ = 1e-41 cm⁻² in the Rung lab: about +18 and +3 ″/cy.
//!
//! # Tolerances
//! - System mode, Sun-centred γ0: `REL_SYSTEM = 2e-3`. The run starts from the header.440
//!   osculating state of JD 2440400.5 while the reference uses the SW92 J2000 mean a and e, and the
//!   100-yr fit sees Mercury's short-period terms; DESIGN E2 measures the same mismatch for GR as
//!   about 5e-5 relative, and the fit floor (E1, 1e-3 ″/cy) is 2e-5 of the signal, so 2e-3 leaves a
//!   wide margin while the printed (1−e²)² factor of Sultana et al. (7 % off) would fail.
//! - Rung lab: `REL_LAB = 1e-3`. The lab integrates exactly the SW92 state the reference uses.
//! - Uniform γ0: `ABS_UNIFORM = 1e-6 ″/cy`, the PHY-001 bound. With Newtonian (velocity-independent)
//!   forces the uniform field moves the Sun and Mercury alike, so the heliocentric rate changes only
//!   by round-off, against −60 ″/cy had the same γ0 been Sun-centred.
//!
//! All tests are PR-tier; none is `#[ignore]`.

mod common;

use common::{LAB, WHFAST, configured, rate};
use ferroastro_viewer::config::{
    CG_FIT_GAMMA_STAR_PER_CM, CG_FIT_GAMMA0_PER_CM, CG_FIT_KAPPA_PER_CM2, Param, ViewerConfig, bits,
};
use ferroastro_viewer::reference::conformal_reference;
use ferroastro_viewer::{ABI_VERSION, FAV_E_ARG, FAV_OK, Viewer};

const CENTURY: u32 = 100;
/// Exaggerated γ0 (cm⁻¹), module docs.
const GAMMA0: f64 = 1e-27; // lint: allow-literal(exaggerated gamma0, see module docs)
/// Exaggerated γ* (cm⁻¹), module docs.
const GAMMA_STAR: f64 = -3e-28; // lint: allow-literal(exaggerated gamma*, see module docs)
/// Exaggerated κ (cm⁻²), module docs.
const KAPPA: f64 = 1e-41; // lint: allow-literal(exaggerated kappa, see module docs)
/// Relative bound, System mode (module docs).
const REL_SYSTEM: f64 = 2e-3; // lint: allow-literal(tolerance justified in the module docs)
/// Relative bound, Rung lab (module docs).
const REL_LAB: f64 = 1e-3; // lint: allow-literal(tolerance justified in the module docs)
/// Absolute bound for the uniform centring, ″/cy (module docs; PHY-001's bound).
const ABS_UNIFORM: f64 = 1e-6; // lint: allow-literal(PHY-001 integrator bound)

fn id(p: Param) -> u32 {
    p as u32
}

/// ABI v2: version 2, bit 12 accepted and bit 13 refused, parameters 13–16 with their defaults
/// (the OI-79 fit values, uniform centring) and ranges, the default mask without bit 12, and the
/// metadata's `conformal` object labelled "comparison model".
///
/// Verifies: API-004, PHY-020
#[test]
fn abi_v2_defines_bit_12_and_parameters_13_to_16() {
    assert_eq!(ABI_VERSION, 2);
    let mut v = Viewer::new();
    assert_eq!(
        v.config().mask() & bits::CONFORMAL,
        0,
        "default mask unchanged"
    );
    assert_eq!(v.set_toggles(bits::DEFAULT | bits::CONFORMAL), FAV_OK);
    assert_eq!(v.set_toggles(1 << 13), FAV_E_ARG);
    assert!(v.error().starts_with("FA-E0002"), "{}", v.error());
    assert_eq!(
        v.get_param(id(Param::CgGamma0)).to_bits(),
        CG_FIT_GAMMA0_PER_CM.to_bits()
    );
    assert_eq!(
        v.get_param(id(Param::CgGammaStar)).to_bits(),
        CG_FIT_GAMMA_STAR_PER_CM.to_bits()
    );
    assert_eq!(
        v.get_param(id(Param::CgKappa)).to_bits(),
        CG_FIT_KAPPA_PER_CM2.to_bits()
    );
    assert_eq!(
        v.get_param(id(Param::CgGamma0Centre)).to_bits(),
        0.0_f64.to_bits()
    );
    assert_eq!(v.set_param(id(Param::CgGamma0), -GAMMA0), FAV_OK);
    assert_eq!(v.set_param(id(Param::CgGamma0Centre), 1.0), FAV_OK);
    assert_eq!(v.set_param(id(Param::CgGamma0Centre), 0.5), FAV_E_ARG);
    assert_eq!(v.set_param(id(Param::CgKappa), f64::INFINITY), FAV_E_ARG);
    assert_eq!(v.set_param(17, 0.0), FAV_E_ARG);
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert_eq!(meta["abi"], 2);
    let cg = &meta["conformal"];
    assert_eq!(cg["label"], "comparison model");
    assert_eq!(cg["enabled"], true);
    assert_eq!(cg["gamma0_centre"], "sun");
    assert_eq!(cg["exaggerated"], true);
    assert!(
        cg["uniform_caveat"]
            .as_str()
            .unwrap()
            .contains("velocity-independent")
    );
    assert!(cg["fit_values"].as_str().unwrap().contains("OI-79"));
    assert_eq!(
        meta["nominal"], false,
        "a non-fit coefficient is exaggerated physics"
    );
    assert_eq!(v.reset(), FAV_OK);
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert_eq!(meta["conformal"]["enabled"], false);
    assert_eq!(meta["conformal"]["exaggerated"], false);
}

/// Sun-centred exaggerated γ0 in System mode (`WHFast`, Sun + Mercury, 100 yr): the P-remove
/// contribution is retrograde and equals the engine's Gauss reference; the run is labelled
/// "comparison model" in its gauge, model and scenario.
///
/// Verifies: PHY-020, API-004
#[test]
fn sun_centred_gamma0_gives_the_gauss_retrograde_rate() {
    if !common::system_mode_or_skip("sun_centred_gamma0_gives_the_gauss_retrograde_rate") {
        return;
    }
    let p = [
        WHFAST,
        (Param::CgGamma0, GAMMA0),
        (Param::CgGammaStar, 0.0),
        (Param::CgKappa, 0.0),
        (Param::CgGamma0Centre, 1.0),
    ];
    let mut cfg = ViewerConfig::default();
    cfg.set_mask(bits::CONFORMAL).unwrap();
    for (k, x) in p.iter().skip(1) {
        cfg.set(*k, *x).unwrap();
    }
    let reference = conformal_reference(&cfg.conformal_settings()).unwrap();
    let measured = rate(bits::CONFORMAL, &p, CENTURY) - rate(0, &[WHFAST], CENTURY);
    let dev = (measured / reference.total - 1.0).abs();
    println!(
        "Sun-centred gamma0 = {GAMMA0:e} cm^-1: measured {measured} arcsec/cy, Gauss {} \
         (gamma0 part {}), |rel dev| {dev:e}",
        reference.total, reference.gamma0_part
    );
    assert!(reference.total < 0.0 && measured < 0.0, "retrograde");
    assert!(dev <= REL_SYSTEM, "|rel dev| {dev:e} > {REL_SYSTEM:e}");
    let mut v = configured(bits::CONFORMAL, &p, 1);
    let meta: serde_json::Value = serde_json::from_str(&v.meta_json()).unwrap();
    assert!(meta["gauge"].as_str().unwrap().contains("comparison model"));
    assert!(meta["model"].as_str().unwrap().contains("comparison model"));
    let toml = String::from_utf8(v.scenario_bytes().to_vec()).unwrap();
    assert!(toml.contains("gamma0_centre = \"sun\""), "{toml}");
}

/// Uniform centring (the default) with the same exaggerated γ0 and Newtonian forces: the
/// heliocentric rate is unchanged to the PHY-001 bound, and the reference's γ0 part is exactly 0.
///
/// Verifies: PHY-020, API-004
#[test]
fn uniform_gamma0_leaves_the_rate_unchanged() {
    if !common::system_mode_or_skip("uniform_gamma0_leaves_the_rate_unchanged") {
        return;
    }
    let p = [
        WHFAST,
        (Param::CgGamma0, GAMMA0),
        (Param::CgGammaStar, 0.0),
        (Param::CgKappa, 0.0),
    ];
    let mut cfg = ViewerConfig::default();
    cfg.set_mask(bits::CONFORMAL).unwrap();
    cfg.set(Param::CgGamma0, GAMMA0).unwrap();
    let reference = conformal_reference(&cfg.conformal_settings()).unwrap();
    assert_eq!(reference.gamma0_part.to_bits(), 0.0_f64.to_bits());
    let measured = rate(bits::CONFORMAL, &p, CENTURY) - rate(0, &[WHFAST], CENTURY);
    println!("uniform gamma0 = {GAMMA0:e} cm^-1: measured {measured:e} arcsec/cy");
    assert!(measured.abs() <= ABS_UNIFORM, "{measured:e}");
}

/// Rung lab, Newton rung plus exaggerated γ\* (Sun-sourced, so it acts) and κ, with γ0 uniform
/// (no test-particle term): the rate equals the Gauss reference; κ > 0 is prograde.
///
/// Verifies: PHY-020, API-004
#[test]
fn rung_lab_gamma_star_and_kappa_give_the_gauss_rates() {
    if !common::rung_lab_or_skip("rung_lab_gamma_star_and_kappa_give_the_gauss_rates") {
        return;
    }
    let p = [
        LAB,
        (Param::Rung, 0.0),
        (Param::CgGamma0, GAMMA0),
        (Param::CgGammaStar, GAMMA_STAR),
        (Param::CgKappa, KAPPA),
    ];
    let mut cfg = ViewerConfig::default();
    cfg.set_mask(bits::CONFORMAL).unwrap();
    for (k, x) in p.iter().skip(2) {
        cfg.set(*k, *x).unwrap();
    }
    let reference = conformal_reference(&cfg.conformal_settings()).unwrap();
    assert!(reference.quadratic > 0.0, "kappa > 0 is prograde");
    let measured = rate(bits::CONFORMAL, &p, CENTURY);
    let dev = (measured / reference.total - 1.0).abs();
    println!(
        "Rung lab gamma* = {GAMMA_STAR:e}, kappa = {KAPPA:e}: measured {measured}, Gauss {} \
         (linear {}, quadratic {}), |rel dev| {dev:e}",
        reference.total, reference.linear, reference.quadratic
    );
    assert!(dev <= REL_LAB, "|rel dev| {dev:e} > {REL_LAB:e}");
}
