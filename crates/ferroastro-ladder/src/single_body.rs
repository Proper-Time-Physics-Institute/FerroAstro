// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Single-body rungs (ROADMAP M2): SR-a, SR-b (two gauges), GR-SCHW/PPN, the PHY-009
//! coefficient study, J2 and Lense–Thirring, each a test particle about a fixed Sun.
//!
//! A run integrates [`SingleBody::orbits`] anomalistic orbits with IAS15, locates the perihelion
//! passages ([`crate::passage_run`]), fits the per-orbit advance
//! (`ferroastro_precession::passages`) and compares it with the rung's analytic oracle
//! (`ferroastro_analytic`, NUM-015). Every result carries a [`RateReport`] (PHY-015).

use crate::passage_run::{find_perihelia, sample_uniform};
use crate::{LadderError, Scenario};
use ferroastro_analytic::advance::{
    j2_inclined_rate, keplerian_period, lense_thirring_rate, pn_unit, ppn_scaling,
    rate_arcsec_per_century, sr_exact_advance,
};
use ferroastro_analytic::kepler::Elements;
use ferroastro_analytic::vector::{cross, dot, norm};
use ferroastro_constants::De440;
use ferroastro_core::frames::{EclipticJ2000, Frame, Heliocentric, State};
use ferroastro_core::time::{Epoch, J2000_JD, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Angle, AngularRate, GravParam, Length, SpinGm, TimeSpan, Velocity};
use ferroastro_forces::{
    CentralTerm, ForceModel, Gauge, LenseThirring, PnCoefficients, PnLagrangian, Pole,
    SchwarzschildPpn, SolarJ2, SrKepler, TestParticle,
};
use ferroastro_integrators::{Ias15Config, Ias15Stats};
use ferroastro_precession::fit::fit_line;
use ferroastro_precession::park2017::{
    FitModel, LongitudeDefinition, Park2017, PassageConfig, ReferencePlane, SampledOrbit,
};
use ferroastro_precession::passages::{self, OrbitPlane, PerOrbitFit, fit_per_orbit};
use ferroastro_precession::report::{RateReport, RateReportBuilder};

/// The dynamics of a rung (all add to Newtonian `−μr/r³` about a fixed Sun).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Model {
    /// Rung SR-a: `d(γv)/dt = −μr/r³` (Journal #4 R2).
    SrKepler,
    /// Rung SR-b, isotropic/harmonic gauge: velocity-dependent 1PN terms only.
    SrBHarmonic,
    /// Rung SR-b, areal Schwarzschild gauge (pedagogical): velocity-dependent 1PN terms only.
    SrBAreal,
    /// Rungs GR-SCHW and PPN: 1PN Schwarzschild with PPN β, γ (Journal #4 R3).
    Schwarzschild {
        /// PPN β.
        beta: f64,
        /// PPN γ.
        gamma: f64,
    },
    /// PHY-009 coefficient study: the general 1PN Lagrangian.
    Lagrangian(PnCoefficients),
    /// Rung J2 (Journal #4 R1) with this unnormalised J2.
    J2 {
        /// Solar J2.
        j2: f64,
    },
    /// Rung LT (Journal #4 R3b) with the DE440 solar spin.
    LenseThirring,
}

/// Where the pole of the J2 / LT terms points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoleSpec {
    /// The initial orbit normal tilted by this angle towards the initial periapsis direction
    /// (zero: the orbit lies in the equatorial plane, the geometry of the planar oracles).
    OrbitNormalTilted(Angle),
    /// A fixed direction in the run's frame (e.g. `[0, 0, 1]` in the `IAU_SUN` frame).
    Fixed([f64; 3]),
}

/// One rung of the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct Rung {
    /// Short identifier, e.g. `"GR-SCHW"`.
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Requirement(s) the rung serves.
    pub requirements: &'static str,
    /// Dynamics.
    pub model: Model,
    /// Pole of the J2 / LT terms (ignored by the other models).
    pub pole: PoleSpec,
    /// Constants beyond DE440 used by this rung (with citation), or empty.
    pub extra_constants: String,
}

impl Rung {
    fn new(id: &str, title: &str, requirements: &'static str, model: Model) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            requirements,
            model,
            pole: PoleSpec::OrbitNormalTilted(Angle::ZERO),
            extra_constants: String::new(),
        }
    }

    /// A rung with the given dynamics and an equatorial pole (helper for tests and studies).
    #[must_use]
    pub fn custom(id: &str, title: &str, model: Model) -> Self {
        Self::new(id, title, "study", model)
    }

    /// This rung with another pole.
    #[must_use]
    pub fn with_pole(self, pole: PoleSpec) -> Self {
        Self { pole, ..self }
    }
}

/// The catalogue of `ferroastro ladder --rungs single-body` (ROADMAP M2 acceptance).
///
/// # Errors
/// `FA-E0701` if a scenario value is not representable.
pub fn single_body_rungs(scn: &Scenario, de: &De440) -> Result<Vec<Rung>, LadderError> {
    let tilt = Angle::from_radians(scn.park17_tilt_deg.to_radians()).map_err(|_| {
        LadderError::Scenario {
            name: scn.name.clone(),
            problem: "solar-equator tilt".into(),
        }
    })?;
    let park_j2 = format!("J2 = {:e} ({}, Table 1)", scn.park17_j2, scn.park17_source);
    let mut j2_park = Rung::new(
        "J2",
        "J2 (Park17 J2), equatorial orbit",
        "PHY-010, NUM-015",
        Model::J2 { j2: scn.park17_j2 },
    );
    j2_park.extra_constants.clone_from(&park_j2);
    let mut j2_incl = j2_park.clone().with_pole(PoleSpec::OrbitNormalTilted(tilt));
    j2_incl.id = "J2-INCL".into();
    j2_incl.title = format!(
        "J2 (Park17 J2), solar equator tilted {} deg (Park17 Eq. 3)",
        scn.park17_tilt_deg
    );
    j2_incl.extra_constants = format!(
        "{park_j2}; tilt {} deg ({}, Sec. 4)",
        scn.park17_tilt_deg, scn.park17_source
    );
    Ok(vec![
        Rung::new(
            "SR-a",
            "SR-a: SR-Kepler (H = gamma m c^2 - mu m/r)",
            "PHY-007, NUM-015, NUM-016",
            Model::SrKepler,
        ),
        Rung::new(
            "SR-b/harmonic",
            "SR-b: velocity-only 1PN terms, isotropic/harmonic gauge",
            "PHY-008",
            Model::SrBHarmonic,
        ),
        Rung::new(
            "SR-b/areal",
            "SR-b: velocity-only 1PN terms, areal gauge (pedagogical)",
            "PHY-008",
            Model::SrBAreal,
        ),
        Rung::new(
            "GR-SCHW",
            "GR-SCHW numerical: 1PN Schwarzschild, beta = gamma = 1",
            "PHY-005, NUM-015, NUM-017",
            Model::Schwarzschild {
                beta: de.ppn_beta(),
                gamma: de.ppn_gamma(),
            },
        ),
        j2_park,
        Rung::new(
            "J2-DE440",
            "J2 (DE440 J2SUN), equatorial orbit",
            "PHY-010, NUM-015",
            Model::J2 { j2: de.sun_j2() },
        ),
        j2_incl,
        Rung::new(
            "LT",
            "LT: solar Lense-Thirring (DE440 spin), equatorial orbit",
            "PHY-011, NUM-015",
            Model::LenseThirring,
        ),
    ])
}

/// Select a rung set by name (`single-body`).
///
/// # Errors
/// `FA-E0700` for an unknown set.
pub fn rung_set(name: &str, scn: &Scenario, de: &De440) -> Result<Vec<Rung>, LadderError> {
    match name {
        "single-body" => single_body_rungs(scn, de),
        other => Err(LadderError::Selection {
            what: "rung set",
            value: other.into(),
            valid: "single-body",
        }),
    }
}

/// The set-up shared by all single-body rungs: initial state, constants and run settings.
#[derive(Debug, Clone)]
pub struct SingleBody<F: Frame> {
    mu: GravParam,
    c: Velocity,
    gamma: f64,
    radius: Length,
    gs: SpinGm,
    r0: [f64; 3],
    v0: [f64; 3],
    epoch: Epoch<Tdb>,
    frame: String,
    constants: String,
    /// Anomalistic orbits per run (passages − 1).
    pub orbits: u32,
    /// Integrator settings.
    pub ias15: Ias15Config,
    _frame: std::marker::PhantomData<fn() -> F>,
}

/// `G·S` of the Sun from DE440: `GM☉ · MOISUN · ASUN² · OMGSUN`.
///
/// # Errors
/// `FA-E0001` (via `FA-E0701`) if the product is not finite.
pub fn de440_solar_spin(de: &De440) -> Result<SpinGm, LadderError> {
    let r = de.sun_radius().km();
    let gs = de.gm_sun().km3_per_s2()
        * de.sun_moment_of_inertia()
        * r
        * r
        * de.sun_rotation_rate().rad_per_s();
    SpinGm::from_km5_per_s3(gs).map_err(|e| LadderError::Scenario {
        name: "DE440".into(),
        problem: e.to_string(),
    })
}

impl SingleBody<EclipticJ2000> {
    /// Mercury from the scenario's mean elements, `μ = GM☉` (test-particle limit).
    ///
    /// # Errors
    /// Scenario or element-conversion errors.
    pub fn mercury(scn: &Scenario, de: &De440) -> Result<Self, LadderError> {
        let mu = de.gm_sun();
        let el = scn.kepler_elements(de.au())?;
        let state: State<EclipticJ2000, Heliocentric> = el.to_state(mu)?;
        let (r0, v0) = TestParticle::pack(&state);
        let frame = format!(
            "initial osculating orbit plane of the test particle (axes fixed in {}, {}); zero at the initial periapsis",
            EclipticJ2000::NAME,
            scn.elements_frame
        );
        let constants = format!(
            "DE440 (header.440 sha256 {}): GMS, CLIGHT, AU, BETA, GAMMA, ASUN, J2SUN, MOISUN, OMGSUN; initial state: {} converted with mu = GM_sun",
            de.table().source().sha256,
            scn.elements_source
        );
        Self::from_state(r0, v0, de, scn.orbits, frame, constants)
    }
}

impl<F: Frame> SingleBody<F> {
    /// A set-up from an explicit heliocentric state (km, km/s) at J2000 TDB.
    ///
    /// # Errors
    /// `FA-E0600` for a degenerate state; constants errors.
    pub fn from_state(
        r0: [f64; 3],
        v0: [f64; 3],
        de: &De440,
        orbits: u32,
        frame: String,
        constants: String,
    ) -> Result<Self, LadderError> {
        let mu = de.gm_sun();
        // Validates that the orbit is elliptic and non-degenerate.
        Elements::from_vectors(r0, v0, mu.km3_per_s2())?;
        Ok(Self {
            mu,
            c: de.speed_of_light(),
            gamma: de.ppn_gamma(),
            radius: de.sun_radius(),
            gs: de440_solar_spin(de)?,
            r0,
            v0,
            epoch: Epoch::j2000(),
            frame,
            constants,
            orbits,
            ias15: Ias15Config::default(),
            _frame: std::marker::PhantomData,
        })
    }

    /// The initial heliocentric position (km) and velocity (km/s).
    #[must_use]
    pub const fn initial_state(&self) -> ([f64; 3], [f64; 3]) {
        (self.r0, self.v0)
    }

    /// Hash of the initial conditions: FNV-1a (64 bit) over the IEEE-754 bit patterns of the six
    /// components of `r0` (km) and `v0` (km/s), as 16 hex digits. Differential runs (PHY-006,
    /// NUM-013) assert that it is identical for every run they difference.
    #[must_use]
    pub fn ic_hash(&self) -> String {
        const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut h = OFFSET;
        for z in self.r0.iter().chain(&self.v0) {
            for byte in z.to_bits().to_le_bytes() {
                h ^= u64::from(byte);
                h = h.wrapping_mul(PRIME);
            }
        }
        format!("{h:016x}")
    }

    /// The gravitational parameter of every term.
    #[must_use]
    pub const fn mu(&self) -> GravParam {
        self.mu
    }

    /// Osculating elements of the initial state.
    ///
    /// # Errors
    /// Never for a set-up that was built (checked at construction).
    pub fn initial_elements(&self) -> Result<Elements, LadderError> {
        Ok(Elements::from_vectors(
            self.r0,
            self.v0,
            self.mu.km3_per_s2(),
        )?)
    }

    fn pole(&self, spec: PoleSpec) -> Result<Pole<F>, LadderError> {
        let v = match spec {
            PoleSpec::Fixed(v) => v,
            PoleSpec::OrbitNormalTilted(t) => {
                let h = cross(self.r0, self.v0);
                let hn = norm(h);
                let e = ferroastro_analytic::vector::eccentricity_vector(
                    self.r0,
                    self.v0,
                    self.mu.km3_per_s2(),
                );
                let en = norm(e);
                let (s, c) = t.radians().sin_cos();
                [
                    c.mul_add(h[0] / hn, s * e[0] / en),
                    c.mul_add(h[1] / hn, s * e[1] / en),
                    c.mul_add(h[2] / hn, s * e[2] / en),
                ]
            }
        };
        Ok(Pole::from_components(v)?)
    }

    /// The force model of a rung.
    ///
    /// # Errors
    /// Force-model construction errors.
    pub fn model(&self, rung: &Rung) -> Result<TestParticle<F>, LadderError> {
        let (mu, c) = (self.mu, self.c);
        let term: Box<dyn CentralTerm<F>> = match rung.model {
            Model::SrKepler => Box::new(SrKepler::new(mu, c)?),
            Model::SrBHarmonic => Box::new(PnLagrangian::sr_b_harmonic(mu, c)?),
            Model::SrBAreal => Box::new(PnLagrangian::sr_b_areal(mu, c)?),
            Model::Schwarzschild { beta, gamma } => {
                Box::new(SchwarzschildPpn::new(mu, c, beta, gamma)?)
            }
            Model::Lagrangian(k) => Box::new(PnLagrangian::generic(mu, c, k)?),
            Model::J2 { j2 } => Box::new(SolarJ2::new(mu, j2, self.radius, self.pole(rung.pole)?)?),
            Model::LenseThirring => Box::new(LenseThirring::new(
                self.gs,
                c,
                self.gamma,
                self.pole(rung.pole)?,
            )?),
        };
        Ok(TestParticle::new(mu, vec![term])?)
    }

    /// Inclination of the rung's pole to the initial orbit normal (zero for non-J2/LT models).
    ///
    /// # Errors
    /// Pole construction errors.
    pub fn inclination(&self, rung: &Rung) -> Result<Angle, LadderError> {
        let h = cross(self.r0, self.v0);
        let p = self.pole(rung.pole)?.unit();
        let cosi = (dot(h, p) / norm(h)).clamp(-1.0, 1.0);
        Ok(Angle::from_radians(cosi.acos()).unwrap_or(Angle::ZERO))
    }

    /// The analytic per-orbit advance of a rung for this initial state (NUM-015 oracles).
    ///
    /// # Errors
    /// Domain errors of the analytic formulas.
    pub fn analytic_per_orbit(&self, rung: &Rung) -> Result<Angle, LadderError> {
        let el = self.initial_elements()?;
        let p = Length::from_km(el.a.km() * el.e.mul_add(-el.e, 1.0)).map_err(|_| {
            LadderError::Scenario {
                name: rung.id.clone(),
                problem: "semi-latus rectum".into(),
            }
        })?;
        let unit = pn_unit(self.mu, self.c, p)?;
        let period = keplerian_period(self.mu, el.a)?;
        let incl = self.inclination(rung)?;
        let per_period = |r: AngularRate| -> Result<Angle, LadderError> {
            Angle::from_radians(r.rad_per_s() * period.seconds()).map_err(|_| {
                LadderError::Scenario {
                    name: rung.id.clone(),
                    problem: "analytic advance".into(),
                }
            })
        };
        Ok(match rung.model {
            Model::SrKepler => sr_exact_advance(self.mu, self.c, self.r0, self.v0)?,
            Model::SrBHarmonic => {
                unit * PnCoefficients::ppn(1.0, 1.0)
                    .velocity_only()
                    .advance_units()
            }
            Model::SrBAreal => {
                unit * PnCoefficients::schwarzschild_areal()
                    .velocity_only()
                    .advance_units()
            }
            Model::Schwarzschild { beta, gamma } => unit * (6.0 * ppn_scaling(beta, gamma)),
            Model::Lagrangian(k) => unit * k.advance_units(),
            Model::J2 { j2 } => per_period(j2_inclined_rate(
                self.mu,
                j2,
                self.radius,
                el.a,
                el.e,
                incl,
            )?)?,
            Model::LenseThirring => per_period(lense_thirring_rate(
                self.gs, self.c, self.gamma, el.a, el.e, incl,
            )?)?,
        })
    }

    /// The 1PN unit `πμ/(c²p)` of the initial osculating orbit (PHY-007, PHY-009).
    ///
    /// # Errors
    /// Domain errors.
    pub fn pn_unit(&self) -> Result<Angle, LadderError> {
        let el = self.initial_elements()?;
        let p = Length::from_km(el.a.km() * el.e.mul_add(-el.e, 1.0)).map_err(|_| {
            LadderError::Scenario {
                name: "pn unit".into(),
                problem: "semi-latus rectum".into(),
            }
        })?;
        Ok(pn_unit(self.mu, self.c, p)?)
    }

    /// Run a rung.
    ///
    /// # Errors
    /// Force-model, integrator, estimator or report errors.
    pub fn run(&self, rung: &Rung) -> Result<RungResult, LadderError> {
        let started = std::time::Instant::now();
        let model = self.model(rung)?;
        let el = self.initial_elements()?;
        let period = keplerian_period(self.mu, el.a)?;
        let passages_wanted = usize::try_from(self.orbits).unwrap_or(usize::MAX) + 1;
        let max_time = period.seconds() * (f64::from(self.orbits) + 2.0);
        let run = find_perihelia(
            &model,
            self.r0,
            self.v0,
            self.mu.km3_per_s2(),
            passages_wanted,
            max_time,
            self.ias15,
        )?;
        let plane = OrbitPlane::osculating(self.r0, self.v0, self.mu.km3_per_s2())?;
        let fit = fit_per_orbit(&run.passages, &plane)?;
        let lrl = fit_line(&run.lrl_times, &run.lrl_longitudes)?;
        let lrl_rate = AngularRate::from_rad_per_s(lrl.slope).unwrap_or(AngularRate::ZERO);
        let analytic = self.analytic_per_orbit(rung)?;
        let analytic_rate = analytic / period;
        let gauge = model.gauge();
        let cfg = self.ias15;
        let mut constants = self.constants.clone();
        if !rung.extra_constants.is_empty() {
            constants.push_str("; ");
            constants.push_str(&rung.extra_constants);
        }
        let report = RateReportBuilder::new()
            .rung(rung.title.clone())
            .rate(fit.rate())
            .frame(self.frame.clone())
            .epoch(self.epoch)
            .span(fit.span)
            .fit_model(passages::FIT_MODEL)
            .constants(constants)
            .gauge(gauge.label())
            .estimator(passages::ESTIMATOR)
            .dynamics(format!(
                "{}; IAS15 (FerroAstro clean-room) epsilon = {:e}, safety = {}, PC tolerance = {:e}, f64 + compensated summation",
                model.describe(),
                cfg.epsilon,
                cfg.safety,
                cfg.pc_tolerance
            ))
            .build()?;
        Ok(RungResult {
            rung: rung.clone(),
            report,
            fit,
            analytic_per_orbit: analytic,
            analytic_rate,
            kepler_period: period,
            lrl_rate,
            stats: run.stats,
            max_residual: run.max_residual,
            wall_seconds: started.elapsed().as_secs_f64(),
            gauge,
            inclination: self.inclination(rung)?,
            ic_hash: self.ic_hash(),
        })
    }

    /// NUM-012 cross-estimator comparison for one rung: the perihelion-passage rate of
    /// [`SingleBody::run`], its LRL (osculating eccentricity-vector) cross-check, and the
    /// Park2017 estimator (NUM-011) applied to the same dynamics sampled every `cadence`, in the
    /// initial osculating orbit plane (x at the initial periapsis), projected longitude, with a
    /// linear and a quadratic fit and no periodic terms (a single body has none).
    ///
    /// # Errors
    /// Run, sampling and estimator errors.
    pub fn estimator_spread(
        &self,
        rung: &Rung,
        cadence: TimeSpan,
    ) -> Result<EstimatorSpread, LadderError> {
        let base = self.run(rung)?;
        let model = self.model(rung)?;
        let step = cadence.seconds();
        // Cover the passage run's span plus two orbits (passages near the ends need neighbours).
        let span = 2.0f64.mul_add(base.fit.period.seconds(), base.fit.span.seconds());
        let mut grid_points: u32 = 2;
        while f64::from(grid_points - 1) * step < span && grid_points < u32::MAX {
            grid_points += 1;
        }
        let samples = usize::try_from(grid_points).unwrap_or(usize::MAX);
        let (t, r, v) = sample_uniform(&model, self.r0, self.v0, step, samples, self.ias15)?;
        let jd: Vec<f64> = t.iter().map(|s| J2000_JD + s / SECONDS_PER_DAY).collect();
        let orbit = SampledOrbit::<F>::from_raw(&jd, &r, &v)?;
        let h = cross(self.r0, self.v0);
        let e = ferroastro_analytic::vector::eccentricity_vector(
            self.r0,
            self.v0,
            self.mu.km3_per_s2(),
        );
        let plane = ReferencePlane::<F>::from_normal_and_x(
            h,
            e,
            "initial osculating orbit plane, x at the initial periapsis",
        )?;
        let park = |quadratic: bool| -> Result<(f64, f64), LadderError> {
            let est = Park2017 {
                plane: plane.clone(),
                definition: LongitudeDefinition::Projected,
                passages: PassageConfig::default(),
                model: FitModel {
                    quadratic,
                    frequencies: Vec::new(),
                },
                jackknife_blocks: 20,
            };
            let res = est.estimate(&orbit)?;
            Ok((
                res.fit.arcsec_per_century(),
                res.fit.jackknife_sigma_arcsec_per_century(),
            ))
        };
        let (park_linear, park_linear_jk) = park(false)?;
        let (park_quadratic, _) = park(true)?;
        Ok(EstimatorSpread {
            rung: rung.id.clone(),
            ic_hash: self.ic_hash(),
            passages: base.numerical_arcsec_per_century(),
            lrl: base.lrl_arcsec_per_century(),
            park_linear,
            park_linear_jackknife: park_linear_jk,
            park_quadratic,
            analytic: base.analytic_arcsec_per_century(),
            cadence_days: step / SECONDS_PER_DAY,
            samples,
        })
    }
}

/// Rates of one rung from the NUM-012 estimators (all ″/cy).
#[derive(Debug, Clone, PartialEq)]
pub struct EstimatorSpread {
    /// Rung id.
    pub rung: String,
    /// Initial-condition hash shared by the runs.
    pub ic_hash: String,
    /// Perihelion passages: per-orbit advance / anomalistic period.
    pub passages: f64,
    /// LRL (osculating eccentricity-vector) longitude, OLS over every accepted step.
    pub lrl: f64,
    /// Park2017 on uniform samples, linear fit.
    pub park_linear: f64,
    /// Jack-knife σ of [`EstimatorSpread::park_linear`] (20 blocks).
    pub park_linear_jackknife: f64,
    /// Park2017 on uniform samples, quadratic fit (ϖ̇ at J2000.0).
    pub park_quadratic: f64,
    /// Analytic oracle (per-orbit advance / Keplerian period of the initial orbit).
    pub analytic: f64,
    /// Sampling cadence of the Park2017 input (days).
    pub cadence_days: f64,
    /// Number of samples.
    pub samples: usize,
}

impl EstimatorSpread {
    /// Largest minus smallest of the four numerical estimates.
    #[must_use]
    pub fn spread(&self) -> f64 {
        let all = [
            self.passages,
            self.lrl,
            self.park_linear,
            self.park_quadratic,
        ];
        let hi = all.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let lo = all.iter().copied().fold(f64::INFINITY, f64::min);
        hi - lo
    }
}

/// The outcome of one rung run.
#[derive(Debug, Clone)]
pub struct RungResult {
    /// The rung.
    pub rung: Rung,
    /// The reported rate with its metadata (PHY-015).
    pub report: RateReport,
    /// Per-orbit fit from the perihelion passages.
    pub fit: PerOrbitFit,
    /// Analytic per-orbit advance (NUM-015 oracle).
    pub analytic_per_orbit: Angle,
    /// Analytic rate: analytic per-orbit advance / Keplerian period of the initial orbit.
    pub analytic_rate: AngularRate,
    /// Keplerian period of the initial osculating orbit.
    pub kepler_period: TimeSpan,
    /// Cross-check: rate of the LRL eccentricity-vector longitude (all steps, OLS).
    pub lrl_rate: AngularRate,
    /// Integrator counters.
    pub stats: Ias15Stats,
    /// Largest normalised `|r·v|` left at a refined passage.
    pub max_residual: f64,
    /// Wall-clock time of the run (s).
    pub wall_seconds: f64,
    /// Gauge of the dynamics.
    pub gauge: Gauge,
    /// Inclination of the J2/LT pole to the initial orbit normal.
    pub inclination: Angle,
    /// [`SingleBody::ic_hash`] of the run's initial conditions.
    pub ic_hash: String,
}

impl RungResult {
    /// Numerical rate in ″/cy (per-orbit advance / anomalistic period).
    #[must_use]
    pub fn numerical_arcsec_per_century(&self) -> f64 {
        self.report.arcsec_per_century()
    }

    /// Analytic rate in ″/cy (analytic per-orbit advance / Keplerian period).
    #[must_use]
    pub fn analytic_arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.analytic_rate)
    }

    /// LRL cross-check rate in ″/cy.
    #[must_use]
    pub fn lrl_arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.lrl_rate)
    }

    /// `numerical − analytic` per orbit (rad).
    #[must_use]
    pub fn per_orbit_difference(&self) -> f64 {
        self.fit.advance.radians() - self.analytic_per_orbit.radians()
    }

    /// `(numerical − analytic)/analytic` per orbit.
    #[must_use]
    pub fn per_orbit_relative_deviation(&self) -> f64 {
        self.per_orbit_difference() / self.analytic_per_orbit.radians()
    }
}
