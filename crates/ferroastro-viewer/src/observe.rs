// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Observables (DESIGN §6.1, §4.4): osculating elements and ϖ of the display series, and the
//! incremental location of perihelion passages by the Park2017 estimator in Mercury's sampled
//! heliocentric states.
//!
//! The passage search calls [`find_passages`] on a sliding window of samples. Each passage
//! depends only on the `order` samples around its bracket, so the passages found chunk by chunk
//! are bitwise those of one call on the whole track (the diffrun `SampledStates` estimator),
//! whatever the chunking (DESIGN E12, E13).

#![forbid(unsafe_code)]

use crate::ViewerError;
use ferroastro_analytic::kepler::Elements;
use ferroastro_analytic::vector::{cross, dot, eccentricity_vector, norm};
use ferroastro_core::frames::{EclipticJ2000, Frame, Heliocentric, Position, State};
use ferroastro_core::time::{Epoch, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_precession::park2017::{
    PassageConfig, ReferencePlane, SampledOrbit, SampledPassage, find_passages,
};
use std::f64::consts::TAU;

/// Seconds per day.
const DAY: f64 = ferroastro_core::time::SECONDS_PER_DAY;

/// Arcseconds per radian.
pub(crate) fn arcsec(rad: f64) -> f64 {
    rad.to_degrees() * 3600.0
}

/// Unwrapping of an angle series, with the arithmetic of
/// [`ferroastro_precession::park2017::perihelion_longitudes`] (so the passage longitudes are
/// bitwise the estimator's).
#[derive(Debug, Clone, Default)]
pub(crate) struct Unwrap {
    prev: Option<f64>,
    offset: f64,
}

impl Unwrap {
    pub(crate) fn push(&mut self, a: f64) -> f64 {
        if !a.is_finite() {
            return f64::NAN;
        }
        if let Some(p) = self.prev {
            let d = a + self.offset - p;
            self.offset -= TAU * (d / TAU).round();
        }
        let x = a + self.offset;
        self.prev = Some(x);
        x
    }
}

/// One sample: TDB seconds since the run epoch, heliocentric position (km) and velocity (km/s).
pub(crate) type Sample = (f64, [f64; 3], [f64; 3]);

/// Locates perihelion passages in a growing sampled track.
#[derive(Debug, Clone)]
pub(crate) struct PassageLocator {
    epoch_jd: f64,
    window: Vec<Sample>,
    keep: usize,
    last_t_s: Option<f64>,
    config: PassageConfig,
}

fn bad(e: impl std::fmt::Display) -> ViewerError {
    ViewerError::run(e.to_string())
}

impl PassageLocator {
    /// A locator for samples every `sample_days` from `epoch_jd` (TDB); it keeps a window of
    /// about 3.4 orbits of Mercury, enough for two complete interpolation windows.
    pub(crate) fn new(epoch_jd: f64, sample_days: f64) -> Self {
        let config = PassageConfig::default();
        let per_window = (300.0 / sample_days).ceil();
        let keep = crate::engine::to_usize(per_window).unwrap_or(usize::MAX / 2) + 4 * config.order;
        Self {
            epoch_jd,
            window: Vec::new(),
            keep,
            last_t_s: None,
            config,
        }
    }

    /// The samples as a [`SampledOrbit`], built exactly as `ferroastro_diffrun::Estimator` builds
    /// it from a track (typed epochs `JD = epoch + t/86400` and heliocentric states).
    fn orbit<F: Frame>(&self) -> Result<SampledOrbit<F>, ViewerError> {
        let mut epochs = Vec::with_capacity(self.window.len());
        let mut states = Vec::with_capacity(self.window.len());
        for (t, r, v) in &self.window {
            epochs.push(Epoch::<Tdb>::from_jd_parts(self.epoch_jd, t / DAY).map_err(bad)?);
            let l = |x: f64| Length::from_km(x).map_err(bad);
            let w = |x: f64| Velocity::from_km_per_s(x).map_err(bad);
            states.push(State::<F, Heliocentric>::new(
                Position::new(l(r[0])?, l(r[1])?, l(r[2])?),
                [w(v[0])?, w(v[1])?, w(v[2])?],
            ));
        }
        SampledOrbit::from_states(&epochs, &states).map_err(bad)
    }

    /// Append samples (in time order) and return the passages completed by them.
    ///
    /// # Errors
    /// `FAV_E_RUN` if the window is full and the passage search still fails (e.g. sampling too
    /// coarse) or a sample is not finite.
    pub(crate) fn push<F: Frame>(
        &mut self,
        samples: impl IntoIterator<Item = Sample>,
    ) -> Result<Vec<SampledPassage>, ViewerError> {
        self.window.extend(samples);
        if self.window.len() < 16 {
            return Ok(Vec::new());
        }
        let found = match find_passages(&self.orbit::<F>()?, &self.config) {
            Ok(ps) => ps,
            Err(e) if self.window.len() > self.keep => return Err(bad(e)),
            // Fewer than two passages in the window yet (the first orbits of a run).
            Err(_) => Vec::new(),
        };
        let fresh: Vec<SampledPassage> = found
            .into_iter()
            .filter(|p| self.last_t_s.is_none_or(|l| p.t_s > l))
            .collect();
        if let Some(p) = fresh.last() {
            self.last_t_s = Some(p.t_s);
        }
        if self.window.len() > self.keep {
            let cut = self.window.len() - self.keep;
            self.window.drain(..cut);
        }
        Ok(fresh)
    }
}

/// How ϖ is formed on the reference plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarpiForm {
    /// `Ω + ω` with the node on the plane (DESIGN §6.1; System mode).
    DogLeg,
    /// The in-plane angle of the projected eccentricity vector (Rung lab, where the orbit can lie
    /// in the reference plane and the node is undefined).
    ProjectedEccentricity,
}

/// Osculating elements and ϖ of the display series.
#[derive(Debug, Clone)]
pub(crate) struct ElementObserver<F: Frame> {
    mu: f64,
    au_km: f64,
    plane: ReferencePlane<F>,
    form: VarpiForm,
    ecliptic: ReferencePlane<EclipticJ2000>,
    varpi: Unwrap,
    varpi_ecl: Unwrap,
}

/// `Ω + ω` of the orbit with eccentricity vector `e` and angular momentum `h` on `plane`, or the
/// projected angle of `e` when the node is undefined.
fn varpi_on<F: Frame>(plane: &ReferencePlane<F>, e: [f64; 3], h: [f64; 3], form: VarpiForm) -> f64 {
    match form {
        VarpiForm::DogLeg => plane
            .dog_leg_angle(e, cross(h, e))
            .unwrap_or_else(|_| plane.projected_angle(e)),
        VarpiForm::ProjectedEccentricity => plane.projected_angle(e),
    }
}

impl<F: Frame> ElementObserver<F> {
    pub(crate) fn new(mu: f64, au_km: f64, plane: ReferencePlane<F>, form: VarpiForm) -> Self {
        Self {
            mu,
            au_km,
            plane,
            form,
            ecliptic: ReferencePlane::xy_plane(),
            varpi: Unwrap::default(),
            varpi_ecl: Unwrap::default(),
        }
    }

    /// `[t, varpi_deg, e, a_au, i_deg, varpi_ecl_deg]` from `(r, v)` in frame `F` and the same
    /// state in ECLIPJ2000 (`re`, `ve`).
    pub(crate) fn row(
        &mut self,
        t_years: f64,
        (r, v): ([f64; 3], [f64; 3]),
        (re, ve): ([f64; 3], [f64; 3]),
    ) -> [f64; 6] {
        let ev = eccentricity_vector(r, v, self.mu);
        let h = cross(r, v);
        let (e, a) = Elements::from_vectors(r, v, self.mu)
            .map_or((f64::NAN, f64::NAN), |el| (el.e, el.a.km() / self.au_km));
        let cos_i = (dot(h, self.plane.pole()) / norm(h)).clamp(-1.0, 1.0);
        let w = self.varpi.push(varpi_on(&self.plane, ev, h, self.form));
        let eve = eccentricity_vector(re, ve, self.mu);
        let we = self.varpi_ecl.push(varpi_on(
            &self.ecliptic,
            eve,
            cross(re, ve),
            VarpiForm::DogLeg,
        ));
        [
            t_years,
            w.to_degrees(),
            e,
            a,
            cos_i.acos().to_degrees(),
            we.to_degrees(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies: NUM-011
    #[test]
    fn unwrap_matches_the_estimator() {
        use ferroastro_core::frames::Icrf;
        use ferroastro_precession::park2017::{LongitudeDefinition, perihelion_longitudes};
        let plane = ReferencePlane::<Icrf>::xy_plane();
        let ps: Vec<SampledPassage> = (0..40_u32)
            .map(|k| {
                let a = f64::from(k) * 1.9; // lint: allow-literal(a step that makes the angle wrap)
                SampledPassage {
                    t_s: f64::from(k),
                    r: [a.cos(), a.sin(), 0.0],
                    v: [-a.sin(), a.cos(), 0.0],
                    direction_error_rad: 0.0,
                }
            })
            .collect();
        let want = perihelion_longitudes(&ps, &plane, LongitudeDefinition::Projected).unwrap();
        let mut u = Unwrap::default();
        for (p, w) in ps.iter().zip(want) {
            assert_eq!(u.push(plane.projected_angle(p.r)).to_bits(), w.to_bits());
        }
    }
}
