// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! A test particle in the field of a fixed central body, with composable correction terms.
//!
//! The single-body rungs (SR-a, SR-b, GR-SCHW, PPN, J2, LT) all take this form: Newtonian
//! `−μr/r³` plus zero or more [`CentralTerm`]s evaluated at the heliocentric position `r` and
//! velocity `v`. The central body does not move (test-particle limit), which is the limit the
//! analytic oracles of NUM-015 assume. Internal units: km, km/s, TDB seconds, km³/s².

use crate::{ForceError, ForceModel, Gauge};
use ferroastro_core::frames::{Frame, Heliocentric, Position, State};
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use std::fmt;
use std::marker::PhantomData;

/// A unit vector in frame `F`, e.g. a spin axis or the pole of a quadrupole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pole<F: Frame> {
    unit: [f64; 3],
    _frame: PhantomData<fn() -> F>,
}

impl<F: Frame> Pole<F> {
    /// Normalise `components` (any non-zero length) into a unit vector.
    ///
    /// # Errors
    /// `FA-E0500` if the vector is zero or not finite.
    pub fn from_components(components: [f64; 3]) -> Result<Self, ForceError> {
        let n = norm(components);
        if !(n.is_finite() && n > 0.0) {
            return Err(ForceError::Invalid {
                name: "pole vector length",
                value: n,
                valid: "finite and > 0",
            });
        }
        Ok(Self {
            unit: components.map(|c| c / n),
            _frame: PhantomData,
        })
    }

    /// The unit vector.
    #[must_use]
    pub const fn unit(&self) -> [f64; 3] {
        self.unit
    }
}

/// One correction term of a [`TestParticle`] model.
pub trait CentralTerm<F: Frame>: fmt::Debug + Send + Sync {
    /// Add this term's acceleration (km/s²) at heliocentric position `r` (km) and velocity
    /// `v` (km/s) to `a`.
    fn add_acceleration(&self, r: [f64; 3], v: [f64; 3], a: &mut [f64; 3]);
    /// The term's gauge.
    fn gauge(&self) -> Gauge;
    /// A one-line description with the term's equation and parameters, for reports.
    fn describe(&self) -> String;
}

/// A test particle orbiting a fixed central body of gravitational parameter `μ`, with
/// correction terms. The system has 3 coordinates: the heliocentric position.
#[derive(Debug)]
pub struct TestParticle<F: Frame> {
    mu: f64,
    terms: Vec<Box<dyn CentralTerm<F>>>,
    gauge: Gauge,
}

impl<F: Frame> TestParticle<F> {
    /// Build the model. All relativistic terms must share one gauge.
    ///
    /// # Errors
    /// `FA-E0500` if `μ ≤ 0`; `FA-E0502` if two terms declare different relativistic gauges.
    pub fn new(mu: GravParam, terms: Vec<Box<dyn CentralTerm<F>>>) -> Result<Self, ForceError> {
        let m = mu.km3_per_s2();
        if m <= 0.0 {
            return Err(ForceError::Invalid {
                name: "central GM (km^3/s^2)",
                value: m,
                valid: "> 0",
            });
        }
        let mut gauge = Gauge::Newtonian;
        for t in &terms {
            let g = t.gauge();
            if !g.is_relativistic() {
                continue;
            }
            if gauge.is_relativistic() && gauge != g {
                return Err(ForceError::GaugeMismatch {
                    first: gauge.label(),
                    second: g.label(),
                });
            }
            gauge = g;
        }
        Ok(Self {
            mu: m,
            terms,
            gauge,
        })
    }

    /// The central gravitational parameter.
    #[must_use]
    pub fn mu(&self) -> GravParam {
        GravParam::from_km3_per_s2(self.mu).unwrap_or(GravParam::ZERO)
    }

    /// A multi-line description of the model: the Newtonian term and every correction.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut s = format!(
            "test particle about a fixed central body, Newtonian -mu r/r^3 with mu = {} km^3/s^2",
            self.mu
        );
        for t in &self.terms {
            s.push_str("; + ");
            s.push_str(&t.describe());
        }
        s.push_str("; gauge: ");
        s.push_str(self.gauge.label());
        s
    }

    /// The heliocentric `(r, v)` of a typed state, in km and km/s.
    #[must_use]
    pub fn pack(state: &State<F, Heliocentric>) -> ([f64; 3], [f64; 3]) {
        (
            state.position().components().map(Length::km),
            state.velocity().map(Velocity::km_per_s),
        )
    }

    /// Rebuild a typed state from `(r, v)` in km and km/s.
    ///
    /// # Errors
    /// `FA-E0500` if a component is not finite.
    pub fn unpack(r: [f64; 3], v: [f64; 3]) -> Result<State<F, Heliocentric>, ForceError> {
        let bad = |value: f64| ForceError::Invalid {
            name: "state component",
            value,
            valid: "finite",
        };
        let l = |z: f64| Length::from_km(z).map_err(|_| bad(z));
        let w = |z: f64| Velocity::from_km_per_s(z).map_err(|_| bad(z));
        Ok(State::new(
            Position::new(l(r[0])?, l(r[1])?, l(r[2])?),
            [w(v[0])?, w(v[1])?, w(v[2])?],
        ))
    }
}

impl<F: Frame> SecondOrderSystem for TestParticle<F> {
    fn dimension(&self) -> usize {
        3
    }

    fn acceleration(
        &self,
        t: f64,
        x: &[f64],
        v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        let (Ok(r), Ok(w), Some(out)) = (
            <[f64; 3]>::try_from(x),
            <[f64; 3]>::try_from(v),
            a.get_mut(0..3),
        ) else {
            return Err(IntegratorError::Dimension {
                what: "test-particle state",
                got: x.len(),
                expected: 3,
            });
        };
        let r2 = dot(r, r);
        if r2 == 0.0 {
            return Err(ForceError::Singular { i: 0, j: 1, t }.into());
        }
        let k = -self.mu / (r2 * r2.sqrt());
        let mut acc = r.map(|c| k * c);
        for term in &self.terms {
            term.add_acceleration(r, w, &mut acc);
        }
        out.copy_from_slice(&acc);
        Ok(())
    }
}

impl<F: Frame> ForceModel for TestParticle<F> {
    fn gauge(&self) -> Gauge {
        self.gauge
    }
    fn name(&self) -> &'static str {
        "central-body-test-particle"
    }
}

/// `a · b`.
pub(crate) fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

/// `a × b`.
pub(crate) fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1].mul_add(b[2], -a[2] * b[1]),
        a[2].mul_add(b[0], -a[0] * b[2]),
        a[0].mul_add(b[1], -a[1] * b[0]),
    ]
}

/// `|a|`.
pub(crate) fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SchwarzschildPpn, SrKepler};
    use ferroastro_core::ErrorCode;
    use ferroastro_core::frames::Icrf;

    /// Verifies: NUM-017, PHY-008
    #[test]
    fn terms_of_different_relativistic_gauges_cannot_be_mixed() {
        let mu = GravParam::from_km3_per_s2(1.0).unwrap();
        let c = Velocity::from_km_per_s(10.0).unwrap();
        let sr: Box<dyn CentralTerm<Icrf>> = Box::new(SrKepler::new(mu, c).unwrap());
        let ppn: Box<dyn CentralTerm<Icrf>> =
            Box::new(SchwarzschildPpn::new(mu, c, 1.0, 1.0).unwrap());
        let err = TestParticle::new(mu, vec![sr, ppn]).unwrap_err();
        assert_eq!(err.code(), ErrorCode::ForceModelGaugeMismatch);
        let ok = TestParticle::<Icrf>::new(
            mu,
            vec![Box::new(SchwarzschildPpn::new(mu, c, 1.0, 1.0).unwrap())],
        )
        .unwrap();
        assert_eq!(ok.gauge(), Gauge::PpnIsotropicHarmonic);
        assert!(ok.describe().contains("PPN isotropic/harmonic"));
    }

    /// Verifies: API-002
    #[test]
    fn invalid_inputs_are_coded_errors() {
        assert!(Pole::<Icrf>::from_components([0.0; 3]).is_err());
        let p = Pole::<Icrf>::from_components([0.0, 0.0, 2.0]).unwrap();
        assert!(p.unit()[2] > 1.0 - f64::EPSILON && p.unit()[0].abs() < f64::EPSILON);
        let m =
            TestParticle::<Icrf>::new(GravParam::from_km3_per_s2(1.0).unwrap(), vec![]).unwrap();
        let mut a = [0.0; 3];
        let e = m
            .acceleration(0.0, &[0.0; 3], &[0.0; 3], &mut a)
            .unwrap_err();
        assert_eq!(e.code(), ErrorCode::ForceModelSingular);
        assert!(TestParticle::<Icrf>::new(GravParam::ZERO, vec![]).is_err());
    }
}
