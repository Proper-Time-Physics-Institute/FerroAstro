// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Relativistic correction terms for a test particle (Journal #4 §4 R2, R3, R3b; PHY-009).
//!
//! Every term adds to the Newtonian `−μr/r³` of [`crate::TestParticle`]. Notation: `r = |r|`,
//! `n = r/r`, `ṙ = n·v`, `U = μ/r`, `ε = 1/c²`.
//!
//! * [`SrKepler`] — R2, exact: `a = −μ/(γr³)[r − (r·v)v/c²]`, from `d(γv)/dt = −μr/r³`; the
//!   correction to Newton is `(μ/r³)[(1 − 1/γ) r + (r·v) v/(γc²)]`, evaluated without
//!   cancellation (`1 − 1/γ = β²/(1 + √(1 − β²))`).
//! * [`SchwarzschildPpn`] — R3, PPN isotropic/harmonic gauge (same form as the IERS 2010 Ch. 10
//!   Schwarzschild term; equation number OI-36):
//!   `δa = μ/(c²r³)[(2(β+γ)μ/r − γv²) r + 2(1+γ)(r·v) v]`.
//! * [`PnLagrangian`] — the general test-particle Lagrangian
//!   `L = ½v² + U + ε[A v⁴ + B U v² + C U² + D U (n·v)²]`. Its Euler–Lagrange equations, reduced
//!   to first order in `ε` by substituting the Newtonian acceleration into the `O(ε)` terms, give
//!   `δa = (εμ/r²)[((4A − B − 2D)v² + 2(B − C + D)U + 3D ṙ²) n + (8A + 2B) ṙ v]`
//!   (derived by hand and checked symbolically by `tools/pn_lagrangian.py`). With the PPN
//!   coefficients `A, B, C, D = 1/8, γ + ½, ½ − β, 0` (from `L = −c² dτ/dt` of the PPN metric) it
//!   is identical to R3; with `1/8, ½, ½, 1` it is the 1PN Schwarzschild equation in areal
//!   coordinates.
//! * [`LenseThirring`] — R3b, IERS-style form with `μJ = G·S`:
//!   `δa = (1+γ) GS/(c²r³)[(3/r²)(r×v)(r·ŝ) + v×ŝ]`. Its typography relative to the DE440 and
//!   `INPOP21a` equations is open item OI-35; the rung tests check it against Park et al. 2017
//!   Eq. 2 instead.

use crate::central::{CentralTerm, Pole, cross, dot};
use crate::{ForceError, Gauge};
use ferroastro_core::frames::Frame;
use ferroastro_core::units::{GravParam, SpinGm, Velocity};

fn positive(name: &'static str, value: f64) -> Result<f64, ForceError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(ForceError::Invalid {
            name,
            value,
            valid: "finite and > 0",
        })
    }
}

fn finite(name: &'static str, value: f64) -> Result<f64, ForceError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ForceError::Invalid {
            name,
            value,
            valid: "finite",
        })
    }
}

/// Rung SR-a: special-relativistic Kepler motion, `d(γv)/dt = −μr/r³` (Journal #4 R2).
///
/// Lagrangian `L = −mc²√(1 − v²/c²) + μm/r`, Hamiltonian `H = γmc² − μm/r` (NUM-016). This is
/// **one** model of "special relativity plus Newtonian gravity"; other couplings give other
/// fractions of the GR advance (e.g. 1/3 with a relativistic potential energy, Lemmon &
/// Mondragon, arXiv:1012.5438), so the SR share of the advance is model dependent. For speeds
/// `v ≥ c` the acceleration is NaN and the integrator stops with `FA-E0400`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrKepler {
    mu: f64,
    c2: f64,
}

impl SrKepler {
    /// Build with the central `μ` and the speed of light.
    ///
    /// # Errors
    /// `FA-E0500` if `μ` or `c` is not positive and finite.
    pub fn new(mu: GravParam, c: Velocity) -> Result<Self, ForceError> {
        let cc = positive("c (km/s)", c.km_per_s())?;
        Ok(Self {
            mu: positive("GM (km^3/s^2)", mu.km3_per_s2())?,
            c2: cc * cc,
        })
    }
}

impl<F: Frame> CentralTerm<F> for SrKepler {
    fn add_acceleration(&self, r: [f64; 3], v: [f64; 3], a: &mut [f64; 3]) {
        let b2 = dot(v, v) / self.c2;
        let inv_gamma = (1.0 - b2).sqrt();
        let one_minus_inv_gamma = b2 / (1.0 + inv_gamma);
        let r2 = dot(r, r);
        let k = self.mu / (r2 * r2.sqrt());
        let kv = k * dot(r, v) * inv_gamma / self.c2;
        for ((ai, ri), vi) in a.iter_mut().zip(r).zip(v) {
            *ai += k.mul_add(one_minus_inv_gamma * ri, kv * vi);
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::SpecialRelativistic
    }
    fn describe(&self) -> String {
        "SR-Kepler (Journal #4 R2): d(gamma v)/dt = -mu r/r^3, H = gamma m c^2 - mu m/r".into()
    }
}

/// Rung GR-SCHW / PPN: the 1PN Schwarzschild field with PPN β, γ in isotropic/harmonic gauge
/// (Journal #4 R3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchwarzschildPpn {
    mu: f64,
    c2: f64,
    beta: f64,
    gamma: f64,
}

impl SchwarzschildPpn {
    /// Build with the central `μ`, the speed of light and the PPN parameters.
    ///
    /// # Errors
    /// `FA-E0500` if `μ` or `c` is not positive and finite, or β, γ are not finite.
    pub fn new(mu: GravParam, c: Velocity, beta: f64, gamma: f64) -> Result<Self, ForceError> {
        let cc = positive("c (km/s)", c.km_per_s())?;
        Ok(Self {
            mu: positive("GM (km^3/s^2)", mu.km3_per_s2())?,
            c2: cc * cc,
            beta: finite("PPN beta", beta)?,
            gamma: finite("PPN gamma", gamma)?,
        })
    }
}

impl<F: Frame> CentralTerm<F> for SchwarzschildPpn {
    fn add_acceleration(&self, r: [f64; 3], v: [f64; 3], a: &mut [f64; 3]) {
        let r2 = dot(r, r);
        let rn = r2.sqrt();
        let k = self.mu / (self.c2 * r2 * rn);
        let radial =
            (2.0 * (self.beta + self.gamma)).mul_add(self.mu / rn, -self.gamma * dot(v, v));
        let along = 2.0 * (1.0 + self.gamma) * dot(r, v);
        for ((ai, ri), vi) in a.iter_mut().zip(r).zip(v) {
            *ai += k * radial.mul_add(ri, along * vi);
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::PpnIsotropicHarmonic
    }
    fn describe(&self) -> String {
        format!(
            "1PN Schwarzschild PPN (Journal #4 R3), beta = {}, gamma = {}",
            self.beta, self.gamma
        )
    }
}

/// Coefficients `A, B, C, D` of `ε[A v⁴ + B U v² + C U² + D U (n·v)²]` (PHY-009).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PnCoefficients {
    /// Coefficient of `v⁴`.
    pub a: f64,
    /// Coefficient of `U v²`.
    pub b: f64,
    /// Coefficient of `U²`.
    pub c: f64,
    /// Coefficient of `U (n·v)²`.
    pub d: f64,
}

impl PnCoefficients {
    /// PPN metric, isotropic/harmonic gauge: `1/8, γ + ½, ½ − β, 0`.
    #[must_use]
    pub fn ppn(beta: f64, gamma: f64) -> Self {
        Self {
            a: 0.125,
            b: gamma + 0.5,
            c: 0.5 - beta,
            d: 0.0,
        }
    }

    /// Schwarzschild metric in areal (standard) coordinates: `1/8, ½, ½, 1`.
    #[must_use]
    pub const fn schwarzschild_areal() -> Self {
        Self {
            a: 0.125,
            b: 0.5,
            c: 0.5,
            d: 1.0,
        }
    }

    /// Keep only the velocity-dependent terms (`C = 0`), as rung SR-b does.
    #[must_use]
    pub const fn velocity_only(self) -> Self {
        Self { c: 0.0, ..self }
    }

    /// The 1PN per-orbit advance in units of `πμ/(c²p)`: `8A + 4B + 2C + 2D` (red-team §1,
    /// Delaunay average). For the PPN coefficients it is `2(2 + 2γ − β)`.
    #[must_use]
    pub fn advance_units(self) -> f64 {
        8.0f64.mul_add(
            self.a,
            4.0f64.mul_add(self.b, 2.0f64.mul_add(self.c, 2.0 * self.d)),
        )
    }
}

/// The general 1PN test-particle Lagrangian (PHY-009), reduced to first order in `1/c²`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PnLagrangian {
    mu: f64,
    inv_c2: f64,
    k: PnCoefficients,
    gauge: Gauge,
    label: &'static str,
}

impl PnLagrangian {
    fn build(
        mu: GravParam,
        c: Velocity,
        k: PnCoefficients,
        gauge: Gauge,
        label: &'static str,
    ) -> Result<Self, ForceError> {
        let cc = positive("c (km/s)", c.km_per_s())?;
        for (name, v) in [
            ("coefficient A", k.a),
            ("coefficient B", k.b),
            ("coefficient C", k.c),
            ("coefficient D", k.d),
        ] {
            finite(name, v)?;
        }
        Ok(Self {
            mu: positive("GM (km^3/s^2)", mu.km3_per_s2())?,
            inv_c2: 1.0 / (cc * cc),
            k,
            gauge,
            label,
        })
    }

    /// Full PPN Lagrangian in isotropic/harmonic gauge (equivalent to [`SchwarzschildPpn`]).
    ///
    /// # Errors
    /// `FA-E0500` for invalid `μ`, `c`, β or γ.
    pub fn ppn(mu: GravParam, c: Velocity, beta: f64, gamma: f64) -> Result<Self, ForceError> {
        Self::build(
            mu,
            c,
            PnCoefficients::ppn(finite("PPN beta", beta)?, finite("PPN gamma", gamma)?),
            Gauge::PpnIsotropicHarmonic,
            "PPN Lagrangian",
        )
    }

    /// Rung SR-b in isotropic/harmonic gauge: the velocity-dependent terms `A v⁴ + B U v²` of
    /// the PPN Lagrangian with `γ = 1` (`A = 1/8`, `B = 3/2`). The split is **not an
    /// observable**: it changes with the gauge (see [`PnLagrangian::sr_b_areal`]).
    ///
    /// # Errors
    /// `FA-E0500` for invalid `μ` or `c`.
    pub fn sr_b_harmonic(mu: GravParam, c: Velocity) -> Result<Self, ForceError> {
        Self::build(
            mu,
            c,
            PnCoefficients::ppn(1.0, 1.0).velocity_only(),
            Gauge::PpnIsotropicHarmonic,
            "SR-b velocity-only terms",
        )
    }

    /// Rung SR-b in areal Schwarzschild gauge (pedagogical only; NUM-017 exemption): the
    /// velocity-dependent terms `A v⁴ + B U v² + D U (n·v)²` with `1/8, ½, 1`.
    ///
    /// # Errors
    /// `FA-E0500` for invalid `μ` or `c`.
    pub fn sr_b_areal(mu: GravParam, c: Velocity) -> Result<Self, ForceError> {
        Self::build(
            mu,
            c,
            PnCoefficients::schwarzschild_areal().velocity_only(),
            Gauge::SchwarzschildAreal,
            "SR-b velocity-only terms",
        )
    }

    /// Full Schwarzschild Lagrangian in areal coordinates (pedagogical only; NUM-017 exemption).
    ///
    /// # Errors
    /// `FA-E0500` for invalid `μ` or `c`.
    pub fn schwarzschild_areal(mu: GravParam, c: Velocity) -> Result<Self, ForceError> {
        Self::build(
            mu,
            c,
            PnCoefficients::schwarzschild_areal(),
            Gauge::SchwarzschildAreal,
            "Schwarzschild Lagrangian",
        )
    }

    /// Arbitrary coefficients for the PHY-009 coefficient study; gauge
    /// [`Gauge::GenericPnLagrangian`].
    ///
    /// # Errors
    /// `FA-E0500` for invalid `μ`, `c` or a non-finite coefficient.
    pub fn generic(mu: GravParam, c: Velocity, k: PnCoefficients) -> Result<Self, ForceError> {
        Self::build(
            mu,
            c,
            k,
            Gauge::GenericPnLagrangian,
            "generic 1PN Lagrangian",
        )
    }

    /// The coefficients.
    #[must_use]
    pub const fn coefficients(&self) -> PnCoefficients {
        self.k
    }
}

impl<F: Frame> CentralTerm<F> for PnLagrangian {
    fn add_acceleration(&self, r: [f64; 3], v: [f64; 3], a: &mut [f64; 3]) {
        let PnCoefficients { a: ca, b, c, d } = self.k;
        let r2 = dot(r, r);
        let rn = r2.sqrt();
        let rdot = dot(r, v) / rn;
        let u = self.mu / rn;
        let radial = 2.0f64.mul_add(-d, 4.0f64.mul_add(ca, -b)).mul_add(
            dot(v, v),
            (2.0 * (b - c + d)).mul_add(u, 3.0 * d * rdot * rdot),
        );
        let along = 8.0f64.mul_add(ca, 2.0 * b) * rdot;
        let k = self.inv_c2 * self.mu / r2;
        for ((ai, ri), vi) in a.iter_mut().zip(r).zip(v) {
            *ai += k * radial.mul_add(ri / rn, along * vi);
        }
    }
    fn gauge(&self) -> Gauge {
        self.gauge
    }
    fn describe(&self) -> String {
        let k = self.k;
        format!(
            "{} (PHY-009 form), A = {}, B = {}, C = {}, D = {}",
            self.label, k.a, k.b, k.c, k.d
        )
    }
}

/// Rung LT: the central body's gravitomagnetic (Lense–Thirring) field (Journal #4 R3b).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LenseThirring<F: Frame> {
    gs: f64,
    c2: f64,
    gamma: f64,
    pole: Pole<F>,
}

impl<F: Frame> LenseThirring<F> {
    /// Build from `G·S`, the speed of light, PPN γ and the spin axis.
    ///
    /// # Errors
    /// `FA-E0500` if `G·S` is negative or not finite, `c` is not positive, or γ is not finite.
    pub fn new(gs: SpinGm, c: Velocity, gamma: f64, pole: Pole<F>) -> Result<Self, ForceError> {
        let g = gs.km5_per_s3();
        if !(g.is_finite() && g >= 0.0) {
            return Err(ForceError::Invalid {
                name: "G S (km^5/s^3)",
                value: g,
                valid: "finite and >= 0",
            });
        }
        let cc = positive("c (km/s)", c.km_per_s())?;
        Ok(Self {
            gs: g,
            c2: cc * cc,
            gamma: finite("PPN gamma", gamma)?,
            pole,
        })
    }
}

impl<F: Frame> CentralTerm<F> for LenseThirring<F> {
    fn add_acceleration(&self, r: [f64; 3], v: [f64; 3], a: &mut [f64; 3]) {
        let s = self.pole.unit();
        let r2 = dot(r, r);
        let k = (1.0 + self.gamma) * self.gs / (self.c2 * r2 * r2.sqrt());
        let h = cross(r, v);
        let vxs = cross(v, s);
        let w = 3.0 * dot(r, s) / r2;
        for ((ai, hi), wi) in a.iter_mut().zip(h).zip(vxs) {
            *ai += k * w.mul_add(hi, wi);
        }
    }
    fn gauge(&self) -> Gauge {
        Gauge::PpnIsotropicHarmonic
    }
    fn describe(&self) -> String {
        let s = self.pole.unit();
        format!(
            "Lense-Thirring (Journal #4 R3b), G S = {:e} km^5/s^3, gamma = {}, spin axis [{}, {}, {}]",
            self.gs, self.gamma, s[0], s[1], s[2]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferroastro_core::frames::Icrf;

    fn gm(v: f64) -> GravParam {
        GravParam::from_km3_per_s2(v).unwrap()
    }
    fn speed(v: f64) -> Velocity {
        Velocity::from_km_per_s(v).unwrap()
    }
    fn accel<T: CentralTerm<Icrf>>(t: &T, r: [f64; 3], v: [f64; 3]) -> [f64; 3] {
        let mut a = [0.0; 3];
        t.add_acceleration(r, v, &mut a);
        a
    }
    /// Deterministic, generic sample states (dimensionless units: μ = 1, c = 7).
    fn samples() -> Vec<([f64; 3], [f64; 3])> {
        (1..=12)
            .map(|k| {
                let x = f64::from(k);
                let r = [x.sin() + 1.5, 0.7 * x.cos(), 0.3 * (2.0 * x).sin()]; // lint: allow-literal(arbitrary dimensionless sample geometry)
                let v = [0.4 * x.cos(), 0.05f64.mul_add(-x, 0.9), 0.2 * x.sin()]; // lint: allow-literal(arbitrary dimensionless sample geometry)
                (r, v)
            })
            .collect()
    }

    /// With the PPN coefficients the reduced Lagrangian equations equal R3 for any β, γ
    /// (two independent implementations of the same 1PN field; NUM-017 gauge statement).
    ///
    /// Verifies: NUM-017, PHY-009
    #[test]
    fn ppn_lagrangian_equals_schwarzschild_ppn_term() {
        // Arbitrary PPN test points, not physical values.
        let points = [(1.0, 1.0), (1.3, 0.6), (0.2, 2.5)]; // lint: allow-literal(arbitrary PPN test points)
        for (beta, gamma) in points {
            let l = PnLagrangian::ppn(gm(1.0), speed(7.0), beta, gamma).unwrap();
            let s = SchwarzschildPpn::new(gm(1.0), speed(7.0), beta, gamma).unwrap();
            for (r, v) in samples() {
                let (p, q) = (accel(&l, r, v), accel(&s, r, v));
                for k in 0..3 {
                    let scale = p[k].abs().max(q[k].abs()).max(f64::MIN_POSITIVE);
                    assert!(
                        (p[k] - q[k]).abs() <= 64.0 * f64::EPSILON * scale.max(1e-3), // lint: allow-literal(round-off bound in ulps and a floor for near-zero components)
                        "beta {beta} gamma {gamma}: {p:?} vs {q:?}"
                    );
                }
            }
        }
    }

    /// Areal Schwarzschild coefficients (1/8, ½, ½, 1) give the closed-form 1PN areal-gauge
    /// equation `δa = μ/(c²r²)[(2U − 2v² + 3ṙ²) n + 2ṙ v]` component by component. The
    /// `3ṙ² n` term has zero secular effect on the perihelion (its orbit average vanishes), so
    /// only an acceleration-level check like this one constrains it (WP4 review; the closed form
    /// was re-derived independently with sympy from the areal-metric Lagrangian).
    ///
    /// Verifies: PHY-009, PHY-008
    #[test]
    fn areal_lagrangian_matches_closed_form_areal_eom() {
        let c = 7.0; // lint: allow-literal(dimensionless test speed of light, as samples())
        let l = PnLagrangian::schwarzschild_areal(gm(1.0), speed(c)).unwrap();
        for (r, v) in samples() {
            let p = accel(&l, r, v);
            let rn = dot(r, r).sqrt();
            let rdot = dot(r, v) / rn;
            let u = 1.0 / rn;
            // lint: allow-literal(closed-form areal 1PN coefficients 2, 2, 3, 2)
            let radial = (3.0 * rdot).mul_add(rdot, 2.0f64.mul_add(u, -2.0 * dot(v, v)));
            let along = 2.0 * rdot; // lint: allow-literal(closed-form areal 1PN coefficient)
            for k in 0..3 {
                let want = radial.mul_add(r[k] / rn, along * v[k]) / (c * c * rn * rn);
                assert!(
                    (p[k] - want).abs() <= 64.0 * f64::EPSILON * want.abs().max(1e-3), // lint: allow-literal(round-off bound in ulps and a floor)
                    "component {k}: {p:?} vs {want}"
                );
            }
        }
    }

    /// The SR correction plus Newton equals the closed form `−μ/(γr³)[r − (r·v)v/c²]`.
    ///
    /// Verifies: PHY-007, NUM-015
    #[test]
    fn sr_kepler_matches_closed_form() {
        let c = 7.0;
        let sr = SrKepler::new(gm(1.0), speed(c)).unwrap();
        for (r, v) in samples() {
            let mut a = accel(&sr, r, v);
            let r2 = dot(r, r);
            let r3 = r2 * r2.sqrt();
            for k in 0..3 {
                a[k] -= r[k] / r3;
            }
            let g = 1.0 / (1.0 - dot(v, v) / (c * c)).sqrt();
            let rv = dot(r, v);
            for k in 0..3 {
                let want = -(r[k] - rv * v[k] / (c * c)) / (g * r3);
                assert!((a[k] - want).abs() <= 16.0 * f64::EPSILON * want.abs().max(1e-3)); // lint: allow-literal(round-off bound in ulps and a floor)
            }
        }
    }

    /// Advance units: PPN gives 2(2+2γ−β) (6 at β = γ = 1), areal 6, SR-b 7 and 5.
    ///
    /// Verifies: PHY-008, PHY-009
    #[test]
    fn advance_units_of_the_named_coefficient_sets() {
        let eq = |x: f64, y: f64| (x - y).abs() <= 8.0 * f64::EPSILON;
        assert!(eq(PnCoefficients::ppn(1.0, 1.0).advance_units(), 6.0));
        assert!(eq(
            PnCoefficients::schwarzschild_areal().advance_units(),
            6.0
        ));
        assert!(eq(
            PnCoefficients::ppn(1.0, 1.0)
                .velocity_only()
                .advance_units(),
            7.0
        ));
        assert!(eq(
            PnCoefficients::schwarzschild_areal()
                .velocity_only()
                .advance_units(),
            5.0
        ));
        let sr_b = PnLagrangian::sr_b_areal(gm(1.0), speed(7.0)).unwrap();
        assert_eq!(
            <PnLagrangian as CentralTerm<Icrf>>::gauge(&sr_b),
            Gauge::SchwarzschildAreal
        );
    }

    /// In the equatorial plane the LT acceleration is in-plane and along `v × ŝ`.
    ///
    /// Verifies: PHY-011
    #[test]
    fn lense_thirring_in_equatorial_plane() {
        let pole = Pole::<Icrf>::from_components([0.0, 0.0, 1.0]).unwrap();
        let lt = LenseThirring::new(SpinGm::from_km5_per_s3(2.0).unwrap(), speed(1.0), 1.0, pole)
            .unwrap();
        // r = x̂, v = ŷ: v × ẑ = x̂, so a = (1+γ)·GS/(c² r³)·x̂ = 4 x̂ (outward for a prograde orbit).
        let a = accel(&lt, [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert!((a[0] - 4.0).abs() < 4.0 * f64::EPSILON && a[1].abs() < f64::EPSILON);
        assert!(a[2].abs() < f64::EPSILON);
    }
}
