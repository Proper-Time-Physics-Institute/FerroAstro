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

/// f64 in the paired-difference record ([`paired`]).
pub(crate) const PAIR_STRIDE: usize = 6;

/// One leg of a paired difference: its passages (TDB s from J2000, unwrapped projected longitude
/// in rad) and the fit-model choice of its own run.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Leg<'a> {
    pub(crate) t_s: &'a [f64],
    pub(crate) lon: &'a [f64],
    pub(crate) choice: ModelChoice,
}

/// A leg's full fit with its own model (the same fit `fav_fit` gives that leg over every
/// passage).
fn leg_fit(leg: Leg<'_>) -> Result<(Park2017Fit, FitModel), ViewerError> {
    let n = leg.t_s.len();
    if n < MIN_PASSAGES || leg.lon.len() != n {
        return Err(ViewerError::fit(format!(
            "FA-E0603: a paired leg has {n} passages and {} longitudes (need >= {MIN_PASSAGES} \
             of each)",
            leg.lon.len()
        )));
    }
    let (first, last) = match (leg.t_s.first(), leg.t_s.last()) {
        (Some(a), Some(b)) => (*a, *b),
        _ => return Err(ViewerError::fit("FA-E0603: no passages")),
    };
    let m = model((last - first) / (100.0 * SECONDS_PER_YEAR), leg.choice)?;
    let f = fit_longitudes(leg.t_s, leg.lon, &m, jackknife_blocks(n))
        .map_err(|e| ViewerError::fit(e.to_string()))?;
    Ok((f, m))
}

/// The rate of `leg` refitted with `m` after deleting the passages for which `drop` is true.
fn refit(leg: Leg<'_>, m: &FitModel, drop: impl Fn(f64) -> bool) -> Result<f64, ViewerError> {
    let (ts, ls): (Vec<f64>, Vec<f64>) = leg
        .t_s
        .iter()
        .zip(leg.lon)
        .filter(|(t, _)| !drop(**t))
        .map(|(t, l)| (*t, *l))
        .unzip();
    // Two blocks: the replicate's own jack-knife is not used, only its rate.
    fit_longitudes(&ts, &ls, m, 2)
        .map(|f| f.arcsec_per_century())
        .map_err(|e| ViewerError::fit(format!("FA-E0603: paired jack-knife replicate: {e}")))
}

/// The paired-difference jack-knife of `ϖ̇(a) − ϖ̇(b)` for two legs that share initial conditions
/// and the estimator (DESIGN §6.4): each leg keeps its own fit model, the same time block is
/// deleted from both legs, and the jack-knife runs on the replicate *differences*, so the
/// correlated residuals the legs share (the planets' periodic signatures, sampling phase) cancel
/// instead of being added in quadrature.
///
/// The blocks are `G = jackknife_blocks(min(n_a, n_b))` equal slices of the common span
/// `[max(first), min(last)]`; a passage outside it belongs to the nearest end block.
///
/// Returns `[difference, paired_sigma, independent_sigma, blocks, n_a, n_b]` in ″/cy, where
/// `independent_sigma = √(σ_a² + σ_b²)` from each leg's own jack-knife (the old combination,
/// kept for comparison).
///
/// # Errors
/// `FAV_E_FIT` for fewer than [`MIN_PASSAGES`] passages in a leg, legs that do not overlap in
/// time, or an ill-posed fit.
pub(crate) fn paired(a: Leg<'_>, b: Leg<'_>) -> Result<[f64; PAIR_STRIDE], ViewerError> {
    let (fa, ma) = leg_fit(a)?;
    let (fb, mb) = leg_fit(b)?;
    let n = a.t_s.len().min(b.t_s.len());
    let blocks = jackknife_blocks(n);
    let t0 = fa.span_s.0.max(fb.span_s.0);
    let t1 = fa.span_s.1.min(fb.span_s.1);
    if !(t1 - t0).is_finite() || t1 <= t0 {
        return Err(ViewerError::fit(
            "FA-E0603: the two legs of a paired difference do not overlap in time",
        ));
    }
    #[expect(clippy::cast_precision_loss, reason = "block counts are small")]
    let g = blocks as f64;
    let block = |t: f64| ((t - t0) / (t1 - t0) * g).floor().clamp(0.0, g - 1.0);
    let mut d = Vec::with_capacity(blocks);
    for k in 0..blocks {
        #[expect(clippy::cast_precision_loss, reason = "block counts are small")]
        let k = k as f64;
        let ra = refit(a, &ma, |t| (block(t) - k).abs() < 0.5)?;
        let rb = refit(b, &mb, |t| (block(t) - k).abs() < 0.5)?;
        d.push(ra - rb);
    }
    let mean = d.iter().sum::<f64>() / g;
    let paired_sigma = ((g - 1.0) / g * d.iter().map(|x| (x - mean).powi(2)).sum::<f64>()).sqrt();
    let independent = fa
        .jackknife_sigma_arcsec_per_century()
        .hypot(fb.jackknife_sigma_arcsec_per_century());
    #[expect(
        clippy::cast_precision_loss,
        reason = "passage counts are far below 2^53"
    )]
    Ok([
        fa.arcsec_per_century() - fb.arcsec_per_century(),
        paired_sigma,
        independent,
        g,
        a.t_s.len() as f64,
        b.t_s.len() as f64,
    ])
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
        assert_eq!(model(10.0, c(0)).unwrap().frequencies.len(), 0);
        assert_eq!(jackknife_blocks(MIN_PASSAGES), 2);
    }

    // The synthetic signal of `legs` (not physical values: a shaped test input).
    const STEP_DAYS: f64 = 88.0; // lint: allow-literal(one passage per Mercury orbit, roughly)
    const LCG_MUL: u64 = 6_364_136_223_846_793_005; // lint: allow-literal(Knuth's MMIX LCG multiplier)
    const WALK_STEP: f64 = 0.02; // lint: allow-literal(random-walk step of the shared residual, arcsec)
    const SINE_AMPL: f64 = 0.3; // lint: allow-literal(shared periodic residual amplitude, arcsec)
    const SINE_PER_CY: f64 = 4.0; // lint: allow-literal(shared periodic residual, cycles per century)
    const TREND_A: f64 = 43.0; // lint: allow-literal(leg a trend, arcsec/cy)
    const TREND_B: f64 = 42.9; // lint: allow-literal(leg b trend, arcsec/cy)
    const TREND_DIFF: f64 = 0.1; // lint: allow-literal(TREND_A - TREND_B)
    const DIFF_TOL: f64 = 1e-2; // lint: allow-literal(slack on the recovered trend difference)
    const OWN_NOISE: [f64; 3] = [0.0, 1e-3, 1e-2]; // lint: allow-literal(per-leg white noise, arcsec)
    const ROUND_OFF: f64 = 1e-9; // lint: allow-literal(round-off relative to the independent sigma)
    const ZERO: f64 = 1e-12; // lint: allow-literal(identical legs: zero to round-off)
    const CENTURY_YEARS: f64 = 100.0; // lint: allow-literal(a century of synthetic passages)
    const HALF_CENTURY_YEARS: f64 = 50.0; // lint: allow-literal(half a century of synthetic passages)

    /// Synthetic legs over `years`, one passage per [`STEP_DAYS`]: a shared correlated signal (a
    /// sinusoid and a red random walk) on top of a linear trend per leg, plus `own` of white
    /// noise that differs between the legs.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::suboptimal_flops,
        reason = "a synthetic test signal: small positive counts, readability over fused ops"
    )]
    fn legs(years: f64, own: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let step = STEP_DAYS * ferroastro_core::time::SECONDS_PER_DAY;
        let n = (years * SECONDS_PER_YEAR / step) as usize;
        let mut seed = 0x2545_f491_u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(LCG_MUL).wrapping_add(1);
            ((seed >> 11) as f64 / (1_u64 << 53) as f64) - 0.5
        };
        let (mut t, mut a, mut b) = (Vec::new(), Vec::new(), Vec::new());
        let mut walk = 0.0;
        let rad = |x: f64| x / crate::observe::arcsec(1.0);
        for k in 0..n {
            let ts = step * k as f64;
            let cy = ts / (CENTURY_YEARS * SECONDS_PER_YEAR);
            walk += WALK_STEP * rnd();
            let common = SINE_AMPL * (TAU * cy * SINE_PER_CY).sin() + walk;
            t.push(ts);
            a.push(rad(TREND_A * cy + common + own * rnd()));
            b.push(rad(TREND_B * cy + common + own * rnd()));
        }
        (t, a, b)
    }

    const ONE_BODY: ModelChoice = ModelChoice {
        mask: 0,
        periodic: false,
        single_body: true,
    };

    /// Verifies: NUM-011
    #[test]
    fn paired_sigma_is_at_most_the_independent_combination() {
        for own in OWN_NOISE {
            let (t, a, b) = legs(CENTURY_YEARS, own);
            let leg = |lon| Leg {
                t_s: &t,
                lon,
                choice: ONE_BODY,
            };
            let r = paired(leg(&a), leg(&b)).unwrap();
            let [diff, paired_sigma, independent, blocks, n_a, n_b] = r;
            assert!(
                (diff - TREND_DIFF).abs() < paired_sigma.mul_add(10.0, DIFF_TOL),
                "{r:?}"
            );
            assert!(paired_sigma <= independent, "{r:?}");
            assert!(paired_sigma >= 0.0 && blocks >= 2.0 && n_a.to_bits() == n_b.to_bits());
            if own <= 0.0 {
                // Only the trends differ: the shared residuals cancel to round-off.
                assert!(paired_sigma < ROUND_OFF * independent.max(1.0), "{r:?}");
            }
        }
    }

    /// Verifies: NUM-011
    #[test]
    fn paired_sigma_of_identical_legs_is_zero() {
        // The uniform γ0 centring with velocity-independent forces: both legs are the same run.
        let (t, a, _) = legs(HALF_CENTURY_YEARS, DIFF_TOL);
        let leg = Leg {
            t_s: &t,
            lon: &a,
            choice: ONE_BODY,
        };
        let r = paired(leg, leg).unwrap();
        assert!(r[0].abs() < ZERO && r[1] < ZERO, "{r:?}");
        assert!(r[2] > 0.0, "the independent combination is not zero: {r:?}");
        let short = Leg {
            t_s: &t[..5],
            lon: &a[..5],
            choice: ONE_BODY,
        };
        assert!(paired(short, leg).is_err());
    }
}
