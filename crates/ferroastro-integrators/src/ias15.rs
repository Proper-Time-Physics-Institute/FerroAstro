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
//! # Units and precision
//! The integrator is unit-agnostic: it advances raw coordinates of a second-order system
//! `ẍ = f(t, x, ẋ)`. Typed quantities (ADR-0001) live at the force-model boundary
//! (`ferroastro-forces`), which documents its internal units. The scalar is any backend
//! `R: Real` (DEP-007, NUM-006); `Ias15` alone means `Ias15<f64>`, whose arithmetic is exactly that
//! of the M0 `f64` integrator. The step-size controller (a choice of `Δt`, not part of the
//! solution's accuracy) is evaluated in `f64` for every backend.

use crate::IntegratorError;
use crate::compensated::kahan_add;
use crate::radau::{NODES, Radau, TERMS};
use ferroastro_real::Real;

/// A second-order system `ẍ = f(t, x, ẋ)` in flat coordinates (usually 3 per body), evaluated in
/// backend `R` (default `f64`).
pub trait SecondOrderSystem<R: Real = f64> {
    /// Number of coordinates.
    fn dimension(&self) -> usize;

    /// Write the acceleration at time `t`, position `x` and velocity `v` into `a`.
    /// All three slices have length [`SecondOrderSystem::dimension`].
    ///
    /// # Errors
    /// A force-model error ([`IntegratorError::Dynamics`]) if the configuration is singular.
    fn acceleration(&self, t: R, x: &[R], v: &[R], a: &mut [R]) -> Result<(), IntegratorError>;
}

/// IAS15 settings. The defaults are those of Rein & Spiegel 2015.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Keep the polynomial of the last accepted step for [`Ias15::dense_state`] and
    /// [`Ias15::locate_event`] (NUM-010). Off by default: it copies the step's coefficients after
    /// every step, which the plain propagation does not need.
    pub dense_output: bool,
    /// EXPERIMENTAL (NUM-023 diagnosis, WP7): after each node update, recompute every
    /// `Bₘ = Σₖ c[k][m] gₖ` from the Newton coefficients instead of accumulating the increments
    /// `c[node][m]·Δg` into `B`. In WP7's 32 × 20 000-orbit ensemble this removed the systematic
    /// energy drift (Brouwer slopes 0.51 / 1.47 instead of 0.63 / 1.59) at ~1.5× the wall time.
    /// Off by default: adopting it changes every f64 result and is a maintainer decision.
    pub pc_b_from_g: bool,
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
            dense_output: false,
            pc_b_from_g: false,
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
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
    /// Accepted steps whose iteration ended because the correction stopped decreasing.
    pub stalled: u64,
    /// Predictor restarts from zero after a step shortened to land on a requested time.
    pub cold_starts: u64,
    /// Largest final `max|ΔB₇|/max|a|` of any accepted step.
    pub max_pc_error: f64,
}

/// What one accepted step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport<R: Real = f64> {
    /// Time at the end of the step.
    pub t: R,
    /// Step actually taken.
    pub dt_done: R,
    /// Predictor–corrector iterations of the accepted attempt.
    pub iterations: u32,
    /// True when the step was shortened to land exactly on the requested limit.
    pub reached_limit: bool,
}

/// How a predictor–corrector iteration ended (with the final relative `ΔB₇` where relevant).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Exit {
    Converged,
    Stalled(f64),
    RanOut(f64),
}

/// Coefficient divisors of the position integral, `(m+1)(m+2)` for `τᵐ`, m = 0…7.
const X_DIV: [f64; NODES] = [2.0, 6.0, 12.0, 20.0, 30.0, 42.0, 56.0, 72.0];
/// Coefficient divisors of the velocity integral, `m+1` for `τᵐ`, m = 0…7.
const V_DIV: [f64; NODES] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];

/// The IAS15 integrator state, in backend `R` (default `f64`).
#[derive(Debug, Clone)]
pub struct Ias15<R: Real = f64> {
    cfg: Ias15Config,
    rc: Radau<R>,
    n: usize,
    t: R,
    t_comp: R,
    x: Vec<R>,
    v: Vec<R>,
    x_comp: Vec<R>,
    v_comp: Vec<R>,
    dt_trial: R,
    /// `b[m-1][i]` = `Bₘ` of coordinate `i` (IAS15's `b_{m−1}`).
    b: [Vec<R>; TERMS],
    g: [Vec<R>; TERMS],
    a0: Vec<R>,
    an: Vec<R>,
    xs: Vec<R>,
    vs: Vec<R>,
    b7_prev: Vec<R>,
    stats: Ias15Stats,
    last: Option<LastStep<R>>,
}

/// The polynomial of the last accepted step (dense output, NUM-010).
#[derive(Debug, Clone)]
struct LastStep<R: Real> {
    t0: R,
    dt: R,
    /// Start-of-step position and velocity, compensation applied (`x − x_comp`).
    x0: Vec<R>,
    v0: Vec<R>,
    a0: Vec<R>,
    b: [Vec<R>; TERMS],
}

impl<R: Real> LastStep<R> {
    /// Position and velocity at step fraction `tau` (the integrals of the step's acceleration
    /// polynomial: degree 9 in position, 8 in velocity).
    #[expect(
        clippy::indexing_slicing,
        reason = "all vectors have the integrator dimension; i < x.len()"
    )]
    fn eval(&self, tau: R, x: &mut [R], v: &mut [R]) {
        for i in 0..x.len().min(v.len()).min(self.x0.len()) {
            let mut sx = self.b[TERMS - 1][i] / div::<R>(&X_DIV, TERMS);
            let mut sv = self.b[TERMS - 1][i] / div::<R>(&V_DIV, TERMS);
            for m in (0..TERMS - 1).rev() {
                sx = sx.mul_add(tau, self.b[m][i] / div::<R>(&X_DIV, m + 1));
                sv = sv.mul_add(tau, self.b[m][i] / div::<R>(&V_DIV, m + 1));
            }
            sx = sx.mul_add(tau, self.a0[i] / div::<R>(&X_DIV, 0));
            sv = sv.mul_add(tau, self.a0[i] / div::<R>(&V_DIV, 0));
            let tdt = tau * self.dt;
            x[i] = self.x0[i] + tdt * sx.mul_add(tdt, self.v0[i]);
            v[i] = self.v0[i] + tdt * sv;
        }
    }
}

pub(crate) fn check_len(
    what: &'static str,
    got: usize,
    expected: usize,
) -> Result<(), IntegratorError> {
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

fn max_abs<R: Real>(values: &[R]) -> R {
    values.iter().fold(R::ZERO, |m, &x| m.max(x.abs()))
}

/// `X_DIV[k]` or `V_DIV[k]` as a backend value (exact small integers).
fn div<R: Real>(table: &[f64; NODES], k: usize) -> R {
    R::from_f64(table.get(k).copied().unwrap_or(1.0))
}

impl<R: Real> Ias15<R> {
    /// Start an integration at time `t0` with positions `x0`, velocities `v0` and a first trial
    /// step `dt0` (its sign sets the direction; the step control adapts it from the first step).
    ///
    /// # Errors
    /// `FA-E0402` for an invalid configuration or a zero/non-finite `dt0`, `FA-E0403` if the
    /// slices do not match `dimension`, `FA-E0400` for non-finite initial values.
    pub fn new(
        dimension: usize,
        t0: R,
        x0: &[R],
        v0: &[R],
        dt0: R,
        cfg: Ias15Config,
    ) -> Result<Self, IntegratorError> {
        cfg.validate()?;
        check_len("initial position", x0.len(), dimension)?;
        check_len("initial velocity", v0.len(), dimension)?;
        if !(dt0.is_finite() && dt0 != R::ZERO) {
            return Err(IntegratorError::Config {
                name: "dt0",
                value: dt0.to_f64(),
                valid: "finite and non-zero",
            });
        }
        if !t0.is_finite() || x0.iter().chain(v0).any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "initial state",
                t: t0.to_f64(),
            });
        }
        let zeros = || vec![R::ZERO; dimension];
        Ok(Self {
            cfg,
            rc: Radau::new(),
            n: dimension,
            t: t0,
            t_comp: R::ZERO,
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
            last: None,
        })
    }

    /// Current time.
    #[must_use]
    pub const fn time(&self) -> R {
        self.t
    }

    /// Current positions.
    #[must_use]
    pub fn positions(&self) -> &[R] {
        &self.x
    }

    /// Current velocities.
    #[must_use]
    pub fn velocities(&self) -> &[R] {
        &self.v
    }

    /// The step the next call to [`Ias15::step`] will try.
    #[must_use]
    pub const fn next_dt(&self) -> R {
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
            bm.fill(R::ZERO);
        }
    }

    /// Predict position and velocity at node fraction `tau` of step `dt` into `xs`, `vs`.
    #[expect(
        clippy::indexing_slicing,
        reason = "all state vectors have length self.n, checked in Ias15::new"
    )]
    fn predict(&mut self, tau: R, dt: R) {
        for i in 0..self.n {
            // Horner in τ for Σ_m coef_m τ^m / div with coef_0 = a0, coef_m = B_m.
            let mut sx = self.b[TERMS - 1][i] / div::<R>(&X_DIV, TERMS);
            let mut sv = self.b[TERMS - 1][i] / div::<R>(&V_DIV, TERMS);
            for m in (0..TERMS - 1).rev() {
                sx = sx.mul_add(tau, self.b[m][i] / div::<R>(&X_DIV, m + 1));
                sv = sv.mul_add(tau, self.b[m][i] / div::<R>(&V_DIV, m + 1));
            }
            sx = sx.mul_add(tau, self.a0[i] / div::<R>(&X_DIV, 0));
            sv = sv.mul_add(tau, self.a0[i] / div::<R>(&V_DIV, 0));
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
                let mut s = R::ZERO;
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
            if self.cfg.pc_b_from_g {
                for m in 1..=TERMS {
                    let mut s = R::ZERO;
                    for k in (m..=TERMS).rev() {
                        s = self.rc.c[k][m].mul_add(self.g[k - 1][i], s);
                    }
                    self.b[m - 1][i] = s;
                }
            } else {
                for m in 1..=node {
                    self.b[m - 1][i] = self.rc.c[node][m].mul_add(delta, self.b[m - 1][i]);
                }
            }
        }
    }

    /// Scale `B` for a retried step from the same start: `a(τ_old) = a(q τ_new)`.
    fn rescale_b(&mut self, q: R) {
        let mut qm = R::ONE;
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
    fn extrapolate_b(&mut self, q: R) {
        // Binomial coefficients C(m, k) for m, k <= 7, built by Pascal's rule (exact integers).
        let mut binom = [[0.0_f64; NODES]; NODES];
        for m in 0..NODES {
            binom[m][0] = 1.0;
            for k in 1..=m {
                binom[m][k] = binom[m - 1][k - 1] + if k < m { binom[m - 1][k] } else { 0.0 };
            }
        }
        for i in 0..self.n {
            let old: [R; NODES] =
                std::array::from_fn(|m| if m == 0 { R::ZERO } else { self.b[m - 1][i] });
            let mut qk = R::ONE;
            for k in 1..=TERMS {
                qk *= q;
                let mut s = R::ZERO;
                for m in k..=TERMS {
                    s = R::from_f64(binom[m][k]).mul_add(old[m], s);
                }
                self.b[k - 1][i] = qk * s;
            }
        }
    }

    /// Predictor–corrector iterations for a step of length `dt`. Returns the iteration count,
    /// the largest acceleration component seen, and whether the iteration ran out (hit
    /// `max_iterations` without converging or stalling).
    fn correct<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        dt: R,
    ) -> Result<(u32, R, Exit), IntegratorError> {
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
                        t: t_node.to_f64(),
                    });
                }
                max_a = max_a.max(max_abs(&self.an));
                self.update_node(node);
            }
            let change = self.b[TERMS - 1]
                .iter()
                .zip(&self.b7_prev)
                .fold(R::ZERO, |m, (&a, &b)| m.max((a - b).abs()));
            let err = if max_a > R::ZERO {
                (change / max_a).to_f64()
            } else {
                0.0
            };
            // At least two iterations: with a zero predictor, B7 can look converged after one
            // pass while the lower coefficients are not.
            let converged = iterations >= 2 && err <= self.cfg.pc_tolerance;
            let stalled = iterations > 2 && err >= last_err;
            if converged || stalled || iterations >= self.cfg.max_iterations {
                self.stats.iterations += u64::from(iterations);
                let exit = if converged {
                    Exit::Converged
                } else if stalled {
                    Exit::Stalled(err)
                } else {
                    Exit::RanOut(err)
                };
                return Ok((iterations, max_a, exit));
            }
            last_err = err;
        }
    }

    /// Keep the converged polynomial of the step about to be taken (dense output, NUM-010).
    fn save_last_step(&mut self, dt: R) {
        self.last = Some(LastStep {
            t0: self.t,
            dt,
            x0: self
                .x
                .iter()
                .zip(&self.x_comp)
                .map(|(&a, &c)| a - c)
                .collect(),
            v0: self
                .v
                .iter()
                .zip(&self.v_comp)
                .map(|(&a, &c)| a - c)
                .collect(),
            a0: self.a0.clone(),
            b: self.b.clone(),
        });
    }

    /// Compensated update of `x`, `v` to the end (`τ = 1`) of a converged step of length `dt`.
    #[expect(
        clippy::indexing_slicing,
        reason = "fixed TERMS x n tables; indices bounded by TERMS and self.n"
    )]
    fn advance_state(&mut self, dt: R) -> Result<(), IntegratorError> {
        for i in 0..self.n {
            let mut sx = self.b[TERMS - 1][i] / div::<R>(&X_DIV, TERMS);
            let mut sv = self.b[TERMS - 1][i] / div::<R>(&V_DIV, TERMS);
            for m in (0..TERMS - 1).rev() {
                sx += self.b[m][i] / div::<R>(&X_DIV, m + 1);
                sv += self.b[m][i] / div::<R>(&V_DIV, m + 1);
            }
            sx += self.a0[i] / div::<R>(&X_DIV, 0);
            sv += self.a0[i] / div::<R>(&V_DIV, 0);
            let dx = dt * sx.mul_add(dt, self.v[i]);
            let dv = dt * sv;
            kahan_add(&mut self.x[i], &mut self.x_comp[i], dx);
            kahan_add(&mut self.v[i], &mut self.v_comp[i], dv);
        }
        if self.x.iter().chain(&self.v).any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "position",
                t: self.t.to_f64(),
            });
        }
        Ok(())
    }

    /// Step-size proposal after a converged step of length `dt` (Rein & Spiegel 2015 global
    /// criterion, Eq. 11): `dt·(ε/b̃)^(1/7)` with `b̃ = max|B₇|/max|a|`; the factor is evaluated
    /// in f64 for every backend.
    fn proposal(&self, dt: R, max_a: R) -> Result<R, IntegratorError> {
        let safety = R::from_f64(self.cfg.safety);
        let b7 = max_abs(&self.b[TERMS - 1]);
        let ratio = if max_a > R::ZERO {
            (b7 / max_a).to_f64()
        } else {
            0.0
        };
        let dt_new = if ratio > 0.0 && ratio.is_finite() {
            dt * R::from_f64((self.cfg.epsilon / ratio).powf(1.0 / 7.0))
        } else {
            dt / safety
        };
        if dt_new.is_finite() {
            Ok(dt_new)
        } else {
            Err(IntegratorError::NonFinite {
                what: "step",
                t: self.t.to_f64(),
            })
        }
    }

    /// Take one accepted step, never stepping past `t_limit` (pass an infinite value, e.g.
    /// `R::from_f64(f64::INFINITY)`, for no limit in the direction of integration).
    ///
    /// Position and velocity at the end of the step are, with `τ = 1`,
    /// `x = x₀ + Δt·v₀ + Δt²·(a₀/2 + B₁/6 + B₂/12 + … + B₇/72)` and
    /// `v = v₀ + Δt·(a₀ + B₁/2 + … + B₇/8)`, added with compensated summation.
    ///
    /// # Errors
    /// Force-model errors, `FA-E0400` on non-finite values, `FA-E0401` when the step collapses.
    pub fn step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t_limit: R,
    ) -> Result<StepReport<R>, IntegratorError> {
        check_len("system", sys.dimension(), self.n)?;
        sys.acceleration(self.t, &self.x, &self.v, &mut self.a0)?;
        self.stats.force_evaluations += 1;
        if self.a0.iter().any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "acceleration",
                t: self.t.to_f64(),
            });
        }
        let safety = R::from_f64(self.cfg.safety);
        let mut rejections = 0_u32;
        loop {
            let remaining = t_limit - self.t;
            let mut dt = self.dt_trial;
            let clamped = remaining.is_finite() && dt.abs() >= remaining.abs();
            if clamped {
                dt = remaining;
            }
            if dt == R::ZERO {
                return Ok(StepReport {
                    t: self.t,
                    dt_done: R::ZERO,
                    iterations: 0,
                    reached_limit: true,
                });
            }
            let (iterations, max_a, exit) = self.correct(sys, dt)?;

            let mut dt_new = self.proposal(dt, max_a)?;
            if dt_new.abs() < safety * dt.abs() {
                rejections += 1;
                self.stats.rejected += 1;
                if dt_new.abs().to_f64() <= self.cfg.min_dt
                    || dt_new == R::ZERO
                    || rejections > self.cfg.max_rejections
                {
                    return Err(IntegratorError::StepUnderflow {
                        dt: dt_new.to_f64(),
                        min_dt: self.cfg.min_dt,
                        t: self.t.to_f64(),
                    });
                }
                self.rescale_b(dt_new / dt);
                self.dt_trial = dt_new;
                continue;
            }
            if dt_new.abs() > dt.abs() / safety {
                dt_new = dt / safety;
            }
            // When clamped, `dt_new` was computed from the short step; keep the step the
            // controller had proposed before clamping, if that is larger.
            let next = if clamped && self.dt_trial.abs() > dt_new.abs() {
                self.dt_trial
            } else {
                dt_new
            };

            if self.cfg.dense_output {
                self.save_last_step(dt);
            }
            self.advance_state(dt)?;
            kahan_add(&mut self.t, &mut self.t_comp, dt);
            if clamped {
                // Land exactly on the limit.
                self.t = t_limit;
                self.t_comp = R::ZERO;
            }
            self.stats.steps += 1;
            match exit {
                Exit::Converged => {}
                Exit::Stalled(err) => {
                    self.stats.stalled += 1;
                    self.stats.max_pc_error = self.stats.max_pc_error.max(err);
                }
                Exit::RanOut(err) => {
                    self.stats.unconverged += 1;
                    self.stats.max_pc_error = self.stats.max_pc_error.max(err);
                }
            }
            let q = next / dt;
            if q.abs() <= R::ONE / safety {
                self.extrapolate_b(q);
            } else {
                // Only after a step shortened to hit `t_limit`: extrapolating that short step's
                // polynomial by a large factor amplifies its round-off, so restart the predictor
                // from zero (as on the first step) instead.
                for bm in &mut self.b {
                    bm.fill(R::ZERO);
                }
                self.stats.cold_starts += 1;
            }
            self.dt_trial = next;
            return Ok(StepReport {
                t: self.t,
                dt_done: dt,
                iterations,
                reached_limit: clamped,
            });
        }
    }

    /// The time span `(t₀, t₀ + Δt)` of the last accepted step, when dense output is on.
    #[must_use]
    pub fn last_step_span(&self) -> Option<(R, R)> {
        self.last.as_ref().map(|l| (l.t0, l.t0 + l.dt))
    }

    /// Dense output (NUM-010): position and velocity at time `t` inside the last accepted step,
    /// from that step's collocation polynomial, written into `x` and `v`.
    ///
    /// # Errors
    /// `FA-E0402` if dense output is off, no step was taken yet, or `t` lies outside the last
    /// step; `FA-E0403` if `x` or `v` has the wrong length.
    pub fn dense_state(&self, t: R, x: &mut [R], v: &mut [R]) -> Result<(), IntegratorError> {
        check_len("dense-output position", x.len(), self.n)?;
        check_len("dense-output velocity", v.len(), self.n)?;
        let last = self.last.as_ref().ok_or(IntegratorError::Config {
            name: "dense_output",
            value: 0.0,
            valid: "enabled (Ias15Config::dense_output) and at least one accepted step",
        })?;
        let tau = (t - last.t0) / last.dt;
        // Accept round-off excursions of a few ulp at either end of the step.
        let slack = R::EPSILON.scale(16.0);
        if !(tau >= -slack && tau <= R::ONE + slack) {
            return Err(IntegratorError::Config {
                name: "dense-output time (fraction of the last step)",
                value: tau.to_f64(),
                valid: "0 <= tau <= 1",
            });
        }
        let tau = tau.max(R::ZERO).min(R::ONE);
        last.eval(tau, x, v);
        Ok(())
    }

    /// Event location (NUM-010): the time of a sign change of `g(t, x, v)` inside the last
    /// accepted step, found on the dense-output polynomial by the Illinois variant of regula
    /// falsi in the step fraction `τ` until the bracket is below 4 ulp of 1 (or 200 iterations). Returns `None` when `g`
    /// has the same sign at both ends of the step. For a heliocentric orbit,
    /// `g = r·v` locates perihelion (rising through zero) and aphelion (falling).
    ///
    /// # Errors
    /// The errors of [`Ias15::dense_state`].
    pub fn locate_event<G>(&self, g: G) -> Result<Option<R>, IntegratorError>
    where
        G: Fn(R, &[R], &[R]) -> R,
    {
        let Some(last) = self.last.as_ref() else {
            return Err(IntegratorError::Config {
                name: "dense_output",
                value: 0.0,
                valid: "enabled (Ias15Config::dense_output) and at least one accepted step",
            });
        };
        let t0 = last.t0;
        // Solve in the step fraction τ ∈ [0, 1] (exact end points), then map to t = t₀ + τΔt.
        let (mut x, mut v) = (vec![R::ZERO; self.n], vec![R::ZERO; self.n]);
        let mut at = |tau: R| -> R {
            last.eval(tau, &mut x, &mut v);
            g(tau.mul_add(last.dt, t0), &x, &v)
        };
        let to_t = |tau: R| tau.mul_add(last.dt, t0);
        let (mut a, mut b) = (R::ZERO, R::ONE);
        let (mut fa, mut fb) = (at(a), at(b));
        if fa == R::ZERO {
            return Ok(Some(to_t(a)));
        }
        if (fa < R::ZERO) == (fb < R::ZERO) {
            return Ok(None);
        }
        let mut side = 0_i8;
        let tiny = R::EPSILON.scale(4.0);
        for _ in 0..200 {
            let c = (a * fb - b * fa) / (fb - fa);
            let fc = at(c);
            if fc == R::ZERO || (b - a).abs() <= tiny {
                return Ok(Some(to_t(c)));
            }
            if (fc < R::ZERO) == (fb < R::ZERO) {
                b = c;
                fb = fc;
                if side == -1 {
                    fa = fa.scale(0.5);
                }
                side = -1;
            } else {
                a = c;
                fa = fc;
                if side == 1 {
                    fb = fb.scale(0.5);
                }
                side = 1;
            }
        }
        Ok(Some(to_t((a * fb - b * fa) / (fb - fa))))
    }

    /// Integrate to `t_end`, calling `observer` after every accepted step.
    ///
    /// # Errors
    /// Any error of [`Ias15::step`], or the first error returned by `observer`.
    pub fn integrate_to<S, F>(
        &mut self,
        sys: &S,
        t_end: R,
        mut observer: F,
    ) -> Result<(), IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        F: FnMut(&Self, &StepReport<R>) -> Result<(), IntegratorError>,
    {
        if !t_end.is_finite() {
            return Err(IntegratorError::Config {
                name: "t_end",
                value: t_end.to_f64(),
                valid: "finite",
            });
        }
        if (t_end - self.t) * self.dt_trial < R::ZERO {
            self.reverse();
        }
        loop {
            let report = self.step(sys, t_end)?;
            if report.dt_done != R::ZERO {
                observer(self, &report)?;
            }
            if report.reached_limit {
                return Ok(());
            }
        }
    }
}

/// A binary64 pair `(hi, lo)` whose exact sum is one backend value (`lo = 0` for f64).
pub type Parts = [f64; 2];

/// Everything an [`Ias15`] carries from one accepted step to the next, as `(hi, lo)` binary64
/// pairs (CI-010 checkpoint/resume). Restoring it with [`Ias15::restore`] and stepping on gives
/// bit-for-bit the steps the original integrator would have taken.
///
/// Carried state: the time and its Kahan compensation, positions and velocities and their
/// compensations, the trial step, the eight predictor coefficient vectors `B₁…B₇`
/// (`b[m-1][i]`; IAS15's "predictor history": the next step starts from their extrapolation),
/// and the run counters. Not carried, because every step recomputes it before use: the Newton
/// coefficients `g` (rebuilt from `B`), the start-of-step acceleration, the node scratch
/// vectors, and the dense-output polynomial of the last step (so [`Ias15::dense_state`] needs one
/// new step after a restore).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ias15Snapshot {
    /// [`Real::NAME`] of the backend that took the snapshot.
    pub backend: String,
    /// Settings.
    pub config: Ias15Config,
    /// Number of coordinates.
    pub dimension: u64,
    /// Time.
    pub t: Parts,
    /// Kahan compensation of the time.
    pub t_comp: Parts,
    /// Positions.
    pub x: Vec<Parts>,
    /// Velocities.
    pub v: Vec<Parts>,
    /// Compensations of the positions.
    pub x_comp: Vec<Parts>,
    /// Compensations of the velocities.
    pub v_comp: Vec<Parts>,
    /// The step the next call to [`Ias15::step`] will try.
    pub dt_trial: Parts,
    /// Predictor coefficients `B₁…B₇`, each of length `dimension`.
    pub b: Vec<Vec<Parts>>,
    /// Run counters.
    pub stats: Ias15Stats,
}

pub(crate) fn checkpoint_err(what: &'static str, problem: impl Into<String>) -> IntegratorError {
    IntegratorError::Checkpoint {
        what,
        problem: problem.into(),
    }
}

/// Split a value into parts, refusing a backend whose parts do not reconstruct it exactly.
pub(crate) fn to_parts_exact<R: Real>(what: &'static str, z: R) -> Result<Parts, IntegratorError> {
    let (hi, lo) = z.to_parts();
    let back = R::from_parts(hi, lo);
    if back.to_parts() == (hi, lo) && (back == z || (z.is_nan() && back.is_nan())) {
        Ok([hi, lo])
    } else {
        Err(checkpoint_err(
            what,
            format!(
                "the {} backend does not split {:e} into an exact (hi, lo) pair",
                R::NAME,
                z.to_f64()
            ),
        ))
    }
}

pub(crate) fn parts_vec<R: Real>(
    what: &'static str,
    zs: &[R],
) -> Result<Vec<Parts>, IntegratorError> {
    zs.iter().map(|&z| to_parts_exact(what, z)).collect()
}

/// Rebuild backend values from parts, checking the length and finiteness.
pub(crate) fn from_parts_vec<R: Real>(
    what: &'static str,
    parts: &[Parts],
    n: usize,
) -> Result<Vec<R>, IntegratorError> {
    if parts.len() != n {
        return Err(checkpoint_err(
            what,
            format!("has {} values, expected {n}", parts.len()),
        ));
    }
    parts.iter().map(|&p| from_parts_one(what, p)).collect()
}

pub(crate) fn from_parts_one<R: Real>(
    what: &'static str,
    [hi, lo]: Parts,
) -> Result<R, IntegratorError> {
    let z = R::from_parts(hi, lo);
    if !z.is_finite() {
        return Err(checkpoint_err(
            what,
            format!("non-finite value ({hi:e}, {lo:e})"),
        ));
    }
    if z.to_parts() != (hi, lo) {
        return Err(checkpoint_err(
            what,
            format!(
                "({hi:e}, {lo:e}) is not a normalised {} value (was the file edited?)",
                R::NAME
            ),
        ));
    }
    Ok(z)
}

impl<R: Real> Ias15<R> {
    /// The state carried between steps, for a checkpoint (CI-010). Take it between steps (for
    /// example in the observer of [`Ias15::integrate_to`] or after [`Ias15::step`]).
    ///
    /// # Errors
    /// `FA-E0405` if the backend cannot represent a value as an exact `(hi, lo)` pair.
    pub fn snapshot(&self) -> Result<Ias15Snapshot, IntegratorError> {
        Ok(Ias15Snapshot {
            backend: R::NAME.to_owned(),
            config: self.cfg,
            dimension: u64::try_from(self.n)
                .map_err(|_| checkpoint_err("dimension", "does not fit in 64 bits"))?,
            t: to_parts_exact("time", self.t)?,
            t_comp: to_parts_exact("time compensation", self.t_comp)?,
            x: parts_vec("positions", &self.x)?,
            v: parts_vec("velocities", &self.v)?,
            x_comp: parts_vec("position compensations", &self.x_comp)?,
            v_comp: parts_vec("velocity compensations", &self.v_comp)?,
            dt_trial: to_parts_exact("trial step", self.dt_trial)?,
            b: self
                .b
                .iter()
                .map(|bm| parts_vec("predictor coefficients", bm))
                .collect::<Result<_, _>>()?,
            stats: self.stats,
        })
    }

    /// An integrator that continues exactly where the one that took `snap` stopped.
    ///
    /// # Errors
    /// `FA-E0405` for another backend, inconsistent lengths, or non-finite or non-normalised
    /// values; `FA-E0402` for invalid settings.
    pub fn restore(snap: &Ias15Snapshot) -> Result<Self, IntegratorError> {
        if snap.backend != R::NAME {
            return Err(checkpoint_err(
                "backend",
                format!(
                    "snapshot taken with {:?}, restoring into {:?}",
                    snap.backend,
                    R::NAME
                ),
            ));
        }
        snap.config.validate()?;
        let n = usize::try_from(snap.dimension)
            .map_err(|_| checkpoint_err("dimension", "does not fit this platform"))?;
        if snap.b.len() != TERMS {
            return Err(checkpoint_err(
                "predictor coefficients",
                format!("{} vectors, expected {TERMS}", snap.b.len()),
            ));
        }
        let b_vecs = snap
            .b
            .iter()
            .map(|bm| from_parts_vec::<R>("predictor coefficients", bm, n))
            .collect::<Result<Vec<_>, _>>()?;
        let mut b_iter = b_vecs.into_iter();
        let b: [Vec<R>; TERMS] = std::array::from_fn(|_| b_iter.next().unwrap_or_default());
        let dt_trial: R = from_parts_one("trial step", snap.dt_trial)?;
        if dt_trial == R::ZERO {
            return Err(checkpoint_err("trial step", "is zero"));
        }
        let zeros = || vec![R::ZERO; n];
        Ok(Self {
            cfg: snap.config,
            rc: Radau::new(),
            n,
            t: from_parts_one("time", snap.t)?,
            t_comp: from_parts_one("time compensation", snap.t_comp)?,
            x: from_parts_vec("positions", &snap.x, n)?,
            v: from_parts_vec("velocities", &snap.v, n)?,
            x_comp: from_parts_vec("position compensations", &snap.x_comp, n)?,
            v_comp: from_parts_vec("velocity compensations", &snap.v_comp, n)?,
            dt_trial,
            b,
            g: std::array::from_fn(|_| zeros()),
            a0: zeros(),
            an: zeros(),
            xs: zeros(),
            vs: zeros(),
            b7_prev: zeros(),
            stats: snap.stats,
            last: None,
        })
    }
}
