// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Accuracy columns computed identically from every tool's output (WP18): so that speed is never
//! traded away silently, each timed row of the speed table carries the accuracy that run achieved.
//!
//! * [`relative_energy_change`]: Newtonian point-mass energy of the sampled system at the first
//!   and the last output time, in the system's centre-of-mass frame (so barycentric and
//!   heliocentric outputs give the same number), with the GMs supplied by the caller.
//! * [`mercury_max_dr`] / [`max_dr`]: the largest heliocentric (or barycentric) position
//!   difference of a body between two series at their common output times (WP28: the round-off
//!   column against a double-double twin).
//! * [`mercury_track`]: Mercury relative to the Sun as a `ferroastro-diffrun` [`Track`], for the
//!   Park2017 estimator (CI-009: one estimator for every tool).

use crate::BenchError;
use crate::series::{NAIF_MERCURY, NAIF_SUN, Row, Series, sub3};
use ferroastro_core::frames::Barycentric;
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_diffrun::Track;
use ferroastro_ephem::{Ephemeris, NaifId, Spk};
use std::collections::BTreeMap;

/// Two output times are the same sample when they differ by at most this (days; ~0.1 ms):
/// FerroAstro stores `k · cadence` in seconds, the oracles `k · cadence` in days.
pub const TIME_MATCH_DAYS: f64 = 1e-9;

const SECONDS_PER_DAY: f64 = ferroastro_core::time::SECONDS_PER_DAY;

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// Newtonian energy / G (km⁵/s⁴) of one output time in the centre-of-mass frame of the sampled
/// bodies: `Σ ½ μᵢ |vᵢ − v_cm|² − Σᵢ<ⱼ μᵢ μⱼ / rᵢⱼ`. A heliocentric slice (centre 10) that
/// omits the Sun gets it at the origin at rest.
///
/// # Errors
/// `FA-E1600` if a body has no GM in `gms`.
pub fn com_energy(
    slice: &[Row],
    center_naif: i32,
    gms: &BTreeMap<i32, f64>,
) -> Result<f64, BenchError> {
    let gm = |id: i32| {
        gms.get(&id)
            .copied()
            .ok_or_else(|| BenchError::input(format!("body {id}"), "no GM supplied"))
    };
    let mut bodies: Vec<(f64, [f64; 3], [f64; 3])> = Vec::with_capacity(slice.len() + 1);
    if center_naif != 0 && !slice.iter().any(|r| r.naif == center_naif) {
        bodies.push((gm(center_naif)?, [0.0; 3], [0.0; 3]));
    }
    for r in slice {
        bodies.push((gm(r.naif)?, r.r, r.v));
    }
    let total: f64 = bodies.iter().map(|b| b.0).sum();
    let mut vcm = [0.0; 3];
    for (m, _, v) in &bodies {
        vcm = [
            m.mul_add(v[0], vcm[0]),
            m.mul_add(v[1], vcm[1]),
            m.mul_add(v[2], vcm[2]),
        ];
    }
    let vcm = vcm.map(|x| x / total);
    let mut kinetic = 0.0;
    let mut potential = 0.0;
    for (i, (mi, ri, vi)) in bodies.iter().enumerate() {
        let w = sub3(*vi, vcm);
        kinetic = (0.5 * mi).mul_add(dot(w, w), kinetic);
        for (mj, rj, _) in bodies.iter().skip(i + 1) {
            let d = sub3(*ri, *rj);
            potential -= mi * mj / dot(d, d).sqrt();
        }
    }
    Ok(kinetic + potential)
}

/// `|E(last) − E(first)| / |E(first)|` with [`com_energy`], and the number of bodies in the
/// energy (the sampled bodies, plus the Sun of a heliocentric series that omits it).
///
/// # Errors
/// `FA-E1600` if the series has fewer than two output times, the first and last times hold
/// different bodies, or a GM is missing.
pub fn relative_energy_change(
    series: &Series,
    gms: &BTreeMap<i32, f64>,
) -> Result<(f64, usize), BenchError> {
    let slices = series.slices();
    let (Some(first), Some(last)) = (slices.first(), slices.last()) else {
        return Err(BenchError::input(&series.file, "no output times"));
    };
    let ids = |s: &[Row]| {
        let mut v: Vec<i32> = s.iter().map(|r| r.naif).collect();
        v.sort_unstable();
        v
    };
    if slices.len() < 2 || ids(first) != ids(last) {
        return Err(BenchError::input(
            &series.file,
            "the energy needs two output times holding the same bodies",
        ));
    }
    let e0 = com_energy(first, series.center_naif, gms)?;
    let e1 = com_energy(last, series.center_naif, gms)?;
    let centre_added =
        series.center_naif != 0 && !first.iter().any(|r| r.naif == series.center_naif);
    Ok((
        ((e1 - e0) / e0).abs(),
        first.len() + usize::from(centre_added),
    ))
}

/// The origin a position difference is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Centre {
    /// Relative to the Sun (heliocentric).
    Sun,
    /// The series' own solar-system-barycentric positions (both series must be barycentric).
    Barycentre,
}

/// `body`'s samples `(t_days, r)` of `series` relative to `centre`.
///
/// # Errors
/// As [`Series::heliocentric`]; `FA-E1600` for [`Centre::Barycentre`] on a series that is not
/// barycentric.
pub fn positions(
    series: &Series,
    body: i32,
    centre: Centre,
) -> Result<Vec<(f64, [f64; 3])>, BenchError> {
    match centre {
        Centre::Sun => Ok(series
            .heliocentric(body)?
            .into_iter()
            .map(|(t, r, _)| (t, r))
            .collect()),
        Centre::Barycentre => {
            if series.center_naif != 0 {
                return Err(BenchError::input(
                    &series.file,
                    format!("centre {}: not barycentric", series.center_naif),
                ));
            }
            Ok(series
                .rows
                .iter()
                .filter(|r| r.naif == body)
                .map(|r| (r.t_days, r.r))
                .collect())
        }
    }
}

/// The largest position difference of `body` between `a` and `b` relative to `centre` (km), the
/// time of that maximum (days) and the number of common output times.
///
/// # Errors
/// `FA-E1600` if a series has no such body, or the two series do not share every output time of
/// the shorter one.
pub fn max_dr(
    a: &Series,
    b: &Series,
    body: i32,
    centre: Centre,
) -> Result<(f64, f64, usize), BenchError> {
    let ha = positions(a, body, centre)?;
    let hb = positions(b, body, centre)?;
    let mut j = 0;
    let mut matched = 0;
    let mut worst: Option<(f64, f64)> = None;
    for (ta, ra) in &ha {
        while hb.get(j).is_some_and(|(tb, _)| *tb < ta - TIME_MATCH_DAYS) {
            j += 1;
        }
        if let Some((tb, rb)) = hb.get(j) {
            if (tb - ta).abs() <= TIME_MATCH_DAYS {
                matched += 1;
                let d = sub3(*ra, *rb);
                let dr = dot(d, d).sqrt();
                if worst.is_none_or(|w| dr > w.0) {
                    worst = Some((dr, *ta));
                }
            }
        }
    }
    let need = ha.len().min(hb.len());
    match worst {
        Some((dr, t)) if matched == need => Ok((dr, t, matched)),
        _ => Err(BenchError::input(
            format!("{} vs {}", a.file, b.file),
            format!("{matched} common samples of body {body} of {need}"),
        )),
    }
}

/// The largest heliocentric position difference of Mercury between `a` and `b` (km), the time
/// of that maximum (days) and the number of common output times.
///
/// # Errors
/// As [`max_dr`].
pub fn mercury_max_dr(a: &Series, b: &Series) -> Result<(f64, f64, usize), BenchError> {
    max_dr(a, b, NAIF_MERCURY, Centre::Sun)
}

/// `body`'s heliocentric position difference from an SPK ephemeris (WP28: de440.bsp) over the
/// samples of `series` up to `t_at_days`: `(|Δr| at t_at_days, max |Δr|, time of the max)` (km,
/// km, days). Both positions are relative to the Sun, so the result does not depend on how the
/// run's barycentre was defined.
///
/// # Errors
/// As [`Series::heliocentric`]; `FA-E1600` if the SPK cannot evaluate an epoch, or there is no
/// sample at `t_at_days` (within [`TIME_MATCH_DAYS`]).
pub fn body_vs_spk(
    series: &Series,
    spk: &Spk<'_>,
    body: i32,
    t_at_days: f64,
) -> Result<(f64, f64, f64), BenchError> {
    let bad = |p: String| BenchError::input(&series.file, p);
    let pos = |id: i32, t: f64| -> Result<[f64; 3], BenchError> {
        let e = Epoch::<Tdb>::from_jd_parts(series.epoch_jd_tdb, t)
            .map_err(|e| bad(format!("epoch {t} d: {e}")))?;
        let s = spk
            .state::<Barycentric, f64>(NaifId(id), e)
            .map_err(|e| bad(format!("SPK body {id} at {t} d: {e}")))?;
        Ok(s.position()
            .components()
            .map(ferroastro_core::units::Length::km))
    };
    let (mut at, mut worst, mut t_worst) = (None, 0.0_f64, 0.0);
    for (t, r, _) in series.heliocentric(body)? {
        if t > t_at_days + TIME_MATCH_DAYS {
            break;
        }
        let d = sub3(r, sub3(pos(body, t)?, pos(NAIF_SUN, t)?));
        let dr = dot(d, d).sqrt();
        if dr > worst {
            (worst, t_worst) = (dr, t);
        }
        if (t - t_at_days).abs() <= TIME_MATCH_DAYS {
            at = Some(dr);
        }
    }
    at.map(|a| (a, worst, t_worst))
        .ok_or_else(|| bad(format!("no sample of body {body} at t = {t_at_days} d")))
}

/// Mercury relative to the Sun as a [`Track`] (sample times in TDB seconds since the epoch).
///
/// # Errors
/// As [`Series::heliocentric`]; `FA-E1600` if there is no Mercury sample.
pub fn mercury_track(series: &Series) -> Result<Track, BenchError> {
    let h = series.heliocentric(NAIF_MERCURY)?;
    if h.is_empty() {
        return Err(BenchError::input(&series.file, "no Mercury samples"));
    }
    let mut t = Track::new(series.epoch_jd_tdb);
    for (td, r, v) in h {
        t.t.push(td * SECONDS_PER_DAY);
        t.r.push(r);
        t.v.push(v);
    }
    Ok(t)
}
