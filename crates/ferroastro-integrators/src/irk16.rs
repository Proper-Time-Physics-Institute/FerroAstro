// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Gauss–Legendre implicit Runge–Kutta, s = 8 stages, order 16: the NUM-003 reference
//! integrator (WP29).
//!
//! Clean-room from the numerical-analysis literature only (DEP-008): Butcher 1964 (Math. Comp.
//! 18, 50) and Hairer, Lubich & Wanner 2006, *Geometric Numerical Integration*, 2nd ed., §II.1.3
//! (Gauss collocation; the simplifying assumptions B(2s), C(s)), and Hairer, `McLachlan` &
//! Razakarivony 2008 (BIT 48, 231, doi:10.1007/s10543-008-0170-3: round-off-aware implementation
//! of implicit Runge–Kutta methods for Brouwer's law). No integrator source was consulted.
//!
//! # Scheme
//! A second-order system `ẍ = a(t, x, ẋ)` is integrated as the first-order system `y = (x, v)`,
//! `y′ = (v, a)`, by the s = 8 Gauss method (collocation at the zeros of the shifted Legendre
//! polynomial P₈(2θ − 1)). Eliminating the stage velocities, with stage accelerations `F_j`:
//!
//! * `V_i = v₀ + h Σ_j a_ij F_j`, `X_i = x₀ + c_i h v₀ + h² Σ_j (A²)_ij F_j`,
//!   `F_i = a(t₀ + c_i h, X_i, V_i)`;
//! * `v₁ = v₀ + h Σ_i b_i F_i`, `x₁ = x₀ + h v₀ + h² Σ_j (bA)_j F_j`.
//!
//! This is the Gauss method on `(x, v)` exactly (NUM-003 notes: symmetric, but not canonically
//! symplectic once `p ≠ m v`), so velocity-dependent forces need no special treatment.
//!
//! # Round-off (Hairer, `McLachlan` & Razakarivony 2008)
//! * The coefficients are computed once in double-double from first principles (Newton on P₈,
//!   exact Gauss-rule integrals of the Lagrange basis in product form) and rounded to the backend, so every coefficient is
//!   correctly rounded and satisfies the order conditions to the backend's precision.
//! * The stage equations are solved by fixed-point iteration on the stage accelerations, started
//!   from the previous step's collocation polynomial extrapolated to the new nodes; the
//!   iteration stops when the largest change of a stage acceleration is zero or stops decreasing
//!   (HMR's criterion: further iterations only exchange round-off).
//! * The increments are added to the state with Kahan compensation ([`Irk16Config::compensated`]).
//! * `t_k = t₀ + k·h` is computed from the step index, never accumulated.
//!
//! # Dense output (NUM-010)
//! Inside a step, the collocation polynomial: `V(θ) = v₀ + h Σ_j α_j(θ) F_j`,
//! `X(θ) = x₀ + θ h v₀ + h² Σ_j (Σ_k α_k(θ) a_kj) F_j` with `α_j(θ) = ∫₀^θ l_j`. It is of order 8
//! between the step points and reproduces the step end at `θ = 1`.

use crate::IntegratorError;
use crate::compensated::kahan_add;
use crate::ias15::{
    Parts, SecondOrderSystem, check_len, checkpoint_err, from_parts_one, from_parts_vec, parts_vec,
    to_parts_exact,
};
use ferroastro_real::{DoubleDouble, Real};

/// Number of stages.
pub const STAGES: usize = 8;

/// Default iteration cap per step.
pub const DEFAULT_MAX_ITERATIONS: u32 = 64;

type Mat<R> = [[R; STAGES]; STAGES];

/// The s = 8 Gauss–Legendre coefficients in backend `R`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussLegendre8<R: Real = f64> {
    /// Nodes `c_i` on (0, 1), ascending.
    pub c: [R; STAGES],
    /// Weights `b_i` (sum 1).
    pub b: [R; STAGES],
    /// `a_ij = ∫₀^{c_i} l_j(θ) dθ`.
    pub a: Mat<R>,
    /// `A²` (position stage coefficients).
    pub a2: Mat<R>,
    /// `bA` (position update coefficients).
    pub ba: [R; STAGES],
    /// Barycentric weights `1/Π_{m≠j}(c_j − c_m)`: the Lagrange basis in product form,
    /// `l_j(θ) = w_j Π_{m≠j}(θ − c_m)` (well-conditioned, unlike the monomial form).
    pub basis_weights: [R; STAGES],
    /// Extrapolation `l_j(1 + c_i)` (row i, column j): the next step's starting guess.
    pub extrapolation: Mat<R>,
}

type Dd = DoubleDouble;

fn dd(x: f64) -> Dd {
    Dd::from_f64(x)
}

/// `(P_n(x), P_{n−1}(x))` by the three-term recurrence.
fn legendre(n: usize, x: Dd) -> (Dd, Dd) {
    let (mut p_prev, mut p) = (Dd::ONE, x);
    for k in 1..n {
        let kk = Dd::from_i32(i32::try_from(k).unwrap_or(i32::MAX));
        let next = ((kk + kk + Dd::ONE) * x * p - kk * p_prev) / (kk + Dd::ONE);
        p_prev = p;
        p = next;
    }
    (p, p_prev)
}

/// `1/Π_{m≠j}(c_j − c_m)`.
fn barycentric_weights<R: Real>(c: &[R; STAGES]) -> [R; STAGES] {
    let mut w = [R::ONE; STAGES];
    for (j, wj) in w.iter_mut().enumerate() {
        let cj = c.get(j).copied().unwrap_or(R::ZERO);
        let mut den = R::ONE;
        for (m, &cm) in c.iter().enumerate() {
            if m != j {
                den *= cj - cm;
            }
        }
        *wj = R::ONE / den;
    }
    w
}

/// `l_j(θ) = w_j Π_{m≠j}(θ − c_m)`.
fn basis<R: Real>(c: &[R; STAGES], w: &[R; STAGES], j: usize, theta: R) -> R {
    let mut p = w.get(j).copied().unwrap_or(R::ZERO);
    for (m, &cm) in c.iter().enumerate() {
        if m != j {
            p *= theta - cm;
        }
    }
    p
}

/// `α_j(θ) = ∫₀^θ l_j = θ Σ_m b_m l_j(θ c_m)` (the Gauss rule is exact for degree ≤ 15).
fn alpha<R: Real>(c: &[R; STAGES], b: &[R; STAGES], w: &[R; STAGES], theta: R) -> [R; STAGES] {
    let mut out = [R::ZERO; STAGES];
    for (j, oj) in out.iter_mut().enumerate() {
        let mut s = R::ZERO;
        for (&bm, &cm) in b.iter().zip(c) {
            s = bm.mul_add(basis(c, w, j, theta * cm), s);
        }
        *oj = theta * s;
    }
    out
}

impl GaussLegendre8<DoubleDouble> {
    /// Compute the coefficients in double-double from first principles.
    #[must_use]
    pub fn compute() -> Self {
        let n = STAGES;
        let nn = Dd::from_i32(8);
        let mut x = [Dd::ZERO; STAGES];
        let mut w = [Dd::ZERO; STAGES];
        for (i, (xi, wi)) in x.iter_mut().zip(w.iter_mut()).enumerate() {
            // Initial guess cos(π (i + 3/4) / (n + 1/2)) (decreasing in i), then Newton.
            let i_f = f64::from(u32::try_from(i).unwrap_or(0));
            let mut z = dd((std::f64::consts::PI * (i_f + 0.75) / 8.5).cos());
            let mut deriv = Dd::ONE;
            for _ in 0..100 {
                let (p, p1) = legendre(n, z);
                deriv = nn * (z * p - p1) / (z * z - Dd::ONE);
                let dz = p / deriv;
                z -= dz;
                if dz.abs() <= Dd::EPSILON.scale(0.25) * z.abs() {
                    break;
                }
            }
            let (p, p1) = legendre(n, z);
            deriv = if (z * z - Dd::ONE) == Dd::ZERO {
                deriv
            } else {
                nn * (z * p - p1) / (z * z - Dd::ONE)
            };
            *xi = z;
            // Gauss weight on [−1, 1] is 2/((1 − x²) P'²); on [0, 1] half of it.
            *wi = Dd::ONE / ((Dd::ONE - z * z) * deriv * deriv);
        }
        // Ascending nodes on (0, 1): c = (1 − x)/2 for x descending.
        let mut c = [Dd::ZERO; STAGES];
        let mut b = [Dd::ZERO; STAGES];
        for (k, (ck, bk)) in c.iter_mut().zip(b.iter_mut()).enumerate() {
            let xi = x.get(k).copied().unwrap_or(Dd::ZERO);
            *ck = (Dd::ONE - xi) * Dd::HALF;
            *bk = w.get(k).copied().unwrap_or(Dd::ZERO);
        }
        // Lagrange basis in product form; a_ij = ∫₀^{c_i} l_j by the 8-point Gauss rule itself
        // (exact for the degree-7 l_j): a_ij = c_i Σ_m b_m l_j(c_i c_m).
        let basis_weights = barycentric_weights(&c);
        let mut a = [[Dd::ZERO; STAGES]; STAGES];
        let mut extrapolation = [[Dd::ZERO; STAGES]; STAGES];
        for (i, (ai, ei)) in a.iter_mut().zip(extrapolation.iter_mut()).enumerate() {
            let ci = c.get(i).copied().unwrap_or(Dd::ZERO);
            *ai = alpha(&c, &b, &basis_weights, ci);
            for (j, eij) in ei.iter_mut().enumerate() {
                *eij = basis(&c, &basis_weights, j, Dd::ONE + ci);
            }
        }
        let mut a2 = [[Dd::ZERO; STAGES]; STAGES];
        for (i, row) in a2.iter_mut().enumerate() {
            for (j, out) in row.iter_mut().enumerate() {
                let mut s = Dd::ZERO;
                for k in 0..STAGES {
                    let aik = a.get(i).and_then(|r| r.get(k)).copied().unwrap_or(Dd::ZERO);
                    let akj = a.get(k).and_then(|r| r.get(j)).copied().unwrap_or(Dd::ZERO);
                    s += aik * akj;
                }
                *out = s;
            }
        }
        let mut ba = [Dd::ZERO; STAGES];
        for (j, out) in ba.iter_mut().enumerate() {
            let mut s = Dd::ZERO;
            for (k, &bk) in b.iter().enumerate() {
                s += bk * a.get(k).and_then(|r| r.get(j)).copied().unwrap_or(Dd::ZERO);
            }
            *out = s;
        }
        Self {
            c,
            b,
            a,
            a2,
            ba,
            basis_weights,
            extrapolation,
        }
    }
}

impl<R: Real> GaussLegendre8<R> {
    /// The double-double coefficients rounded to backend `R`.
    #[must_use]
    pub fn new() -> Self {
        let d = GaussLegendre8::<DoubleDouble>::compute();
        let cv = |z: Dd| R::from_parts(z.hi(), z.lo());
        let cm = |m: Mat<Dd>| m.map(|row| row.map(cv));
        Self {
            c: d.c.map(cv),
            b: d.b.map(cv),
            a: cm(d.a),
            a2: cm(d.a2),
            ba: d.ba.map(cv),
            basis_weights: d.basis_weights.map(cv),
            extrapolation: cm(d.extrapolation),
        }
    }

    /// `α_j(θ) = ∫₀^θ l_j`.
    fn alpha(&self, theta: R) -> [R; STAGES] {
        alpha(&self.c, &self.b, &self.basis_weights, theta)
    }
}

impl<R: Real> Default for GaussLegendre8<R> {
    fn default() -> Self {
        Self::new()
    }
}

/// IRK16 settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Irk16Config {
    /// Kahan-compensated state update. Default on.
    pub compensated: bool,
    /// Fixed-point iterations per step, at most (1 … 1000). Default 64.
    pub max_iterations: u32,
}

impl Default for Irk16Config {
    fn default() -> Self {
        Self {
            compensated: true,
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }
}

impl Irk16Config {
    fn validate(self) -> Result<(), IntegratorError> {
        if !(1..=1000).contains(&self.max_iterations) {
            return Err(IntegratorError::Config {
                name: "irk16 max_iterations",
                value: f64::from(self.max_iterations),
                valid: "1 <= max_iterations <= 1000",
            });
        }
        Ok(())
    }

    /// One-line description.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "IRK16 Gauss-Legendre s = 8 (fixed-point iteration to round-off, at most {} per step, \
             {} state update)",
            self.max_iterations,
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
pub struct Irk16Stats {
    /// Steps taken.
    pub steps: u64,
    /// Force evaluations.
    pub force_evaluations: u64,
    /// Fixed-point iterations (all steps).
    pub iterations: u64,
    /// Steps that reached the iteration cap.
    pub capped_steps: u64,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Irk16StepReport<R: Real = f64> {
    /// Time at the end of the step.
    pub t: R,
    /// The step.
    pub dt: R,
    /// Fixed-point iterations of this step.
    pub iterations: u32,
}

/// The IRK16 integrator state, in backend `R` (default `f64`).
#[derive(Debug, Clone)]
pub struct Irk16<R: Real = f64> {
    cfg: Irk16Config,
    coef: GaussLegendre8<R>,
    n: usize,
    t0: R,
    h: R,
    k: u64,
    x: Vec<R>,
    v: Vec<R>,
    xc: Vec<R>,
    vc: Vec<R>,
    /// Stage accelerations of the last step, `STAGES × n` (empty before the first step).
    f: Vec<Vec<R>>,
    /// Start state of the last step (dense output); `None` before the first step and after a
    /// restore.
    last: Option<(Vec<R>, Vec<R>)>,
    stats: Irk16Stats,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "step indices stay far below 2^53"
)]
fn index_f64(k: u64) -> f64 {
    k as f64
}

impl<R: Real> Irk16<R> {
    /// A new integrator at `(t0, x0, v0)` with fixed step `h` (negative integrates backwards).
    ///
    /// # Errors
    /// `FA-E0402` for an invalid setting or step; `FA-E0403` for slices of the wrong length.
    pub fn new(
        n: usize,
        t0: R,
        x0: &[R],
        v0: &[R],
        h: R,
        cfg: Irk16Config,
    ) -> Result<Self, IntegratorError> {
        cfg.validate()?;
        check_len("initial position", x0.len(), n)?;
        check_len("initial velocity", v0.len(), n)?;
        if !(h.is_finite() && h != R::ZERO && t0.is_finite()) {
            return Err(IntegratorError::Config {
                name: "irk16 step",
                value: h.to_f64(),
                valid: "finite and non-zero",
            });
        }
        Ok(Self {
            cfg,
            coef: GaussLegendre8::new(),
            n,
            t0,
            h,
            k: 0,
            x: x0.to_vec(),
            v: v0.to_vec(),
            xc: vec![R::ZERO; n],
            vc: vec![R::ZERO; n],
            f: Vec::new(),
            last: None,
            stats: Irk16Stats::default(),
        })
    }

    fn grid(&self, k: u64) -> R {
        R::from_f64(index_f64(k)).mul_add(self.h, self.t0)
    }

    /// Current time.
    #[must_use]
    pub fn time(&self) -> R {
        self.grid(self.k)
    }

    /// The fixed step.
    #[must_use]
    pub const fn step_size(&self) -> R {
        self.h
    }

    /// Positions.
    #[must_use]
    pub fn positions(&self) -> &[R] {
        &self.x
    }

    /// Velocities.
    #[must_use]
    pub fn velocities(&self) -> &[R] {
        &self.v
    }

    /// Counters.
    #[must_use]
    pub const fn stats(&self) -> Irk16Stats {
        self.stats
    }

    /// Settings.
    #[must_use]
    pub const fn config(&self) -> Irk16Config {
        self.cfg
    }

    /// The coefficients.
    #[must_use]
    pub const fn coefficients(&self) -> &GaussLegendre8<R> {
        &self.coef
    }

    /// Continue from the current state with the step negated (forward-then-backward checks).
    /// The starting guess history and the dense output are dropped; the compensation is kept.
    pub fn reverse(&mut self) {
        self.t0 = self.time();
        self.k = 0;
        self.h = -self.h;
        self.f.clear();
        self.last = None;
    }

    fn eval<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t: R,
        x: &[R],
        v: &[R],
        out: &mut [R],
    ) -> Result<(), IntegratorError> {
        sys.acceleration(t, x, v, out)?;
        self.stats.force_evaluations += 1;
        if out.iter().any(|z| !z.is_finite()) {
            return Err(IntegratorError::NonFinite {
                what: "acceleration",
                t: t.to_f64(),
            });
        }
        Ok(())
    }

    /// Add the step's increments to the state (compensated unless configured plain).
    fn update(&mut self, f: &[Vec<R>], t_start: R) -> Result<(), IntegratorError> {
        let (n, h, coef) = (self.n, self.h, self.coef);
        for q in 0..n {
            let (mut sv, mut sx) = (R::ZERO, R::ZERO);
            for ((fj, &bj), &baj) in f.iter().zip(&coef.b).zip(&coef.ba) {
                let fq = fj.get(q).copied().unwrap_or(R::ZERO);
                sv = bj.mul_add(fq, sv);
                sx = baj.mul_add(fq, sx);
            }
            let v0 = self.v.get(q).copied().unwrap_or(R::ZERO);
            let dx = (h * h).mul_add(sx, h * v0);
            let dv = h * sv;
            if let (Some(xq), Some(xcq), Some(vq), Some(vcq)) = (
                self.x.get_mut(q),
                self.xc.get_mut(q),
                self.v.get_mut(q),
                self.vc.get_mut(q),
            ) {
                if self.cfg.compensated {
                    kahan_add(xq, xcq, dx);
                    kahan_add(vq, vcq, dv);
                } else {
                    *xq += dx;
                    *vq += dv;
                }
                if !(xq.is_finite() && vq.is_finite()) {
                    return Err(IntegratorError::NonFinite {
                        what: "position",
                        t: t_start.to_f64(),
                    });
                }
            }
        }
        Ok(())
    }

    /// One step of `h`.
    ///
    /// # Errors
    /// `FA-E0400` for a non-finite acceleration or state; any error of the force model.
    pub fn step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<Irk16StepReport<R>, IntegratorError> {
        let n = self.n;
        let h = self.h;
        let t_start = self.time();
        let coef = self.coef;
        // Starting guess.
        let mut f: Vec<Vec<R>> = if self.f.len() == STAGES {
            coef.extrapolation
                .iter()
                .map(|row| {
                    (0..n)
                        .map(|q| {
                            let mut acc = R::ZERO;
                            for (fj, &e) in self.f.iter().zip(row) {
                                acc = e.mul_add(fj.get(q).copied().unwrap_or(R::ZERO), acc);
                            }
                            acc
                        })
                        .collect()
                })
                .collect()
        } else {
            let mut a0 = vec![R::ZERO; n];
            let (x, v) = (self.x.clone(), self.v.clone());
            self.eval(sys, t_start, &x, &v, &mut a0)?;
            vec![a0; STAGES]
        };
        let (mut xs, mut vs) = (vec![R::ZERO; n], vec![R::ZERO; n]);
        let mut g = vec![vec![R::ZERO; n]; STAGES];
        let mut delta_prev: Option<R> = None;
        let mut iterations = 0_u32;
        loop {
            iterations += 1;
            for (i, gi) in g.iter_mut().enumerate() {
                let (ai, a2i) = (
                    coef.a.get(i).copied().unwrap_or([R::ZERO; STAGES]),
                    coef.a2.get(i).copied().unwrap_or([R::ZERO; STAGES]),
                );
                let ci = coef.c.get(i).copied().unwrap_or(R::ZERO);
                for q in 0..n {
                    let (mut sv, mut sx) = (R::ZERO, R::ZERO);
                    for (j, fj) in f.iter().enumerate() {
                        let fq = fj.get(q).copied().unwrap_or(R::ZERO);
                        sv = ai.get(j).copied().unwrap_or(R::ZERO).mul_add(fq, sv);
                        sx = a2i.get(j).copied().unwrap_or(R::ZERO).mul_add(fq, sx);
                    }
                    let (x0, v0) = (
                        self.x.get(q).copied().unwrap_or(R::ZERO),
                        self.v.get(q).copied().unwrap_or(R::ZERO),
                    );
                    if let (Some(vq), Some(xq)) = (vs.get_mut(q), xs.get_mut(q)) {
                        *vq = h.mul_add(sv, v0);
                        *xq = (h * h).mul_add(sx, (ci * h).mul_add(v0, x0));
                    }
                }
                let ti = ci.mul_add(h, t_start);
                sys.acceleration(ti, &xs, &vs, gi)?;
                self.stats.force_evaluations += 1;
                if gi.iter().any(|z| !z.is_finite()) {
                    return Err(IntegratorError::NonFinite {
                        what: "acceleration",
                        t: ti.to_f64(),
                    });
                }
            }
            let mut delta = R::ZERO;
            for (gi, fi) in g.iter().zip(f.iter_mut()) {
                for (gq, fq) in gi.iter().zip(fi.iter_mut()) {
                    delta = delta.max((*gq - *fq).abs());
                    *fq = *gq;
                }
            }
            let stalled = delta_prev.is_some_and(|p| delta >= p);
            if delta == R::ZERO || stalled {
                break;
            }
            if iterations >= self.cfg.max_iterations {
                self.stats.capped_steps += 1;
                break;
            }
            delta_prev = Some(delta);
        }
        self.stats.iterations += u64::from(iterations);
        let start = (self.x.clone(), self.v.clone());
        self.update(&f, t_start)?;
        self.f = f;
        self.last = Some(start);
        self.k += 1;
        self.stats.steps += 1;
        Ok(Irk16StepReport {
            t: self.time(),
            dt: h,
            iterations,
        })
    }

    /// The time span `(t_k, t_{k+1})` of the last step (`None` before the first step and after
    /// a restore or [`Irk16::reverse`]).
    #[must_use]
    pub fn last_step_span(&self) -> Option<(R, R)> {
        self.last.as_ref()?;
        let k = self.k.checked_sub(1)?;
        Some((self.grid(k), self.grid(self.k)))
    }

    fn no_dense() -> IntegratorError {
        IntegratorError::Config {
            name: "irk16 dense output",
            value: 0.0,
            valid: "at least one step taken since the start, the last restore or reverse",
        }
    }

    fn eval_dense(&self, theta: R, x: &mut [R], v: &mut [R]) -> Result<(), IntegratorError> {
        let (x0, v0) = self.last.as_ref().ok_or_else(Self::no_dense)?;
        let h = self.h;
        let alpha = self.coef.alpha(theta);
        // Position weights Σ_k α_k a_kj.
        let mut beta = [R::ZERO; STAGES];
        for (j, bj) in beta.iter_mut().enumerate() {
            let mut s = R::ZERO;
            for (k, &ak) in alpha.iter().enumerate() {
                s = ak.mul_add(
                    self.coef
                        .a
                        .get(k)
                        .and_then(|r| r.get(j))
                        .copied()
                        .unwrap_or(R::ZERO),
                    s,
                );
            }
            *bj = s;
        }
        for (q, (xq, vq)) in x.iter_mut().zip(v.iter_mut()).enumerate() {
            let (mut sv, mut sx) = (R::ZERO, R::ZERO);
            for ((fj, &aj), &bj) in self.f.iter().zip(&alpha).zip(&beta) {
                let fq = fj.get(q).copied().unwrap_or(R::ZERO);
                sv = aj.mul_add(fq, sv);
                sx = bj.mul_add(fq, sx);
            }
            let (xs, vs) = (
                x0.get(q).copied().unwrap_or(R::ZERO),
                v0.get(q).copied().unwrap_or(R::ZERO),
            );
            *vq = h.mul_add(sv, vs);
            *xq = (h * h).mul_add(sx, (theta * h).mul_add(vs, xs));
        }
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
        let (t0, _) = self.last_step_span().ok_or_else(Self::no_dense)?;
        let theta = (t - t0) / self.h;
        // The grid times t₀ + k·h are rounded, so θ at the step end can differ from 1 by ~k ulp.
        let slack = R::EPSILON.scale(16.0) * (t.abs() / self.h.abs()).max(R::ONE);
        if !(theta >= -slack && theta <= R::ONE + slack) {
            return Err(IntegratorError::Config {
                name: "dense-output time (fraction of the last step)",
                value: theta.to_f64(),
                valid: "0 <= theta <= 1",
            });
        }
        if t == self.time() || theta >= R::ONE {
            // The step end itself (with its compensated update), not the interpolant.
            x.copy_from_slice(&self.x);
            v.copy_from_slice(&self.v);
            return Ok(());
        }
        self.eval_dense(theta.max(R::ZERO), x, v)
    }

    /// Event location (NUM-010): the time of a sign change of `g(t, x, v)` inside the last step,
    /// by the Illinois variant of regula falsi on the dense output; `None` if `g` has the same
    /// sign at both ends.
    ///
    /// # Errors
    /// As [`Irk16::dense_state`].
    pub fn locate_event<G>(&self, g: G) -> Result<Option<R>, IntegratorError>
    where
        G: Fn(R, &[R], &[R]) -> R,
    {
        let (t0, _) = self.last_step_span().ok_or_else(Self::no_dense)?;
        let h = self.h;
        let (mut x, mut v) = (vec![R::ZERO; self.n], vec![R::ZERO; self.n]);
        let mut at = |theta: R| -> Result<R, IntegratorError> {
            if theta >= R::ONE {
                x.copy_from_slice(&self.x);
                v.copy_from_slice(&self.v);
            } else {
                self.eval_dense(theta, &mut x, &mut v)?;
            }
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

    /// Step until `t_end` is reached or passed (the last step may end after `t_end`: use
    /// [`Irk16::dense_state`] for the state at `t_end`), calling `observer` after each step.
    ///
    /// # Errors
    /// Any error of [`Irk16::step`], or the first error returned by `observer`.
    pub fn integrate_to<S, F>(
        &mut self,
        sys: &S,
        t_end: R,
        mut observer: F,
    ) -> Result<(), IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        F: FnMut(&Self, &Irk16StepReport<R>) -> Result<(), IntegratorError>,
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

/// Everything an [`Irk16`] carries from one step to the next, as exact `(hi, lo)` binary64
/// pairs (CI-010): the grid, the state and its compensation, the last step's stage
/// accelerations (they start the next iteration, so they decide its bits) and the counters.
/// Not carried: the dense-output start state (one new step is needed after a restore).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Irk16Snapshot {
    /// [`Real::NAME`] of the backend that took the snapshot.
    pub backend: String,
    /// Settings.
    pub config: Irk16Config,
    /// Number of coordinates.
    pub dimension: u64,
    /// Initial time of the grid.
    pub t0: Parts,
    /// Fixed step.
    pub h: Parts,
    /// Steps taken on this grid.
    pub steps_on_grid: u64,
    /// Positions.
    pub x: Vec<Parts>,
    /// Velocities.
    pub v: Vec<Parts>,
    /// Kahan compensation of the positions.
    pub x_comp: Vec<Parts>,
    /// Kahan compensation of the velocities.
    pub v_comp: Vec<Parts>,
    /// Stage accelerations of the last step (empty before the first step).
    pub stages: Vec<Vec<Parts>>,
    /// Counters.
    pub stats: Irk16Stats,
}

impl<R: Real> Irk16<R> {
    /// The state carried between steps, for a checkpoint (CI-010).
    ///
    /// # Errors
    /// `FA-E0405` if the backend cannot represent a value as an exact `(hi, lo)` pair.
    pub fn snapshot(&self) -> Result<Irk16Snapshot, IntegratorError> {
        Ok(Irk16Snapshot {
            backend: R::NAME.to_owned(),
            config: self.cfg,
            dimension: u64::try_from(self.n)
                .map_err(|_| checkpoint_err("dimension", "too large"))?,
            t0: to_parts_exact("initial time", self.t0)?,
            h: to_parts_exact("step", self.h)?,
            steps_on_grid: self.k,
            x: parts_vec("positions", &self.x)?,
            v: parts_vec("velocities", &self.v)?,
            x_comp: parts_vec("position compensation", &self.xc)?,
            v_comp: parts_vec("velocity compensation", &self.vc)?,
            stages: self
                .f
                .iter()
                .map(|s| parts_vec("stage accelerations", s))
                .collect::<Result<_, _>>()?,
            stats: self.stats,
        })
    }

    /// Rebuild an integrator from [`Irk16::snapshot`]'s output; stepping on gives bit for bit
    /// the steps the original would have taken.
    ///
    /// # Errors
    /// `FA-E0405` for a snapshot of another backend or a malformed snapshot; `FA-E0402` for
    /// invalid settings.
    pub fn restore(snap: &Irk16Snapshot) -> Result<Self, IntegratorError> {
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
        if !(snap.stages.is_empty() || snap.stages.len() == STAGES) {
            return Err(checkpoint_err(
                "stage accelerations",
                format!("has {} vectors, expected 0 or {STAGES}", snap.stages.len()),
            ));
        }
        let h: R = from_parts_one("step", snap.h)?;
        if h == R::ZERO {
            return Err(checkpoint_err("step", "is zero"));
        }
        Ok(Self {
            cfg: snap.config,
            coef: GaussLegendre8::new(),
            n,
            t0: from_parts_one("initial time", snap.t0)?,
            h,
            k: snap.steps_on_grid,
            x: from_parts_vec("positions", &snap.x, n)?,
            v: from_parts_vec("velocities", &snap.v, n)?,
            xc: from_parts_vec("position compensation", &snap.x_comp, n)?,
            vc: from_parts_vec("velocity compensation", &snap.v_comp, n)?,
            f: snap
                .stages
                .iter()
                .map(|s| from_parts_vec("stage accelerations", s, n))
                .collect::<Result<_, _>>()?,
            last: None,
            stats: snap.stats,
        })
    }
}
