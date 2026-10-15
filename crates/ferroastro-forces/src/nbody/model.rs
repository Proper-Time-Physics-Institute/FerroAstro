// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! [`NBodyModel`]: a layout plus an ordered list of [`ForceTerm`]s, as a [`SecondOrderSystem`].

use super::layout::{Accelerations, Layout, MoonVariables, NBody, View, slot};
use super::terms::ForceTerm;
use crate::{ForceError, ForceModel, Gauge};
use ferroastro_core::frames::{Barycentric, Center, Frame, Position, State};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use ferroastro_real::{Neumaier, Real};
use ferroastro_system::{EarthMoon, SystemState, Tier};
use std::fmt::Write as _;
use std::marker::PhantomData;

/// An N-body force model in frame `F` (barycentric), backend `R` (default `f64`).
///
/// The terms are evaluated in the order given, each in its own fixed pair order, so the
/// acceleration is a deterministic function of the state (TST-005). With
/// [`NBodyModel::with_compensation`] every acceleration component is accumulated with
/// Neumaier's compensated sum.
#[derive(Debug)]
pub struct NBodyModel<F: Frame, R: Real = f64> {
    layout: Layout<R>,
    terms: Vec<Box<dyn ForceTerm<F, R>>>,
    gauge: Gauge,
    compensated: bool,
    _frame: PhantomData<fn() -> F>,
}

fn invalid(name: &'static str, value: f64, valid: &'static str) -> ForceError {
    ForceError::Invalid { name, value, valid }
}

/// A typed state from raw km and km/s, `None` if a component is not finite.
fn typed_state<F: Frame, C: Center, R: Real>(p: [R; 3], w: [R; 3]) -> Option<State<F, C, R>> {
    let l = |z: R| Length::from_km(z).ok();
    let s = |z: R| Velocity::from_km_per_s(z).ok();
    let [p0, p1, p2] = p;
    let [w0, w1, w2] = w;
    Some(State::new(
        Position::new(l(p0)?, l(p1)?, l(p2)?),
        [s(w0)?, s(w1)?, s(w2)?],
    ))
}

impl<F: Frame, R: Real> NBodyModel<F, R> {
    /// Build a model from bodies, the Earth–Moon representation and the terms (in evaluation
    /// order). Each term is bound to the layout.
    ///
    /// # Errors
    /// `FA-E0500` for an invalid layout, a term that cannot bind, or a geocentric pair without
    /// exactly one term that supplies Moon − Earth (or such a term without the pair);
    /// `FA-E0502` if two terms declare different relativistic gauges.
    pub fn new(
        bodies: &[NBody<R>],
        moon: MoonVariables<R>,
        mut terms: Vec<Box<dyn ForceTerm<F, R>>>,
    ) -> Result<Self, ForceError> {
        let layout = Layout::new(bodies, moon)?;
        let suppliers = terms.iter().filter(|t| t.supplies_moon_relative()).count();
        let want = usize::from(layout.moon_pair().is_some());
        if suppliers != want {
            return Err(invalid(
                "terms supplying the geocentric Moon acceleration",
                f64::from(u32::try_from(suppliers).unwrap_or(u32::MAX)),
                "exactly 1 with MoonVariables::Geocentric, 0 otherwise",
            ));
        }
        let mut gauge = Gauge::Newtonian;
        for t in &mut terms {
            t.bind(&layout)?;
            let g = t.gauge();
            if g.is_relativistic() {
                if gauge.is_relativistic() && gauge != g {
                    return Err(ForceError::GaugeMismatch {
                        first: gauge.label(),
                        second: g.label(),
                    });
                }
                gauge = g;
            }
        }
        Ok(Self {
            layout,
            terms,
            gauge,
            compensated: false,
            _frame: PhantomData,
        })
    }

    /// Accumulate accelerations with Neumaier's compensated sum (off by default).
    #[must_use]
    pub fn with_compensation(mut self, on: bool) -> Self {
        self.compensated = on;
        self
    }

    /// The layout.
    #[must_use]
    pub const fn layout(&self) -> &Layout<R> {
        &self.layout
    }

    /// The terms, in evaluation order.
    #[must_use]
    pub fn terms(&self) -> &[Box<dyn ForceTerm<F, R>>] {
        &self.terms
    }

    /// A multi-line description: gauge, layout and each term with its tiers and parameters.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut s = format!(
            "N-body model [{}]: {} bodies ({} Major, {} Minor, {} Ring), Moon variables: {}, \
             accumulation: {}",
            self.gauge.label(),
            self.layout.len(),
            self.layout.indices(Tier::Major).len(),
            self.layout.indices(Tier::Minor).len(),
            self.layout.indices(Tier::Ring).len(),
            if self.layout.moon_pair().is_some() {
                "geocentric (EMB + Moon - Earth)"
            } else {
                "barycentric"
            },
            if self.compensated {
                "Neumaier"
            } else {
                "plain"
            },
        );
        for t in &self.terms {
            let _ = write!(
                s,
                "\n  {} [{}] acts on {:?}:",
                t.name(),
                t.gauge().label(),
                t.tiers()
            );
            for p in t.parameters() {
                let _ = write!(s, " {} = {} {};", p.name, p.value, p.unit);
            }
        }
        s
    }

    /// Flatten a barycentric system into the integration variables (component-major `SoA`).
    /// With a geocentric pair, `earth_moon` supplies the EMB and Moon − Earth directly (e.g. the
    /// header.440 `XB`/`XM` blocks); without it they are formed from the Earth and Moon states.
    ///
    /// # Errors
    /// `FA-E0500` if a body of the model is missing from `states`.
    pub fn pack(
        &self,
        states: &SystemState<F, Barycentric, R>,
        earth_moon: Option<&EarthMoon<F, R>>,
    ) -> Result<(Vec<R>, Vec<R>), ForceError> {
        let n = self.layout.len();
        let mut x = vec![R::ZERO; 3 * n];
        let mut v = vec![R::ZERO; 3 * n];
        let missing = |id: i32| {
            invalid(
                "body missing from the state (NAIF id)",
                f64::from(id),
                "present",
            )
        };
        let mut put = |i: usize, p: [R; 3], w: [R; 3]| {
            for k in 0..3 {
                if let (Some(xs), Some(vs), Some(&pk), Some(&wk)) = (
                    x.get_mut(k * n + i),
                    v.get_mut(k * n + i),
                    p.get(k),
                    w.get(k),
                ) {
                    *xs = pk;
                    *vs = wk;
                }
            }
        };
        let raw = |s: &State<F, Barycentric, R>| -> ([R; 3], [R; 3]) {
            (
                s.position().components().map(Length::km),
                s.velocity().map(Velocity::km_per_s),
            )
        };
        for (i, &id) in self.layout.ids().iter().enumerate() {
            let s = states.state(id).map_err(|_| missing(id.0))?;
            let (p, w) = raw(&s);
            put(i, p, w);
        }
        if let Some(pair) = self.layout.moon_pair() {
            let em = if let Some(em) = earth_moon {
                *em
            } else {
                let e = states
                    .state(ferroastro_ephem::NaifId::EARTH)
                    .map_err(|_| missing(399))?;
                let m = states
                    .state(ferroastro_ephem::NaifId::MOON)
                    .map_err(|_| missing(301))?;
                let emrat = pair.earth_fraction / pair.moon_fraction;
                EarthMoon::from_bodies(&e, &m, emrat)
            };
            let (bp, bv) = raw(&em.emb);
            put(pair.earth, bp, bv);
            put(
                pair.moon,
                em.moon.position().components().map(Length::km),
                em.moon.velocity().map(Velocity::km_per_s),
            );
        }
        Ok((x, v))
    }

    /// The Earth–Moon pair of flat vectors `x`, `v` (geocentric models only).
    #[must_use]
    pub fn earth_moon(&self, x: &[R], v: &[R]) -> Option<EarthMoon<F, R>> {
        let pair = self.layout.moon_pair()?;
        let n = self.layout.len();
        Some(EarthMoon {
            emb: typed_state(slot(x, n, pair.earth), slot(v, n, pair.earth))?,
            moon: typed_state(slot(x, n, pair.moon), slot(v, n, pair.moon))?,
        })
    }

    /// Rebuild the barycentric system from flat vectors (Earth and Moon split from the pair
    /// with `EMRAT`, exactly as [`EarthMoon::split`]).
    ///
    /// # Errors
    /// `FA-E0500` on a length mismatch or a non-finite component.
    pub fn unpack(&self, x: &[R], v: &[R]) -> Result<SystemState<F, Barycentric, R>, ForceError> {
        self.layout.check_len(x.len())?;
        self.layout.check_len(v.len())?;
        let n = self.layout.len();
        let nonfinite = |z: R| invalid("state component", z.to_f64(), "finite");
        let typed = |p: [R; 3], w: [R; 3]| -> Result<State<F, Barycentric, R>, ForceError> {
            let l = |z: R| Length::from_km(z).map_err(|_| nonfinite(z));
            let s = |z: R| Velocity::from_km_per_s(z).map_err(|_| nonfinite(z));
            let [p0, p1, p2] = p;
            let [w0, w1, w2] = w;
            Ok(State::new(
                Position::new(l(p0)?, l(p1)?, l(p2)?),
                [s(w0)?, s(w1)?, s(w2)?],
            ))
        };
        let split = match self.layout.moon_pair() {
            Some(p) => {
                let em = self
                    .earth_moon(x, v)
                    .ok_or_else(|| invalid("Earth-Moon pair", f64::NAN, "finite"))?;
                Some((
                    p.earth,
                    p.moon,
                    em.split(p.earth_fraction / p.moon_fraction),
                ))
            }
            None => None,
        };
        let mut out = SystemState::new();
        for (i, &id) in self.layout.ids().iter().enumerate() {
            let st = match split {
                Some((e, _, (earth, _))) if i == e => earth,
                Some((_, m, (_, moon))) if i == m => moon,
                _ => typed(slot(x, n, i), slot(v, n, i))?,
            };
            out.push(id, &st)
                .map_err(|_| invalid("duplicated NAIF id", f64::from(id.0), "unique"))?;
        }
        Ok(out)
    }

    /// Total energy divided by G (km⁵/s⁴): `Σ ½ μᵢ vᵢ²` over the moving bodies plus every
    /// term's potential; `None` if a term is not conservative. Neumaier-summed.
    #[must_use]
    pub fn energy_over_g(&self, x: &[R], v: &[R]) -> Option<R> {
        if self.layout.check_len(x.len()).is_err() || self.layout.check_len(v.len()).is_err() {
            return None;
        }
        let view = View::new(&self.layout, x, v);
        let mut e = Neumaier::new();
        for i in (0..self.layout.len()).filter(|&i| self.layout.moves(i)) {
            let w = view.velocity(i);
            let v2 = w[0].mul_add(w[0], w[1].mul_add(w[1], w[2] * w[2]));
            e.add(R::HALF * self.layout.gm(i) * v2);
        }
        for t in &self.terms {
            e.add(t.potential_over_g(&view)?);
        }
        Some(e.total())
    }

    /// Total linear momentum divided by G, `Σ μᵢ vᵢ` over the moving bodies (km⁴/s³), and the
    /// scale `Σ μᵢ |vᵢ|` against which its round-off is judged. Neumaier-summed.
    #[must_use]
    pub fn momentum_over_g(&self, x: &[R], v: &[R]) -> ([R; 3], R) {
        let view = View::new(&self.layout, x, v);
        let mut p = [Neumaier::new(), Neumaier::new(), Neumaier::new()];
        let mut scale = Neumaier::new();
        for i in (0..self.layout.len()).filter(|&i| self.layout.moves(i)) {
            let w = view.velocity(i);
            let g = self.layout.gm(i);
            for (pk, wk) in p.iter_mut().zip(w) {
                pk.add(g * wk);
            }
            scale.add(g * w[0].hypot(w[1]).hypot(w[2]));
        }
        (p.map(|a| a.total()), scale.total())
    }
}

impl<F: Frame, R: Real> SecondOrderSystem<R> for NBodyModel<F, R> {
    fn dimension(&self) -> usize {
        3 * self.layout.len()
    }

    fn acceleration(&self, t: R, x: &[R], v: &[R], a: &mut [R]) -> Result<(), IntegratorError> {
        self.layout.check_len(x.len())?;
        self.layout.check_len(v.len())?;
        self.layout.check_len(a.len())?;
        let view = View::new(&self.layout, x, v);
        let mut acc = Accelerations::new(&self.layout, self.compensated, a);
        for term in &self.terms {
            term.accumulate(t, &view, &mut acc)?;
        }
        acc.finish(&self.layout);
        Ok(())
    }
}

impl<F: Frame, R: Real> ForceModel<R> for NBodyModel<F, R> {
    fn gauge(&self) -> Gauge {
        self.gauge
    }
    fn name(&self) -> &'static str {
        "n-body"
    }
}
