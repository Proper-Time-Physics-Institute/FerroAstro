// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The live rate fit (DESIGN §6.2, §6.3): [`fit_longitudes`] on the projected passage
//! longitudes, with the fit model chosen from the span.

#![forbid(unsafe_code)]

use crate::config::bits;
use crate::{FIT_STRIDE, SECONDS_PER_YEAR, ViewerError};
use ferroastro_precession::park2017::{
    FitModel, Frequency, PARK17_TABLE2, Park2017Fit, fit_longitudes, park17_table2_frequencies,
};
use std::f64::consts::TAU;

/// Fewest passages a fit accepts (≈ 5 yr).
pub(crate) const MIN_PASSAGES: usize = 20;
/// Shortest fitted span (Julian centuries) with the quadratic term `Q t²`.
const QUADRATIC_MIN_CENTURIES: f64 = 5.0;

/// Jack-knife blocks for `n` passages: `n / 20`, clamped to 2 … 10 (DESIGN §6.2).
pub(crate) fn jackknife_blocks(n: usize) -> usize {
    (n / 20).clamp(2, 10)
}

/// What a fit may include.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ModelChoice {
    /// The toggle mask (a Table 2 term needs every planet it involves).
    pub(crate) mask: u32,
    /// `FIT_PERIODIC = 1`.
    pub(crate) periodic: bool,
    /// No periodic terms at all (Rung lab: a single body has none).
    pub(crate) single_body: bool,
}

/// The fit model for a span of `span_cy` Julian centuries (DESIGN §6.3): `Q` from 5 cy, and the
/// Park17 Table 2 terms (SW92 Table 2a mean motions) of toggled planets whose period is at most
/// half the span.
///
/// # Errors
/// `FAV_E_ARG` if the embedded mean motions are missing.
pub(crate) fn model(span_cy: f64, choice: ModelChoice) -> Result<FitModel, ViewerError> {
    let quadratic = span_cy >= QUADRATIC_MIN_CENTURIES;
    if choice.single_body || !choice.periodic {
        return Ok(FitModel {
            quadratic,
            frequencies: Vec::new(),
        });
    }
    let n = crate::ic::sw92_mean_motions()?;
    let all = park17_table2_frequencies(n, "SW92 T2A");
    // Planets of [k_M, k_V, k_E, k_J, k_S]: Mercury is always on.
    let needs = [0, bits::VENUS, bits::EMB, bits::JUPITER, bits::SATURN];
    let frequencies: Vec<Frequency> = all
        .into_iter()
        .zip(PARK17_TABLE2.iter())
        .filter(|(f, (k, _))| {
            let planets_on = k
                .iter()
                .zip(needs)
                .all(|(&ki, b)| ki == 0 || b == 0 || choice.mask & b != 0);
            let period_cy = TAU / f.rad_per_century.abs();
            planets_on && period_cy <= span_cy / 2.0
        })
        .map(|(f, _)| f)
        .collect();
    Ok(FitModel {
        quadratic,
        frequencies,
    })
}

/// A fit and its model.
#[derive(Debug, Clone)]
pub(crate) struct FitResult {
    pub(crate) fit: Park2017Fit,
    pub(crate) model: FitModel,
    pub(crate) record: [f64; FIT_STRIDE],
    pub(crate) span_years: (f64, f64),
}

/// Fit the passages with `t` (years since the run epoch) in `[t0, t1]` (NaN, NaN: all).
/// `t_s` are TDB seconds from J2000, `lon` the unwrapped longitudes (rad), `offset_s` the run
/// epoch in seconds from J2000.
///
/// # Errors
/// `FAV_E_ARG` for a bad window; `FAV_E_FIT` for fewer than [`MIN_PASSAGES`] passages or an
/// ill-posed fit.
pub(crate) fn fit(
    t_s: &[f64],
    lon: &[f64],
    offset_s: f64,
    window: (f64, f64),
    choice: ModelChoice,
) -> Result<FitResult, ViewerError> {
    let (t0, t1) = window;
    let all = t0.is_nan() && t1.is_nan();
    if !(all || (t0.is_finite() && t1.is_finite() && t0 <= t1)) {
        return Err(ViewerError::arg(format!(
            "FA-E0002: fit window [{t0}, {t1}] years: pass two finite times t0 <= t1, or NaN, NaN \
             for every passage"
        )));
    }
    let years = |t: f64| (t - offset_s) / SECONDS_PER_YEAR;
    let (ts, ls): (Vec<f64>, Vec<f64>) = t_s
        .iter()
        .zip(lon)
        .filter(|(t, _)| all || (t0..=t1).contains(&years(**t)))
        .map(|(t, l)| (*t, *l))
        .unzip();
    let n = ts.len();
    if n < MIN_PASSAGES {
        return Err(ViewerError::fit(format!(
            "FA-E0603: {n} perihelion passages in the fit window (need >= {MIN_PASSAGES}, about 5 \
             years); keep running"
        )));
    }
    let (first, last) = match (ts.first(), ts.last()) {
        (Some(a), Some(b)) => (*a, *b),
        _ => return Err(ViewerError::fit("FA-E0603: no passages")),
    };
    let span_cy = (last - first) / (100.0 * SECONDS_PER_YEAR);
    let model = model(span_cy, choice)?;
    let fit = fit_longitudes(&ts, &ls, &model, jackknife_blocks(n))
        .map_err(|e| ViewerError::fit(e.to_string()))?;
    let span_years = (years(first), years(last));
    #[expect(
        clippy::cast_precision_loss,
        reason = "passage and term counts are far below 2^53"
    )]
    let record = [
        fit.arcsec_per_century(),
        fit.formal_sigma_arcsec_per_century(),
        fit.jackknife_sigma_arcsec_per_century(),
        fit.q_arcsec_per_cy2,
        fit.rms_residual.arcsec(),
        n as f64,
        span_years.0,
        span_years.1,
        model.frequencies.len() as f64,
        fit.b.arcsec(),
    ];
    Ok(FitResult {
        fit,
        model,
        record,
        span_years,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: NUM-011
    #[test]
    fn model_follows_the_span_and_the_toggles() {
        let c = |mask| ModelChoice {
            mask,
            periodic: true,
            single_body: false,
        };
        let short = model(0.1, c(bits::PLANETS)).unwrap(); // lint: allow-literal(a 10-yr span)
        let long = model(10.0, c(bits::PLANETS)).unwrap();
        assert!(!short.quadratic && long.quadratic);
        assert!(short.frequencies.len() < long.frequencies.len());
        assert_eq!(long.frequencies.len(), PARK17_TABLE2.len());
        assert!(model(10.0, c(0)).unwrap().frequencies.is_empty());
        assert_eq!(jackknife_blocks(MIN_PASSAGES), 2);
    }
}
