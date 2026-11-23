// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The opt-in, **inferred** time-varying solar gravitational parameter (issue #42, OI-04;
//! maintainer decision of 2026-11-23, decision record issue #70). Off by default: a model
//! contains it only if the caller adds [`SolarGmVariation`] to its terms.
//!
//! The convention, what it rests on and which force terms use it are documented on
//! [`SolarGmRate`] and [`SolarGmVariation`].

use super::layout::{Accelerations, Layout, View};
use super::terms::{ForceTerm, TermParameter, major_sun, r2_of, singular, sub};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, Epoch, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::GravParam;
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// Seconds per Julian year (365.25 d of 86 400 s), the time unit of the inferred rate.
const SECONDS_PER_JULIAN_YEAR: f64 = SECONDS_PER_DAY * DAYS_PER_JULIAN_YEAR;

/// The **inferred** linear law of the Sun's gravitational parameter (OI-04, issue #42):
/// `GM☉(t) = GM☉(t0) · [1 + k (t − t0)/(1 Julian yr)]`, with the dimensionless rate `k` per
/// Julian year and the reference epoch `t0` (TDB) both explicit. No public JPL document states
/// the formula, the unit or the epoch. The type carries no default: a model varies GM☉ only if
/// a [`SolarGmVariation`] built from one of these is added to it.
///
/// # The convention is inferred, not documented
/// ```text
/// GM☉(t) = GM☉(t0) · [1 + k · (t − t0) / (1 Julian year)],     t, t0 in TDB,
/// ```
/// with `k` a dimensionless rate per Julian year (365.25 d of 86 400 s) and `t0` the reference
/// epoch at which the tabulated GM☉ holds. For DE440 the inferred reading is `k` = the header's
/// `GMSDOT` and `t0` = J2000.0 TDB (JD 2451545.0). **No public JPL document states this formula,
/// the unit of `GMSDOT` or its reference epoch.** The reading is inferred from:
///
/// 1. FerroAstro's own fit against `de440.bsp` (issue #42 investigation, 2026-11-22): with the
///    term, Mercury's 10-yr heliocentric residual of the 353-body model falls from 86 m to under
///    1 m; a fit of rate and reference epoch gives the header value to about 0.3 % and J2000 to
///    about ±0.1 yr, while the reference epoch JDEPOC makes the residual worse;
/// 2. JPL IOM 335N-20-01 (Williams & Boggs 2020, the lunar-laser-ranging model), which lists the
///    solution parameter "Rate of change of solar mass, (dMS/dt)/MS, from J2000" for JPL's
///    software, without a time unit and without saying that it is how DE440 was integrated;
/// 3. the non-zero `GMSDOT` in the comment block "Initial conditions and constants used for
///    integration" of the DE440 files, listed without unit, formula or epoch.
///
/// The DE440 paper (Park et al. 2021) does not mention a time-varying GM☉. Treat every result
/// obtained with this term as resting on an inferred convention (OI-04).
///
/// # Linear, not exponential
/// `GM☉(t0)·exp(k (t − t0))` differs from the linear law by `(k Δt)²/2`, about 2e-21 relative
/// over the DE440 span (|Δt| ≤ 650 yr at the DE440 rate): the two cannot be told apart, and the
/// linear law is used. It is meaningful only while `|k (t − t0)| ≪ 1`.
///
/// # Example
/// ```
/// use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, Epoch, Tdb};
/// use ferroastro_forces::SolarGmRate;
/// // A synthetic rate (not the DE440 value, which is read from header.440 at run time).
/// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
/// assert_eq!(law.reference_epoch(), Epoch::<Tdb>::j2000());
/// let (day, frac) = Epoch::<Tdb>::j2000().jd_parts();
/// let two_years_later = Epoch::<Tdb>::from_jd_parts(day + 2.0 * DAYS_PER_JULIAN_YEAR, frac)?;
/// assert_eq!(law.fractional_offset_at(two_years_later), -2.0e-13);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarGmRate {
    fraction_per_julian_year: f64,
    reference_epoch: Epoch<Tdb>,
}

impl SolarGmRate {
    /// The law with rate `fraction_per_julian_year` (dimensionless, per Julian year of
    /// 365.25 × 86 400 s) and the TDB `reference_epoch` at which the tabulated GM☉ holds. Both
    /// are the caller's statement: the unit and the epoch of DE440's `GMSDOT` are inferred, not
    /// documented (OI-04).
    ///
    /// # Errors
    /// `FA-E0500` if the rate is not finite.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::new(-1.0e-13, Epoch::<Tdb>::j2000())?;
    /// assert_eq!(law.fraction_per_julian_year(), -1.0e-13);
    /// let err = SolarGmRate::new(f64::NAN, Epoch::<Tdb>::j2000()).unwrap_err();
    /// assert!(err.to_string().starts_with("FA-E0500"));
    /// assert!(err.to_string().contains("inferred"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(
        fraction_per_julian_year: f64,
        reference_epoch: Epoch<Tdb>,
    ) -> Result<Self, ForceError> {
        if !fraction_per_julian_year.is_finite() {
            return Err(ForceError::Invalid {
                name: "solar GM fractional rate (1/Julian yr; inferred convention, OI-04)",
                value: fraction_per_julian_year,
                valid: "finite (for DE440: the header's GMSDOT under the inferred reading)",
            });
        }
        Ok(Self {
            fraction_per_julian_year,
            reference_epoch,
        })
    }

    /// The law with the **inferred** DE440 reference epoch J2000.0 TDB (JD 2451545.0): pass the
    /// header's `GMSDOT` as `fraction_per_julian_year`
    /// (`ferroastro_propagate::de440_inferred_solar_gm_rate` does both from the constants).
    /// The epoch comes from FerroAstro's fit against `de440.bsp` (to about ±0.1 yr) and from the
    /// "(dMS/dt)/MS, from J2000" wording of JPL IOM 335N-20-01; no JPL document states it for
    /// DE440.
    ///
    /// # Errors
    /// As [`SolarGmRate::new`].
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
    /// assert_eq!(law, SolarGmRate::new(-1.0e-13, Epoch::<Tdb>::j2000())?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn inferred_j2000(fraction_per_julian_year: f64) -> Result<Self, ForceError> {
        Self::new(fraction_per_julian_year, Epoch::<Tdb>::j2000())
    }

    /// The rate `k`: dimensionless, per Julian year.
    ///
    /// ```
    /// use ferroastro_forces::SolarGmRate;
    /// assert_eq!(SolarGmRate::inferred_j2000(-1.0e-13)?.fraction_per_julian_year(), -1.0e-13);
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub const fn fraction_per_julian_year(&self) -> f64 {
        self.fraction_per_julian_year
    }

    /// The reference epoch `t0` (TDB) at which the tabulated GM☉ holds.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::inferred_j2000(0.0)?;
    /// assert_eq!(law.reference_epoch(), Epoch::<Tdb>::j2000());
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub const fn reference_epoch(&self) -> Epoch<Tdb> {
        self.reference_epoch
    }

    /// `(epoch − t0)` in Julian years, in backend `R`.
    ///
    /// ```
    /// use ferroastro_core::time::{DAYS_PER_JULIAN_YEAR, Epoch, Tdb};
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::inferred_j2000(0.0)?;
    /// let (day, frac) = Epoch::<Tdb>::j2000().jd_parts();
    /// let earlier = Epoch::<Tdb>::from_jd_parts(day - DAYS_PER_JULIAN_YEAR, frac)?;
    /// assert_eq!(law.julian_years_since_reference(earlier), -1.0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn julian_years_since_reference<R: Real>(&self, epoch: Epoch<Tdb, R>) -> R {
        (epoch - self.reference_epoch.cast::<R>()).seconds() / R::from_f64(SECONDS_PER_JULIAN_YEAR)
    }

    /// `GM☉(epoch)/GM☉(t0) − 1 = k (epoch − t0)/(1 Julian yr)` under the inferred linear law.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
    /// assert_eq!(law.fractional_offset_at(Epoch::<Tdb>::j2000()), 0.0);
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub fn fractional_offset_at(&self, epoch: Epoch<Tdb>) -> f64 {
        self.fraction_per_julian_year * self.julian_years_since_reference(epoch)
    }

    /// `GM☉(epoch) = gm_at_reference · [1 + k (epoch − t0)/(1 Julian yr)]` under the inferred
    /// linear law (for reports; the force term adds `GM☉(t) − GM☉(t0)` separately, see
    /// [`SolarGmVariation`]).
    ///
    /// # Errors
    /// `FA-E0500` if the result is not finite and positive (the linear law is meaningful only
    /// for `|k (epoch − t0)| ≪ 1`).
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_core::units::GravParam;
    /// use ferroastro_forces::SolarGmRate;
    /// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
    /// let gm = GravParam::from_km3_per_s2(1.0)?;
    /// assert_eq!(law.gm_at(gm, Epoch::<Tdb>::j2000())?, gm);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn gm_at(
        &self,
        gm_at_reference: GravParam,
        epoch: Epoch<Tdb>,
    ) -> Result<GravParam, ForceError> {
        let gm0 = gm_at_reference.km3_per_s2();
        let gm = gm0.mul_add(self.fractional_offset_at(epoch), gm0);
        let invalid = ForceError::Invalid {
            name: "GM of the Sun at the requested epoch (km^3/s^2; inferred linear law, OI-04)",
            value: gm,
            valid: "finite and > 0 (the linear law holds only for |rate x (t - t0)| << 1)",
        };
        if !(gm.is_finite() && gm > 0.0) {
            return Err(invalid);
        }
        GravParam::from_km3_per_s2(gm).map_err(|_| invalid)
    }
}

/// The Newtonian effect of the **inferred** time-varying GM☉ ([`SolarGmRate`]; OI-04, issue
/// #42), as a separate term: every moving body `i` other than the Sun, Major and Minor, receives
///
/// ```text
/// aᵢ = −δGM(t) (rᵢ − r☉)/|rᵢ − r☉|³,     δGM(t) = GM☉ · k · (t − t0)/(1 Julian yr),
/// ```
///
/// where GM☉ is the layout's (constant) solar GM, taken as the value at `t0`. Together with the
/// constant-GM Newtonian terms this is the Sun's point-mass pull with `GM☉(t)`. The Sun itself
/// receives nothing (its acceleration depends on the other bodies' GMs), Ring bodies are fixed,
/// and the EIH, J2 and Lense–Thirring terms keep the constant GM☉ (the difference is below
/// 1e-5 m on Mercury over 10 yr; see below). A paired Earth and Moon
/// receive it through [`Accelerations::add`], so the Sun's differential pull on Moon − Earth is
/// scaled as well.
///
/// **Opt-in and inferred.** No scenario switch adds this term; a model contains it only if the
/// caller pushes it. The convention (rate per Julian year, reference epoch) is not documented by
/// JPL for DE440; see [`SolarGmRate`].
///
/// **Time origin.** [`ForceTerm::accumulate`] receives `t` in TDB seconds from the model epoch,
/// so the term must know that epoch: pass the epoch the propagation starts from as
/// `model_epoch`. [`crate::NBodyPropagator::new`] refuses (`FA-E0500`) a start epoch that
/// differs from it. A driver that advances the model by another route must start its time
/// variable at `model_epoch` itself.
///
/// # Which force terms see GM☉(t)
/// Only the Sun's **Newtonian point-mass pull** on the moving bodies ([`SolarGmVariation`] adds
/// `−δGM(t) (rᵢ − r☉)/|rᵢ − r☉|³`, `δGM(t) = GM☉(t) − GM☉(t0)`, to every Major and Minor body
/// other than the Sun). This is what reproduced `de440.bsp` in the issue #42 investigation. The
/// other places where the Sun's GM enters keep the constant layout value:
///
/// * the Sun's own acceleration: it depends on the other bodies' GMs, not on GM☉;
/// * the EIH 1PN terms, solar J2 and solar Lense–Thirring: scaling them by `1 + k Δt`
///   (|k Δt| ≈ 2.7e-12 at JDEPOC for the DE440 rate) changes Mercury's position over 10 yr by
///   about 3e-6 m (1PN: an along-track effect of order `a n t · 3GM/(ac²)` ≈ 1e6 m, times
///   2.7e-12), 4e-9 m (J2: 144 m/yr) and 2e-10 m (LT: 61 m per 10 yr); these are estimates, all
///   more than three orders below 1 cm, so the choice cannot be measured against `de440.bsp`,
///   and whether DE440 varies GM☉ inside those terms is unknown;
/// * the relativistic barycentre (`μ*`), the reaction ratios `GMᵢ/GM☉` of J2, Lense–Thirring and
///   the comparison models, and the Earth–Moon split: unchanged for the same reason.
///
/// Adding a separate small term, rather than changing the Sun's GM inside the Newtonian sums,
/// keeps the default model bit for bit unchanged and keeps the effect's precision: `δGM/GM☉` is
/// of order 1e-12, so inside the main sum only about four of its digits would survive in
/// binary64, while as its own term it is carried to full precision.
///
/// # Diagnostics that assume constant masses
/// With this term the model is explicitly time dependent:
///
/// * [`crate::NBodyModel::energy_over_g`] returns `None` (the term has no time-independent
///   potential; the energy with the instantaneous GM☉(t) is not conserved,
///   `dE/dt = −Σᵢ μᵢ · dGM☉/dt / |rᵢ − r☉|` per unit G);
/// * [`crate::NBodyModel::momentum_over_g`] still sums `μᵢ vᵢ` with the constant layout GMs. The
///   Sun's pull on the planets is scaled and the planets' pull on the Sun is not, so that sum
///   drifts at the relative rate `k (t − t0)` of the Sun–planet momentum exchange; no conserved
///   momentum is defined for a body that loses mass isotropically without an equation for the
///   lost mass;
/// * barycentre conventions built from constant GMs (the header's SSB, re-barycentring) are not
///   changed. Heliocentric residuals do not depend on them.
///
/// # Example
/// ```
/// use ferroastro_core::frames::Icrf;
/// use ferroastro_core::time::{Epoch, Tdb};
/// use ferroastro_forces::{ForceTerm, SolarGmRate, SolarGmVariation};
/// // A synthetic rate; the DE440 value is read from header.440 at run time.
/// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
/// let term = SolarGmVariation::<f64>::new(law, Epoch::<Tdb>::j2000());
/// assert_eq!(ForceTerm::<Icrf>::name(&term), "solar-gm-rate-inferred");
/// assert_eq!(term.law(), law);
/// # Ok::<(), ferroastro_forces::ForceError>(())
/// ```
#[derive(Debug, Clone)]
pub struct SolarGmVariation<R: Real = f64> {
    law: SolarGmRate,
    model_epoch: Epoch<Tdb, R>,
    /// The rate `k` in backend `R`.
    rate: R,
    /// `(model epoch − t0)` in Julian years.
    offset_years: R,
    sun: usize,
    sun_gm: R,
    bodies: Vec<usize>,
}

impl<R: Real> SolarGmVariation<R> {
    /// The term for `law`, for a model whose time variable is TDB seconds from `model_epoch`
    /// (the epoch of the initial conditions).
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::{SolarGmRate, SolarGmVariation};
    /// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
    /// let term = SolarGmVariation::<f64>::new(law, Epoch::<Tdb>::j2000());
    /// assert_eq!(term.model_epoch(), Epoch::<Tdb>::j2000());
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub fn new(law: SolarGmRate, model_epoch: Epoch<Tdb, R>) -> Self {
        Self {
            law,
            model_epoch,
            rate: R::from_f64(law.fraction_per_julian_year()),
            offset_years: law.julian_years_since_reference(model_epoch),
            sun: 0,
            sun_gm: R::ZERO,
            bodies: Vec::new(),
        }
    }

    /// The inferred law this term applies.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::{SolarGmRate, SolarGmVariation};
    /// let law = SolarGmRate::inferred_j2000(0.0)?;
    /// assert_eq!(SolarGmVariation::<f64>::new(law, Epoch::<Tdb>::j2000()).law(), law);
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub const fn law(&self) -> SolarGmRate {
        self.law
    }

    /// The epoch at which the model's time variable is zero.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::{SolarGmRate, SolarGmVariation};
    /// let law = SolarGmRate::inferred_j2000(0.0)?;
    /// let term = SolarGmVariation::<f64>::new(law, Epoch::<Tdb>::j2000());
    /// assert_eq!(term.model_epoch(), Epoch::<Tdb>::j2000());
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub const fn model_epoch(&self) -> Epoch<Tdb, R> {
        self.model_epoch
    }

    /// `δGM(t)/GM☉ = k · (t/(1 Julian yr) + (model epoch − t0)/(1 Julian yr))` at `t` TDB
    /// seconds from the model epoch.
    ///
    /// ```
    /// use ferroastro_core::time::{Epoch, Tdb};
    /// use ferroastro_forces::{SolarGmRate, SolarGmVariation};
    /// let law = SolarGmRate::inferred_j2000(-1.0e-13)?;
    /// let term = SolarGmVariation::<f64>::new(law, Epoch::<Tdb>::j2000());
    /// assert_eq!(term.fractional_offset(0.0), 0.0);
    /// # Ok::<(), ferroastro_forces::ForceError>(())
    /// ```
    #[must_use]
    pub fn fractional_offset(&self, t: R) -> R {
        self.rate * (t / R::from_f64(SECONDS_PER_JULIAN_YEAR) + self.offset_years)
    }
}

impl<F: Frame, R: Real> ForceTerm<F, R> for SolarGmVariation<R> {
    fn name(&self) -> &'static str {
        "solar-gm-rate-inferred"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        let (rd, rf) = self.law.reference_epoch().jd_parts();
        let (md, mf) = self.model_epoch.jd_parts();
        vec![
            TermParameter {
                name: "GM_sun fractional rate (inferred convention, OI-04)",
                value: self.law.fraction_per_julian_year(),
                unit: "1/Julian yr",
            },
            TermParameter {
                name: "reference epoch t0 (inferred convention, OI-04)",
                value: rd + rf,
                unit: "JD TDB",
            },
            TermParameter {
                name: "model epoch (t = 0)",
                value: (md + mf).to_f64(),
                unit: "JD TDB",
            },
        ]
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major, Tier::Minor]
    }
    fn time_origin(&self) -> Option<Epoch<Tdb, R>> {
        Some(self.model_epoch)
    }
    fn bind(&mut self, layout: &Layout<R>) -> Result<(), ForceError> {
        let (sun, gm) = major_sun(
            layout,
            "the inferred solar GM rate term needs the Sun (NAIF 10) as a Major body",
        )?;
        self.sun = sun;
        self.sun_gm = gm;
        // Every moving body other than the Sun, Major and Minor (Minor bodies feel the Majors'
        // Newtonian pull, NUM-009); Ring bodies are fixed.
        self.bodies = (0..layout.len())
            .filter(|&i| i != sun && layout.moves(i))
            .collect();
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let dgm = self.sun_gm * self.fractional_offset(t);
        let ps = view.position(self.sun)?;
        for &i in &self.bodies {
            let d = sub(view.position(i)?, ps);
            let r2 = r2_of(d);
            if r2 == R::ZERO {
                return Err(singular(self.sun, i, t));
            }
            let k = -dgm / (r2 * r2.sqrt());
            acc.add(i, d.map(|x| k * x));
        }
        Ok(())
    }
    /// `None`: the term depends explicitly on time, so the model has no conserved energy and
    /// [`crate::NBodyModel::energy_over_g`] is unavailable with it.
    fn potential_over_g(&self, _view: &View<'_, R>) -> Option<R> {
        None
    }
}
