// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! N-body force terms behind [`ForceTerm`] (NUM-009, NUM-008, NUM-019).
//!
//! Every term declares its gauge, its parameters and the tiers it acts on, resolves its body
//! indices once ([`ForceTerm::bind`]) and adds accelerations in a fixed order. All terms here are
//! Newtonian. Clean-room (DEP-008): the equations are the textbook point-mass sums, the DE440 J2
//! description (Park et al. 2021, NUM-019) and the classical third-body expansion derived in
//! [`GeocentricMoon`]; no REBOUND/REBOUNDx/ASSIST/Swiftest/Mercury6 source was consulted.

use super::layout::{Accelerations, Layout, View};
use crate::central::{Pole, dot};
use crate::oblateness::{j2_field, j2_potential};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_ephem::NaifId;
use ferroastro_real::Real;
use ferroastro_system::Tier;
use std::fmt;

/// One declared parameter of a term (for reports and scenario records).
#[derive(Debug, Clone, PartialEq)]
pub struct TermParameter {
    /// Name.
    pub name: &'static str,
    /// Value (f64 for reporting; the term keeps its own backend value).
    pub value: f64,
    /// Unit (`"1"` for dimensionless).
    pub unit: &'static str,
}

/// A force term of an [`crate::NBodyModel`], in frame `F` and backend `R`.
pub trait ForceTerm<F: Frame, R: Real = f64>: fmt::Debug + Send + Sync {
    /// Stable name, e.g. `"newtonian-mutual"`.
    fn name(&self) -> &'static str;
    /// The term's gauge (AGENTS.md rule 4).
    fn gauge(&self) -> Gauge;
    /// The term's parameters.
    fn parameters(&self) -> Vec<TermParameter>;
    /// The tiers of the bodies whose motion this term changes.
    fn tiers(&self) -> &'static [Tier];
    /// True only for the term that supplies the geocentric Moon's acceleration
    /// ([`GeocentricMoon`]); a model with a geocentric pair needs exactly one.
    fn supplies_moon_relative(&self) -> bool {
        false
    }
    /// Resolve body indices against the model's layout (called once, when the model is built).
    ///
    /// # Errors
    /// `FA-E0500` if a body the term needs is missing.
    fn bind(&mut self, layout: &Layout<R>) -> Result<(), ForceError>;
    /// Add this term's accelerations at time `t` (TDB seconds from the model epoch).
    ///
    /// # Errors
    /// `FA-E0501` for a singular configuration.
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError>;
    /// The term's potential energy divided by G (km⁵/s⁴), if it is conservative; `None` if not
    /// (the model energy is then unavailable).
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R>;
}

/// `p_j − p_i`.
#[inline]
fn sub<R: Real>(a: [R; 3], b: [R; 3]) -> [R; 3] {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [a0 - b0, a1 - b1, a2 - b2]
}

/// `|d|²` with the M0 kernel's operation order.
#[inline]
fn r2_of<R: Real>(d: [R; 3]) -> R {
    let [d0, d1, d2] = d;
    d0.mul_add(d0, d1.mul_add(d1, d2 * d2))
}

/// `1/r³` from `r²` (one square root, one product, one division: ≤ 2 ulp in f64).
#[inline]
fn inv_r3<R: Real>(r2: R) -> R {
    R::ONE / (r2 * r2.sqrt())
}

/// `[gm · d_k · s]` in the M0 kernel's order `(gm · d_k) · s`.
#[inline]
fn scaled<R: Real>(gm: R, d: [R; 3], s: R) -> [R; 3] {
    d.map(|dk| gm * dk * s)
}

fn singular<R: Real>(i: usize, j: usize, t: R) -> ForceError {
    ForceError::Singular {
        i,
        j,
        t: t.to_f64(),
    }
}

/// Newtonian mutual attraction of the Major bodies: pairs `(i, j)`, `i < j` in storage order,
/// each visited once and applied with opposite signs (momentum is conserved to round-off).
/// With a geocentric Moon pair, the Earth–Moon pair is skipped and the Earth's and Moon's
/// third-body terms reach the EMB only; [`GeocentricMoon`] supplies Moon − Earth.
#[derive(Debug, Clone, Copy, Default)]
pub struct NewtonianMutual;

impl<F: Frame, R: Real> ForceTerm<F, R> for NewtonianMutual {
    fn name(&self) -> &'static str {
        "newtonian-mutual"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        Vec::new()
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major]
    }
    fn bind(&mut self, _layout: &Layout<R>) -> Result<(), ForceError> {
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let layout = view.layout();
        let pair = layout.moon_pair().map(|p| (p.earth, p.moon));
        let is_pair = |i: usize| pair.is_some_and(|(e, m)| i == e || i == m);
        let major = layout.indices(Tier::Major);
        for (k, &i) in major.iter().enumerate() {
            let (pi, gi) = (view.position(i)?, layout.gm(i));
            for &j in major.iter().skip(k + 1) {
                if is_pair(i) && is_pair(j) {
                    continue;
                }
                let d = sub(view.position(j)?, pi);
                let r2 = r2_of(d);
                if r2 == R::ZERO {
                    return Err(singular(i, j, t));
                }
                let s = inv_r3(r2);
                acc.add_emb_only(i, scaled(layout.gm(j), d, s));
                acc.add_emb_only(j, scaled(-gi, d, s));
            }
        }
        Ok(())
    }
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R> {
        let layout = view.layout();
        let major = layout.indices(Tier::Major);
        let mut u = R::ZERO;
        for (k, &i) in major.iter().enumerate() {
            for &j in major.iter().skip(k + 1) {
                let d = sub(view.position(j).ok()?, view.position(i).ok()?);
                u -= layout.gm(i) * layout.gm(j) / r2_of(d).sqrt();
            }
        }
        Some(u)
    }
}

/// Whether the Minor bodies pull on the Majors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinorReaction {
    /// Minor bodies attract the Majors (DE practice for the asteroid masses; default here).
    On,
    /// Minor bodies are test particles (no reaction; momentum is then not conserved).
    Off,
}

/// Minor bodies attracted by the Majors (NUM-009); Minor bodies do not interact with each other.
/// The reaction on the Majors is [`MinorReaction::On`] by default: NUM-009's text does not say,
/// and the N-AST ladder row is the asteroids' pull on Mercury (IMPLEMENTATION-NOTES WP11 review,
/// NEEDS HUMAN REVIEW).
#[derive(Debug, Clone, Copy)]
pub struct MinorFromMajors {
    reaction: MinorReaction,
}

impl MinorFromMajors {
    /// The term with the given reaction setting.
    #[must_use]
    pub const fn new(reaction: MinorReaction) -> Self {
        Self { reaction }
    }
}

impl Default for MinorFromMajors {
    fn default() -> Self {
        Self::new(MinorReaction::On)
    }
}

impl<F: Frame, R: Real> ForceTerm<F, R> for MinorFromMajors {
    fn name(&self) -> &'static str {
        "minor-from-majors"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        vec![TermParameter {
            name: "reaction on Majors",
            value: if self.reaction == MinorReaction::On {
                1.0
            } else {
                0.0
            },
            unit: "1",
        }]
    }
    fn tiers(&self) -> &'static [Tier] {
        match self.reaction {
            MinorReaction::On => &[Tier::Minor, Tier::Major],
            MinorReaction::Off => &[Tier::Minor],
        }
    }
    fn bind(&mut self, _layout: &Layout<R>) -> Result<(), ForceError> {
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let layout = view.layout();
        for &i in layout.indices(Tier::Minor) {
            let (pi, gi) = (view.position(i)?, layout.gm(i));
            for &j in layout.indices(Tier::Major) {
                let d = sub(view.position(j)?, pi);
                let r2 = r2_of(d);
                if r2 == R::ZERO {
                    return Err(singular(i, j, t));
                }
                let s = inv_r3(r2);
                acc.add(i, scaled(layout.gm(j), d, s));
                if self.reaction == MinorReaction::On {
                    acc.add(j, scaled(-gi, d, s));
                }
            }
        }
        Ok(())
    }
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R> {
        if self.reaction == MinorReaction::Off {
            return None;
        }
        let layout = view.layout();
        let mut u = R::ZERO;
        for &i in layout.indices(Tier::Minor) {
            for &j in layout.indices(Tier::Major) {
                let d = sub(view.position(j).ok()?, view.position(i).ok()?);
                u -= layout.gm(i) * layout.gm(j) / r2_of(d).sqrt();
            }
        }
        Some(u)
    }
}

/// Ring bodies as fixed point masses: they attract every Major and Minor body and never move
/// (their acceleration is held at zero; give them zero velocity). An external fixed field: the
/// total momentum of the moving bodies is not conserved, the energy is.
#[derive(Debug, Clone, Copy, Default)]
pub struct RingMasses;

impl<F: Frame, R: Real> ForceTerm<F, R> for RingMasses {
    fn name(&self) -> &'static str {
        "ring-fixed-masses"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        Vec::new()
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major, Tier::Minor]
    }
    fn bind(&mut self, _layout: &Layout<R>) -> Result<(), ForceError> {
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let layout = view.layout();
        for &k in layout.indices(Tier::Ring) {
            let (pk, gk) = (view.position(k)?, layout.gm(k));
            for i in (0..layout.len()).filter(|&i| layout.moves(i)) {
                let d = sub(pk, view.position(i)?);
                let r2 = r2_of(d);
                if r2 == R::ZERO {
                    return Err(singular(i, k, t));
                }
                acc.add(i, scaled(gk, d, inv_r3(r2)));
            }
        }
        Ok(())
    }
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R> {
        let layout = view.layout();
        let mut u = R::ZERO;
        for &k in layout.indices(Tier::Ring) {
            for i in (0..layout.len()).filter(|&i| layout.moves(i)) {
                let d = sub(view.position(k).ok()?, view.position(i).ok()?);
                u -= layout.gm(i) * layout.gm(k) / r2_of(d).sqrt();
            }
        }
        Some(u)
    }
}

/// Default number of Taylor terms of [`GeocentricMoon`] (truncation `|c₁₃ q¹³| < 1e-21` for
/// `|q| ≤ 0.02`, Venus at its closest to the Earth–Moon pair).
pub const MOON_TAYLOR_TERMS_DEFAULT: u32 = 12;
/// NUM-008's minimum number of Taylor terms.
pub const MOON_TAYLOR_TERMS_MIN: u32 = 4;
/// Above this `|q|` the closed form replaces the series (never reached by the planets).
pub const MOON_TAYLOR_Q_MAX: f64 = 0.05;

/// The geocentric Moon's Newtonian acceleration (NUM-008):
///
/// `ρ̈ = −(μ_E + μ_M) ρ/ρ³ + Σ_j μ_j [(d_j − ρ)/|d_j − ρ|³ − d_j/d_j³]`, `ρ = r_M − r_E`,
/// `d_j = r_j − r_E`, over the Major bodies `j` other than the Earth and the Moon.
///
/// The bracket loses ~|d|/|ρ| bits when evaluated directly. With `q = ρ·(ρ − 2d)/d²`,
/// `|d − ρ|² = d²(1 + q)` and the bracket is exactly `[d F(q) − ρ (1 + F(q))]/d³` with
/// `F(q) = (1 + q)^{−3/2} − 1 = Σ_{k≥1} c_k q^k`, `c_k = −c_{k−1}(2k+1)/(2k)`, `c₀ = 1`
/// (binomial series; derived here). `F` is summed with Horner's rule over `terms` terms
/// (NUM-008: at least 4); for `|q| >` [`MOON_TAYLOR_Q_MAX`] the closed form
/// `F = −(s − 1)(s² + s + 1)/s³`, `s − 1 = q/(s + 1)`, `s = √(1+q)` is used instead.
#[derive(Debug, Clone)]
pub struct GeocentricMoon<R: Real = f64> {
    coeffs: Vec<R>,
    third: Vec<usize>,
    earth_moon_gm: R,
}

impl<R: Real> GeocentricMoon<R> {
    /// The term with `terms` Taylor terms.
    ///
    /// # Errors
    /// `FA-E0500` if `terms` < [`MOON_TAYLOR_TERMS_MIN`] or > 40.
    pub fn new(terms: u32) -> Result<Self, ForceError> {
        if !(MOON_TAYLOR_TERMS_MIN..=40).contains(&terms) {
            return Err(ForceError::Invalid {
                name: "geocentric Moon Taylor terms",
                value: f64::from(terms),
                valid: "4 ..= 40 (NUM-008: at least 4)",
            });
        }
        let mut coeffs = Vec::with_capacity(terms as usize);
        let mut c = R::ONE;
        for k in 1..=terms {
            let k2 = R::from_f64(f64::from(2 * k));
            c = -c * (k2 + R::ONE) / k2;
            coeffs.push(c);
        }
        Ok(Self {
            coeffs,
            third: Vec::new(),
            earth_moon_gm: R::ZERO,
        })
    }

    /// Number of Taylor terms.
    #[must_use]
    pub fn terms(&self) -> usize {
        self.coeffs.len()
    }

    /// `F(q) = (1+q)^{−3/2} − 1` as this term evaluates it (series or closed form).
    #[must_use]
    pub fn f_of_q(&self, q: R) -> R {
        if q.abs().to_f64() > MOON_TAYLOR_Q_MAX {
            let s = (R::ONE + q).sqrt();
            let sm1 = q / (s + R::ONE);
            return -(sm1 * (s * s + s + R::ONE)) / (s * s * s);
        }
        let mut f = R::ZERO;
        for &c in self.coeffs.iter().rev() {
            f = (f + c) * q;
        }
        f
    }

    /// `(d − ρ)/|d − ρ|³ − d/d³` by the expansion (km⁻²).
    #[must_use]
    pub fn differential(&self, d: [R; 3], rho: [R; 3]) -> [R; 3] {
        let d2 = dot(d, d);
        let q = (dot(rho, rho) - R::TWO * dot(rho, d)) / d2;
        let f = self.f_of_q(q);
        let inv_d3 = inv_r3(d2);
        let [d0, d1, d2c] = d;
        let [p0, p1, p2] = rho;
        let one_f = R::ONE + f;
        [
            d0.mul_add(f, -(p0 * one_f)) * inv_d3,
            d1.mul_add(f, -(p1 * one_f)) * inv_d3,
            d2c.mul_add(f, -(p2 * one_f)) * inv_d3,
        ]
    }
}

impl<F: Frame, R: Real> ForceTerm<F, R> for GeocentricMoon<R> {
    fn name(&self) -> &'static str {
        "geocentric-moon-taylor"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        vec![
            TermParameter {
                name: "Taylor terms",
                value: f64::from(u32::try_from(self.coeffs.len()).unwrap_or(u32::MAX)),
                unit: "1",
            },
            TermParameter {
                name: "closed form above |q|",
                value: MOON_TAYLOR_Q_MAX,
                unit: "1",
            },
        ]
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major]
    }
    fn supplies_moon_relative(&self) -> bool {
        true
    }
    fn bind(&mut self, layout: &Layout<R>) -> Result<(), ForceError> {
        let p = layout.moon_pair().ok_or(ForceError::Invalid {
            name: "geocentric Moon term without a geocentric Earth-Moon pair",
            value: 0.0,
            valid: "use MoonVariables::Geocentric with this term",
        })?;
        self.earth_moon_gm = layout.gm(p.earth) + layout.gm(p.moon);
        self.third = layout
            .indices(Tier::Major)
            .iter()
            .copied()
            .filter(|&j| j != p.earth && j != p.moon)
            .collect();
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let (Some((rho, _)), Some(p)) = (view.moon_relative(), view.layout().moon_pair()) else {
            return Ok(());
        };
        let r2 = dot(rho, rho);
        if r2 == R::ZERO {
            return Err(singular(p.earth, p.moon, t));
        }
        acc.add_moon_relative(scaled(-self.earth_moon_gm, rho, inv_r3(r2)));
        let pe = view.position(p.earth)?;
        for &j in &self.third {
            let d = sub(view.position(j)?, pe);
            if r2_of(d) == R::ZERO {
                return Err(singular(p.earth, j, t));
            }
            let g = view.layout().gm(j);
            acc.add_moon_relative(self.differential(d, rho).map(|x| g * x));
        }
        Ok(())
    }
    /// The Earth–Moon and third-body potentials are counted by [`NewtonianMutual`] (which owns
    /// all Major pairs); this term only re-expresses their forces, so it adds 0.
    fn potential_over_g(&self, _view: &View<'_, R>) -> Option<R> {
        Some(R::ZERO)
    }
}

/// Solar J2 with the reaction on the Sun (NUM-019; DE440, Park et al. 2021): every Major body
/// `i` other than the Sun receives the J2 field of the Sun at `rᵢ − r_Sun` (pole `k̂`, configurable)
/// and the Sun receives `−(μᵢ/μ_Sun) aᵢ`, so total linear momentum is conserved. Minor bodies do
/// not feel it (NUM-009: Minor bodies feel the Majors' Newtonian forces only).
#[derive(Debug, Clone)]
pub struct SolarJ2WithReaction<F: Frame, R: Real = f64> {
    j2: R,
    radius: R,
    pole: Pole<F, R>,
    sun: usize,
    sun_gm: R,
    bodies: Vec<usize>,
}

impl<F: Frame, R: Real> SolarJ2WithReaction<F, R> {
    /// J2 (unnormalised), the reference radius (km) and the pole.
    ///
    /// # Errors
    /// `FA-E0500` if `j2` is not finite or the radius is not positive.
    pub fn new(
        j2: R,
        radius: ferroastro_core::units::Length<R>,
        pole: Pole<F, R>,
    ) -> Result<Self, ForceError> {
        let r = radius.km();
        if !j2.is_finite() {
            return Err(ForceError::Invalid {
                name: "J2",
                value: j2.to_f64(),
                valid: "finite",
            });
        }
        if !(r.is_finite() && r > R::ZERO) {
            return Err(ForceError::Invalid {
                name: "J2 reference radius (km)",
                value: r.to_f64(),
                valid: "finite and > 0",
            });
        }
        Ok(Self {
            j2,
            radius: r,
            pole,
            sun: 0,
            sun_gm: R::ZERO,
            bodies: Vec::new(),
        })
    }

    /// The pole.
    #[must_use]
    pub const fn pole(&self) -> Pole<F, R> {
        self.pole
    }
}

impl<F: Frame, R: Real> ForceTerm<F, R> for SolarJ2WithReaction<F, R> {
    fn name(&self) -> &'static str {
        "solar-j2-with-reaction"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        let [px, py, pz] = self.pole.unit().map(Real::to_f64);
        vec![
            TermParameter {
                name: "J2",
                value: self.j2.to_f64(),
                unit: "1",
            },
            TermParameter {
                name: "reference radius",
                value: self.radius.to_f64(),
                unit: "km",
            },
            TermParameter {
                name: "pole x",
                value: px,
                unit: "1",
            },
            TermParameter {
                name: "pole y",
                value: py,
                unit: "1",
            },
            TermParameter {
                name: "pole z",
                value: pz,
                unit: "1",
            },
        ]
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major]
    }
    fn bind(&mut self, layout: &Layout<R>) -> Result<(), ForceError> {
        let sun = layout
            .index_of(NaifId::SUN)
            .filter(|&i| layout.tiers().get(i) == Some(&Tier::Major))
            .ok_or(ForceError::Invalid {
                name: "solar J2 needs the Sun (NAIF 10) as a Major body",
                value: f64::from(NaifId::SUN.0),
                valid: "a Major Sun",
            })?;
        self.sun_gm = layout.gm(sun);
        if self.sun_gm <= R::ZERO {
            return Err(ForceError::Invalid {
                name: "solar GM (km^3/s^2)",
                value: self.sun_gm.to_f64(),
                valid: "> 0",
            });
        }
        self.sun = sun;
        self.bodies = layout
            .indices(Tier::Major)
            .iter()
            .copied()
            .filter(|&i| i != sun)
            .collect();
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let ps = view.position(self.sun)?;
        let k_hat = self.pole.unit();
        for &i in &self.bodies {
            let r = sub(view.position(i)?, ps);
            if r2_of(r) == R::ZERO {
                return Err(singular(self.sun, i, t));
            }
            let a = j2_field(self.sun_gm, self.j2, self.radius, k_hat, r);
            let ratio = view.layout().gm(i) / self.sun_gm;
            acc.add(i, a);
            acc.add(self.sun, a.map(|x| -(ratio * x)));
        }
        Ok(())
    }
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R> {
        let ps = view.position(self.sun).ok()?;
        let k_hat = self.pole.unit();
        let mut u = R::ZERO;
        for &i in &self.bodies {
            let r = sub(view.position(i).ok()?, ps);
            u += view.layout().gm(i) * j2_potential(self.sun_gm, self.j2, self.radius, k_hat, r);
        }
        Some(u)
    }
}
