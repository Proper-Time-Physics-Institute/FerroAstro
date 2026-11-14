// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The comparison dashboard's reference columns (API-004; F4 of the conformal-gravity plan,
//! ADR-0016): everything the contribution table shows beside a live value, so the page holds no
//! physics numbers of its own (DESIGN §1).
//!
//! * **Engine analytic estimate**, per effect, for the current parameters: the first-order secular
//!   formulas of `ferroastro_analytic::advance` (GR 1PN with the PPN scaling, the planar solar J2,
//!   Lense–Thirring, the Rung-lab rungs in 1PN units, and the conformal-gravity Gauss rate of
//!   [`crate::reference`]) for Mercury's SW92 J2000 mean `a`, `e` with the DE440 constants.
//! * **M5 evidence**: FerroAstro's own nightly results, **computed, not observed**, embedded from
//!   `tests/vectors/m5-evidence.toml` (`kind = "computed-reference"`, with run, commit and
//!   toolchain).
//! * **Literature**: Park et al. 2017 Table 3, embedded from the vectors that already carry it
//!   (`park2017-table3.toml`, `single-body-rungs.toml`, `phy004-gr-analytic.toml`).
//! * **Observational bound**: the `INPOP15a` supplementary-advance interval, embedded from
//!   `tests/vectors/comparison-bounds.toml`; it applies to comparison models only.
//!
//! Each column carries its frame, window and protocol, so the page can label it. The values are
//! passed through unchanged; `tests/viewer_dashboard.rs` checks that against the vector files.

#![forbid(unsafe_code)]

use crate::ViewerError;
use crate::config::{LabRung, ViewerConfig, bits};
use ferroastro_analytic::advance::{
    j2_planar_advance, lense_thirring_rate, pn_unit, ppn_scaling, rate_arcsec_per_century,
};
use ferroastro_core::units::{Angle, Length, SpinGm};
use serde_json::{Map, Value, json};
use std::sync::OnceLock;

const M5_VECTORS: &str = include_str!("../../../tests/vectors/m5-evidence.toml");
const BOUND_VECTORS: &str = include_str!("../../../tests/vectors/comparison-bounds.toml");
const PARK17_T3_VECTORS: &str = include_str!("../../../tests/vectors/park2017-table3.toml");
const RUNG_VECTORS: &str = include_str!("../../../tests/vectors/single-body-rungs.toml");
const GR_VECTORS: &str = include_str!("../../../tests/vectors/phy004-gr-analytic.toml");

/// Frame of every reference column that has one.
const PARK17_FRAME: &str = "Park17 mean-orbit plane";

/// A vector's `(value, quantity)` by id from one embedded file.
fn vector(file: &str, id: &str) -> Result<(f64, String), String> {
    let table: toml::Table = toml::from_str(file).map_err(|e| e.to_string())?;
    let v = table
        .get("vector")
        .and_then(toml::Value::as_array)
        .and_then(|vs| {
            vs.iter()
                .find(|v| v.get("id").and_then(toml::Value::as_str) == Some(id))
        })
        .ok_or_else(|| format!("vector {id} missing"))?;
    let value = match v.get("value") {
        Some(toml::Value::String(s)) => s.trim().parse::<f64>().map_err(|e| e.to_string())?,
        Some(toml::Value::Float(x)) => *x,
        Some(toml::Value::Integer(i)) => {
            f64::from(i32::try_from(*i).map_err(|e| format!("vector {id}: {e}"))?)
        }
        _ => return Err(format!("vector {id}: value is not a number")),
    };
    let quantity = v
        .get("quantity")
        .and_then(toml::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok((value, quantity))
}

fn num(x: f64) -> Value {
    serde_json::Number::from_f64(x).map_or(Value::Null, Value::Number)
}

/// One M5 row: `(effect key, value id, sigma id, protocol, note)`.
const M5_ROWS: [(&str, &str, &str, &str, &str); 5] = [
    (
        "total",
        "M5-EIH-FULL-RATE",
        "M5-EIH-FULL-RATE-JK-SIGMA",
        "total rate, full EIH model with 343 asteroids, KBOs and ring (393 bodies), IAS15",
        "the viewer's full model has no KBOs or ring and a forward window from 1969",
    ),
    (
        "b7",
        "M5-J2-PREMOVE-RATE",
        "M5-J2-LT-PREMOVE-SIGMA-RSS",
        "P-remove under EIH, 11 bodies, IAS15; J2 = 2.25e-7 (Park17's, not DE440's)",
        "at Park17's J2, 2.4 % above the viewer's nominal DE440 J2",
    ),
    (
        "b8",
        "M5-GR-SUN-MERCURY-RATE",
        "M5-GR-SUN-MERCURY-RATE-JK-SIGMA",
        "Sun + Mercury only, EIH 1PN on minus off, IAS15",
        "two-body GR on - off, not a P-remove in the full system",
    ),
    (
        "b9",
        "M5-LT-PREMOVE-RATE",
        "M5-J2-LT-PREMOVE-SIGMA-RSS",
        "P-remove under EIH, 11 bodies, IAS15; DE440 solar spin",
        "the legs share initial conditions, so the rss sigma is a loose upper estimate",
    ),
    (
        "b11",
        "M5-GR-SUN-MERCURY-RATE",
        "M5-GR-SUN-MERCURY-RATE-JK-SIGMA",
        "Sun + Mercury only, EIH 1PN on minus off, IAS15",
        "the Sun's 1PN term only; EIH in the full system also carries the planets' 1PN terms",
    ),
];

/// One literature row: `(effect key, file, id, note)`.
const LITERATURE_ROWS: [(&str, &str, &str, &str); 13] = [
    ("b0", PARK17_T3_VECTORS, "PARK17-T3-VENUS", ""),
    ("b1", PARK17_T3_VECTORS, "PARK17-T3-EARTH-MOON", ""),
    ("b2", PARK17_T3_VECTORS, "PARK17-T3-MARS", ""),
    ("b3", PARK17_T3_VECTORS, "PARK17-T3-JUPITER", ""),
    ("b4", PARK17_T3_VECTORS, "PARK17-T3-SATURN", ""),
    ("b5", PARK17_T3_VECTORS, "PARK17-T3-URANUS", ""),
    ("b6", PARK17_T3_VECTORS, "PARK17-T3-NEPTUNE", ""),
    (
        "b7",
        RUNG_VECTORS,
        "PARK17-J2-RATE",
        "at Park17's J2 = 2.25e-7; the viewer's nominal J2 is DE440's",
    ),
    (
        "b8",
        GR_VECTORS,
        "PARK17-GE-RATE",
        "Park17's gravitoelectric (Schwarzschild-like) row",
    ),
    (
        "b9",
        RUNG_VECTORS,
        "PARK17-LT-RATE",
        "Park17's Lense-Thirring row (S = 190e39 kg m^2/s)",
    ),
    (
        "b10",
        PARK17_T3_VECTORS,
        "PARK17-T3-ASTEROIDS",
        "343 asteroids; the viewer's count is its asteroid parameter",
    ),
    (
        "b11",
        GR_VECTORS,
        "PARK17-GE-RATE",
        "Park17's gravitoelectric row; EIH also carries the planets' 1PN terms",
    ),
    (
        "cross_js",
        PARK17_T3_VECTORS,
        "PARK17-T3-X-JUPITER-SATURN",
        "Park17's largest planetary interaction row (Jupiter + Saturn), for scale",
    ),
];

/// The id of the `INPOP15a` central value.
pub const BOUND_ID: &str = "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE";
/// The id of the `INPOP15a` interval half-width.
pub const BOUND_INTERVAL_ID: &str = "INPOP15A-MERCURY-SUPPLEMENTARY-ADVANCE-INTERVAL";

/// The parts that do not depend on the configuration, parsed once.
fn fixed() -> Result<&'static Value, ViewerError> {
    static F: OnceLock<Result<Value, String>> = OnceLock::new();
    F.get_or_init(|| {
        let mut m5 = Map::new();
        for (key, id, sigma_id, protocol, note) in M5_ROWS {
            let (v, q) = vector(M5_VECTORS, id)?;
            let (s, _) = vector(M5_VECTORS, sigma_id)?;
            m5.insert(
                key.to_owned(),
                json!({
                    "id": id, "value": num(v), "sigma": num(s), "sigma_id": sigma_id,
                    "protocol": protocol, "note": note, "quantity": q,
                }),
            );
        }
        let mut lit = Map::new();
        for (key, file, id, note) in LITERATURE_ROWS {
            let (v, q) = vector(file, id)?;
            lit.insert(
                key.to_owned(),
                json!({ "id": id, "value": num(v), "note": note, "quantity": q }),
            );
        }
        let (b, bq) = vector(BOUND_VECTORS, BOUND_ID)?;
        let (w, _) = vector(BOUND_VECTORS, BOUND_INTERVAL_ID)?;
        Ok(json!({
            "m5": {
                "label": "M5 evidence (computed, not observed)",
                "short": "Park17 plane · 1000-3000 CE · computed, not observed",
                "kind": "computed-reference",
                "source": "tests/vectors/m5-evidence.toml",
                "run": "M5 E2 nightly run 2026-11-12/13, FerroAstro commit d1ccbae, rustc 1.95.0",
                "frame": PARK17_FRAME,
                "window": "1000-3000 CE (J2000 +/- 1000 yr), quadratic + 14 SW92 terms",
                "protocol": "per row (P-remove under EIH, or two-body on - off)",
                "rows": m5,
            },
            "literature": {
                "label": "Park et al. 2017, Table 3",
                "short": "Park17 plane · 2000 yr centred on J2000 · P-remove",
                "source": "tests/vectors (park2017-table3, single-body-rungs, phy004-gr-analytic)",
                "frame": PARK17_FRAME,
                "window": "2000 yr centred on J2000 (DE430 fit)",
                "protocol": "P-remove (GM set to zero in the full system)",
                "rows": lit,
            },
            "bound": {
                "id": BOUND_ID,
                "interval_id": BOUND_INTERVAL_ID,
                "label": "INPOP15a (Fienga et al. 2016)",
                "short": "INPOP15a · anomalous advance · comparison models only",
                "value": num(b),
                "half_width": num(w),
                "unit": "arcsec/cy",
                "quantity": bq,
                "frame": "Mercury's perihelion in the INPOP15a ephemeris fit",
                "window": "INPOP15a data span, with MESSENGER ranging",
                "protocol": "supplementary (non-GR) advance fitted on top of GR",
                "applies_to": ["b12"],
                "criterion": "within if |rate| <= half_width, otherwise exceeds; applies to \
                              comparison models only, never to a ladder term",
            },
        }))
    })
    .as_ref()
    .map_err(|e| ViewerError::arg(format!("FA-E0002: dashboard vectors: {e}")))
}

/// One analytic estimate.
fn est(value: f64, basis: &str) -> Value {
    json!({ "value": num(value), "basis": basis })
}

/// The engine analytic estimates for `cfg`, keyed by effect (`b7`, `b8`, `b9`, `b11`, `b12`,
/// `rung`); effects without a closed form (the planets, the asteroids) have no entry.
///
/// # Errors
/// `FAV_E_UNSUPPORTED` without `embed-header`; `FAV_E_RUN` if a formula refuses its inputs.
pub fn analytic(cfg: &ViewerConfig) -> Result<Map<String, Value>, ViewerError> {
    if crate::ic::HEADER_440.is_none() {
        return Err(ViewerError::new(
            crate::FAV_E_UNSUPPORTED,
            crate::ic::WITHOUT_EMBED_HEADER,
        ));
    }
    let i = crate::reference::inputs()?;
    let de = crate::ic::de440()?;
    let run = |e: &dyn std::fmt::Display| ViewerError::run(e.to_string());
    let p = Length::from_km(i.a.km() * i.e.mul_add(-i.e, 1.0)).map_err(|e| run(&e))?;
    let per_cy = |x: Angle| x.arcsec() * i.orbits_per_century;
    let unit = per_cy(pn_unit(i.gm_sun, i.c, p).map_err(|e| run(&e))?);
    let (beta, gamma) = cfg.ppn();
    let gr = 6.0 * unit * ppn_scaling(beta, gamma);
    let j2 = cfg.j2_override().unwrap_or_else(|| de.sun_j2());
    let j2_rate = per_cy(j2_planar_advance(j2, de.sun_radius(), p).map_err(|e| run(&e))?);
    let gs = ferroastro_ladder::single_body::de440_solar_spin(de).map_err(|e| run(&e))?;
    let gs = SpinGm::from_km5_per_s3(gs.km5_per_s3() * cfg.spin_scale().unwrap_or(1.0))
        .map_err(|e| run(&e))?;
    let zero = Angle::from_radians(0.0).map_err(|e| run(&e))?;
    let lt = rate_arcsec_per_century(
        lense_thirring_rate(gs, i.c, gamma, i.a, i.e, zero).map_err(|e| run(&e))?,
    );
    let cg = crate::reference::conformal_reference(&cfg.conformal_settings())?;

    let mut m = Map::new();
    m.insert(
        "b7".into(),
        est(
            j2_rate,
            "planar 3 pi J2 R^2/p^2 per orbit (orbit in the solar equator) at the run's J2",
        ),
    );
    m.insert(
        "b8".into(),
        est(gr, "6 pi GM/(c^2 p) per orbit x (2 + 2 gamma - beta)/3"),
    );
    m.insert(
        "b9".into(),
        est(
            lt,
            "-2 (1 + gamma) G S/(c^2 a^3 (1-e^2)^1.5), spin axis normal to the orbit, run's spin",
        ),
    );
    m.insert(
        "b11".into(),
        est(
            gr,
            "the Sun's 1PN term only (6 pi GM/(c^2 p) x PPN factor); EIH adds the planets' terms",
        ),
    );
    m.insert(
        "b12".into(),
        est(
            cg.total,
            "comparison model: first-order Gauss secular average (conformal.reference)",
        ),
    );
    if cfg.is_lab() {
        let rung = match cfg.rung() {
            LabRung::Newton => None,
            LabRung::SrA => Some((unit, "SR-a: 1 x pi GM/(c^2 p) per orbit (GR/6, ADR-0012)")),
            LabRung::SrBHarmonic => Some((
                7.0 * unit,
                "SR-b harmonic: 7 x pi GM/(c^2 p) per orbit (7/6 GR)",
            )),
            LabRung::SrBAreal => Some((
                5.0 * unit,
                "SR-b areal: 5 x pi GM/(c^2 p) per orbit (5/6 GR)",
            )),
            LabRung::GrPpn => Some((gr, "6 pi GM/(c^2 p) per orbit x (2 + 2 gamma - beta)/3")),
        };
        if let Some((v, basis)) = rung {
            m.insert("rung".into(), est(v, basis));
        }
    }
    Ok(m)
}

/// Whether each literature row applies to `cfg`: Park17's values are for nominal physics, so an
/// override of the effect's own parameter makes the row informative only.
fn literature_applies(cfg: &ViewerConfig, key: &str) -> bool {
    match key {
        "b7" => cfg.j2_override().is_none(),
        "b8" | "b11" => cfg.ppn_is_gr(),
        "b9" => cfg.spin_scale().is_none() && cfg.ppn_is_gr(),
        _ => true,
    }
}

/// The metadata's `dashboard` object (ABI v2, additive): the fixed reference columns, the engine
/// analytic estimates for `cfg` (`null` without `embed-header`), and per-row literature
/// applicability. `null` only if an embedded vector file is unreadable (a build error in practice).
pub(crate) fn json(cfg: &ViewerConfig) -> Value {
    let Ok(fixed) = fixed() else {
        return Value::Null;
    };
    let mut v = fixed.clone();
    if let Some(rows) = v
        .get_mut("literature")
        .and_then(|l| l.get_mut("rows"))
        .and_then(Value::as_object_mut)
    {
        for (key, row) in rows.iter_mut() {
            if let Some(o) = row.as_object_mut() {
                o.insert("applies".into(), Value::Bool(literature_applies(cfg, key)));
            }
        }
    }
    let analytic = analytic(cfg).map_or(Value::Null, |m| {
        json!({
            "label": "engine analytic estimate",
            "short": "secular, no window · SW92 mean a, e · first order",
            "frame": "orbit plane (secular rate of the longitude of perihelion)",
            "window": "none (secular average)",
            "protocol": "first-order closed form for Mercury's SW92 J2000 mean a, e, DE440 constants",
            "rows": m,
        })
    });
    if let Some(o) = v.as_object_mut() {
        o.insert("analytic".into(), analytic);
        o.insert(
            "comparison_bits".into(),
            json!([bits::CONFORMAL.trailing_zeros()]),
        );
    }
    v
}
