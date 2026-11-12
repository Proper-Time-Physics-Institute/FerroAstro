// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! One estimator for every run of a plan (CI-009): the Park2017 estimator (NUM-011) applied to a
//! [`Track`], either to its sampled states (the path any tool's state output takes) or to the
//! passages located on IAS15 dense output (NUM-010).

use crate::DiffrunError;
use crate::track::Track;
use ferroastro_core::frames::{Heliocentric, Icrf, Position, State};
use ferroastro_core::time::{Epoch, J2000_JD, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_precession::PrecessionError;
use ferroastro_precession::park2017::{
    ESTIMATOR, Park2017, Park2017Fit, SampledOrbit, SampledPassage, fit_longitudes,
    perihelion_longitudes,
};
use ferroastro_scenario::BodyName;
use serde::{Deserialize, Serialize};

/// Where the passages come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassageSource {
    /// Located by the estimator in the sampled relative states (Lagrange interpolation; the path
    /// of any tool's state output, CI-009).
    SampledStates,
    /// Located by the run driver on IAS15 dense output (NUM-010 events).
    DenseOutputEvents,
}

impl PassageSource {
    /// Label for reports.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            PassageSource::SampledStates => {
                "passages located in the sampled relative states (Lagrange interpolant)"
            }
            PassageSource::DenseOutputEvents => {
                "passages located on IAS15 dense output (NUM-010 events, r.v rising)"
            }
        }
    }
}

/// The estimator of a plan: Park2017 settings, the passage source, the tracked body and an
/// optional fit window.
#[derive(Debug, Clone, PartialEq)]
pub struct Estimator {
    /// Park2017 settings (plane, longitude definition, passage settings, fit model, jack-knife).
    pub park: Park2017<Icrf>,
    /// Passage source of the primary estimate.
    pub source: PassageSource,
    /// Tracked body.
    pub body: BodyName,
    /// Central body (the longitude is of `body − central`).
    pub central: BodyName,
    /// Fit window, TDB Julian dates (inclusive); `None` fits every passage.
    pub window_jd: Option<(f64, f64)>,
}

/// The fitted rate of one configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    /// ϖ̇ (″/cy).
    pub rate: f64,
    /// Formal σ (″/cy).
    pub formal_sigma: f64,
    /// Jack-knife σ (″/cy).
    pub jackknife_sigma: f64,
    /// Quadratic coefficient Q (″/cy²).
    pub q: f64,
    /// RMS of the post-fit residuals (″).
    pub rms_residual: f64,
    /// Passages fitted.
    pub passages: usize,
    /// First and last fitted passage, TDB Julian dates.
    pub span_jd: (f64, f64),
    /// Largest per-passage interpolation-error estimate (″; 0 for dense-output passages).
    pub max_direction_error_arcsec: f64,
}

fn arcsec(rad: f64) -> f64 {
    rad.to_degrees() * 3600.0
}

impl Estimate {
    fn from_fit(f: &Park2017Fit, max_direction_error_rad: f64) -> Self {
        let jd = |t: f64| J2000_JD + t / SECONDS_PER_DAY;
        Self {
            rate: f.arcsec_per_century(),
            formal_sigma: f.formal_sigma_arcsec_per_century(),
            jackknife_sigma: f.jackknife_sigma_arcsec_per_century(),
            q: f.q_arcsec_per_cy2,
            rms_residual: f.rms_residual.arcsec(),
            passages: f.samples,
            span_jd: (jd(f.span_s.0), jd(f.span_s.1)),
            max_direction_error_arcsec: arcsec(max_direction_error_rad),
        }
    }
}

fn bad(e: impl std::fmt::Display) -> DiffrunError {
    DiffrunError::Precession(PrecessionError::EstimatorInput(e.to_string()))
}

impl Estimator {
    /// The estimator id that every row of a plan's table carries (CI-009, NUM-012, PHY-015).
    #[must_use]
    pub fn id(&self, source: PassageSource) -> String {
        let window = self.window_jd.map_or_else(
            || "all passages".to_owned(),
            |(a, b)| format!("passages in JD {a} .. {b} TDB"),
        );
        format!(
            "{ESTIMATOR}; longitude {}; plane {}; fit {}; {} jack-knife blocks; {}; Lagrange \
             order {}; {window}; body {} relative to {}",
            self.park.definition,
            self.park.plane.label(),
            self.park.model.describe(),
            self.park.jackknife_blocks,
            source.label(),
            self.park.passages.order,
            self.body.label(),
            self.central.label()
        )
    }

    fn in_window(&self, jd: f64) -> bool {
        self.window_jd.is_none_or(|(a, b)| (a..=b).contains(&jd))
    }

    /// Estimate from `source`.
    ///
    /// # Errors
    /// `FA-E0603` for too few samples or passages, `FA-E0601` for a degenerate fit.
    pub fn estimate(&self, track: &Track, source: PassageSource) -> Result<Estimate, DiffrunError> {
        match source {
            PassageSource::SampledStates => self.on_samples(track),
            PassageSource::DenseOutputEvents => self.on_passages(track),
        }
    }

    fn on_samples(&self, track: &Track) -> Result<Estimate, DiffrunError> {
        let mut epochs = Vec::with_capacity(track.t.len());
        let mut states = Vec::with_capacity(track.t.len());
        for ((t, r), v) in track.t.iter().zip(&track.r).zip(&track.v) {
            epochs.push(
                Epoch::<Tdb>::from_jd_parts(track.epoch_jd_tdb, t / SECONDS_PER_DAY)
                    .map_err(bad)?,
            );
            let l = |x: f64| Length::from_km(x).map_err(bad);
            let w = |x: f64| Velocity::from_km_per_s(x).map_err(bad);
            states.push(State::<Icrf, Heliocentric>::new(
                Position::new(l(r[0])?, l(r[1])?, l(r[2])?),
                [w(v[0])?, w(v[1])?, w(v[2])?],
            ));
        }
        let mut orbit = SampledOrbit::from_states(&epochs, &states)?;
        if let Some((a, b)) = self.window_jd {
            orbit = orbit.window(a, b)?;
        }
        let r = self.park.estimate(&orbit)?;
        Ok(Estimate::from_fit(&r.fit, r.max_direction_error_rad))
    }

    fn on_passages(&self, track: &Track) -> Result<Estimate, DiffrunError> {
        let offset = (track.epoch_jd_tdb - J2000_JD) * SECONDS_PER_DAY;
        let passages: Vec<SampledPassage> = track
            .passages
            .iter()
            .map(|p| SampledPassage {
                t_s: offset + p.t,
                r: p.r,
                v: p.v,
                direction_error_rad: 0.0,
            })
            .filter(|p| self.in_window(J2000_JD + p.t_s / SECONDS_PER_DAY))
            .collect();
        let lon = perihelion_longitudes(&passages, &self.park.plane, self.park.definition)?;
        let t: Vec<f64> = passages.iter().map(|p| p.t_s).collect();
        let fit = fit_longitudes(&t, &lon, &self.park.model, self.park.jackknife_blocks)?;
        Ok(Estimate::from_fit(&fit, 0.0))
    }
}
