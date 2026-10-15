// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Newtonian point-mass gravity: `ẍᵢ = Σⱼ≠ᵢ GMⱼ (xⱼ − xᵢ)/|xⱼ − xᵢ|³` (rungs N-2B, N-SUM).

use crate::{ForceError, ForceModel, Gauge};
use ferroastro_core::frames::{Center, Frame, Position, State};
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_integrators::{IntegratorError, SecondOrderSystem};
use std::marker::PhantomData;

/// Newtonian mutual gravity of `n` point masses in frame `F` about centre `C` (normally the
/// barycentre). Body `i` occupies coordinates `3i..3i+3`.
#[derive(Debug, Clone)]
pub struct NewtonianPointMasses<F: Frame, C: Center> {
    gm: Vec<f64>,
    _tag: PhantomData<(F, C)>,
}

impl<F: Frame, C: Center> NewtonianPointMasses<F, C> {
    /// Build from the bodies' gravitational parameters (`GM ≥ 0`; zero is a test particle).
    ///
    /// # Errors
    /// `FA-E0500` if no body is given or a GM is negative.
    pub fn new(gms: &[GravParam]) -> Result<Self, ForceError> {
        if gms.is_empty() {
            return Err(ForceError::Invalid {
                name: "number of bodies",
                value: 0.0,
                valid: ">= 1",
            });
        }
        if let Some(bad) = gms.iter().find(|g| g.km3_per_s2() < 0.0) {
            return Err(ForceError::Invalid {
                name: "GM (km^3/s^2)",
                value: bad.km3_per_s2(),
                valid: ">= 0",
            });
        }
        Ok(Self {
            gm: gms.iter().map(|g| g.km3_per_s2()).collect(),
            _tag: PhantomData,
        })
    }

    /// Number of bodies.
    #[must_use]
    pub fn bodies(&self) -> usize {
        self.gm.len()
    }

    /// Flatten typed states into the integrator's `(x, v)` in km and km/s.
    ///
    /// # Errors
    /// `FA-E0500` if the number of states differs from the number of bodies.
    pub fn pack(&self, states: &[State<F, C>]) -> Result<(Vec<f64>, Vec<f64>), ForceError> {
        if states.len() != self.gm.len() {
            return Err(ForceError::Invalid {
                name: "number of states",
                value: f64::from(u32::try_from(states.len()).unwrap_or(u32::MAX)),
                valid: "equal to the number of bodies",
            });
        }
        let mut x = Vec::with_capacity(3 * states.len());
        let mut v = Vec::with_capacity(3 * states.len());
        for s in states {
            x.extend(s.position().components().map(Length::km));
            v.extend(s.velocity().map(Velocity::km_per_s));
        }
        Ok((x, v))
    }

    /// Rebuild typed states from the integrator's `(x, v)`.
    ///
    /// # Errors
    /// `FA-E0001` (via [`ferroastro_core::Error`]) if a value is not finite, or `FA-E0500` on a
    /// length mismatch.
    pub fn unpack(&self, x: &[f64], v: &[f64]) -> Result<Vec<State<F, C>>, ForceError> {
        let n = self.gm.len();
        if x.len() != 3 * n || v.len() != 3 * n {
            return Err(ForceError::Invalid {
                name: "state length",
                value: f64::from(u32::try_from(x.len()).unwrap_or(u32::MAX)),
                valid: "3 x number of bodies",
            });
        }
        let nonfinite = |value: f64| ForceError::Invalid {
            name: "state component",
            value,
            valid: "finite",
        };
        x.chunks_exact(3)
            .zip(v.chunks_exact(3))
            .map(|(p, q)| {
                let l = |z: f64| Length::from_km(z).map_err(|_| nonfinite(z));
                let w = |z: f64| Velocity::from_km_per_s(z).map_err(|_| nonfinite(z));
                match (p, q) {
                    ([px, py, pz], [vx, vy, vz]) => Ok(State::new(
                        Position::new(l(*px)?, l(*py)?, l(*pz)?),
                        [w(*vx)?, w(*vy)?, w(*vz)?],
                    )),
                    _ => Err(nonfinite(f64::NAN)),
                }
            })
            .collect()
    }

    /// Total energy divided by G, `Σ ½ GMᵢ vᵢ² − Σᵢ<ⱼ GMᵢ GMⱼ / rᵢⱼ` (km⁵/s⁴), a conserved
    /// quantity for the round-off diagnostics (NUM-023).
    #[must_use]
    pub fn energy_over_g(&self, x: &[f64], v: &[f64]) -> f64 {
        let mut kinetic = 0.0;
        let mut potential = 0.0;
        let bodies: Vec<Body<'_>> = self
            .gm
            .iter()
            .zip(x.chunks_exact(3).zip(v.chunks_exact(3)))
            .collect();
        for (k, &(gi, (xi, vi))) in bodies.iter().enumerate() {
            let v2: f64 = vi.iter().map(|z| z * z).sum();
            kinetic += 0.5 * gi * v2;
            for &(gj, (xj, _)) in bodies.iter().skip(k + 1) {
                let r = distance(xi, xj);
                potential += gi * gj / r;
            }
        }
        kinetic - potential
    }

    /// Total angular momentum divided by G, `Σ GMᵢ xᵢ × vᵢ` (km⁵/s³).
    #[must_use]
    pub fn angular_momentum_over_g(&self, x: &[f64], v: &[f64]) -> [f64; 3] {
        let mut l = [0.0; 3];
        for (gi, (xi, vi)) in self.gm.iter().zip(x.chunks_exact(3).zip(v.chunks_exact(3))) {
            if let ([a, b, c], [d, e, f]) = (xi, vi) {
                l[0] += gi * b.mul_add(*f, -c * e);
                l[1] += gi * c.mul_add(*d, -a * f);
                l[2] += gi * a.mul_add(*e, -b * d);
            }
        }
        l
    }
}

/// One body's `(GM, (position, velocity))` view into the flat state.
type Body<'a> = (&'a f64, (&'a [f64], &'a [f64]));

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| (p - q) * (p - q))
        .sum::<f64>()
        .sqrt()
}

impl<F: Frame, C: Center> SecondOrderSystem for NewtonianPointMasses<F, C> {
    fn dimension(&self) -> usize {
        3 * self.gm.len()
    }

    /// Pairwise evaluation: each pair is visited once and its terms are applied with opposite
    /// signs, so the force sum is antisymmetric (momentum is conserved to round-off).
    #[expect(
        clippy::indexing_slicing,
        reason = "x and a have length 3n (checked by the integrator); i, j < n"
    )]
    fn acceleration(
        &self,
        t: f64,
        x: &[f64],
        _v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError> {
        a.fill(0.0);
        let n = self.gm.len();
        for i in 0..n {
            for j in (i + 1)..n {
                let d = [
                    x[3 * j] - x[3 * i],
                    x[3 * j + 1] - x[3 * i + 1],
                    x[3 * j + 2] - x[3 * i + 2],
                ];
                let r2 = d[0].mul_add(d[0], d[1].mul_add(d[1], d[2] * d[2]));
                if r2 == 0.0 {
                    return Err(ForceError::Singular { i, j, t }.into());
                }
                let inv_r3 = 1.0 / (r2 * r2.sqrt());
                for k in 0..3 {
                    a[3 * i + k] += self.gm[j] * d[k] * inv_r3;
                    a[3 * j + k] -= self.gm[i] * d[k] * inv_r3;
                }
            }
        }
        Ok(())
    }
}

impl<F: Frame, C: Center> ForceModel for NewtonianPointMasses<F, C> {
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn name(&self) -> &'static str {
        "newtonian-point-masses"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::ErrorCode;
    use ferroastro_core::frames::{Barycentric, Icrf};

    type Model = NewtonianPointMasses<Icrf, Barycentric>;

    fn gm(v: f64) -> GravParam {
        GravParam::from_km3_per_s2(v).unwrap()
    }

    /// Momentum balance: GM-weighted accelerations sum to zero; inverse-square magnitude.
    ///
    /// Verifies: PHY-001
    #[test]
    fn pairwise_forces_balance_and_follow_inverse_square() {
        let m = Model::new(&[gm(3.0), gm(1.0)]).unwrap();
        let x = [0.0, 0.0, 0.0, 2.0, 0.0, 0.0];
        let mut a = [0.0; 6];
        m.acceleration(0.0, &x, &[0.0; 6], &mut a).unwrap();
        // Body 0 is pulled towards +x by GM_1/r^2 = 1/4, body 1 towards -x by 3/4.
        assert!((a[0] - 0.25).abs() < f64::EPSILON);
        assert!((a[3] + 0.75).abs() < f64::EPSILON); // lint: allow-literal(3 x 1/4, exact)
        let balance: f64 = 3.0_f64.mul_add(a[0], a[3]);
        assert!(balance.abs() < f64::EPSILON);
        assert_eq!(m.gauge(), Gauge::Newtonian);
        assert_eq!(m.dimension(), 6);
    }

    /// Verifies: API-002
    #[test]
    fn invalid_and_singular_inputs_are_coded_errors() {
        assert_eq!(
            Model::new(&[]).unwrap_err().code(),
            ErrorCode::ForceModelInvalid
        );
        let m = Model::new(&[gm(1.0), gm(1.0)]).unwrap();
        let mut a = [0.0; 6];
        let e = m
            .acceleration(0.0, &[1.0; 6], &[0.0; 6], &mut a)
            .unwrap_err();
        assert_eq!(e.code(), ErrorCode::ForceModelSingular);
    }

    /// Verifies: API-001
    #[test]
    fn pack_and_unpack_round_trip() {
        let m = Model::new(&[gm(1.0)]).unwrap();
        let l = |z| Length::from_km(z).unwrap();
        let w = |z| Velocity::from_km_per_s(z).unwrap();
        let s = State::new(
            Position::new(l(1.0), l(2.0), l(3.0)),
            [w(4.0), w(5.0), w(6.0)],
        );
        let (x, v) = m.pack(&[s]).unwrap();
        let back = m.unpack(&x, &v).unwrap();
        assert_eq!(back, vec![s]);
        assert!(m.pack(&[s, s]).is_err());
    }
}
