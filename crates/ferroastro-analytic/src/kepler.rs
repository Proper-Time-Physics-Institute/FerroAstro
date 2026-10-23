// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Osculating Keplerian elements of elliptic orbits and their conversion to and from Cartesian
//! states (two-body formulae, e.g. Murray & Dermott 1999, *Solar System Dynamics*, §2.8–2.9).

use crate::AnalyticError;
use crate::vector::{cross, dot, eccentricity_vector, norm};
use ferroastro_core::frames::{Center, Frame, Position, State};
use ferroastro_core::units::{Angle, GravParam, Length, Velocity};
use ferroastro_real::Real;

/// Osculating elements of an elliptic orbit (`0 ≤ e < 1`), angles in the frame of the state,
/// in backend `R` (default `f64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elements<R: Real = f64> {
    /// Semi-major axis.
    pub a: Length<R>,
    /// Eccentricity.
    pub e: R,
    /// Inclination to the frame's xy-plane.
    pub inclination: Angle<R>,
    /// Longitude of the ascending node Ω.
    pub node: Angle<R>,
    /// Argument of periapsis ω.
    pub arg_periapsis: Angle<R>,
    /// Mean anomaly M.
    pub mean_anomaly: Angle<R>,
}

fn domain<R: Real>(what: &'static str, value: R) -> AnalyticError {
    AnalyticError::Domain {
        what,
        value: value.to_f64(),
    }
}

/// Solve Kepler's equation `E − e sin E = M` for the eccentric anomaly (Newton iteration from
/// `E₀ = M + e sin M`, at most 50 iterations, stopping at a change ≤ 4 ulp of the backend).
///
/// # Errors
/// `FA-E0600` if `e` is outside `[0, 1)` or `M` is not finite.
pub fn eccentric_anomaly<R: Real>(mean_anomaly: R, e: R) -> Result<R, AnalyticError> {
    if !(e >= R::ZERO && e < R::ONE) {
        return Err(domain("eccentricity (elliptic orbits only)", e));
    }
    if !mean_anomaly.is_finite() {
        return Err(domain("mean anomaly", mean_anomaly));
    }
    let m = mean_anomaly.rem_euclid(R::TAU);
    let mut big_e = e.mul_add(m.sin(), m);
    let four_eps = R::from_f64(4.0) * R::EPSILON;
    for _ in 0..50 {
        let f = (-e).mul_add(big_e.sin(), big_e) - m;
        let step = f / (-e).mul_add(big_e.cos(), R::ONE);
        big_e -= step;
        if step.abs() <= four_eps * big_e.abs().max(R::ONE) {
            break;
        }
    }
    Ok(big_e)
}

impl<R: Real> Elements<R> {
    /// Build from the longitude form used by the SW92 / JPL "approximate positions" table:
    /// longitude of perihelion `ϖ = Ω + ω` and mean longitude `L = ϖ + M`.
    #[must_use]
    pub fn from_longitudes(
        a: Length<R>,
        e: R,
        inclination: Angle<R>,
        node: Angle<R>,
        long_perihelion: Angle<R>,
        mean_longitude: Angle<R>,
    ) -> Self {
        Self {
            a,
            e,
            inclination,
            node,
            arg_periapsis: long_perihelion - node,
            mean_anomaly: mean_longitude - long_perihelion,
        }
    }

    /// Longitude of periapsis `ϖ = Ω + ω`.
    #[must_use]
    pub fn long_periapsis(&self) -> Angle<R> {
        self.node + self.arg_periapsis
    }

    /// Cartesian state for gravitational parameter `mu` (the sum of the two GMs for a relative
    /// two-body orbit).
    ///
    /// # Errors
    /// `FA-E0600` for `e ∉ [0, 1)`, `a ≤ 0` or `mu ≤ 0`.
    pub fn to_state<F: Frame, C: Center>(
        &self,
        mu: GravParam<R>,
    ) -> Result<State<F, C, R>, AnalyticError> {
        let a = self.a.km();
        let mu = mu.km3_per_s2();
        if a.is_nan() || a <= R::ZERO {
            return Err(domain("semi-major axis (km)", a));
        }
        if mu.is_nan() || mu <= R::ZERO {
            return Err(domain("gravitational parameter (km^3/s^2)", mu));
        }
        let big_e = eccentric_anomaly(self.mean_anomaly.radians(), self.e)?;
        let (se, ce) = big_e.sin_cos();
        let b_over_a = self.e.mul_add(-self.e, R::ONE).sqrt();
        let r = a * (-self.e).mul_add(ce, R::ONE);
        let xp = a * (ce - self.e);
        let yp = a * b_over_a * se;
        let k = (mu * a).sqrt() / r;
        let vxp = -k * se;
        let vyp = k * b_over_a * ce;
        let (so, co) = self.node.radians().sin_cos();
        let (si, ci) = self.inclination.radians().sin_cos();
        let (sw, cw) = self.arg_periapsis.radians().sin_cos();
        // Columns P and Q of R3(−Ω) R1(−i) R3(−ω).
        let p = [
            co.mul_add(cw, -so * sw * ci),
            so.mul_add(cw, co * sw * ci),
            sw * si,
        ];
        let q = [
            (-co).mul_add(sw, -so * cw * ci),
            (-so).mul_add(sw, co * cw * ci),
            cw * si,
        ];
        let len = |z: R| Length::from_km(z).map_err(|_| domain("position component", z));
        let vel = |z: R| Velocity::from_km_per_s(z).map_err(|_| domain("velocity component", z));
        let comp = |u: [R; 3], s: R, t: R| -> [R; 3] {
            [
                u[0].mul_add(s, q[0] * t),
                u[1].mul_add(s, q[1] * t),
                u[2].mul_add(s, q[2] * t),
            ]
        };
        let x = comp(p, xp, yp);
        let v = comp(p, vxp, vyp);
        Ok(State::new(
            Position::new(len(x[0])?, len(x[1])?, len(x[2])?),
            [vel(v[0])?, vel(v[1])?, vel(v[2])?],
        ))
    }

    /// Osculating elements of a Cartesian state for gravitational parameter `mu`.
    ///
    /// # Errors
    /// `FA-E0600` if the orbit is not elliptic, the state is degenerate (zero angular momentum)
    /// or `mu ≤ 0`.
    pub fn from_state<F: Frame, C: Center>(
        state: &State<F, C, R>,
        mu: GravParam<R>,
    ) -> Result<Self, AnalyticError> {
        let r = state.position().components().map(Length::km);
        let v = state.velocity().map(Velocity::km_per_s);
        Self::from_vectors(r, v, mu.km3_per_s2())
    }

    /// As [`Elements::from_state`], from raw km and km/s vectors (integrator output).
    ///
    /// # Errors
    /// As [`Elements::from_state`].
    pub fn from_vectors(r: [R; 3], v: [R; 3], mu: R) -> Result<Self, AnalyticError> {
        if mu.is_nan() || mu <= R::ZERO {
            return Err(domain("gravitational parameter (km^3/s^2)", mu));
        }
        let rn = norm(r);
        let h = cross(r, v);
        let hn = norm(h);
        if rn.is_nan() || hn.is_nan() || rn <= R::ZERO || hn <= R::ZERO {
            return Err(domain("angular momentum (degenerate orbit)", hn));
        }
        let ev = eccentricity_vector(r, v, mu);
        let e = norm(ev);
        let inv_a = R::TWO / rn - dot(v, v) / mu;
        if inv_a.is_nan() || inv_a <= R::ZERO || e >= R::ONE {
            return Err(domain("eccentricity (elliptic orbits only)", e));
        }
        let a = R::ONE / inv_a;
        let inclination = h[0].hypot(h[1]).atan2(h[2]);
        let node = h[0].atan2(-h[1]).rem_euclid(R::TAU);
        // Unit vectors of the node line and of the in-plane normal to it.
        let n_hat = [node.cos(), node.sin(), R::ZERO];
        let m_hat = cross([h[0] / hn, h[1] / hn, h[2] / hn], n_hat);
        let arg_periapsis = dot(ev, m_hat).atan2(dot(ev, n_hat)).rem_euclid(R::TAU);
        // True anomaly from the eccentricity vector and the in-plane normal to it.
        let e_hat = [ev[0] / e, ev[1] / e, ev[2] / e];
        let f_hat = cross([h[0] / hn, h[1] / hn, h[2] / hn], e_hat);
        let nu = dot(r, f_hat).atan2(dot(r, e_hat));
        let (sn, cn) = nu.sin_cos();
        let big_e = (e.mul_add(-e, R::ONE).sqrt() * sn).atan2(e + cn);
        let mean_anomaly = (-e).mul_add(big_e.sin(), big_e).rem_euclid(R::TAU);
        let ang = |z: R| Angle::from_radians(z).map_err(|_| domain("angle", z));
        Ok(Self {
            a: Length::from_km(a).map_err(|_| domain("semi-major axis (km)", a))?,
            e,
            inclination: ang(inclination)?,
            node: ang(node)?,
            arg_periapsis: ang(arg_periapsis)?,
            mean_anomaly: ang(mean_anomaly)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::frames::{EclipticJ2000, Heliocentric};

    /// Dimensionless test orbit (GM = 1, a = 1); angles are arbitrary small integers in radians.
    fn orbit(e: f64) -> Elements {
        let r = |z: f64| Angle::from_radians(z).unwrap();
        Elements {
            a: Length::from_km(1.0).unwrap(),
            e,
            inclination: r(0.5),
            node: r(2.0),
            arg_periapsis: r(1.0),
            mean_anomaly: r(3.0),
        }
    }

    /// Elements -> state -> elements is the identity to round-off.
    #[test]
    fn elements_state_round_trip() {
        let mu = GravParam::from_km3_per_s2(1.0).unwrap();
        for e in [0.0, 0.25, 0.5] {
            let el = orbit(e);
            let s: State<EclipticJ2000, Heliocentric> = el.to_state(mu).unwrap();
            let back = Elements::from_state(&s, mu).unwrap();
            let tol = 64.0 * f64::EPSILON; // lint: allow-literal(round-off allowance in ulps)
            assert!((back.a.km() - 1.0).abs() < tol, "a {e}");
            assert!((back.e - e).abs() < tol, "e {e}");
            assert!((back.inclination.radians() - 0.5).abs() < tol);
            assert!((back.node.radians() - 2.0).abs() < tol);
            if e > 0.0 {
                assert!(
                    (back.arg_periapsis.radians() - 1.0).abs() < tol / e,
                    "w {e}"
                );
                assert!((back.mean_anomaly.radians() - 3.0).abs() < tol / e, "M {e}");
            }
        }
        assert_eq!(
            orbit(1.0)
                .to_state::<EclipticJ2000, Heliocentric>(mu)
                .unwrap_err()
                .code(),
            ferroastro_core::ErrorCode::OrbitDomain
        );
    }

    /// The same round trip in double-double closes to double-double round-off (DEP-007), and
    /// its f64 rounding equals the f64 result to f64 round-off.
    ///
    /// Verifies: DEP-007, NUM-006
    #[test]
    fn elements_state_round_trip_double_double() {
        use ferroastro_real::{DoubleDouble as Dd, Real};
        let d = Dd::from_f64;
        let mu = GravParam::from_km3_per_s2(d(1.0)).unwrap();
        let r = |z: f64| Angle::from_radians(d(z)).unwrap();
        for e in [0.25, 0.5] {
            let el = Elements {
                a: Length::from_km(d(1.0)).unwrap(),
                e: d(e),
                inclination: r(0.5),
                node: r(2.0),
                arg_periapsis: r(1.0),
                mean_anomaly: r(3.0),
            };
            let s: State<EclipticJ2000, Heliocentric, Dd> = el.to_state(mu).unwrap();
            let back = Elements::from_state(&s, mu).unwrap();
            let tol = Dd::EPSILON.scale(64.0) / d(e); // lint: allow-literal(round-off allowance in ulps)
            assert!((back.a.km() - d(1.0)).abs() < tol, "a {e}");
            assert!((back.e - d(e)).abs() < tol, "e {e}");
            assert!((back.arg_periapsis.radians() - d(1.0)).abs() < tol, "w {e}");
            assert!((back.mean_anomaly.radians() - d(3.0)).abs() < tol, "M {e}");
            let f: State<EclipticJ2000, Heliocentric> = orbit(e)
                .to_state(GravParam::from_km3_per_s2(1.0).unwrap())
                .unwrap();
            for (x, y) in s
                .position()
                .components()
                .iter()
                .zip(f.position().components())
            {
                assert!((x.km().to_f64() - y.km()).abs() < 8.0 * f64::EPSILON);
            }
        }
    }

    /// Kepler's equation is solved to a few ulp.
    #[test]
    fn kepler_equation_is_solved() {
        for (m, e) in [(0.25, 0.5), (3.0, 0.25), (6.0, 0.5)] {
            let big_e = eccentric_anomaly(m, e).unwrap();
            assert!(((-e).mul_add(big_e.sin(), big_e) - m).abs() < 8.0 * f64::EPSILON);
        }
    }
}
