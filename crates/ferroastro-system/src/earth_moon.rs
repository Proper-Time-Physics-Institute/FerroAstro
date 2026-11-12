// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Geocentric Moon variables: the Earth–Moon pair as (Earth–Moon barycentre, Moon − Earth).
//!
//! DE integrates the Moon relative to the Earth (header.440 `XM…ZDM`) and the Earth–Moon
//! barycentre (`XB…ZDB`) instead of two barycentric states, because the geocentric vector is
//! ~1/400 of a barycentric coordinate and keeps that many more significant bits. With
//! `EMRAT = M_Earth / M_Moon`:
//!
//! * Earth = EMB − (Moon − Earth) / (1 + EMRAT)
//! * Moon  = EMB + (Moon − Earth) · EMRAT / (1 + EMRAT)
//!
//! and inversely Moon − Earth = Moon − Earth, EMB = (EMRAT·Earth + Moon) / (1 + EMRAT).

use crate::vec3::zip3;
use ferroastro_core::frames::{Barycentric, Displacement, Frame, Geocentric, Position, State};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_real::Real;

/// The Earth–Moon pair in DE's integration variables.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthMoon<F: Frame, R: Real = f64> {
    /// Earth–Moon barycentre relative to the SSB.
    pub emb: State<F, Barycentric, R>,
    /// The Moon relative to the Earth (the "geocentric Moon").
    pub moon: State<F, Geocentric, R>,
}

/// `EMRAT/(1+EMRAT)` and `1/(1+EMRAT)` in `R`.
fn fractions<R: Real>(emrat: R) -> (R, R) {
    let d = R::ONE + emrat;
    (emrat / d, R::ONE / d)
}

fn scaled<F: Frame, R: Real>(p: [Length<R>; 3], k: R) -> Displacement<F, R> {
    let [x, y, z] = p;
    Displacement::new(x * k, y * k, z * k)
}

fn position<F: Frame, C: ferroastro_core::frames::Center, R: Real>(
    p: [Length<R>; 3],
) -> Position<F, C, R> {
    let [x, y, z] = p;
    Position::new(x, y, z)
}

impl<F: Frame, R: Real> EarthMoon<F, R> {
    /// Barycentric Earth and Moon `(earth, moon)` from the pair and the mass ratio `emrat`.
    #[must_use]
    pub fn split(&self, emrat: R) -> (State<F, Barycentric, R>, State<F, Barycentric, R>) {
        let (fe, fm) = fractions(emrat);
        let g = self.moon.position().components();
        let gv = self.moon.velocity();
        let ep = self.emb.position();
        let ev = self.emb.velocity();
        let earth_p = ep + scaled(g, -fm);
        let moon_p = ep + scaled(g, fe);
        let earth_v = zip3(ev, gv, |e, g| e - g * fm);
        let moon_v = zip3(ev, gv, |e, g| e + g * fe);
        (State::new(earth_p, earth_v), State::new(moon_p, moon_v))
    }

    /// The pair from barycentric Earth and Moon states and the mass ratio `emrat`.
    #[must_use]
    pub fn from_bodies(
        earth: &State<F, Barycentric, R>,
        moon: &State<F, Barycentric, R>,
        emrat: R,
    ) -> Self {
        let (fe, fm) = fractions(emrat);
        let (e, m) = (earth.position().components(), moon.position().components());
        let (ev, mv) = (earth.velocity(), moon.velocity());
        let geo = position(zip3(m, e, |a, b| a - b));
        let geo_v: [Velocity<R>; 3] = zip3(mv, ev, |a, b| a - b);
        let emb = position(zip3(e, m, |a, b| a * fe + b * fm));
        let emb_v = zip3(ev, mv, |a, b| a * fe + b * fm);
        Self {
            emb: State::new(emb, emb_v),
            moon: State::new(geo, geo_v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::frames::Icrf;

    /// Verifies: NUM-008, NUM-009
    #[test]
    fn split_and_recombine_are_inverse_to_round_off() {
        // Structural fixture (not physics): an arbitrary pair and mass ratio.
        let l = |x: f64| Length::from_km(x).unwrap();
        let v = |x: f64| Velocity::from_km_per_s(x).unwrap();
        let pair = EarthMoon::<Icrf> {
            emb: State::new(
                Position::new(l(1.25e8), l(-7.5e7), l(3.0e7)), // lint: allow-literal(structural fixture)
                [v(15.0), v(25.0), v(-1.0)], // lint: allow-literal(structural fixture)
            ),
            moon: State::new(
                Position::new(l(3.0e5), l(-2.0e5), l(1.0e5)), // lint: allow-literal(structural fixture)
                [v(0.5), v(0.75), v(-0.25)], // lint: allow-literal(structural fixture)
            ),
        };
        let ratio = 80.0; // lint: allow-literal(structural fixture)
        let (earth, moon) = pair.split(ratio);
        let back = EarthMoon::from_bodies(&earth, &moon, ratio);
        let pe = back.emb.position().components();
        let pg = back.moon.position().components();
        for k in 0..3 {
            let want_e = pair.emb.position().components()[k].km();
            let want_g = pair.moon.position().components()[k].km();
            assert!((pe[k].km() - want_e).abs() <= 4.0 * f64::EPSILON * want_e.abs());
            assert!((pg[k].km() - want_g).abs() <= 1e3 * f64::EPSILON * want_e.abs()); // lint: allow-literal(round-off budget: geocentric difference of barycentric coordinates)
        }
        // Moon - Earth points from the Earth to the Moon, and the EMB lies between them,
        // closer to the heavier Earth.
        let d_e = (earth.position() - pair.emb.position()).norm().km();
        let d_m = (moon.position() - pair.emb.position()).norm().km();
        assert!((d_m / d_e - ratio).abs() < 1e-6 * ratio); // lint: allow-literal(round-off budget)
    }
}
