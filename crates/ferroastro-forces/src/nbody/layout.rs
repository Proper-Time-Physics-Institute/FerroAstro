// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Body layout, the read-only state view handed to force terms, and the acceleration accumulator.
//!
//! # Coordinate layout (component-major `SoA`)
//! The integrator's flat vectors hold `n` bodies as `[x₀ … x_{n−1}, y₀ … y_{n−1}, z₀ … z_{n−1}]`
//! (km and km/s). IAS15 is indifferent to the order of coordinates, so the force kernels can read
//! each component as one contiguous slice, the layout of
//! [`ferroastro_system::SystemState`].
//!
//! # Geocentric Moon (NUM-008)
//! With [`MoonVariables::Geocentric`], the Earth's slot holds the **Earth–Moon barycentre** and
//! the Moon's slot holds **Moon − Earth**, DE's integration variables (header.440 `XB`, `XM`).
//! [`View::position`] reconstructs the physical barycentric Earth and Moon for the terms, and
//! [`Accelerations`] recombines what the terms add:
//!
//! * EMB acceleration = `f_E a_E + f_M a_M` with `f_E = EMRAT/(1+EMRAT)`, `f_M = 1/(1+EMRAT)`;
//! * Moon − Earth acceleration = the sum of everything added through
//!   [`Accelerations::add_moon_relative`], plus `a_M − a_E` of every contribution added through
//!   [`Accelerations::add`]. Contributions added through [`Accelerations::add_emb_only`] reach the
//!   EMB only: the Newtonian mutual term uses it for the Earth and the Moon, because the
//!   [`crate::GeocentricMoon`] term supplies their difference without cancellation.

use crate::ForceError;
use ferroastro_ephem::NaifId;
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// One body of an N-body model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NBody<R: Real = f64> {
    /// NAIF id.
    pub id: NaifId,
    /// Gravitational parameter GM, km³/s² (≥ 0).
    pub gm: R,
    /// Interaction tier (NUM-009).
    pub tier: Tier,
}

/// How the Earth and the Moon are represented in the integration variables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoonVariables<R: Real = f64> {
    /// Earth and Moon are two barycentric bodies like any other.
    Barycentric,
    /// The Earth slot holds the Earth–Moon barycentre, the Moon slot holds Moon − Earth
    /// (NUM-008). `emrat` is the Earth/Moon mass ratio that splits the pair (header.440 `EMRAT`).
    Geocentric {
        /// Earth/Moon mass ratio.
        emrat: R,
    },
}

/// Slots and mass fractions of a geocentric Earth–Moon pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonPair<R: Real = f64> {
    /// Storage index of the Earth (holds the EMB).
    pub earth: usize,
    /// Storage index of the Moon (holds Moon − Earth).
    pub moon: usize,
    /// `EMRAT/(1+EMRAT)` = `μ_E/(μ_E+μ_M)`.
    pub earth_fraction: R,
    /// `1/(1+EMRAT)` = `μ_M/(μ_E+μ_M)`.
    pub moon_fraction: R,
}

/// The bodies of a model, their tiers and the optional geocentric Moon pair.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout<R: Real = f64> {
    ids: Vec<NaifId>,
    gm: Vec<R>,
    tiers: Vec<Tier>,
    major: Vec<usize>,
    minor: Vec<usize>,
    ring: Vec<usize>,
    pair: Option<MoonPair<R>>,
}

fn invalid(name: &'static str, value: f64, valid: &'static str) -> ForceError {
    ForceError::Invalid { name, value, valid }
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

impl<R: Real> Layout<R> {
    /// Build and validate a layout.
    ///
    /// # Errors
    /// `FA-E0500` for no bodies, a duplicated id, a negative or non-finite GM, or a geocentric
    /// Moon pair without a Major Earth (NAIF 399) and Moon (NAIF 301), or with an invalid `EMRAT`.
    pub fn new(bodies: &[NBody<R>], moon: MoonVariables<R>) -> Result<Self, ForceError> {
        if bodies.is_empty() {
            return Err(invalid("number of bodies", 0.0, ">= 1"));
        }
        let mut ids = Vec::with_capacity(bodies.len());
        for b in bodies {
            if ids.contains(&b.id) {
                return Err(invalid(
                    "duplicated NAIF id",
                    f64::from(b.id.0),
                    "each body once",
                ));
            }
            if !(b.gm.is_finite() && b.gm >= R::ZERO) {
                return Err(invalid("GM (km^3/s^2)", b.gm.to_f64(), "finite and >= 0"));
            }
            ids.push(b.id);
        }
        let tiers: Vec<Tier> = bodies.iter().map(|b| b.tier).collect();
        let of = |t: Tier| -> Vec<usize> {
            tiers
                .iter()
                .enumerate()
                .filter(|&(_, &x)| x == t)
                .map(|(i, _)| i)
                .collect()
        };
        let pair = match moon {
            MoonVariables::Barycentric => None,
            MoonVariables::Geocentric { emrat } => {
                if !(emrat.is_finite() && emrat > R::ZERO) {
                    return Err(invalid("EMRAT", emrat.to_f64(), "finite and > 0"));
                }
                let find = |id: NaifId| {
                    ids.iter()
                        .position(|&x| x == id)
                        .filter(|&i| tiers.get(i) == Some(&Tier::Major))
                        .ok_or_else(|| {
                            invalid(
                                "geocentric Moon pair body (NAIF id)",
                                f64::from(id.0),
                                "the Earth (399) and the Moon (301), both Major",
                            )
                        })
                };
                let d = R::ONE + emrat;
                Some(MoonPair {
                    earth: find(NaifId::EARTH)?,
                    moon: find(NaifId::MOON)?,
                    earth_fraction: emrat / d,
                    moon_fraction: R::ONE / d,
                })
            }
        };
        Ok(Self {
            major: of(Tier::Major),
            minor: of(Tier::Minor),
            ring: of(Tier::Ring),
            ids,
            gm: bodies.iter().map(|b| b.gm).collect(),
            tiers,
            pair,
        })
    }

    /// Number of bodies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Always false (a layout has at least one body).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// NAIF ids in storage order.
    #[must_use]
    pub fn ids(&self) -> &[NaifId] {
        &self.ids
    }

    /// GMs (km³/s²) in storage order.
    #[must_use]
    pub fn gms(&self) -> &[R] {
        &self.gm
    }

    /// GM of body `i` (0 if out of range).
    #[must_use]
    pub fn gm(&self, i: usize) -> R {
        self.gm.get(i).copied().unwrap_or(R::ZERO)
    }

    /// Tiers in storage order.
    #[must_use]
    pub fn tiers(&self) -> &[Tier] {
        &self.tiers
    }

    /// Storage indices of the bodies of `tier`, ascending.
    #[must_use]
    pub fn indices(&self, tier: Tier) -> &[usize] {
        match tier {
            Tier::Major => &self.major,
            Tier::Minor => &self.minor,
            Tier::Ring => &self.ring,
        }
    }

    /// Storage index of `id`.
    #[must_use]
    pub fn index_of(&self, id: NaifId) -> Option<usize> {
        self.ids.iter().position(|&x| x == id)
    }

    /// The geocentric Moon pair, if the model uses it.
    #[must_use]
    pub const fn moon_pair(&self) -> Option<&MoonPair<R>> {
        self.pair.as_ref()
    }

    /// True if body `i` moves (Major or Minor).
    #[must_use]
    pub fn moves(&self, i: usize) -> bool {
        matches!(self.tiers.get(i), Some(Tier::Major | Tier::Minor))
    }

    /// `FA-E0500` unless the flat vectors have `3n` components.
    pub(crate) fn check_len(&self, len: usize) -> Result<(), ForceError> {
        if len == 3 * self.len() {
            Ok(())
        } else {
            Err(invalid(
                "flat state length",
                count(len),
                "3 x number of bodies",
            ))
        }
    }
}

/// Component `k` of body `i` in a component-major flat vector of `n` bodies.
#[inline]
pub(crate) fn slot<R: Real>(flat: &[R], n: usize, i: usize) -> [R; 3] {
    let g = |k: usize| flat.get(k * n + i).copied().unwrap_or(R::ZERO);
    [g(0), g(1), g(2)]
}

/// Read-only view of the state handed to [`crate::ForceTerm::accumulate`]: physical barycentric
/// positions and velocities of every body (Earth and Moon reconstructed if paired).
#[derive(Debug, Clone, Copy)]
pub struct View<'a, R: Real = f64> {
    layout: &'a Layout<R>,
    x: &'a [R],
    v: &'a [R],
    /// `(earth, moon, moon − earth)` positions and velocities when paired.
    pair: Option<PairState<R>>,
}

#[derive(Debug, Clone, Copy)]
struct PairState<R: Real> {
    earth: usize,
    moon: usize,
    earth_p: [R; 3],
    earth_v: [R; 3],
    moon_p: [R; 3],
    moon_v: [R; 3],
    rel_p: [R; 3],
    rel_v: [R; 3],
}

impl<'a, R: Real> View<'a, R> {
    /// A view of flat vectors `x`, `v` (lengths checked by the caller).
    pub(crate) fn new(layout: &'a Layout<R>, x: &'a [R], v: &'a [R]) -> Self {
        let n = layout.len();
        let pair = layout.moon_pair().map(|p| {
            let (bp, bv) = (slot(x, n, p.earth), slot(v, n, p.earth));
            let (rp, rv) = (slot(x, n, p.moon), slot(v, n, p.moon));
            let along = |b: [R; 3], r: [R; 3], f: R| -> [R; 3] {
                let [b0, b1, b2] = b;
                let [r0, r1, r2] = r;
                [r0.mul_add(f, b0), r1.mul_add(f, b1), r2.mul_add(f, b2)]
            };
            PairState {
                earth: p.earth,
                moon: p.moon,
                earth_p: along(bp, rp, -p.moon_fraction),
                earth_v: along(bv, rv, -p.moon_fraction),
                moon_p: along(bp, rp, p.earth_fraction),
                moon_v: along(bv, rv, p.earth_fraction),
                rel_p: rp,
                rel_v: rv,
            }
        });
        Self { layout, x, v, pair }
    }

    /// The layout.
    #[must_use]
    pub const fn layout(&self) -> &'a Layout<R> {
        self.layout
    }

    /// Barycentric position (km) of body `i`.
    #[must_use]
    pub fn position(&self, i: usize) -> [R; 3] {
        if let Some(p) = &self.pair {
            if i == p.earth {
                return p.earth_p;
            }
            if i == p.moon {
                return p.moon_p;
            }
        }
        slot(self.x, self.layout.len(), i)
    }

    /// Barycentric velocity (km/s) of body `i`.
    #[must_use]
    pub fn velocity(&self, i: usize) -> [R; 3] {
        if let Some(p) = &self.pair {
            if i == p.earth {
                return p.earth_v;
            }
            if i == p.moon {
                return p.moon_v;
            }
        }
        slot(self.v, self.layout.len(), i)
    }

    /// Moon − Earth position and velocity (the integration variable) when paired.
    #[must_use]
    pub fn moon_relative(&self) -> Option<([R; 3], [R; 3])> {
        self.pair.map(|p| (p.rel_p, p.rel_v))
    }
}

/// One compensated (or plain) running sum.
#[derive(Debug, Clone, Copy, Default)]
struct Acc<R: Real> {
    sum: R,
    comp: R,
}

/// Neumaier 1974 (ZAMM 54, 39) step, as `ferroastro_real::Neumaier`, or a plain `+=`.
#[inline]
fn acc_add<R: Real>(sum: &mut R, comp: Option<&mut R>, x: R) {
    if let Some(c) = comp {
        let t = *sum + x;
        if sum.abs() >= x.abs() {
            *c += (*sum - t) + x;
        } else {
            *c += (x - t) + *sum;
        }
        *sum = t;
    } else {
        *sum += x;
    }
}

impl<R: Real> Acc<R> {
    #[inline]
    fn add(&mut self, x: R, compensated: bool) {
        acc_add(&mut self.sum, compensated.then_some(&mut self.comp), x);
    }
    fn total(self) -> R {
        self.sum + self.comp
    }
}

/// Where force terms add accelerations (km/s²). Additions happen in the order the terms make
/// them, so a fixed term order and a fixed pair order give a fixed reduction order (TST-005).
/// Ordinary bodies accumulate directly in the output slice (no allocation unless compensated).
#[derive(Debug)]
pub struct Accelerations<'a, R: Real = f64> {
    n: usize,
    out: &'a mut [R],
    comp: Vec<R>,
    pair: Option<MoonPair<R>>,
    earth: [Acc<R>; 3],
    moon: [Acc<R>; 3],
    rel: [Acc<R>; 3],
    compensated: bool,
}

impl<'a, R: Real> Accelerations<'a, R> {
    /// `out` must have length `3n` (checked by the caller); it is zeroed here.
    pub(crate) fn new(layout: &Layout<R>, compensated: bool, out: &'a mut [R]) -> Self {
        out.fill(R::ZERO);
        Self {
            n: layout.len(),
            comp: if compensated {
                vec![R::ZERO; out.len()]
            } else {
                Vec::new()
            },
            out,
            pair: layout.moon_pair().copied(),
            earth: [Acc::default(); 3],
            moon: [Acc::default(); 3],
            rel: [Acc::default(); 3],
            compensated,
        }
    }

    fn add_body(&mut self, i: usize, a: [R; 3]) {
        let n = self.n;
        for (k, ak) in a.into_iter().enumerate() {
            if let Some(s) = self.out.get_mut(k * n + i) {
                acc_add(s, self.comp.get_mut(k * n + i), ak);
            }
        }
    }

    fn add3(acc: &mut [Acc<R>; 3], a: [R; 3], negate: bool, c: bool) {
        for (s, ak) in acc.iter_mut().zip(a) {
            s.add(if negate { -ak } else { ak }, c);
        }
    }

    /// Add `a` to the barycentric acceleration of body `i`. For a paired Earth or Moon it also
    /// enters Moon − Earth by difference (fine for small perturbations such as J2 or asteroids).
    pub fn add(&mut self, i: usize, a: [R; 3]) {
        let c = self.compensated;
        match self.pair {
            Some(p) if i == p.earth => {
                Self::add3(&mut self.earth, a, false, c);
                Self::add3(&mut self.rel, a, true, c);
            }
            Some(p) if i == p.moon => {
                Self::add3(&mut self.moon, a, false, c);
                Self::add3(&mut self.rel, a, false, c);
            }
            _ => self.add_body(i, a),
        }
    }

    /// Add `a` to body `i`; for a paired Earth or Moon it reaches the Earth–Moon barycentre only
    /// (the [`crate::GeocentricMoon`] term supplies Moon − Earth).
    pub fn add_emb_only(&mut self, i: usize, a: [R; 3]) {
        let c = self.compensated;
        match self.pair {
            Some(p) if i == p.earth => Self::add3(&mut self.earth, a, false, c),
            Some(p) if i == p.moon => Self::add3(&mut self.moon, a, false, c),
            _ => self.add_body(i, a),
        }
    }

    /// Add `a` directly to the Moon − Earth acceleration (no effect without a pair).
    pub fn add_moon_relative(&mut self, a: [R; 3]) {
        if self.pair.is_some() {
            let c = self.compensated;
            Self::add3(&mut self.rel, a, false, c);
        }
    }

    /// Fold the compensations and the Earth–Moon pair into the output. Ring bodies get 0.
    pub(crate) fn finish(self, layout: &Layout<R>) {
        self.finish_into(layout, None);
    }

    /// As [`Accelerations::finish`], but with `split` the compensation of each component is
    /// written there instead of being added to the output: the exact value is then
    /// `out[i] + split[i]` to the accuracy of Neumaier's sum (WP25 mixed mode). The EMB slot gets
    /// `f_E (Σ+c)_E + f_M (Σ+c)_M` split the same way term by term.
    pub(crate) fn finish_into(self, layout: &Layout<R>, mut split: Option<&mut [R]>) {
        let Self {
            n,
            out,
            comp,
            pair,
            earth,
            moon,
            rel,
            ..
        } = self;
        let splitting = split.is_some();
        match split.as_deref_mut() {
            Some(lo) => {
                lo.fill(R::ZERO);
                for (l, c) in lo.iter_mut().zip(&comp) {
                    *l = *c;
                }
            }
            None => {
                for (o, c) in out.iter_mut().zip(&comp) {
                    *o += *c;
                }
            }
        }
        let mut put = |i: usize, hi: [R; 3], lo: [R; 3]| {
            for k in 0..3 {
                let (Some(&h), Some(&l)) = (hi.get(k), lo.get(k)) else {
                    continue;
                };
                match split.as_deref_mut() {
                    Some(sp) => {
                        if let (Some(o), Some(c)) = (out.get_mut(k * n + i), sp.get_mut(k * n + i))
                        {
                            *o = h;
                            *c = l;
                        }
                    }
                    None => {
                        if let Some(o) = out.get_mut(k * n + i) {
                            *o = h + l;
                        }
                    }
                }
            }
        };
        if let Some(p) = pair {
            let (fe, fm) = (p.earth_fraction, p.moon_fraction);
            let hi = [0, 1, 2].map(|k| match (earth.get(k), moon.get(k)) {
                (Some(e), Some(m)) => fe * e.sum + fm * m.sum,
                _ => R::ZERO,
            });
            let lo = [0, 1, 2].map(|k| match (earth.get(k), moon.get(k)) {
                (Some(e), Some(m)) => fe * e.comp + fm * m.comp,
                _ => R::ZERO,
            });
            if splitting {
                put(p.earth, hi, lo);
            } else {
                // Bitwise as before WP25: the fraction-weighted totals.
                let [e0, e1, e2] = earth.map(Acc::total);
                let [m0, m1, m2] = moon.map(Acc::total);
                put(
                    p.earth,
                    [fe * e0 + fm * m0, fe * e1 + fm * m1, fe * e2 + fm * m2],
                    [R::ZERO; 3],
                );
            }
            if splitting {
                put(p.moon, rel.map(|a| a.sum), rel.map(|a| a.comp));
            } else {
                put(p.moon, rel.map(Acc::total), [R::ZERO; 3]);
            }
        }
        for &i in layout.indices(Tier::Ring) {
            put(i, [R::ZERO; 3], [R::ZERO; 3]);
        }
    }
}
