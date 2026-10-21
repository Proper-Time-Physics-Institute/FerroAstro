// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Fixed-step Adams–Bashforth–Moulton integration in PECE and PECEC modes (NUM-002).
//!
//! Clean-room from the numerical-analysis literature only (DEP-008): Hairer, Nørsett & Wanner
//! 1993, *Solving Ordinary Differential Equations I* (2nd ed.), §III.1 (explicit and implicit Adams
//! methods; the P(EC)^m E^{1−t} predictor–corrector modes) and §II.10 (the DOP853 formula used for
//! the start-up), and the configuration of Subbotin, Kodukov & Pavlov 2023 (`CeMDA`,
//! doi:10.1007/s10569-023-10139-2: ABM13 PECEC, h = 1/16 d, DP8 start-up at h/8). No ephemeris
//! or N-body integrator source was consulted.
//!
//! # Scheme
//! A second-order system `ẍ = a(t, x, ẋ)` is integrated as the first-order system
//! `y = (x, v)`, `y′ = f(t, y) = (v, a(t, x, v))`, so velocity-dependent forces (1PN/EIH) need no
//! special treatment. With the history `f_n, f_{n−1}, …, f_{n−p+1}` at the grid `t_k = t₀ + k h`:
//!
//! * **P**: `y* = y_n + h Σ_j β_j f_{n−j}` (p-step Adams–Bashforth, order p);
//! * **E**: `f* = f(t_{n+1}, y*)`;
//! * **C**: `y_{n+1} = y_n + h (α_0 f* + Σ_{j≥1} α_j f_{n+1−j})` (Adams–Moulton on the p nodes
//!   `t_{n+1} … t_{n−p+2}`, order p);
//! * **PECE** evaluates once more at the corrected state, and that derivative enters the history;
//! * **PECEC** (= P(EC)², HNW notation) evaluates at the first corrected state, corrects again with
//!   it, and keeps it in the history; the state is the second correction.
//!
//! Both modes cost **two force evaluations per step**, and both are of order p (8 ≤ p ≤ 14,
//! default 13). The coefficients are exact rationals ([`crate::rational`]) converted once to the
//! backend. Each sum runs from the oldest history entry to the newest, with `mul_add`.
//!
//! # Start-up
//! The first `p − 1` grid points come from the DP8 formula at `h / startup_substeps` (8 by
//! default). They are produced on the first call to [`Abm::step`] and then reported one step at a
//! time, so the caller sees steps of `h` from the start. Their dense output is the interpolant
//! through the `p` start-up nodes.
//!
//! # State update and time
//! `t_k = t₀ + k·h` is computed from the step index, never accumulated. With
//! [`AbmConfig::compensated`] (default) the increment `h·Σ` is added with Kahan compensation (the
//! NUM-001 policy, as IAS15). Without it the sums are plain, as in a straightforward
//! implementation of the scheme (the Subbotin et al. "f64" configuration).
//!
//! # Dense output (NUM-010)
//! Inside the last step, `y(t_n + θh) = y_n + h Σ_j W_j(θ) f_j` with `W_j(θ) = ∫₀^θ L_j(s) ds` over
//! the derivatives the final corrector used, so `θ = 1` gives the corrected state up to rounding.
//! The interpolant is of order p.

use crate::dp8::Dp8;
use crate::rational::{
    Rational, adams_bashforth, adams_moulton, moulton_offsets, weight_polynomials,
};
use ferroastro_integrators::compensated::kahan_add;
use ferroastro_integrators::{IntegratorError, Parts, SecondOrderSystem};
use ferroastro_real::Real;

/// Smallest configurable order (NUM-002).
pub const MIN_ORDER: u32 = 8;
/// Largest configurable order (NUM-002).
pub const MAX_ORDER: u32 = 14;
/// Default order (NUM-002; Subbotin et al. 2023).
pub const DEFAULT_ORDER: u32 = 13;
/// Default number of DP8 substeps per ABM step in the start-up ("DP8 at h/8").
pub const DEFAULT_STARTUP_SUBSTEPS: u32 = 8;

/// Predictor–corrector mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Corrector {
    /// P, E, C, E: the derivative at the corrected state enters the history.
    Pece,
    /// P(EC)²: P, E, C, E, C; the derivative at the first corrected state enters the history.
    Pecec,
}

impl Corrector {
    /// `"PECE"` or `"PECEC"`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Corrector::Pece => "PECE",
            Corrector::Pecec => "PECEC",
        }
    }
}

/// ABM settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AbmConfig {
    /// Order p, 8 … 14. Default 13.
    pub order: u32,
    /// Mode. Default PECEC.
    pub corrector: Corrector,
    /// DP8 substeps per ABM step in the start-up, 1 … 64. Default 8.
    pub startup_substeps: u32,
    /// Kahan-compensated state update. Default on.
    pub compensated: bool,
}

impl Default for AbmConfig {
    fn default() -> Self {
        Self {
            order: DEFAULT_ORDER,
            corrector: Corrector::Pecec,
            startup_substeps: DEFAULT_STARTUP_SUBSTEPS,
            compensated: true,
        }
    }
}

impl AbmConfig {
    fn validate(&self) -> Result<(), IntegratorError> {
        if !(MIN_ORDER..=MAX_ORDER).contains(&self.order) {
            return Err(IntegratorError::Config {
                name: "abm order",
                value: f64::from(self.order),
                valid: "8 <= order <= 14 (NUM-002)",
            });
        }
        if !(1..=64).contains(&self.startup_substeps) {
            return Err(IntegratorError::Config {
                name: "abm startup_substeps",
                value: f64::from(self.startup_substeps),
                valid: "1 <= startup_substeps <= 64",
            });
        }
        Ok(())
    }

    /// One-line description, e.g. `ABM13 PECEC (DP8 start-up at h/8, compensated)`.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "ABM{} {} (DP8 start-up at h/{}, {} state update)",
            self.order,
            self.corrector.label(),
            self.startup_substeps,
            if self.compensated {
                "compensated"
            } else {
                "plain"
            }
        )
    }
}

/// Counters accumulated over a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AbmStats {
    /// Steps of `h` taken (start-up steps included).
    pub steps: u64,
    /// Of which start-up steps.
    pub startup_steps: u64,
    /// Force evaluations (start-up included).
    pub force_evaluations: u64,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbmStepReport<R: Real = f64> {
    /// Time at the end of the step.
    pub t: R,
    /// The step (always `h`).
    pub dt: R,
    /// True for the start-up steps.
    pub startup: bool,
}

/// The start-up: states and derivatives at the first `p` grid points (node 0 is the initial
/// state).
#[derive(Debug, Clone)]
struct Startup<R: Real> {
    ys: Vec<Vec<R>>,
    fs: Vec<Vec<R>>,
}

/// The last step, for dense output.
#[derive(Debug, Clone)]
enum Last<R: Real> {
    /// Start-up step from grid point `k` (nodes 0 … p−1 of the start-up).
    Startup { k: u64 },
    /// Regular step from grid point `k`: the start state and the corrector's new derivative.
    Regular { k: u64, y0: Vec<R>, f_used: Vec<R> },
}

/// The ABM integrator state, in backend `R` (default `f64`).
#[derive(Debug, Clone)]
pub struct Abm<R: Real = f64> {
    cfg: AbmConfig,
    p: usize,
    n: usize,
    t0: R,
    h: R,
    k: u64,
    y: Vec<R>,
    yc: Vec<R>,
    /// `hist[j]` = `f_{k−j}`, `j = 0 … p−1` (empty until the start-up is complete).
    hist: Vec<Vec<R>>,
    startup: Option<Startup<R>>,
    ab: Vec<R>,
    am: Vec<R>,
    /// Dense weight polynomials of the regular step (corrector nodes).
    dense: Vec<Vec<R>>,
    dp8: Dp8<R>,
    stats: AbmStats,
    last: Option<Last<R>>,
    yp: Vec<R>,
    fp: Vec<R>,
    f1: Vec<R>,
    partial: Vec<R>,
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

fn coefficient_error(what: &'static str, p: usize) -> IntegratorError {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the order is at most 14 here; the value is only reported"
    )]
    let value = p as f64;
    IntegratorError::Config {
        name: what,
        value,
        valid: "an order whose exact Adams coefficients fit in i128 (8 ... 14)",
    }
}

fn to_reals<R: Real>(qs: &[Rational], p: usize) -> Result<Vec<R>, IntegratorError> {
    qs.iter()
        .map(|q| {
            q.to_real()
                .ok_or_else(|| coefficient_error("abm coefficient", p))
        })
        .collect()
}

fn dense_polys<R: Real>(offsets: &[i64], p: usize) -> Result<Vec<Vec<R>>, IntegratorError> {
    weight_polynomials(offsets)
        .ok_or_else(|| coefficient_error("abm dense weights", p))?
        .iter()
        .map(|w| to_reals(w, p))
        .collect()
}

/// `W(θ) = Σ_m w[m] θ^{m+1}` by Horner.
fn horner<R: Real>(w: &[R], theta: R) -> R {
    let mut acc = R::ZERO;
    for &c in w.iter().rev() {
        acc = acc.mul_add(theta, c);
    }
    acc * theta
}

#[expect(
    clippy::cast_precision_loss,
    reason = "step indices stay far below 2^53 in any feasible run, so the conversion is exact"
)]
fn index<R: Real>(k: u64) -> R {
    R::from_f64(k as f64)
}

/// `f = (v, a(t, x, v))` for `y = (x, v)`.
fn evaluate<R: Real, S: SecondOrderSystem<R> + ?Sized>(
    sys: &S,
    n: usize,
    t: R,
    y: &[R],
    f: &mut [R],
) -> Result<(), IntegratorError> {
    let (x, v) = y.split_at(n);
    let (fx, fv) = f.split_at_mut(n);
    fx.copy_from_slice(v);
    sys.acceleration(t, x, v, fv)?;
    if fv.iter().any(|z| !z.is_finite()) {
        return Err(IntegratorError::NonFinite {
            what: "acceleration",
            t: t.to_f64(),
        });
    }
    Ok(())
}

impl<R: Real> Abm<R> {
    /// Start an integration at `t0` with positions `x0`, velocities `v0` and the fixed step `h`
    /// (its sign sets the direction).
    ///
    /// # Errors
    /// `FA-E0402` for an invalid configuration or a zero/non-finite `h`, `FA-E0403` if the
    /// slices do not match `dimension`, `FA-E0400` for non-finite initial values.
    pub fn new(
        dimension: usize,
        t0: R,
        x0: &[R],
        v0: &[R],
        h: R,
        cfg: AbmConfig,
    ) -> Result<Self, IntegratorError> {
        cfg.validate()?;
        check_len("initial position", x0.len(), dimension)?;
        check_len("initial velocity", v0.len(), dimension)?;
        if !(h.is_finite() && h != R::ZERO) {
            return Err(IntegratorError::Config {
                name: "abm step h",
                value: h.to_f64(),
                valid: "finite and non-zero",
            });
        }
        if !t0.is_finite() || x0.iter().chain(v0).any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "initial state",
                t: t0.to_f64(),
            });
        }
        let mut y = x0.to_vec();
        y.extend_from_slice(v0);
        Self::assemble(
            cfg,
            dimension,
            t0,
            h,
            0,
            y,
            None,
            Vec::new(),
            AbmStats::default(),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "one internal constructor shared by new() and restore()"
    )]
    fn assemble(
        cfg: AbmConfig,
        n: usize,
        t0: R,
        h: R,
        k: u64,
        y: Vec<R>,
        yc: Option<Vec<R>>,
        hist: Vec<Vec<R>>,
        stats: AbmStats,
    ) -> Result<Self, IntegratorError> {
        let p = usize::try_from(cfg.order).map_err(|_| coefficient_error("abm order", 0))?;
        let ab = to_reals(
            &adams_bashforth(p).ok_or_else(|| coefficient_error("abm predictor", p))?,
            p,
        )?;
        let am = to_reals(
            &adams_moulton(p).ok_or_else(|| coefficient_error("abm corrector", p))?,
            p,
        )?;
        let offsets = moulton_offsets(p).ok_or_else(|| coefficient_error("abm nodes", p))?;
        let dense = dense_polys(&offsets, p)?;
        let m = 2 * n;
        let zeros = || vec![R::ZERO; m];
        Ok(Self {
            cfg,
            p,
            n,
            t0,
            h,
            k,
            yc: yc.unwrap_or_else(zeros),
            y,
            hist,
            startup: None,
            ab,
            am,
            dense,
            dp8: Dp8::new(m),
            stats,
            last: None,
            yp: zeros(),
            fp: zeros(),
            f1: zeros(),
            partial: zeros(),
        })
    }

    /// The settings.
    #[must_use]
    pub const fn config(&self) -> &AbmConfig {
        &self.cfg
    }

    /// Number of coordinates of the second-order system.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.n
    }

    /// The fixed step.
    #[must_use]
    pub const fn step_size(&self) -> R {
        self.h
    }

    /// Time of grid point `k`: `t₀ + k·h`.
    fn grid(&self, k: u64) -> R {
        index::<R>(k).mul_add(self.h, self.t0)
    }

    /// Current time.
    #[must_use]
    pub fn time(&self) -> R {
        self.grid(self.k)
    }

    /// Current positions.
    #[must_use]
    pub fn positions(&self) -> &[R] {
        self.y.get(..self.n).unwrap_or(&[])
    }

    /// Current velocities.
    #[must_use]
    pub fn velocities(&self) -> &[R] {
        self.y.get(self.n..).unwrap_or(&[])
    }

    /// Run counters.
    #[must_use]
    pub const fn stats(&self) -> AbmStats {
        self.stats
    }

    /// Run the DP8 start-up: states and derivatives at grid points 0 … p−1.
    fn run_startup<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<Startup<R>, IntegratorError> {
        let (n, p) = (self.n, self.p);
        let sub = self.cfg.startup_substeps;
        let hs = self.h / R::from_f64(f64::from(sub));
        let mut evals = 0_u64;
        let mut f = |t: R, y: &[R], out: &mut [R]| -> Result<(), IntegratorError> {
            evals += 1;
            evaluate(sys, n, t, y, out)
        };
        let mut ys = Vec::with_capacity(p);
        let mut fs = Vec::with_capacity(p);
        let mut cur = self.y.clone();
        let mut f0 = vec![R::ZERO; 2 * n];
        f(self.t0, &cur, &mut f0)?;
        ys.push(cur.clone());
        fs.push(f0);
        let mut i = 0_u64;
        for j in 1..p {
            for _ in 0..sub {
                let ts = index::<R>(i).mul_add(hs, self.t0);
                self.dp8.step(&mut f, ts, hs, &mut cur)?;
                i += 1;
            }
            if cur.iter().any(|z| !z.is_finite()) {
                return Err(IntegratorError::NonFinite {
                    what: "start-up state",
                    t: self.grid(i / u64::from(sub)).to_f64(),
                });
            }
            let mut fj = vec![R::ZERO; 2 * n];
            let tj = self.grid(u64::try_from(j).unwrap_or(u64::MAX));
            f(tj, &cur, &mut fj)?;
            ys.push(cur.clone());
            fs.push(fj);
        }
        self.stats.force_evaluations += evals;
        Ok(Startup { ys, fs })
    }

    /// Take one step of `h`.
    ///
    /// # Errors
    /// `FA-E0400` for a non-finite state or acceleration; the force model's own error.
    pub fn step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<AbmStepReport<R>, IntegratorError> {
        check_len("system dimension", sys.dimension(), self.n)?;
        let last_startup = u64::try_from(self.p - 1).unwrap_or(u64::MAX);
        if self.k < last_startup {
            if self.startup.is_none() {
                let s = self.run_startup(sys)?;
                self.startup = Some(s);
            }
            let k0 = self.k;
            let next = usize::try_from(k0 + 1).unwrap_or(usize::MAX);
            let s = self.startup.as_ref().ok_or(IntegratorError::Checkpoint {
                what: "start-up",
                problem: "missing start-up data".into(),
            })?;
            let y = s.ys.get(next).ok_or(IntegratorError::Checkpoint {
                what: "start-up",
                problem: "start-up node out of range".into(),
            })?;
            self.y.copy_from_slice(y);
            self.yc.iter_mut().for_each(|c| *c = R::ZERO);
            self.k += 1;
            if self.k == last_startup {
                self.hist = s.fs.iter().rev().cloned().collect();
            }
            self.stats.steps += 1;
            self.stats.startup_steps += 1;
            self.last = Some(Last::Startup { k: k0 });
            return Ok(AbmStepReport {
                t: self.time(),
                dt: self.h,
                startup: true,
            });
        }
        self.regular_step(sys)
    }

    fn regular_step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<AbmStepReport<R>, IntegratorError> {
        let (n, p, h) = (self.n, self.p, self.h);
        let t1 = self.grid(self.k + 1);
        let y0: Vec<R> = self.y.iter().zip(&self.yc).map(|(&s, &c)| s - c).collect();
        // P: y* = y_n + h Σ β_j f_{n−j}, oldest term first.
        for (i, (yp, &yb)) in self.yp.iter_mut().zip(&y0).enumerate() {
            let mut acc = R::ZERO;
            for j in (0..p).rev() {
                let f = self.hist.get(j).and_then(|v| v.get(i)).copied();
                let b = self.ab.get(j).copied();
                if let (Some(f), Some(b)) = (f, b) {
                    acc = f.mul_add(b, acc);
                }
            }
            *yp = h.mul_add(acc, yb);
        }
        // E
        evaluate(sys, n, t1, &self.yp, &mut self.fp)?;
        // The history part of the corrector, Σ_{j≥1} α_j f_{n+1−j}, oldest first.
        for (i, part) in self.partial.iter_mut().enumerate() {
            let mut acc = R::ZERO;
            for j in (1..p).rev() {
                let f = self.hist.get(j - 1).and_then(|v| v.get(i)).copied();
                let a = self.am.get(j).copied();
                if let (Some(f), Some(a)) = (f, a) {
                    acc = f.mul_add(a, acc);
                }
            }
            *part = acc;
        }
        let a0 = self.am.first().copied().unwrap_or(R::ZERO);
        // C (into yp, reused as the corrected state)
        for ((yc, &yb), (&fp, &part)) in self
            .yp
            .iter_mut()
            .zip(&y0)
            .zip(self.fp.iter().zip(&self.partial))
        {
            *yc = h.mul_add(fp.mul_add(a0, part), yb);
        }
        // E at the corrected state
        evaluate(sys, n, t1, &self.yp, &mut self.f1)?;
        self.stats.force_evaluations += 2;
        // The derivative the final corrector uses, and the one that enters the history.
        let (f_used, f_hist) = match self.cfg.corrector {
            Corrector::Pece => (self.fp.clone(), self.f1.clone()),
            Corrector::Pecec => (self.f1.clone(), self.f1.clone()),
        };
        // Final state y_{n+1} = y_n + h (α_0 f_used + partial).
        let compensated = self.cfg.compensated;
        for (i, (ys, yc)) in self.y.iter_mut().zip(self.yc.iter_mut()).enumerate() {
            let fu = f_used.get(i).copied().unwrap_or(R::ZERO);
            let part = self.partial.get(i).copied().unwrap_or(R::ZERO);
            let inc = h * fu.mul_add(a0, part);
            if compensated {
                kahan_add(ys, yc, inc);
            } else {
                *ys += inc;
            }
        }
        if self.y.iter().any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "state",
                t: t1.to_f64(),
            });
        }
        self.hist.rotate_right(1);
        if let Some(front) = self.hist.first_mut() {
            *front = f_hist;
        }
        let k0 = self.k;
        self.k += 1;
        self.stats.steps += 1;
        self.startup = None;
        self.last = Some(Last::Regular { k: k0, y0, f_used });
        Ok(AbmStepReport {
            t: t1,
            dt: h,
            startup: false,
        })
    }

    /// The time span `(t_k, t_{k+1})` of the last step (`None` before the first step and after
    /// a restore).
    #[must_use]
    pub fn last_step_span(&self) -> Option<(R, R)> {
        let k = match self.last.as_ref()? {
            Last::Startup { k } | Last::Regular { k, .. } => *k,
        };
        Some((self.grid(k), self.grid(k + 1)))
    }

    /// The interpolant of the last step at fraction `θ`, written into `x` and `v`.
    fn eval_dense(&self, theta: R, x: &mut [R], v: &mut [R]) -> Result<(), IntegratorError> {
        let no_dense = || IntegratorError::Config {
            name: "abm dense output",
            value: 0.0,
            valid: "at least one step taken since the start or the last restore",
        };
        let last = self.last.as_ref().ok_or_else(no_dense)?;
        let n = self.n;
        let h = self.h;
        let mut out = vec![R::ZERO; 2 * n];
        match last {
            Last::Startup { k } => {
                let s = self.startup.as_ref().ok_or_else(no_dense)?;
                let k = i64::try_from(*k).map_err(|_| no_dense())?;
                let offsets: Vec<i64> = (0..self.p)
                    .map(|j| i64::try_from(j).map(|j| j - k))
                    .collect::<Result<_, _>>()
                    .map_err(|_| no_dense())?;
                let w: Vec<R> = dense_polys::<R>(&offsets, self.p)?
                    .iter()
                    .map(|wj| horner(wj, theta))
                    .collect();
                let y0 =
                    s.ys.get(usize::try_from(k).map_err(|_| no_dense())?)
                        .ok_or_else(no_dense)?;
                for (i, o) in out.iter_mut().enumerate() {
                    let mut acc = R::ZERO;
                    for (fj, &wj) in s.fs.iter().zip(&w).rev() {
                        acc = fj.get(i).copied().unwrap_or(R::ZERO).mul_add(wj, acc);
                    }
                    *o = h.mul_add(acc, y0.get(i).copied().unwrap_or(R::ZERO));
                }
            }
            Last::Regular { y0, f_used, .. } => {
                let w: Vec<R> = self.dense.iter().map(|wj| horner(wj, theta)).collect();
                for (i, o) in out.iter_mut().enumerate() {
                    let mut acc = R::ZERO;
                    // Nodes (oldest first): hist[p−1] … hist[1] (offsets −(p−2) … 0), then f_used.
                    for j in (1..self.p).rev() {
                        let f = self.hist.get(j).and_then(|v| v.get(i)).copied();
                        if let (Some(f), Some(&wj)) = (f, w.get(j)) {
                            acc = f.mul_add(wj, acc);
                        }
                    }
                    let fu = f_used.get(i).copied().unwrap_or(R::ZERO);
                    acc = fu.mul_add(w.first().copied().unwrap_or(R::ZERO), acc);
                    *o = h.mul_add(acc, y0.get(i).copied().unwrap_or(R::ZERO));
                }
            }
        }
        let (ox, ov) = out.split_at(n);
        x.copy_from_slice(ox);
        v.copy_from_slice(ov);
        Ok(())
    }

    /// Dense output (NUM-010): position and velocity at time `t` inside the last step.
    ///
    /// # Errors
    /// `FA-E0402` before the first step (or right after a restore) or for `t` outside the last
    /// step; `FA-E0403` if `x` or `v` has the wrong length.
    pub fn dense_state(&self, t: R, x: &mut [R], v: &mut [R]) -> Result<(), IntegratorError> {
        check_len("dense-output position", x.len(), self.n)?;
        check_len("dense-output velocity", v.len(), self.n)?;
        let (t0, _) = self.last_step_span().ok_or(IntegratorError::Config {
            name: "abm dense output",
            value: 0.0,
            valid: "at least one step taken since the start or the last restore",
        })?;
        let theta = (t - t0) / self.h;
        let slack = R::EPSILON.scale(16.0);
        if !(theta >= -slack && theta <= R::ONE + slack) {
            return Err(IntegratorError::Config {
                name: "dense-output time (fraction of the last step)",
                value: theta.to_f64(),
                valid: "0 <= theta <= 1",
            });
        }
        self.eval_dense(theta.max(R::ZERO).min(R::ONE), x, v)
    }

    /// Event location (NUM-010): the time of a sign change of `g(t, x, v)` inside the last step,
    /// found on the interpolant by the Illinois variant of regula falsi in the step fraction until
    /// the bracket is below 4 ulp of 1 (or 200 iterations). `None` when `g` has the same sign at
    /// both ends of the step. For a heliocentric orbit, `g = r·v` rising through zero is
    /// perihelion.
    ///
    /// # Errors
    /// The errors of [`Abm::dense_state`].
    pub fn locate_event<G>(&self, g: G) -> Result<Option<R>, IntegratorError>
    where
        G: Fn(R, &[R], &[R]) -> R,
    {
        let (t0, _) = self.last_step_span().ok_or(IntegratorError::Config {
            name: "abm dense output",
            value: 0.0,
            valid: "at least one step taken since the start or the last restore",
        })?;
        let h = self.h;
        let (mut x, mut v) = (vec![R::ZERO; self.n], vec![R::ZERO; self.n]);
        let mut at = |theta: R| -> Result<R, IntegratorError> {
            self.eval_dense(theta, &mut x, &mut v)?;
            Ok(g(theta.mul_add(h, t0), &x, &v))
        };
        let to_t = |theta: R| theta.mul_add(h, t0);
        let (mut a, mut b) = (R::ZERO, R::ONE);
        let (mut fa, mut fb) = (at(a)?, at(b)?);
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
            let fc = at(c)?;
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

    /// Step until `t_end` is reached or passed, calling `observer` after every step. A fixed-step
    /// method does not shorten its last step: the final step may end after `t_end` (use
    /// [`Abm::dense_state`] for the state at `t_end`).
    ///
    /// # Errors
    /// Any error of [`Abm::step`], or the first error returned by `observer`.
    pub fn integrate_to<S, F>(
        &mut self,
        sys: &S,
        t_end: R,
        mut observer: F,
    ) -> Result<(), IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        F: FnMut(&Self, &AbmStepReport<R>) -> Result<(), IntegratorError>,
    {
        if !t_end.is_finite() {
            return Err(IntegratorError::Config {
                name: "t_end",
                value: t_end.to_f64(),
                valid: "finite",
            });
        }
        while (t_end - self.time()) * self.h > R::ZERO {
            let rep = self.step(sys)?;
            observer(self, &rep)?;
        }
        Ok(())
    }
}

/// Everything an [`Abm`] carries from one step to the next, as exact `(hi, lo)` binary64 pairs
/// (CI-010). Restoring it with [`Abm::restore`] and stepping on gives bit for bit the steps the
/// original integrator would have taken.
///
/// Carried: the grid (`t₀`, `h`, the step index), the state and its Kahan compensation, the
/// derivative history (`p` vectors once the start-up is complete), the start-up nodes while they
/// are still needed, and the counters. Not carried: the dense-output data of the last step (one
/// new step is needed after a restore) and scratch vectors.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AbmSnapshot {
    /// [`Real::NAME`] of the backend that took the snapshot.
    pub backend: String,
    /// Settings.
    pub config: AbmConfig,
    /// Number of coordinates of the second-order system.
    pub dimension: u64,
    /// Initial time.
    pub t0: Parts,
    /// Fixed step.
    pub h: Parts,
    /// Steps taken (the current time is `t₀ + steps·h`).
    pub steps: u64,
    /// `y = (x, v)`.
    pub y: Vec<Parts>,
    /// Kahan compensation of `y`.
    pub y_comp: Vec<Parts>,
    /// Derivative history, newest first (empty during the start-up).
    pub history: Vec<Vec<Parts>>,
    /// Start-up states at grid points 0 … p−1 (empty when not needed).
    pub startup_states: Vec<Vec<Parts>>,
    /// Start-up derivatives at grid points 0 … p−1 (empty when not needed).
    pub startup_derivatives: Vec<Vec<Parts>>,
    /// Counters.
    pub stats: AbmStats,
}

fn ck(what: &'static str, problem: impl Into<String>) -> IntegratorError {
    IntegratorError::Checkpoint {
        what,
        problem: problem.into(),
    }
}

fn to_parts<R: Real>(what: &'static str, z: R) -> Result<Parts, IntegratorError> {
    let (hi, lo) = z.to_parts();
    let back = R::from_parts(hi, lo);
    if back.to_parts() == (hi, lo) && back == z {
        Ok([hi, lo])
    } else {
        Err(ck(
            what,
            format!(
                "the {} backend does not split {:e} into an exact (hi, lo) pair",
                R::NAME,
                z.to_f64()
            ),
        ))
    }
}

fn to_parts_vec<R: Real>(what: &'static str, zs: &[R]) -> Result<Vec<Parts>, IntegratorError> {
    zs.iter().map(|&z| to_parts(what, z)).collect()
}

fn from_parts<R: Real>(what: &'static str, [hi, lo]: Parts) -> Result<R, IntegratorError> {
    let z = R::from_parts(hi, lo);
    if !z.is_finite() {
        return Err(ck(what, format!("non-finite value ({hi:e}, {lo:e})")));
    }
    if z.to_parts() != (hi, lo) {
        return Err(ck(
            what,
            format!(
                "({hi:e}, {lo:e}) is not a normalised {} value (was the file edited?)",
                R::NAME
            ),
        ));
    }
    Ok(z)
}

fn from_parts_vec<R: Real>(
    what: &'static str,
    ps: &[Parts],
    len: usize,
) -> Result<Vec<R>, IntegratorError> {
    if ps.len() != len {
        return Err(ck(what, format!("has {} values, expected {len}", ps.len())));
    }
    ps.iter().map(|&p| from_parts(what, p)).collect()
}

fn from_parts_mat<R: Real>(
    what: &'static str,
    rows: &[Vec<Parts>],
    count: &[usize],
    len: usize,
) -> Result<Vec<Vec<R>>, IntegratorError> {
    if !count.contains(&rows.len()) {
        return Err(ck(
            what,
            format!("has {} vectors, expected one of {count:?}", rows.len()),
        ));
    }
    rows.iter().map(|r| from_parts_vec(what, r, len)).collect()
}

impl<R: Real> Abm<R> {
    /// The state carried between steps, for a checkpoint (CI-010).
    ///
    /// # Errors
    /// `FA-E0405` if the backend cannot represent a value as an exact `(hi, lo)` pair.
    pub fn snapshot(&self) -> Result<AbmSnapshot, IntegratorError> {
        let mat = |what, m: &[Vec<R>]| -> Result<Vec<Vec<Parts>>, IntegratorError> {
            m.iter().map(|v| to_parts_vec(what, v)).collect()
        };
        let (ss, sf) = match &self.startup {
            Some(s) => (
                mat("start-up states", &s.ys)?,
                mat("start-up derivatives", &s.fs)?,
            ),
            None => (Vec::new(), Vec::new()),
        };
        Ok(AbmSnapshot {
            backend: R::NAME.to_owned(),
            config: self.cfg,
            dimension: u64::try_from(self.n).map_err(|_| ck("dimension", "too large"))?,
            t0: to_parts("initial time", self.t0)?,
            h: to_parts("step", self.h)?,
            steps: self.k,
            y: to_parts_vec("state", &self.y)?,
            y_comp: to_parts_vec("state compensation", &self.yc)?,
            history: mat("derivative history", &self.hist)?,
            startup_states: ss,
            startup_derivatives: sf,
            stats: self.stats,
        })
    }

    /// An integrator that continues exactly where the one that took `snap` stopped.
    ///
    /// # Errors
    /// `FA-E0405` for another backend, inconsistent lengths or phase, or non-finite or
    /// non-normalised values; `FA-E0402` for invalid settings.
    pub fn restore(snap: &AbmSnapshot) -> Result<Self, IntegratorError> {
        if snap.backend != R::NAME {
            return Err(ck(
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
            .map_err(|_| ck("dimension", "does not fit this platform"))?;
        let m = 2 * n;
        let p = usize::try_from(snap.config.order).map_err(|_| ck("order", "too large"))?;
        let h: R = from_parts("step", snap.h)?;
        if h == R::ZERO {
            return Err(ck("step", "is zero"));
        }
        let last_startup = u64::try_from(p - 1).map_err(|_| ck("order", "too large"))?;
        let in_startup = snap.steps < last_startup;
        let hist_count = [if in_startup { 0 } else { p }];
        let hist = from_parts_mat::<R>("derivative history", &snap.history, &hist_count, m)?;
        let need_startup =
            snap.steps <= last_startup && !(snap.steps == 0 && snap.startup_states.is_empty());
        let counts: &[usize] = if need_startup { &[p] } else { &[0, p] };
        let ys = from_parts_mat::<R>("start-up states", &snap.startup_states, counts, m)?;
        let fs = from_parts_mat::<R>(
            "start-up derivatives",
            &snap.startup_derivatives,
            &[ys.len()],
            m,
        )?;
        let mut abm = Self::assemble(
            snap.config,
            n,
            from_parts("initial time", snap.t0)?,
            h,
            snap.steps,
            from_parts_vec("state", &snap.y, m)?,
            Some(from_parts_vec("state compensation", &snap.y_comp, m)?),
            hist,
            snap.stats,
        )?;
        if !ys.is_empty() {
            abm.startup = Some(Startup { ys, fs });
        }
        Ok(abm)
    }
}
