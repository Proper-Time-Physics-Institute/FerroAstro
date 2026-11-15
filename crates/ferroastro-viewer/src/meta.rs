// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The PHY-015 metadata of a run and its last fit, as one JSON object with fixed keys
//! (DESIGN §4.5).

#![forbid(unsafe_code)]

use crate::config::ViewerConfig;
use crate::engine::RunInfo;
use crate::fit::FitResult;
use serde_json::{Map, Value, json};

/// The tail of the metadata's `comparison_model` text while bit 12 or 13 is on.
pub(crate) const COMPARISON_MODEL_NOTE: &str = "(ADR-0016): comparison models outside the GR \
     ladder; this run is not a description of the real solar system (for conformal gravity even at \
     its fit values)";

fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// The `conformal` object (ABI v2): the settings of parameters 13–16, whether bit 12 is on, the
/// exaggeration flag, and the engine's analytic reference for the current settings (null in a
/// build without `embed-header`). Always present, so the page can show the reference before the
/// toggle is switched on.
fn conformal(cfg: &ViewerConfig) -> Value {
    let s = cfg.conformal_settings();
    let reference = crate::reference::conformal_reference(&s).map_or(Value::Null, |r| {
        json!({
            "linear_arcsec_per_cy": num(r.linear),
            "quadratic_arcsec_per_cy": num(r.quadratic),
            "total_arcsec_per_cy": num(r.total),
            "gamma0_arcsec_per_cy": num(r.gamma0_part),
            "basis": crate::reference::BASIS,
        })
    });
    json!({
        "label": "comparison model",
        "enabled": cfg.conformal().is_some(),
        "gamma0_per_cm": num(s.gamma0_per_cm),
        "gamma_star_per_cm": num(s.gamma_star_per_cm),
        "kappa_per_cm2": num(s.kappa_per_cm2),
        "gamma0_centre": s.centre_label(),
        "exaggerated": s.exaggerated(),
        "fit_values": format!(
            "defaults gamma0 {:e} cm^-1, gamma* {:e} cm^-1, kappa {:e} cm^-2 (Mannheim & O'Brien \
             2012 galactic fit): UNVERIFIED (OI-79), never test oracles",
            crate::config::CG_FIT_GAMMA0_PER_CM,
            crate::config::CG_FIT_GAMMA_STAR_PER_CM,
            crate::config::CG_FIT_KAPPA_PER_CM2
        ),
        "uniform_caveat": crate::reference::UNIFORM_CAVEAT,
        "reference": reference,
    })
}

/// The `proper_time` object (ABI v3): whether bit 13 is on, the engine's analytic first-order
/// rate for Mercury and the GR rate it is −1/6 of (null without `embed-header`), the research
/// derivation value (display only, never an oracle), the PPN preset of route 1 and the Bridge A
/// statement. Always present, so the page can show the reference before the toggle is on.
fn proper_time(cfg: &ViewerConfig) -> Value {
    let reference = crate::reference::proper_time_reference().map_or(Value::Null, |r| {
        json!({
            "arcsec_per_cy": num(r.rate),
            "gr_arcsec_per_cy": num(r.gr),
            "ratio_to_gr": num(r.rate / r.gr),
            "basis": crate::reference::PROPER_TIME_BASIS,
        })
    });
    json!({
        "label": "comparison model",
        "enabled": cfg.proper_time(),
        "replaces_bits": [
            crate::config::bits::GR_1PN.trailing_zeros(),
            crate::config::bits::LENSE_THIRRING.trailing_zeros(),
            crate::config::bits::EIH.trailing_zeros(),
        ],
        "ppn_preset": { "beta": 0.5, "gamma": -1.0 },
        "bridge_a": crate::reference::BRIDGE_A,
        "derivation": crate::dashboard::proper_time_derivation(),
        "reference": reference,
    })
}

/// The metadata JSON (`null` values for a field without a run or a fit).
pub(crate) fn json(cfg: &ViewerConfig, info: Option<&RunInfo>, fit: Option<&FitResult>) -> String {
    let params: Map<String, Value> = cfg
        .params()
        .into_iter()
        .map(|(k, v)| (k.to_owned(), num(v)))
        .collect();
    let s = |f: fn(&RunInfo) -> &str| info.map_or(Value::Null, |i| Value::from(f(i)));
    let fit = fit.map_or(Value::Null, |f| {
        json!({
            "model": f.model.describe(),
            "span_years": [num(f.span_years.0), num(f.span_years.1)],
            "passages": f.fit.samples,
            "rate_arcsec_per_cy": num(f.fit.arcsec_per_century()),
            "jackknife_sigma_arcsec_per_cy": num(f.fit.jackknife_sigma_arcsec_per_century()),
            "jackknife_blocks": f.fit.jackknife_blocks,
            "epoch": "J2000.0 TDB (fit time origin)",
        })
    });
    let v = json!({
        "abi": crate::ABI_VERSION,
        "mode": info.map_or_else(
            || Value::from(if cfg.is_lab() { "rung-lab" } else { "system" }),
            |i| Value::from(i.mode),
        ),
        "engine": s(|i| &i.engine_label),
        "nominal": cfg.is_nominal(),
        // A comparison model on (even at its fit values) means the run is not a description of
        // the real solar system; `nominal` only says nothing is exaggerated (DESIGN §4.5).
        "comparison_model": cfg.comparison_models().map_or(Value::Null, |names| {
            Value::from(format!("{names} {COMPARISON_MODEL_NOTE}"))
        }),
        "toggles": cfg.mask(),
        "params": params,
        "model": s(|i| &i.model),
        "gauge": s(|i| &i.gauge),
        "frame": s(|i| &i.frame),
        "display_frame": s(|i| &i.display_frame),
        "epoch_jd_tdb": info.map_or(Value::Null, |i| num(i.epoch_jd)),
        "time_scale": "TDB",
        "ic_source": s(|i| &i.ic_source),
        "ic_hash": s(|i| &i.ic_hash),
        "constants": s(|i| &i.constants),
        "scenario_digest": s(|i| &i.scenario_digest),
        "estimator": s(|i| &i.estimator),
        "fit": fit,
        // The nominal DE440 solar J2, so the page can scale its J2_OVERRIDE references without
        // copying a header value (#22); null if the constants are unavailable.
        "j2_nominal": crate::ic::de440().map_or(Value::Null, |d| num(d.sun_j2())),
        // ABI v2: the conformal-gravity comparison model (bit 12; ADR-0016, PHY-020).
        "conformal": conformal(cfg),
        // ABI v3: the Proper Time (Gill) comparison model (bit 13; ADR-0016 D6, PHY-021).
        "proper_time": proper_time(cfg),
        // The comparison dashboard's reference columns (engine analytic estimates, M5 evidence,
        // Park17 literature, the INPOP15a bound), so the page holds no physics numbers.
        "dashboard": crate::dashboard::json(cfg),
    });
    v.to_string()
}
