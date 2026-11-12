// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The structure-of-arrays (`SoA`) system state, generic over [`Real`], typed by frame and centre.
//!
//! Each component is a contiguous `Vec<R>` (x, y, z, vx, vy, vz), the layout the force kernels
//! of `ferroastro-forces` iterate over. Single bodies go in and out only as typed
//! [`State<F, C, R>`]s, so a heliocentric state cannot be stored in a barycentric system (see
//! the compile-fail test), and the conversion is exact (no arithmetic).

use crate::SystemError;
use crate::vec3::zip3;
use ferroastro_core::frames::{
    Barycentric, Center, Displacement, Frame, Heliocentric, Position, State,
};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_real::Real;
use std::marker::PhantomData;

/// One body's id and typed state.
pub type BodyState<F, C, R> = (NaifId, State<F, C, R>);

/// The states of a set of bodies in frame `F` relative to centre `C`, km and km/s, `SoA` layout.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemState<F: Frame, C: Center, R: Real = f64> {
    ids: Vec<NaifId>,
    pos: [Vec<R>; 3],
    vel: [Vec<R>; 3],
    _tag: PhantomData<(F, C)>,
}

impl<F: Frame, C: Center, R: Real> Default for SystemState<F, C, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: Frame, C: Center, R: Real> SystemState<F, C, R> {
    /// An empty system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            pos: [Vec::new(), Vec::new(), Vec::new()],
            vel: [Vec::new(), Vec::new(), Vec::new()],
            _tag: PhantomData,
        }
    }

    /// Number of bodies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True if there are no bodies.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The NAIF ids, in storage order.
    #[must_use]
    pub fn ids(&self) -> &[NaifId] {
        &self.ids
    }

    /// Storage index of `id`.
    #[must_use]
    pub fn index_of(&self, id: NaifId) -> Option<usize> {
        self.ids.iter().position(|&i| i == id)
    }

    fn write(&mut self, i: usize, state: &State<F, C, R>) {
        let p = state.position().components().map(Length::km);
        let v = state.velocity().map(Velocity::km_per_s);
        for (c, x) in self.pos.iter_mut().zip(p).chain(self.vel.iter_mut().zip(v)) {
            if let Some(slot) = c.get_mut(i) {
                *slot = x;
            }
        }
    }

    /// Append body `id` with `state`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is already present.
    pub fn push(&mut self, id: NaifId, state: &State<F, C, R>) -> Result<(), SystemError> {
        if self.index_of(id).is_some() {
            return Err(SystemError::duplicate(id.0));
        }
        self.ids.push(id);
        for c in self.pos.iter_mut().chain(self.vel.iter_mut()) {
            c.push(R::ZERO);
        }
        self.write(self.ids.len() - 1, state);
        Ok(())
    }

    /// Replace the state of body `id`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is not present.
    pub fn set(&mut self, id: NaifId, state: &State<F, C, R>) -> Result<(), SystemError> {
        let i = self
            .index_of(id)
            .ok_or_else(|| SystemError::unknown(id.0))?;
        self.write(i, state);
        Ok(())
    }

    /// Position `[x, y, z]` (km) and velocity `[vx, vy, vz]` (km/s) at storage index `i`.
    #[must_use]
    pub fn raw_at(&self, i: usize) -> Option<([R; 3], [R; 3])> {
        let g = |c: &[Vec<R>; 3]| -> Option<[R; 3]> {
            let [x, y, z] = c;
            Some([*x.get(i)?, *y.get(i)?, *z.get(i)?])
        };
        Some((g(&self.pos)?, g(&self.vel)?))
    }

    /// The typed state at storage index `i`.
    ///
    /// # Errors
    /// `FA-E1100` if `i` is out of range; `FA-E1101` if a component was made non-finite through
    /// [`SystemState::components_mut`].
    pub fn state_at(&self, i: usize) -> Result<State<F, C, R>, SystemError> {
        let id = self
            .ids
            .get(i)
            .copied()
            .ok_or_else(|| SystemError::unknown(-1))?;
        let (p, v) = self.raw_at(i).ok_or_else(|| SystemError::unknown(id.0))?;
        let km = |x: R| {
            Length::from_km(x).map_err(|_| SystemError::non_finite("position (km)", x.to_f64()))
        };
        let kms = |x: R| {
            Velocity::from_km_per_s(x)
                .map_err(|_| SystemError::non_finite("velocity (km/s)", x.to_f64()))
        };
        let ([x, y, z], [vx, vy, vz]) = (p, v);
        Ok(State::new(
            Position::new(km(x)?, km(y)?, km(z)?),
            [kms(vx)?, kms(vy)?, kms(vz)?],
        ))
    }

    /// The typed state of body `id`.
    ///
    /// # Errors
    /// `FA-E1100` if `id` is not present; `FA-E1101` for a non-finite component.
    pub fn state(&self, id: NaifId) -> Result<State<F, C, R>, SystemError> {
        let i = self
            .index_of(id)
            .ok_or_else(|| SystemError::unknown(id.0))?;
        self.state_at(i)
    }

    /// Every `(id, state)` in storage order.
    ///
    /// # Errors
    /// As [`SystemState::state_at`].
    pub fn states(&self) -> Result<Vec<BodyState<F, C, R>>, SystemError> {
        self.ids
            .iter()
            .enumerate()
            .map(|(i, &id)| Ok((id, self.state_at(i)?)))
            .collect()
    }

    /// The component arrays `([x, y, z], [vx, vy, vz])` in km and km/s.
    #[must_use]
    pub fn components(&self) -> (&[Vec<R>; 3], &[Vec<R>; 3]) {
        (&self.pos, &self.vel)
    }

    /// Mutable component arrays (for integrators). The lengths cannot change through this view.
    pub fn components_mut(&mut self) -> ([&mut [R]; 3], [&mut [R]; 3]) {
        let [px, py, pz] = &mut self.pos;
        let [vx, vy, vz] = &mut self.vel;
        ([px, py, pz], [vx, vy, vz])
    }

    /// The same system in another backend (exact when widening).
    #[must_use]
    pub fn cast<S: Real>(&self) -> SystemState<F, C, S> {
        let c = |a: &[Vec<R>; 3]| -> [Vec<S>; 3] {
            a.each_ref().map(|v| {
                v.iter()
                    .map(|&x| {
                        let (hi, lo) = x.to_parts();
                        S::from_parts(hi, lo)
                    })
                    .collect()
            })
        };
        SystemState {
            ids: self.ids.clone(),
            pos: c(&self.pos),
            vel: c(&self.vel),
            _tag: PhantomData,
        }
    }

    /// Every body translated by `-origin` (position and velocity), relabelled to centre `D`.
    fn recentred<D: Center>(&self, origin_p: [R; 3], origin_v: [R; 3]) -> SystemState<F, D, R> {
        let shift = |a: &[Vec<R>; 3], o: [R; 3]| -> [Vec<R>; 3] {
            zip3(a.each_ref(), o, |v: &Vec<R>, ok| {
                v.iter().map(|&x| x - ok).collect()
            })
        };
        SystemState {
            ids: self.ids.clone(),
            pos: shift(&self.pos, origin_p),
            vel: shift(&self.vel, origin_v),
            _tag: PhantomData,
        }
    }
}

impl<F: Frame, R: Real> SystemState<F, Barycentric, R> {
    /// The same bodies relative to the Sun (the Sun's own entry becomes zero).
    ///
    /// # Errors
    /// `FA-E1100` if the Sun (NAIF 10) is not present.
    pub fn heliocentric(&self) -> Result<SystemState<F, Heliocentric, R>, SystemError> {
        let i = self
            .index_of(NaifId::SUN)
            .ok_or_else(|| SystemError::unknown(NaifId::SUN.0))?;
        let (p, v) = self
            .raw_at(i)
            .ok_or_else(|| SystemError::unknown(NaifId::SUN.0))?;
        Ok(self.recentred(p, v))
    }

    /// Translate every body by `-d` and `-dv` (e.g. to move the origin to a computed barycentre).
    pub fn translate(&mut self, d: &Displacement<F, R>, dv: [Velocity<R>; 3]) {
        let d = d.components().map(Length::km);
        let dv = dv.map(Velocity::km_per_s);
        for (c, o) in self
            .pos
            .iter_mut()
            .zip(d)
            .chain(self.vel.iter_mut().zip(dv))
        {
            for x in c.iter_mut() {
                *x -= o;
            }
        }
    }
}

impl<F: Frame, R: Real> SystemState<F, Heliocentric, R> {
    /// The same bodies relative to the SSB, given the Sun's barycentric state.
    #[must_use]
    pub fn barycentric(&self, sun: &State<F, Barycentric, R>) -> SystemState<F, Barycentric, R> {
        let p = sun.position().components().map(|x| -x.km());
        let v = sun.velocity().map(|x| -x.km_per_s());
        self.recentred(p, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::frames::Icrf;
    use ferroastro_real::DoubleDouble;

    fn st<C: Center>(seed: f64) -> State<Icrf, C> {
        // Structural fixture values (no physics): distinct, non-representable decimals.
        let l = |x: f64| Length::from_km(x).unwrap();
        let v = |x: f64| Velocity::from_km_per_s(x).unwrap();
        State::new(
            Position::new(l(seed * 0.1), l(-seed / 3.0), l(seed.sqrt())), // lint: allow-literal(structural fixture)
            [v(seed / 7.0), v(-seed * 0.3), v(1.0 / seed)], // lint: allow-literal(structural fixture)
        )
    }

    /// Verifies: NUM-009, API-001
    #[test]
    fn typed_states_round_trip_exactly_f64_and_double_double() {
        let mut s = SystemState::<Icrf, Barycentric>::new();
        for (k, id) in [NaifId::SUN, NaifId::MERCURY_BARYCENTER, NaifId::EARTH]
            .into_iter()
            .enumerate()
        {
            s.push(id, &st(f64::from(u8::try_from(k).unwrap()) + 1.5)) // lint: allow-literal(structural fixture)
                .unwrap();
        }
        for (k, id) in s.ids().to_vec().into_iter().enumerate() {
            assert_eq!(
                s.state(id).unwrap(),
                st(f64::from(u8::try_from(k).unwrap()) + 1.5) // lint: allow-literal(structural fixture)
            );
        }
        assert_eq!(
            s.push(NaifId::SUN, &st(2.0)).unwrap_err().code(),
            ferroastro_core::ErrorCode::SystemBody
        );
        // Widening to double-double and back is exact.
        let dd: SystemState<Icrf, Barycentric, DoubleDouble> = s.cast();
        assert_eq!(dd.cast::<f64>(), s);
        assert_eq!(
            dd.state(NaifId::EARTH).unwrap().cast::<f64>(),
            s.state(NaifId::EARTH).unwrap()
        );
        // set replaces in place.
        s.set(NaifId::EARTH, &st(9.0)).unwrap();
        assert_eq!(s.state(NaifId::EARTH).unwrap(), st(9.0));
        assert_eq!(s.len(), 3);
    }

    /// Verifies: API-001
    #[test]
    fn heliocentric_and_back() {
        let mut s = SystemState::<Icrf, Barycentric>::new();
        s.push(NaifId::SUN, &st(0.5)).unwrap();
        s.push(NaifId::MERCURY_BARYCENTER, &st(4.0)).unwrap();
        let h = s.heliocentric().unwrap();
        let sun = h.state(NaifId::SUN).unwrap();
        assert!(sun.position().components().iter().all(|x| x.km() == 0.0));
        let back = h.barycentric(&s.state(NaifId::SUN).unwrap());
        let (a, b) = (back.components().0, s.components().0);
        for k in 0..3 {
            for (x, y) in a[k].iter().zip(&b[k]) {
                assert!((x - y).abs() <= 4.0 * f64::EPSILON * y.abs().max(1.0));
            }
        }
        let empty = SystemState::<Icrf, Barycentric>::new();
        assert_eq!(
            empty.heliocentric().unwrap_err().code(),
            ferroastro_core::ErrorCode::SystemBody
        );
    }
}
