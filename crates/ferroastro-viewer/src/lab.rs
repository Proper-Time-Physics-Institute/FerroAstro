// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Rung lab (DESIGN §3.3): Mercury as a test particle about a fixed Sun, the set-up of the M2
//! single-body rungs (`ferroastro_ladder::SingleBody::mercury`: SW92 J2000 mean elements,
//! J2000 mean ecliptic, μ = GM☉), with the rung's central term from `ferroastro-forces` and
//! optional solar J2 and Lense–Thirring terms about the WGCCRE solar pole.
//!
//! IAS15 is driven here, one call of [`Ias15::integrate_to`] per daily sample (so the step
//! sequence depends only on the sample grid, not on how a span is cut into calls), instead of
//! through the ladder's timed `run`: `std::time::Instant` panics on `wasm32-unknown-unknown`.

#![forbid(unsafe_code)]

use crate::config::{LabRung, Param, ViewerConfig, bits};
use crate::engine::{Buffers, PassageSeries, RunInfo};
use crate::observe::{ElementObserver, PassageLocator, VarpiForm};
use crate::{ENGINE_IAS15_TEST_PARTICLE, FAV_DONE, FAV_OK, SECONDS_PER_YEAR, ViewerError};
use ferroastro_analytic::vector::norm;
use ferroastro_core::frames::EclipticJ2000;
use ferroastro_core::time::{J2000_JD, SECONDS_PER_DAY};
use ferroastro_core::units::SpinGm;
use ferroastro_forces::{
    CentralTerm, ForceModel as _, LenseThirring, PnLagrangian, Pole, SchwarzschildPpn, SolarJ2,
    SrKepler, TestParticle,
};
use ferroastro_frames::icrf_to_ecliptic_j2000;
use ferroastro_integrators::{Ias15, Ias15Config};
use ferroastro_ladder::single_body::{SingleBody, de440_solar_spin};
use ferroastro_ladder::{MERCURY_SINGLE_BODY, Scenario};
use ferroastro_precession::park2017::ReferencePlane;

/// Sample cadence of the lab (days).
const SAMPLE_DAYS: f64 = 1.0;

fn err(e: impl std::fmt::Display) -> ViewerError {
    ViewerError::run(e.to_string())
}

/// The rung's test-particle model: the central term chosen by `RUNG` and, for bits 7 and 9, the
/// solar J2 and Lense-Thirring terms about the WGCCRE 2015 pole (in J2000 ecliptic axes).
fn build_model(
    cfg: &ViewerConfig,
    de: &ferroastro_constants::De440,
    mu: ferroastro_core::units::GravParam,
) -> Result<TestParticle<EclipticJ2000>, ViewerError> {
    let c = de.speed_of_light();
    let (beta, gamma) = cfg.ppn();
    let mut terms: Vec<Box<dyn CentralTerm<EclipticJ2000>>> = Vec::new();
    match cfg.rung() {
        LabRung::Newton => {}
        LabRung::SrA => terms.push(Box::new(SrKepler::new(mu, c).map_err(err)?)),
        LabRung::SrBHarmonic => {
            terms.push(Box::new(PnLagrangian::sr_b_harmonic(mu, c).map_err(err)?));
        }
        LabRung::SrBAreal => {
            terms.push(Box::new(PnLagrangian::sr_b_areal(mu, c).map_err(err)?));
        }
        LabRung::GrPpn => {
            terms.push(Box::new(
                SchwarzschildPpn::new(mu, c, beta, gamma).map_err(err)?,
            ));
        }
    }
    let m = cfg.mask();
    if m & (bits::SUN_J2 | bits::LENSE_THIRRING) != 0 {
        // The WGCCRE 2015 solar pole, rotated into the lab's J2000 ecliptic axes.
        let k = ferroastro_frames::Pole::<f64>::sun_wgccre2015()
            .map_err(err)?
            .unit_vector();
        let pole = || {
            Pole::<EclipticJ2000>::from_components(icrf_to_ecliptic_j2000::<f64>().apply(k))
                .map_err(err)
        };
        if m & bits::SUN_J2 != 0 {
            let j2 = cfg.j2_override().unwrap_or_else(|| de.sun_j2());
            terms.push(Box::new(
                SolarJ2::new(mu, j2, de.sun_radius(), pole()?).map_err(err)?,
            ));
        }
        if m & bits::LENSE_THIRRING != 0 {
            let gs = de440_solar_spin(de).map_err(err)?;
            let gs = SpinGm::from_km5_per_s3(gs.km5_per_s3() * cfg.spin_scale().unwrap_or(1.0))
                .map_err(err)?;
            terms.push(Box::new(
                LenseThirring::new(gs, c, gamma, pole()?).map_err(err)?,
            ));
        }
    }
    TestParticle::new(mu, terms).map_err(err)
}

/// A Rung-lab run.
#[derive(Debug)]
pub(crate) struct LabRun {
    model: TestParticle<EclipticJ2000>,
    ias: Ias15,
    next_sample: u64,
    span_s: f64,
    target_s: f64,
    t_s: f64,
    finished: bool,
    display_every: u64,
    au_km: f64,
    locator: PassageLocator,
    observer: ElementObserver<EclipticJ2000>,
    plane: ReferencePlane<EclipticJ2000>,
    pub(crate) series: PassageSeries,
    pub(crate) info: RunInfo,
}

impl LabRun {
    pub(crate) fn start(cfg: &ViewerConfig, out: &mut Buffers) -> Result<Self, ViewerError> {
        let de = crate::ic::de440()?;
        let scn = Scenario::mercury_single_body().map_err(err)?;
        let sb = SingleBody::mercury(&scn, de).map_err(err)?;
        let (r0, v0) = sb.initial_state();
        let mu = sb.mu();
        let model = build_model(cfg, de, mu)?;
        let (beta, gamma) = cfg.ppn();
        let m = cfg.mask();
        let ias_cfg = Ias15Config {
            epsilon: cfg.get(Param::Ias15Epsilon),
            ..Ias15Config::default()
        };
        let ias = Ias15::new(3, 0.0, &r0, &v0, norm(r0) / norm(v0) / 16.0, ias_cfg).map_err(err)?;
        let h0 = ferroastro_analytic::vector::cross(r0, v0);
        let e0 = ferroastro_analytic::vector::eccentricity_vector(r0, v0, mu.km3_per_s2());
        let plane = ReferencePlane::<EclipticJ2000>::from_normal_and_x(
            h0,
            e0,
            "initial osculating orbit plane of the test particle, x at the initial periapsis",
        )
        .map_err(err)?;
        let au_km = de.au().km();
        let display = cfg.get(Param::DisplayCadenceDays);
        let display_every = crate::engine::to_u64((display / SAMPLE_DAYS).round())
            .unwrap_or(1)
            .max(1);
        let rung_label = match cfg.rung() {
            LabRung::Newton => "Newton only",
            LabRung::SrA => "SR-a (SrKepler)",
            LabRung::SrBHarmonic => "SR-b, isotropic/harmonic gauge (PPN isotropic/harmonic)",
            LabRung::SrBAreal => "SR-b, Schwarzschild areal gauge (pedagogical)",
            LabRung::GrPpn => "GR PPN (1PN Schwarzschild)",
        };
        let gauge = model.gauge().label();
        let info = RunInfo {
            mode: "rung-lab",
            engine: ENGINE_IAS15_TEST_PARTICLE,
            engine_label: format!("ias15 eps={:e} (test particle)", ias_cfg.epsilon),
            epoch_jd: J2000_JD,
            model: format!("rung: {rung_label}; {}", model.describe()),
            gauge: gauge.into(),
            frame: format!(
                "{} (J2000 mean ecliptic axes); the SR/GR split of the SR rungs is not observable \
                 (ADR-0012)",
                plane.label()
            ),
            display_frame: "J2000 mean ecliptic (ladder frame), heliocentric, au".into(),
            ic_source: format!(
                "{} ({}), converted with mu = GM_sun; fixed Sun",
                scn.elements_source, scn.elements_frame
            ),
            ic_hash: sb.ic_hash(),
            constants: format!(
                "DE440 constants table sha256 {}",
                de.table().source().sha256
            ),
            scenario_digest: String::new(),
            estimator: format!(
                "{}; longitude projected on the initial osculating orbit plane; Lagrange order 8 on \
                 daily samples; linear fit (Q from 500 yr), no periodic terms",
                ferroastro_precession::park2017::ESTIMATOR
            ),
            scenario_text: format!(
                "# Rung lab (docs/viewer/DESIGN.md section 3.3): rung = {rung_label}, toggle mask \
                 {m:#x}, beta = {beta:?}, gamma = {gamma:?}, span = {:?} yr.\n# Initial \
                 conditions: ferroastro_ladder::MERCURY_SINGLE_BODY, below.\n{MERCURY_SINGLE_BODY}",
                cfg.get(Param::SpanYears)
            ),
        };
        let mut run = Self {
            model,
            ias,
            next_sample: 0,
            span_s: cfg.get(Param::SpanYears) * SECONDS_PER_YEAR,
            target_s: 0.0,
            t_s: 0.0,
            finished: false,
            display_every,
            au_km,
            locator: PassageLocator::new(J2000_JD, SAMPLE_DAYS),
            observer: ElementObserver::new(
                mu.km3_per_s2(),
                au_km,
                plane.clone(),
                VarpiForm::ProjectedEccentricity,
            ),
            plane,
            series: PassageSeries::default(),
            info,
        };
        run.samples(out, Some((r0, v0)))?;
        Ok(run)
    }

    pub(crate) const fn time_years(&self) -> f64 {
        self.t_s / SECONDS_PER_YEAR
    }

    pub(crate) const fn finished(&self) -> bool {
        self.finished
    }

    pub(crate) fn advance(&mut self, years: f64, out: &mut Buffers) -> Result<i32, ViewerError> {
        if self.finished {
            return Ok(FAV_DONE);
        }
        self.target_s = years
            .mul_add(SECONDS_PER_YEAR, self.target_s)
            .min(self.span_s);
        self.samples(out, None)?;
        Ok(if self.finished { FAV_DONE } else { FAV_OK })
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "sample indices stay far below 2^53"
    )]
    fn sample_time(k: u64) -> f64 {
        k as f64 * SAMPLE_DAYS * SECONDS_PER_DAY
    }

    /// Integrate to every sample time up to the target (the initial sample when `initial`).
    fn samples(
        &mut self,
        out: &mut Buffers,
        initial: Option<([f64; 3], [f64; 3])>,
    ) -> Result<(), ViewerError> {
        let mut new = Vec::new();
        if let Some((r, v)) = initial {
            new.push((0.0, r, v));
            self.next_sample = 1;
        }
        loop {
            let tk = Self::sample_time(self.next_sample);
            if tk > self.target_s || tk > self.span_s {
                break;
            }
            self.ias
                .integrate_to(&self.model, tk, |_, _| Ok(()))
                .map_err(err)?;
            let r: [f64; 3] = self.ias.positions().try_into().map_err(err)?;
            let v: [f64; 3] = self.ias.velocities().try_into().map_err(err)?;
            new.push((tk, r, v));
            self.t_s = tk;
            self.next_sample += 1;
        }
        if Self::sample_time(self.next_sample) > self.span_s {
            self.finished = true;
        }
        for (i, (t, r, v)) in new.iter().enumerate() {
            let g = self.next_sample - u64::try_from(new.len() - i).unwrap_or(0);
            if g % self.display_every != 0 {
                continue;
            }
            let ty = t / SECONDS_PER_YEAR;
            out.bodies.push(ty);
            out.bodies.extend(r.map(|x| x / self.au_km));
            out.bodies.extend([f64::NAN; 21]);
            let row = self.observer.row(ty, (*r, *v), (*r, *v));
            out.elements.extend_from_slice(&row);
        }
        for p in self.locator.push::<EclipticJ2000>(new)? {
            let r = p.r;
            self.series
                .add(p, &self.plane, r, (self.au_km, 0.0), &mut out.passages);
        }
        Ok(())
    }
}
