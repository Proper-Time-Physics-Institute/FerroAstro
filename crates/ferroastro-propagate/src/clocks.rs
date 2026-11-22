// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Three-clock diagnostic (PHY-022, draft): along an integrated orbit, accumulate the three
//! "times" that Gill's proper-time Hamiltonian `H₁ = √(c²π² + (mc² + V)²)` separates into once a
//! potential is present (relativistic-dynamics#88 report `REPORT_tau_and_hamiltonian.md` §2.1):
//!
//! | Clock | Rate `d·/dt` | Clock-test reading (report §2.4) |
//! |---|---|---|
//! | metric proper time `s` | `Ω √(1 − v²/c²)` | α = 0 (passes) |
//! | flat special-relativistic `τ_SR` | `√(1 − v²/c²)` | α = −1 (comparison reading, excluded) |
//! | Gill's flow parameter `τ_K` | `√(1 − v²/c²) / Ω` | α = −2 (comparison reading, excluded) |
//!
//! with `Ω = 1 + V/(mc²) = 1 − U` and `U = Σ GMⱼ/(rⱼc²)` the Newtonian potential depth of the
//! other bodies. The three rates satisfy `dτ_SR = Ω dτ_K` and `ds = Ω² dτ_K` exactly; `s` is the
//! arc length of the Bridge A metric `g = Ω²η`, whose `g₀₀ = 1 − 2U + U²` agrees with the static
//! weak-field (1PN) `g₀₀ = 1 − 2U` to `O(c⁻²)`. That is the order the sources state
//! (`tau_observables.md` §5: `ds = √g₀₀ dt/γ`, first order); the clock rates of the two metrics
//! differ at `O(U²)` (`(3/2)x²` on a circular orbit, 4×10⁻²⁰ at GPS). `τ_K` is evaluated
//! pointwise as `mc²/H₁` on the state, which on an `H₁` trajectory is `(mc²/E) dt`.
//!
//! The diagnostic reads states and never feeds back: dynamics, default outputs and scenario
//! digests are unchanged. It is opt-in ([`crate::run_with_clocks`], or [`ClockAccumulator`] on
//! any integrator's dense output). The `τ_SR` and `τ_K` readings are **comparison readings**,
//! labelled so wherever they are reported, and like every comparison model (ADR-0016) they never
//! enter the GR ladder or any reference value.
//!
//! All clocks are carried as offsets `clock − t` (seconds), and the rates as `rate − 1`, computed
//! without cancellation, so that GPS-size offsets (10⁻¹⁰) keep full binary64 precision.

use crate::PropagateError;
use crate::output::{RunOutput, StateRecord};
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_ephem::NaifId;
use ferroastro_system::Catalogue;

/// Label carried by every report of this diagnostic (ADR-0016's "comparison model" rule for the
/// two excluded readings).
pub const CLOCKS_LABEL: &str = "three-clock diagnostic (PHY-022): metric s of g = Omega^2 eta; \
     comparison readings tau_SR (flat SR) and tau_K (Gill flow parameter), comparison model, \
     outside the GR ladder";

fn domain(problem: &str) -> PropagateError {
    PropagateError::Output {
        problem: format!("three-clock diagnostic: {problem}"),
    }
}

/// One value per clock: rate offsets (`d·/dt − 1`, dimensionless) or accumulated offsets
/// (`clock − t`, seconds), depending on use.
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct ThreeClocks {
    /// Metric proper time `s` of `g = Ω²η`.
    pub s: f64,
    /// Flat special-relativistic proper time `τ_SR` (comparison reading).
    pub tau_sr: f64,
    /// Gill's flow parameter `τ_K` (comparison reading).
    pub tau_k: f64,
}

impl ThreeClocks {
    /// `self + k·other`, per clock.
    #[must_use]
    pub fn add_scaled(self, other: Self, k: f64) -> Self {
        Self {
            s: k.mul_add(other.s, self.s),
            tau_sr: k.mul_add(other.tau_sr, self.tau_sr),
            tau_k: k.mul_add(other.tau_k, self.tau_k),
        }
    }

    /// `self − other`, per clock.
    #[must_use]
    pub fn minus(self, other: Self) -> Self {
        self.add_scaled(other, -1.0)
    }

    /// `k·self`, per clock.
    #[must_use]
    pub fn scaled(self, k: f64) -> Self {
        Self::default().add_scaled(self, k)
    }

    /// Largest absolute component.
    #[must_use]
    pub fn max_abs(self) -> f64 {
        self.s.abs().max(self.tau_sr.abs()).max(self.tau_k.abs())
    }
}

/// The three rate offsets `d·/dt − 1` at potential depth `u = U = Σ GMⱼ/(rⱼc²)` (≥ 0) and
/// `beta2 = v²/c²`, exact in `Ω = 1 − u` and `√(1 − v²/c²)` (module docs).
///
/// # Errors
/// `FA-E1303` unless `0 ≤ u < 1` and `0 ≤ beta2 < 1` (finite).
pub fn rate_offsets(u: f64, beta2: f64) -> Result<ThreeClocks, PropagateError> {
    if !(u.is_finite() && (0.0..1.0).contains(&u)) {
        return Err(domain(&format!(
            "potential depth U = {u} is outside [0, 1) (Omega = 1 - U must be positive)"
        )));
    }
    if !(beta2.is_finite() && (0.0..1.0).contains(&beta2)) {
        return Err(domain(&format!("v^2/c^2 = {beta2} is outside [0, 1)")));
    }
    // √(1 − β²) − 1 without cancellation.
    let sr = -beta2 / (1.0 + (1.0 - beta2).sqrt());
    Ok(ThreeClocks {
        // Ω√(1 − β²) − 1 = (1 − u)(1 + sr) − 1.
        s: (-u).mul_add(sr, sr - u),
        tau_sr: sr,
        // √(1 − β²)/Ω − 1 = (sr + u)/(1 − u).
        tau_k: (sr + u) / (1.0 - u),
    })
}

/// A point-mass potential source: position (km) and `GM`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    /// Position (km), same frame and origin as the clock positions.
    pub position: [f64; 3],
    /// Gravitational parameter.
    pub gm: GravParam,
}

/// The clock model: the speed of light used for `U` and `v²/c²`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockModel {
    c: f64,
}

impl ClockModel {
    /// A model with speed of light `c`.
    ///
    /// # Errors
    /// `FA-E1303` for a non-positive or non-finite `c`.
    pub fn new(c: Velocity) -> Result<Self, PropagateError> {
        let c = c.km_per_s();
        if !(c.is_finite() && c > 0.0) {
            return Err(domain(&format!(
                "speed of light {c} km/s must be finite and > 0"
            )));
        }
        Ok(Self { c })
    }

    /// Speed of light (km/s).
    #[must_use]
    pub const fn c_km_per_s(&self) -> f64 {
        self.c
    }

    /// Rate offsets of a clock at `position` (km) moving with `velocity` (km/s) in the field of
    /// `sources`. A source closer than 1 mm to the clock is the clock's own body and is skipped
    /// (callers that know the clock's own index should simply leave it out).
    ///
    /// # Errors
    /// As [`rate_offsets`].
    pub fn rates(
        &self,
        position: [f64; 3],
        velocity: [f64; 3],
        sources: &[Source],
    ) -> Result<ThreeClocks, PropagateError> {
        let c2 = self.c * self.c;
        let mut u = 0.0;
        for src in sources {
            let d = [
                position[0] - src.position[0],
                position[1] - src.position[1],
                position[2] - src.position[2],
            ];
            let r = norm(d);
            if r > 1.0e-6 {
                u += src.gm.km3_per_s2() / (r * c2);
            }
        }
        let v2 = dot(velocity, velocity);
        rate_offsets(u, v2 / c2)
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

/// 8-point Gauss–Legendre nodes on [−1, 1] (positive half; the rule is symmetric).
const GL_NODES: [f64; 4] = [
    0.183_434_642_495_649_8,
    0.525_532_409_916_329,
    0.796_666_477_413_626_7,
    0.960_289_856_497_536_3,
];
/// The matching weights.
const GL_WEIGHTS: [f64; 4] = [
    0.362_683_783_378_362,
    0.313_706_645_877_887_3,
    0.222_381_034_453_374_5,
    0.101_228_536_290_376_3,
];

/// Accumulates the three clock offsets `clock − t` (seconds) over intervals of an integration,
/// with 8-point Gauss–Legendre quadrature on each interval (exact for rate polynomials of degree
/// 15). Fed with the dense output of each accepted IAS15 step, the quadrature error is far below
/// the integrator's own (the collocation polynomial is of degree 9 in the position).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ClockAccumulator {
    offsets: ThreeClocks,
    elapsed: f64,
}

impl ClockAccumulator {
    /// A fresh accumulator (all offsets zero).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `∫(rate − 1) dt` over `[t0, t1]`, with `rates_at(t)` the rate offsets at time `t`
    /// (e.g. [`ClockModel::rates`] of the dense-output state).
    ///
    /// # Errors
    /// The first error of `rates_at`; `FA-E1303` for a non-finite interval.
    pub fn add_interval<F>(
        &mut self,
        t0: f64,
        t1: f64,
        mut rates_at: F,
    ) -> Result<(), PropagateError>
    where
        F: FnMut(f64) -> Result<ThreeClocks, PropagateError>,
    {
        if !(t0.is_finite() && t1.is_finite()) {
            return Err(domain(&format!("interval [{t0}, {t1}] is not finite")));
        }
        let (mid, half) = (t0.midpoint(t1), 0.5 * (t1 - t0));
        let mut sum = ThreeClocks::default();
        for (x, w) in GL_NODES.iter().zip(GL_WEIGHTS) {
            sum = sum.add_scaled(rates_at(half.mul_add(-x, mid))?, w);
            sum = sum.add_scaled(rates_at(half.mul_add(*x, mid))?, w);
        }
        self.offsets = self.offsets.add_scaled(sum, half);
        self.elapsed += t1 - t0;
        Ok(())
    }

    /// The accumulated offsets `clock − t` (seconds).
    #[must_use]
    pub const fn offsets(&self) -> ThreeClocks {
        self.offsets
    }

    /// The coordinate time accumulated so far (seconds).
    #[must_use]
    pub const fn elapsed(&self) -> f64 {
        self.elapsed
    }
}

/// A clock fixed on the surface of a uniformly rotating spherical body (e.g. a ground clock on
/// the equator), in the body's monopole potential `GM/R`.
///
/// **Assumption (stated, not derived):** a *supported* clock obeys the same rate formula as a
/// free particle at its position and velocity. For `s` this is the standard metric clock
/// hypothesis; for `τ_SR` and `τ_K` it is the convention under which the rd#88 notes evaluate
/// those readings (report §2.4: the quoted σ multiples depend on it, the wrong sign of `τ_K`
/// does not).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotatingGroundClock {
    /// Radius of the clock's circle about the rotation axis, equal to its distance from the
    /// centre (equatorial clock).
    pub radius: Length,
    /// Rotation rate (rad/s); zero for a ground clock at rest.
    pub omega_rad_per_s: f64,
    /// `GM` of the body.
    pub gm: GravParam,
}

impl RotatingGroundClock {
    /// The constant rate offsets of the ground clock: `U = GM/(Rc²)`, `v = ωR`.
    ///
    /// # Errors
    /// As [`rate_offsets`]; `FA-E1303` for a non-positive radius or non-finite rotation rate.
    pub fn rates(&self, model: &ClockModel) -> Result<ThreeClocks, PropagateError> {
        let r = self.radius.km();
        if !(r.is_finite() && r > 0.0 && self.omega_rad_per_s.is_finite()) {
            return Err(domain(&format!(
                "ground clock radius {r} km and rotation rate {} rad/s must be finite, radius > 0",
                self.omega_rad_per_s
            )));
        }
        let c = model.c_km_per_s();
        let v = self.omega_rad_per_s * r;
        rate_offsets(self.gm.km3_per_s2() / (r * c * c), (v / c) * (v / c))
    }
}

/// Cumulative `∫(rate − 1) dt` from the first sample to every sample of a sampled series (e.g. a
/// run's output cadence), with the quadratic through three neighbouring samples on each interval
/// (any spacing; trapezoid if there are only two samples). Returns the cumulative offsets per
/// sample and an error estimate: the summed `|quadratic − trapezoid|` difference per interval,
/// an upper estimate of the trapezoid error and so a conservative bound on the quadratic rule's.
///
/// # Errors
/// `FA-E1303` if the lengths differ, there are no samples, or the times do not increase.
pub fn integrate_sampled(
    t: &[f64],
    rates: &[ThreeClocks],
) -> Result<(Vec<ThreeClocks>, ThreeClocks), PropagateError> {
    if t.len() != rates.len() || t.is_empty() {
        return Err(domain(&format!(
            "{} sample times for {} rate samples (need equal, non-zero counts)",
            t.len(),
            rates.len()
        )));
    }
    if t.windows(2).any(|w| !matches!(w, [a, b] if b > a)) {
        return Err(domain("sample times must increase strictly"));
    }
    let pts: Vec<(f64, ThreeClocks)> = t.iter().copied().zip(rates.iter().copied()).collect();
    let mut cum = Vec::with_capacity(pts.len());
    let mut acc = ThreeClocks::default();
    let mut err = ThreeClocks::default();
    cum.push(acc);
    for (k, w) in pts.windows(2).enumerate() {
        let [(p, fp), (q, fq)] = w else { continue };
        let trap = fp.add_scaled(*fq, 1.0).scaled(0.5 * (q - p));
        // Three nodes containing [p, q]: (k−1, k, k+1) except on the first interval.
        let i = k.saturating_sub(1);
        let quad = match pts.get(i..i + 3) {
            Some(&[n0, n1, n2]) => quadratic_integral([n0, n1, n2], *p, *q),
            _ => trap,
        };
        let d = quad.minus(trap);
        err = err.add_scaled(
            ThreeClocks {
                s: d.s.abs(),
                tau_sr: d.tau_sr.abs(),
                tau_k: d.tau_k.abs(),
            },
            1.0,
        );
        acc = acc.add_scaled(quad, 1.0);
        cum.push(acc);
    }
    Ok((cum, err))
}

/// `∫ₚ^q` of the quadratic through the three nodes `(xⱼ, fⱼ)`, per clock, in the local variable
/// `x − x₀`.
fn quadratic_integral(nodes: [(f64, ThreeClocks); 3], p: f64, q: f64) -> ThreeClocks {
    let [(x0, f0), (x1, f1), (x2, f2)] = nodes;
    let (h1, h2) = (x1 - x0, x2 - x0);
    let (a, b) = (p - x0, q - x0);
    let one = |y0: f64, y1: f64, y2: f64| -> f64 {
        let d01 = (y1 - y0) / h1;
        let d12 = (y2 - y1) / (h2 - h1);
        let d012 = (d12 - d01) / h2;
        // p(u) = y0 + d01 u + d012 u (u − h1); primitive y0 u + d01 u²/2 + d012 (u³/3 − h1 u²/2).
        let prim = |u: f64| -> f64 {
            let u2 = u * u;
            let cubic = (u / 3.0).mul_add(u2, -0.5 * h1 * u2);
            d012.mul_add(cubic, (0.5 * d01).mul_add(u2, y0 * u))
        };
        prim(b) - prim(a)
    };
    ThreeClocks {
        s: one(f0.s, f1.s, f2.s),
        tau_sr: one(f0.tau_sr, f1.tau_sr, f2.tau_sr),
        tau_k: one(f0.tau_k, f1.tau_k, f2.tau_k),
    }
}

/// The three clock offsets of every body at one sample time.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClockSample {
    /// TDB seconds since the initial epoch.
    pub t: f64,
    /// Accumulated offsets `clock − t` (seconds) per body, in [`ClockReport::bodies`] order.
    pub offsets: Vec<ThreeClocks>,
}

/// The opt-in three-clock output of [`crate::run_with_clocks`] (PHY-022). Clocks are integrated
/// on the run's sampled states (output cadence, plus the final state) by [`integrate_sampled`];
/// their accuracy is set by that cadence, and [`ClockReport::quadrature_error`] estimates it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ClockReport {
    /// [`CLOCKS_LABEL`].
    pub label: String,
    /// Frame, origin and potential of the evaluation.
    pub frame: String,
    /// Speed of light used (km/s).
    pub c_km_per_s: f64,
    /// NAIF ids of the clocks, in storage order.
    pub bodies: Vec<i32>,
    /// NAIF ids of the bodies whose `GM` enters `U`.
    pub sources: Vec<i32>,
    /// NAIF ids of moving bodies with no catalogue `GM`, left out of `U` (their potential at a
    /// planet is ≲ 10⁻¹⁷).
    pub omitted_sources: Vec<i32>,
    /// Cumulative offsets per sample.
    pub samples: Vec<ClockSample>,
    /// Per body, the quadrature error estimate of the final offsets (seconds).
    pub quadrature_error: Vec<ThreeClocks>,
}

fn exact(z: [f64; 2]) -> f64 {
    z[0] + z[1]
}

fn run_error(scenario: &str, problem: &str) -> PropagateError {
    domain(&format!("run {scenario}: {problem}"))
}

/// The sampled states of a run, plus its final state if it is later than the last sample.
fn records(out: &RunOutput) -> Vec<&StateRecord> {
    let mut recs: Vec<&StateRecord> = out.states.iter().collect();
    if let Some(last) = &out.final_state {
        if recs.last().is_none_or(|r| exact(r.t) < exact(last.t)) {
            recs.push(last);
        }
    }
    recs
}

/// The `GM` of every body that sources `U` (`None` otherwise), the source ids and the moving
/// bodies left out for want of a catalogue `GM`. In test-particle mode (`central` given) only the
/// central body is a source: the particle is massless by the model.
fn source_gms(
    bodies: &[i32],
    catalogue: &Catalogue,
    central: Option<i32>,
) -> (Vec<Option<GravParam>>, Vec<i32>, Vec<i32>) {
    let (mut sources, mut omitted) = (Vec::new(), Vec::new());
    let gms = bodies
        .iter()
        .map(|&id| {
            let gm = match central {
                Some(c) if c != id => None,
                _ => catalogue.gm(NaifId(id)).ok(),
            };
            match (gm, central) {
                (Some(_), _) => sources.push(id),
                (None, None) => omitted.push(id),
                (None, Some(_)) => {}
            }
            gm
        })
        .collect();
    (gms, sources, omitted)
}

/// Rate offsets of every body at one state record.
fn record_rates(
    scenario: &str,
    rec: &StateRecord,
    gms: &[Option<GravParam>],
    model: ClockModel,
) -> Result<Vec<ThreeClocks>, PropagateError> {
    if rec.positions.len() != gms.len() || rec.velocities.len() != gms.len() {
        return Err(run_error(
            scenario,
            "state record does not match its body list",
        ));
    }
    let pos: Vec<[f64; 3]> = rec.positions.iter().map(|p| p.map(exact)).collect();
    let vel: Vec<[f64; 3]> = rec.velocities.iter().map(|p| p.map(exact)).collect();
    pos.iter()
        .zip(&vel)
        .enumerate()
        .map(|(i, (p, v))| {
            let srcs: Vec<Source> = gms
                .iter()
                .zip(&pos)
                .enumerate()
                .filter(|&(j, _)| j != i)
                .filter_map(|(_, (gm, q))| gm.map(|gm| Source { position: *q, gm }))
                .collect();
            model.rates(*p, *v, &srcs)
        })
        .collect()
}

/// The [`ClockReport`] of a finished run (see [`crate::run_with_clocks`]). `central` is the
/// central body of a test-particle run, `None` for an n-body run.
pub(crate) fn report(
    scenario: &str,
    out: &RunOutput,
    catalogue: &Catalogue,
    model: ClockModel,
    central: Option<i32>,
) -> Result<ClockReport, PropagateError> {
    let recs = records(out);
    let first = recs
        .first()
        .ok_or_else(|| run_error(scenario, "no sampled state"))?;
    let bodies = first.bodies.clone();
    let (gms, sources, omitted_sources) = source_gms(&bodies, catalogue, central);
    let mut times = Vec::with_capacity(recs.len());
    let mut rates: Vec<Vec<ThreeClocks>> = vec![Vec::with_capacity(recs.len()); bodies.len()];
    for rec in &recs {
        if rec.bodies != bodies {
            return Err(run_error(scenario, "body list changes between samples"));
        }
        times.push(exact(rec.t));
        for (series, r) in rates
            .iter_mut()
            .zip(record_rates(scenario, rec, &gms, model)?)
        {
            series.push(r);
        }
    }
    let mut cumulative = Vec::with_capacity(bodies.len());
    let mut quadrature_error = Vec::with_capacity(bodies.len());
    for series in &rates {
        let (cum, err) = integrate_sampled(&times, series)?;
        cumulative.push(cum);
        quadrature_error.push(err);
    }
    let samples = times
        .iter()
        .enumerate()
        .map(|(k, &t)| ClockSample {
            t,
            offsets: cumulative
                .iter()
                .map(|c| c.get(k).copied().unwrap_or_default())
                .collect(),
        })
        .collect();
    let frame = central.map_or_else(
        || {
            "ICRF axes, solar-system barycentre; U = sum of the other bodies' Newtonian \
             monopoles / c^2"
                .to_owned()
        },
        |c| {
            format!(
                "ICRF axes, origin at the central body (NAIF {c}, fixed); U = GM_central/(r c^2)"
            )
        },
    );
    Ok(ClockReport {
        label: CLOCKS_LABEL.to_owned(),
        frame,
        c_km_per_s: model.c_km_per_s(),
        bodies,
        sources,
        omitted_sources,
        samples,
        quadrature_error,
    })
}

#[cfg(test)]
#[allow(
    clippy::suboptimal_flops,
    reason = "test: reference arithmetic is written plainly"
)]
mod tests {
    use super::*;

    /// The rate offsets are exact rewritings of `Ω√(1−β²)`, `√(1−β²)`, `√(1−β²)/Ω`, and satisfy
    /// `dτ_SR = Ω dτ_K`, `ds = Ω² dτ_K`; invalid input is refused.
    ///
    /// Verifies: PHY-022
    #[test]
    fn rate_offsets_are_exact_and_refuse_invalid_input() {
        for (u, b2) in [
            (0.0, 0.0),
            (0.1, 0.0),        // lint: allow-literal(structural depth)
            (0.0, 0.2),        // lint: allow-literal(structural speed)
            (0.05, 0.3),       // lint: allow-literal(structural depth and speed)
            (1.0e-9, 1.0e-10), // lint: allow-literal(GPS-size depth and speed)
        ] {
            let r = rate_offsets(u, b2).unwrap();
            let (om, g) = (1.0 - u, (1.0 - b2).sqrt());
            let tol = 4.0 * f64::EPSILON;
            assert!((1.0 + r.s - om * g).abs() <= tol);
            assert!((1.0 + r.tau_sr - g).abs() <= tol);
            assert!((1.0 + r.tau_k - g / om).abs() <= tol);
            assert!(((1.0 + r.tau_sr) - om * (1.0 + r.tau_k)).abs() <= tol);
            assert!(((1.0 + r.s) - om * om * (1.0 + r.tau_k)).abs() <= tol);
        }
        assert!(rate_offsets(1.0, 0.0).is_err());
        assert!(rate_offsets(-0.1, 0.0).is_err()); // lint: allow-literal(negative depth)
        assert!(rate_offsets(0.0, 1.0).is_err());
        assert!(rate_offsets(f64::NAN, 0.0).is_err());
    }

    /// The sampled quadrature integrates quadratics exactly on uneven grids.
    ///
    /// Verifies: PHY-022
    #[test]
    fn sampled_quadrature_is_exact_for_quadratics() {
        let t = [0.0, 0.5, 1.5, 2.0, 3.5]; // lint: allow-literal(uneven structural grid)
        let f = |x: f64| 3.0 * x * x - 2.0 * x + 1.0;
        let rates: Vec<ThreeClocks> = t
            .iter()
            .map(|&x| ThreeClocks {
                s: f(x),
                tau_sr: -f(x),
                tau_k: 2.0 * f(x),
            })
            .collect();
        let (cum, _) = integrate_sampled(&t, &rates).unwrap();
        for (k, &x) in t.iter().enumerate() {
            let exact = x * x * x - x * x + x;
            assert!((cum[k].s - exact).abs() <= 1.0e-12 * (1.0 + exact.abs())); // lint: allow-literal(round-off bound)
            assert!((cum[k].tau_k - 2.0 * exact).abs() <= 1.0e-12 * (1.0 + exact.abs())); // lint: allow-literal(round-off bound)
        }
        assert!(integrate_sampled(&[0.0, 0.0], &rates[..2]).is_err());
        assert!(integrate_sampled(&[0.0], &[]).is_err());
    }
}
