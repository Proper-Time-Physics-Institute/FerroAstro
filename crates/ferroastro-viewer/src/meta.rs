// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The PHY-015 metadata of a run and its last fit, as one JSON object with fixed keys
//! (DESIGN §4.5).

#![forbid(unsafe_code)]

use crate::config::ViewerConfig;
use crate::engine::RunInfo;
use crate::fit::FitResult;
use serde_json::{Map, Value, json};

fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
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
    });
    v.to_string()
}
