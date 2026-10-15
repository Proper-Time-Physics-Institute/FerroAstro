// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! IAS15-class adaptive 15th-order Gauss–Radau integrator (NUM-001).
//!
//! Clean-room implementation from the papers only (DEP-008): Everhart 1985 (IAU Colloq. 83, 185)
//! for the Gauss–Radau predictor–corrector, and Rein & Spiegel 2015 (MNRAS 446, 1424,
//! doi:10.1093/mnras/stu2164) for the step-size criterion, the iteration limit and the
//! compensated state update. No REBOUND/REBOUNDx/ASSIST/Swiftest source was consulted.
//!
//! # Scheme
//! Over a step `Δt` the acceleration is approximated by the degree-7 polynomial
//! `a(τ) = a₀ + Σₘ Bₘ τᵐ`, `τ ∈ [0, 1]`. Integrating twice gives the position and velocity at any
//! `τ` (see [`Ias15::step`]). The coefficients are found by a predictor–corrector iteration: the
//! state is predicted at each Gauss–Radau node `hₙ`, the force evaluated there with the predicted
//! **position and velocity** (so velocity-dependent forces such as 1PN/EIH are handled natively),
//! and the Newton divided-difference coefficient `gₙ` updated immediately (Gauss–Seidel order).
//! The iteration stops when the change of `B₇` relative to the largest acceleration falls below
//! [`Ias15Config::pc_tolerance`], when it stops decreasing, or after
//! [`Ias15Config::max_iterations`] (12 by default, Rein & Spiegel 2015).
//!
//! # Step control (Rein & Spiegel 2015, "global" criterion)
//! `b̃ = max|B₇| / max|a|` over all coordinates; the next step is `Δt·(ε/b̃)^{1/7}`. A step whose
//! proposed successor is smaller than `safety·Δt` is rejected and redone; growth is capped at
//! `Δt/safety`.
//!
//! # Units
//! The integrator is unit-agnostic: it advances raw `f64` coordinates of a second-order system
//! `ẍ = f(t, x, ẋ)`. Typed quantities (ADR-0001) live at the force-model boundary
//! (`ferroastro-forces`), which documents its internal units.

use crate::IntegratorError;
use crate::compensated::kahan_add;
use crate::radau::{NODES, Radau, TERMS};

/// A second-order system `ẍ = f(t, x, ẋ)` in flat coordinates (usually 3 per body).
pub trait SecondOrderSystem {
    /// Number of coordinates.
    fn dimension(&self) -> usize;

    /// Write the acceleration at time `t`, position `x` and velocity `v` into `a`.
    /// All three slices have length [`SecondOrderSystem::dimension`].
    ///
    /// # Errors
    /// A force-model error ([`IntegratorError::Dynamics`]) if the configuration is singular.
    fn acceleration(
        &self,
        t: f64,
        x: &[f64],
        v: &[f64],
        a: &mut [f64],
    ) -> Result<(), IntegratorError>;
}

/// IAS15 settings. The defaults are those of Rein & Spiegel 2015.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ias15Config {
    /// Step-control precision parameter ε (dimensionless). Default `1e-9`.
    pub epsilon: f64,
    /// Rejection threshold and inverse growth cap. Default `0.25`.
    pub safety: f64,
    /// Maximum predictor–corrector iterations per step. Default 12.
    pub max_iterations: u32,
    /// Convergence threshold for `max|ΔB₇| / max|a|`. Default `1e-16`.
    pub pc_tolerance: f64,
    /// Smallest allowed |Δt| (integrator time units); below it the run fails with `FA-E0401`.
    /// Default 0 (only a zero or non-finite step fails).
    pub min_dt: f64,
    /// Maximum consecutive rejections of one step before failing with `FA-E0401`. Default 64.
    pub max_rejections: u32,
}

impl Default for Ias15Config {
    fn default() -> Self {
        Self {
            epsilon: 1e-9,
            safety: 0.25,
            max_iterations: 12,
            pc_tolerance: 1e-16,
            min_dt: 0.0,
            max_rejections: 64,
        }
    }
}

impl Ias15Config {
    fn validate(&self) -> Result<(), IntegratorError> {
        let bad = |name, value, valid| Err(IntegratorError::Config { name, value, valid });
        if !(self.epsilon.is_finite() && self.epsilon > 0.0) {
            return bad("epsilon", self.epsilon, "finite and > 0");
        }
        if !(self.safety > 0.0 && self.safety < 1.0) {
            return bad("safety", self.safety, "0 < safety < 1");
        }
        if self.max_iterations == 0 {
            return bad("max_iterations", 0.0, ">= 1");
        }
        if !(self.pc_tolerance.is_finite() && self.pc_tolerance >= 0.0) {
            return bad("pc_tolerance", self.pc_tolerance, "finite and >= 0");
        }
        if !(self.min_dt.is_finite() && self.min_dt >= 0.0) {
            return bad("min_dt", self.min_dt, "finite and >= 0");
        }
        Ok(())
    }
}

/// Counters accumulated over a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ias15Stats {
    /// Accepted steps.
    pub steps: u64,
    /// Rejected (redone) steps.
    pub rejected: u64,
    /// Force evaluations.
    pub force_evaluations: u64,
    /// Predictor–corrector iterations, summed over all attempts.
    pub iterations: u64,
    /// Accepted steps whose iteration hit `max_iterations` without meeting `pc_tolerance`
    /// (rejected attempts are not counted: they are redone).
    pub unconverged: u64,
}

/// What one accepted step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// Time at the end of the step.
    pub t: f64,
    /// Step actually taken.
    pub dt_done: f64,
    /// Predictor–corrector iterations of the accepted attempt.
    pub iterations: u32,
    /// True when the step was shortened to land exactly on the requested limit.
    pub reached_limit: bool,
}

/// Coefficient divisors of the position integral, `(m+1)(m+2)` for `τᵐ`, m = 0…7.
const X_DIV: [f64; NODES] = [2.0, 6.0, 12.0, 20.0, 30.0, 42.0, 56.0, 72.0];
/// Coefficient divisors of the velocity integral, `m+1` for `τᵐ`, m = 0…7.
const V_DIV: [f64; NODES] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

/// The IAS15 integrator state.
#[derive(Debug, Clone)]
pub struct Ias15 {
    cfg: Ias15Config,
    rc: Radau,
    n: usize,
    t: f64,
    t_comp: f64,
    x: Vec<f64>,
    v: Vec<f64>,
    x_comp: Vec<f64>,
    v_comp: Vec<f64>,
    dt_trial: f64,
    /// `b[m-1][i]` = `Bₘ` of coordinate `i` (IAS15's `b_{m−1}`).
    b: [Vec<f64>; TERMS],
    g: [Vec<f64>; TERMS],
    a0: Vec<f64>,
    an: Vec<f64>,
    xs: Vec<f64>,
    vs: Vec<f64>,
    b7_prev: Vec<f64>,
    stats: Ias15Stats,
}

fn check_len(what: &'static str, got: usize, expected: usize) -> Result<(), IntegratorError> {
    if got == expected {
        Ok(())
    } else {
        Err(IntegratorError::Dimension {
            what,
            got,
            expected,
        })
    }
}

fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0_f64, |m, &x| m.max(x.abs()))
}

impl Ias15 {
    /// Start an integration at time `t0` with positions `x0`, velocities `v0` and a first trial
    /// step `dt0` (its sign sets the direction; the step control adapts it from the first step).
    ///
    /// # Errors
    /// `FA-E0402` for an invalid configuration or a zero/non-finite `dt0`, `FA-E0403` if the
    /// slices do not match `dimension`, `FA-E0400` for non-finite initial values.
    pub fn new(
        dimension: usize,
        t0: f64,
        x0: &[f64],
        v0: &[f64],
        dt0: f64,
        cfg: Ias15Config,
    ) -> Result<Self, IntegratorError> {
        cfg.validate()?;
        check_len("initial position", x0.len(), dimension)?;
        check_len("initial velocity", v0.len(), dimension)?;
        if !(dt0.is_finite() && dt0 != 0.0) {
            return Err(IntegratorError::Config {
                name: "dt0",
                value: dt0,
                valid: "finite and non-zero",
            });
        }
        if !t0.is_finite() || x0.iter().chain(v0).any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "initial state",
                t: t0,
            });
        }
        let zeros = || vec![0.0; dimension];
        Ok(Self {
            cfg,
            rc: Radau::new(),
            n: dimension,
            t: t0,
            t_comp: 0.0,
            x: x0.to_vec(),
            v: v0.to_vec(),
            x_comp: zeros(),
            v_comp: zeros(),
            dt_trial: dt0,
            b: std::array::from_fn(|_| zeros()),
            g: std::array::from_fn(|_| zeros()),
            a0: zeros(),
            an: zeros(),
            xs: zeros(),
            vs: zeros(),
            b7_prev: zeros(),
            stats: Ias15Stats::default(),
        })
    }

    /// Current time.
    #[must_use]
    pub const fn time(&self) -> f64 {
        self.t
    }

    /// Current positions.
    #[must_use]
    pub fn positions(&self) -> &[f64] {
        &self.x
    }

    /// Current velocities.
    #[must_use]
    pub fn velocities(&self) -> &[f64] {
        &self.v
    }

    /// The step the next call to [`Ias15::step`] will try.
    #[must_use]
    pub const fn next_dt(&self) -> f64 {
        self.dt_trial
    }

    /// Run counters.
    #[must_use]
    pub const fn stats(&self) -> Ias15Stats {
        self.stats
    }

    /// The configuration.
    #[must_use]
    pub const fn config(&self) -> Ias15Config {
        self.cfg
    }

    /// Reverse the direction of integration (for forward–backward reversibility tests, NUM-023).
    /// The polynomial predictor is discarded.
    pub fn reverse(&mut self) {
        self.dt_trial = -self.dt_trial;
        for bm in &mut self.b {
            bm.fill(0.0);
        }
    }

    /// Predict position and velocity at node fraction `tau` of step `dt` into `xs`, `vs`.
    #[expect(
        clippy::indexing_slicing,
        reason = "all state vectors have length self.n, checked in Ias15::new"
    )]
    fn predict(&mut self, tau: f64, dt: f64) {
        for i in 0..self.n {
            // Horner in τ for Σ_m coef_m τ^m / div with coef_0 = a0, coef_m = B_m.
            let mut sx = self.b[TERMS - 1][i] / X_DIV[TERMS];
            let mut sv = self.b[TERMS - 1][i] / V_DIV[TERMS];
            for m in (0..TERMS - 1).rev() {
                sx = sx.mul_add(tau, self.b[m][i] / X_DIV[m + 1]);
                sv = sv.mul_add(tau, self.b[m][i] / V_DIV[m + 1]);
            }
            sx = sx.mul_add(tau, self.a0[i] / X_DIV[0]);
            sv = sv.mul_add(tau, self.a0[i] / V_DIV[0]);
            let tdt = tau * dt;
            let dx = tdt * sx.mul_add(tdt, self.v[i]);
            let dv = tdt * sv;
            self.xs[i] = self.x[i] + (dx - self.x_comp[i]);
            self.vs[i] = self.v[i] + (dv - self.v_comp[i]);
        }
    }

    /// Recompute the Newton coefficients `g` from `B` (`g = d·B`).
    #[expect(
        clippy::indexing_slicing,
        reason = "fixed TERMS x n tables; indices bounded by TERMS and self.n"
    )]
    fn g_from_b(&mut self) {
        for k in 1..=TERMS {
            for i in 0..self.n {
                let mut s = 0.0;
                for m in k..=TERMS {
                    s = self.rc.d[k][m].mul_add(self.b[m - 1][i], s);
                }
                self.g[k - 1][i] = s;
            }
        }
    }

    /// After a new acceleration `an` at node `node`, update `g_node` and the `B` it feeds.
    #[expect(
        clippy::indexing_slicing,
        reason = "fixed TERMS x n tables; node in 1..=TERMS, i < self.n"
    )]
    fn update_node(&mut self, node: usize) {
        let h = &self.rc.h;
        for i in 0..self.n {
            let mut gn = (self.an[i] - self.a0[i]) / h[node];
            for k in 1..node {
                gn = (gn - self.g[k - 1][i]) / (h[node] - h[k]);
            }
            let delta = gn - self.g[node - 1][i];
            self.g[node - 1][i] = gn;
            for m in 1..=node {
                self.b[m - 1][i] = self.rc.c[node][m].mul_add(delta, self.b[m - 1][i]);
            }
        }
    }

    /// Scale `B` for a retried step from the same start: `a(τ_old) = a(q τ_new)`.
    fn rescale_b(&mut self, q: f64) {
        let mut qm = 1.0;
        for bm in &mut self.b {
            qm *= q;
            for z in bm.iter_mut() {
                *z *= qm;
            }
        }
    }

    /// Extrapolate `B` to the next step of length `q·Δt` that starts at `τ = 1`:
    /// `B'ₖ = qᵏ Σₘ₍≥ₖ₎ C(m, k) Bₘ`.
    #[expect(
        clippy::indexing_slicing,
        clippy::needless_range_loop,
        reason = "fixed TERMS x n tables; indices bounded by TERMS and self.n"
    )]
    fn extrapolate_b(&mut self, q: f64) {
        // Binomial coefficients C(m, k) for m, k <= 7, built by Pascal's rule.
        let mut binom = [[0.0_f64; NODES]; NODES];
        for m in 0..NODES {
            binom[m][0] = 1.0;
            for k in 1..=m {
                binom[m][k] = binom[m - 1][k - 1] + if k < m { binom[m - 1][k] } else { 0.0 };
            }
        }
        for i in 0..self.n {
            let old: [f64; NODES] =
                std::array::from_fn(|m| if m == 0 { 0.0 } else { self.b[m - 1][i] });
            let mut qk = 1.0;
            for k in 1..=TERMS {
                qk *= q;
                let mut s = 0.0;
                for m in k..=TERMS {
                    s = binom[m][k].mul_add(old[m], s);
                }
                self.b[k - 1][i] = qk * s;
            }
        }
    }

    /// Predictor–corrector iterations for a step of length `dt`. Returns the iteration count,
    /// the largest acceleration component seen, and whether the iteration ran out (hit
    /// `max_iterations` without converging or stalling).
    fn correct<S: SecondOrderSystem + ?Sized>(
        &mut self,
        sys: &S,
        dt: f64,
    ) -> Result<(u32, f64, bool), IntegratorError> {
        self.g_from_b();
        let mut last_err = f64::INFINITY;
        let mut iterations = 0_u32;
        loop {
            iterations += 1;
            self.b7_prev.clone_from(&self.b[TERMS - 1]);
            let mut max_a = max_abs(&self.a0);
            for (node, &tau) in self.rc.h.clone().iter().enumerate().skip(1) {
                self.predict(tau, dt);
                let t_node = tau.mul_add(dt, self.t);
                sys.acceleration(t_node, &self.xs, &self.vs, &mut self.an)?;
                self.stats.force_evaluations += 1;
                if self.an.iter().any(|z| !z.is_finite()) {
                    return Err(IntegratorError::NonFinite {
                        what: "acceleration",
                        t: t_node,
                    });
                }
                max_a = max_a.max(max_abs(&self.an));
                self.update_node(node);
            }
            let change = self.b[TERMS - 1]
                .iter()
                .zip(&self.b7_prev)
                .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
            let err = if max_a > 0.0 { change / max_a } else { 0.0 };
            // At least two iterations: with a zero predictor, B7 can look converged after one
            // pass while the lower coefficients are not.
            let converged = iterations >= 2 && err <= self.cfg.pc_tolerance;
            let stalled = iterations > 2 && err >= last_err;
            if converged || stalled || iterations >= self.cfg.max_iterations {
                self.stats.iterations += u64::from(iterations);
                return Ok((iterations, max_a, !converged && !stalled));
            }
            last_err = err;
        }
    }

    /// Compensated update of `x`, `v` to the end (`τ = 1`) of a converged step of length `dt`.
    #[expect(
        clippy::indexing_slicing,
        reason = "fixed TERMS x n tables; indices bounded by TERMS and self.n"
    )]
    fn advance_state(&mut self, dt: f64) -> Result<(), IntegratorError> {
        for i in 0..self.n {
            let mut sx = self.b[TERMS - 1][i] / X_DIV[TERMS];
            let mut sv = self.b[TERMS - 1][i] / V_DIV[TERMS];
            for m in (0..TERMS - 1).rev() {
                sx += self.b[m][i] / X_DIV[m + 1];
                sv += self.b[m][i] / V_DIV[m + 1];
            }
            sx += self.a0[i] / X_DIV[0];
            sv += self.a0[i] / V_DIV[0];
            let dx = dt * sx.mul_add(dt, self.v[i]);
            let dv = dt * sv;
            kahan_add(&mut self.x[i], &mut self.x_comp[i], dx);
            kahan_add(&mut self.v[i], &mut self.v_comp[i], dv);
        }
        if self.x.iter().chain(&self.v).any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "position",
                t: self.t,
            });
        }
        Ok(())
    }

    /// Take one accepted step, never stepping past `t_limit` (pass `f64::INFINITY` or
    /// `f64::NEG_INFINITY` for no limit in the direction of integration).
    ///
    /// Position and velocity at the end of the step are, with `τ = 1`,
    /// `x = x₀ + Δt·v₀ + Δt²·(a₀/2 + B₁/6 + B₂/12 + … + B₇/72)` and
    /// `v = v₀ + Δt·(a₀ + B₁/2 + … + B₇/8)`, added with compensated summation.
    ///
    /// # Errors
    /// Force-model errors, `FA-E0400` on non-finite values, `FA-E0401` when the step collapses.
    pub fn step<S: SecondOrderSystem + ?Sized>(
        &mut self,
        sys: &S,
        t_limit: f64,
    ) -> Result<StepReport, IntegratorError> {
        check_len("system", sys.dimension(), self.n)?;
        sys.acceleration(self.t, &self.x, &self.v, &mut self.a0)?;
        self.stats.force_evaluations += 1;
        if self.a0.iter().any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "acceleration",
                t: self.t,
            });
        }
        let mut rejections = 0_u32;
        loop {
            let remaining = t_limit - self.t;
            let mut dt = self.dt_trial;
            let clamped = remaining.is_finite() && dt.abs() >= remaining.abs();
            if clamped {
                dt = remaining;
            }
            if dt == 0.0 {
                return Ok(StepReport {
                    t: self.t,
                    dt_done: 0.0,
                    iterations: 0,
                    reached_limit: true,
                });
            }
            let (iterations, max_a, ran_out) = self.correct(sys, dt)?;

            // Step-size control (Rein & Spiegel 2015 global criterion).
            let b7 = max_abs(&self.b[TERMS - 1]);
            let ratio = if max_a > 0.0 { b7 / max_a } else { 0.0 };
            let mut dt_new = if ratio > 0.0 && ratio.is_finite() {
                dt * (self.cfg.epsilon / ratio).powf(1.0 / 7.0)
            } else {
                dt / self.cfg.safety
            };
            if !dt_new.is_finite() {
                return Err(IntegratorError::NonFinite {
                    what: "step",
                    t: self.t,
                });
            }
            if dt_new.abs() < self.cfg.safety * dt.abs() {
                rejections += 1;
                self.stats.rejected += 1;
                if dt_new.abs() <= self.cfg.min_dt
                    || dt_new == 0.0
                    || rejections > self.cfg.max_rejections
                {
                    return Err(IntegratorError::StepUnderflow {
                        dt: dt_new,
                        min_dt: self.cfg.min_dt,
                        t: self.t,
                    });
                }
                self.rescale_b(dt_new / dt);
                self.dt_trial = dt_new;
                continue;
            }
            if dt_new.abs() > dt.abs() / self.cfg.safety {
                dt_new = dt / self.cfg.safety;
            }
            // When clamped, `dt_new` was computed from the short step; keep the step the
            // controller had proposed before clamping, if that is larger.
            let next = if clamped && self.dt_trial.abs() > dt_new.abs() {
                self.dt_trial
            } else {
                dt_new
            };

            self.advance_state(dt)?;
            kahan_add(&mut self.t, &mut self.t_comp, dt);
            if clamped {
                // Land exactly on the limit.
                self.t = t_limit;
                self.t_comp = 0.0;
            }
            self.stats.steps += 1;
            if ran_out {
                self.stats.unconverged += 1;
            }
            self.extrapolate_b(next / dt);
            self.dt_trial = next;
            return Ok(StepReport {
                t: self.t,
                dt_done: dt,
                iterations,
                reached_limit: clamped,
            });
        }
    }

    /// Integrate to `t_end`, calling `observer` after every accepted step.
    ///
    /// # Errors
    /// Any error of [`Ias15::step`], or the first error returned by `observer`.
    pub fn integrate_to<S, F>(
        &mut self,
        sys: &S,
        t_end: f64,
        mut observer: F,
    ) -> Result<(), IntegratorError>
    where
        S: SecondOrderSystem + ?Sized,
        F: FnMut(&Self, &StepReport) -> Result<(), IntegratorError>,
    {
        if !t_end.is_finite() {
            return Err(IntegratorError::Config {
                name: "t_end",
                value: t_end,
                valid: "finite",
            });
        }
        if (t_end - self.t) * self.dt_trial < 0.0 {
            self.reverse();
        }
        loop {
            let report = self.step(sys, t_end)?;
            if report.dt_done != 0.0 {
                observer(self, &report)?;
            }
            if report.reached_limit {
                return Ok(());
            }
        }
    }
}
