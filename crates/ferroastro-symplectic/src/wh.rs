// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Wisdom–Holman map in Jacobi coordinates with symplectic correctors (NUM-004).
//!
//! # Splitting (Wisdom & Holman 1991; Rein & Tamayo 2015 §2.2)
//! Bodies `0 … n−1` (body 0 the central body) with gravitational parameters `μ_k` and partial
//! sums `η_i = Σ_{k≤i} μ_k`. Jacobi coordinates `x'_i = x_i − X_{i−1}` (`X_{i−1}` the centre of
//! mass of bodies `0 … i−1`; `x'_0` is the centre of mass of all bodies). The kinetic energy is
//! diagonal in Jacobi velocities, so `ẍ'_i = a_i − A_{i−1}` with `A_{i−1}` the mass-weighted
//! mean inertial acceleration of bodies `0 … i−1`. `H = H_K + H_I` with the Kepler part
//! `H_K,i = p'²/(2m'_i) − G m_0 m_i / r'_i`, i.e. a Kepler drift of `x'_i` with
//! `μ'_i = μ_0 η_i / η_{i−1}`, and the interaction kick
//! `a'_i = (a_i − A_{i−1}) + μ'_i x'_i / r'_i³`, where `a` are the **inertial accelerations of the
//! force model**. For two bodies the kick vanishes (to round-off) and the map is exact.
//!
//! One step is `D(h/2) K(h) D(h/2)`; consecutive half drifts are merged, so the stored kernel
//! state is half a drift ahead between steps and is synchronised (on a copy) for output.
//!
//! # Symplectic correctors (derived; IMPLEMENTATION-NOTES WP29)
//! `C = Π_i X(i h, β_i h) X(−i h, −β_i h)`, `X(a, b) = D(a) K(b) D(−a)`, with the exact rational
//! `β_i` of `tools/symplectic_correctors.py` (orders 3, 5, 7, 11). Real coordinates are mapped to
//! kernel coordinates at the start and back at every output; the integration itself is
//! unchanged, so correctors cost nothing per step unless every step is output.
//!
//! # Scope
//! The kick must not depend on velocities: Newtonian point masses, solar J2 and the GR effective
//! potential (`ferroastro_forces::GrPotential`) only. **Full EIH (and any 1PN velocity-dependent
//! term, and Lense–Thirring) is not supported under the Wisdom–Holman map** (NUM-004): the map is
//! then neither symplectic nor of the stated order. [`check_model`] refuses such models.
//!
//! Clean-room (DEP-008): from the published papers only; no REBOUND, `WHFast`, Swiftest or
//! Mercury6 source was consulted.

use crate::corrector_table::{ORDER_3, ORDER_5, ORDER_7, ORDER_11};
use ferroastro_analytic::drift::kepler_drift;
use ferroastro_core::frames::Frame;
use ferroastro_ephem::NaifId;
use ferroastro_forces::NBodyModel;
use ferroastro_integrators::{IntegratorError, Parts, SecondOrderSystem};
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// The force-model terms the Wisdom–Holman kick supports (velocity-independent).
pub const SUPPORTED_TERMS: [&str; 3] = [
    "newtonian-mutual",
    "solar-j2-with-reaction",
    "gr-potential-sun",
];

/// Which symplectic corrector maps real to kernel coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CorrectorOrder {
    /// No corrector.
    None,
    /// Order 3 (one stage pair).
    Third,
    /// Order 5.
    Fifth,
    /// Order 7.
    Seventh,
    /// Order 11 (Rein & Tamayo 2015's default).
    Eleventh,
}

impl CorrectorOrder {
    /// The order (0 for none).
    #[must_use]
    pub const fn order(self) -> u32 {
        match self {
            CorrectorOrder::None => 0,
            CorrectorOrder::Third => 3,
            CorrectorOrder::Fifth => 5,
            CorrectorOrder::Seventh => 7,
            CorrectorOrder::Eleventh => 11,
        }
    }

    /// From an order number (0, 3, 5, 7, 11).
    #[must_use]
    pub const fn from_order(order: u32) -> Option<Self> {
        match order {
            0 => Some(CorrectorOrder::None),
            3 => Some(CorrectorOrder::Third),
            5 => Some(CorrectorOrder::Fifth),
            7 => Some(CorrectorOrder::Seventh),
            11 => Some(CorrectorOrder::Eleventh),
            _ => None,
        }
    }

    const fn table(self) -> &'static [(i64, i64)] {
        match self {
            CorrectorOrder::None => &[],
            CorrectorOrder::Third => &ORDER_3,
            CorrectorOrder::Fifth => &ORDER_5,
            CorrectorOrder::Seventh => &ORDER_7,
            CorrectorOrder::Eleventh => &ORDER_11,
        }
    }
}

/// Wisdom–Holman settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WhConfig {
    /// Symplectic corrector. Default order 11.
    pub corrector: CorrectorOrder,
    /// Synchronise and correct the output state after every step (so [`Whfast::positions`] is
    /// always current; costs one corrector per step). Default on. Off, call
    /// [`Whfast::physical_state`] when an output is needed.
    pub output_every_step: bool,
}

impl Default for WhConfig {
    fn default() -> Self {
        Self {
            corrector: CorrectorOrder::Eleventh,
            output_every_step: true,
        }
    }
}

impl WhConfig {
    /// One-line description.
    #[must_use]
    pub fn describe(&self) -> String {
        let c = match self.corrector {
            CorrectorOrder::None => "no symplectic corrector".to_owned(),
            other => format!("symplectic corrector of order {}", other.order()),
        };
        format!("Wisdom-Holman (WHFast-type) map in Jacobi coordinates, {c}")
    }
}

/// Counters accumulated over a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WhStats {
    /// Steps taken.
    pub steps: u64,
    /// Force evaluations (kicks, including corrector kicks).
    pub force_evaluations: u64,
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhStepReport<R: Real = f64> {
    /// Time at the end of the step.
    pub t: R,
    /// The step.
    pub dt: R,
}

/// The sign with which the corrector maps real to kernel coordinates (fixed by the WP29 check
/// W3: the opposite sign doubles the energy error instead of removing it).
const TO_KERNEL: i32 = 1;

/// Jacobi bookkeeping: masses and the Kepler parameters.
#[derive(Debug, Clone)]
struct Jacobi<R: Real> {
    gm: Vec<R>,
    eta: Vec<R>,
    kmu: Vec<R>,
}

type V3<R> = [R; 3];

/// Jacobi positions and velocities.
type Kernel<R> = (Vec<V3<R>>, Vec<V3<R>>);

/// `[f(a₀, b₀), f(a₁, b₁), f(a₂, b₂)]`.
#[inline]
fn zip3<R: Real>(a: V3<R>, b: V3<R>, f: impl Fn(R, R) -> R) -> V3<R> {
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
}

fn cfg_err(name: &'static str, value: f64, valid: &'static str) -> IntegratorError {
    IntegratorError::Config { name, value, valid }
}

impl<R: Real> Jacobi<R> {
    fn new(gm: &[R]) -> Result<Self, IntegratorError> {
        if gm.len() < 2 {
            return Err(cfg_err(
                "Wisdom-Holman body count",
                f64::from(u32::try_from(gm.len()).unwrap_or(u32::MAX)),
                ">= 2 (a central body and at least one other)",
            ));
        }
        let mut eta = Vec::with_capacity(gm.len());
        let mut s = R::ZERO;
        for &m in gm {
            if !(m.is_finite() && m >= R::ZERO) {
                return Err(cfg_err(
                    "Wisdom-Holman GM (km^3/s^2)",
                    m.to_f64(),
                    "finite and >= 0",
                ));
            }
            s += m;
            eta.push(s);
        }
        let mu0 = gm.first().copied().unwrap_or(R::ZERO);
        if mu0 <= R::ZERO {
            return Err(cfg_err(
                "Wisdom-Holman central GM (km^3/s^2)",
                mu0.to_f64(),
                "> 0",
            ));
        }
        let mut kmu = vec![R::ZERO; gm.len()];
        for i in 1..gm.len() {
            let (ei, ep) = (
                eta.get(i).copied().unwrap_or(R::ONE),
                eta.get(i - 1).copied().unwrap_or(R::ONE),
            );
            if let Some(k) = kmu.get_mut(i) {
                *k = mu0 * ei / ep;
            }
        }
        Ok(Self {
            gm: gm.to_vec(),
            eta,
            kmu,
        })
    }

    fn len(&self) -> usize {
        self.gm.len()
    }

    /// Inertial (component-major `z[k·n + i]`) to Jacobi.
    fn jacobi_of(&self, z: &[R], out: &mut [V3<R>]) {
        let n = self.len();
        let at =
            |i: usize| -> V3<R> { [0, 1, 2].map(|k| z.get(k * n + i).copied().unwrap_or(R::ZERO)) };
        let mut com = at(0);
        for i in 1..n {
            let zi = at(i);
            let (gi, ei, ep) = (
                self.gm.get(i).copied().unwrap_or(R::ZERO),
                self.eta.get(i).copied().unwrap_or(R::ONE),
                self.eta.get(i - 1).copied().unwrap_or(R::ONE),
            );
            if let Some(o) = out.get_mut(i) {
                *o = zip3(zi, com, |z, c| z - c);
            }
            com = zip3(com, zi, |c, z| (ep * c + gi * z) / ei);
        }
        if let Some(o) = out.first_mut() {
            *o = com;
        }
    }

    /// Jacobi to inertial (component-major).
    fn inertial_of(&self, zj: &[V3<R>], out: &mut [R]) {
        let n = self.len();
        let mut com = zj.first().copied().unwrap_or([R::ZERO; 3]);
        for i in (1..n).rev() {
            let zi = zj.get(i).copied().unwrap_or([R::ZERO; 3]);
            let w = self.gm.get(i).copied().unwrap_or(R::ZERO)
                / self.eta.get(i).copied().unwrap_or(R::ONE);
            // X_{i−1} = X_i − (μ_i/η_i) x'_i, x_i = x'_i + X_{i−1}.
            com = zip3(zi, com, |z, c| (-w).mul_add(z, c));
            for (k, (&ck, &zk)) in com.iter().zip(&zi).enumerate() {
                if let Some(o) = out.get_mut(k * n + i) {
                    *o = zk + ck;
                }
            }
        }
        for (k, &ck) in com.iter().enumerate() {
            if let Some(o) = out.get_mut(k * n) {
                *o = ck;
            }
        }
    }

    /// Kepler drift of every Jacobi coordinate for `tau` (the centre of mass moves uniformly).
    fn drift(&self, xj: &mut [V3<R>], vj: &mut [V3<R>], tau: R) -> Result<(), IntegratorError> {
        if let (Some(x0), Some(v0)) = (xj.first_mut(), vj.first()) {
            *x0 = zip3(*x0, *v0, |x, v| tau.mul_add(v, x));
        }
        for ((x, v), &mu) in xj.iter_mut().zip(vj.iter_mut()).zip(&self.kmu).skip(1) {
            kepler_drift(mu, x, v, tau).map_err(|_| IntegratorError::NonFinite {
                what: "Kepler drift (no convergence or degenerate orbit)",
                t: tau.to_f64(),
            })?;
        }
        Ok(())
    }
}

/// Scratch buffers of a kick.
#[derive(Debug, Clone)]
struct Scratch<R: Real> {
    x: Vec<R>,
    v: Vec<R>,
    a: Vec<R>,
}

impl<R: Real> Scratch<R> {
    fn new(n: usize) -> Self {
        Self {
            x: vec![R::ZERO; 3 * n],
            v: vec![R::ZERO; 3 * n],
            a: vec![R::ZERO; 3 * n],
        }
    }
}

/// The interaction kick for `b` at time `t`.
fn kick<R: Real, S: SecondOrderSystem<R> + ?Sized>(
    jac: &Jacobi<R>,
    sys: &S,
    t: R,
    xj: &[V3<R>],
    vj: &mut [V3<R>],
    b: R,
    s: &mut Scratch<R>,
) -> Result<(), IntegratorError> {
    let n = jac.len();
    jac.inertial_of(xj, &mut s.x);
    jac.inertial_of(vj, &mut s.v);
    sys.acceleration(t, &s.x, &s.v, &mut s.a)?;
    if s.a.iter().any(|z| !z.is_finite()) {
        return Err(IntegratorError::NonFinite {
            what: "acceleration",
            t: t.to_f64(),
        });
    }
    let a = &s.a;
    let at =
        |i: usize| -> V3<R> { [0, 1, 2].map(|k| a.get(k * n + i).copied().unwrap_or(R::ZERO)) };
    let mut mean = at(0);
    for i in 1..n {
        let ai = at(i);
        let xi = xj.get(i).copied().unwrap_or([R::ZERO; 3]);
        let r2 = xi[0].mul_add(xi[0], xi[1].mul_add(xi[1], xi[2] * xi[2]));
        let kmu = jac.kmu.get(i).copied().unwrap_or(R::ZERO);
        let s3 = kmu / (r2 * r2.sqrt());
        if let Some(vi) = vj.get_mut(i) {
            let rel = zip3(ai, mean, |a, m| a - m);
            let acc = zip3(xi, rel, |x, d| s3.mul_add(x, d));
            *vi = zip3(*vi, acc, |v, a| b.mul_add(a, v));
        }
        let (gi, ei, ep) = (
            jac.gm.get(i).copied().unwrap_or(R::ZERO),
            jac.eta.get(i).copied().unwrap_or(R::ONE),
            jac.eta.get(i - 1).copied().unwrap_or(R::ONE),
        );
        mean = zip3(mean, ai, |m, a| (ep * m + gi * a) / ei);
    }
    if let Some(v0) = vj.first_mut() {
        *v0 = zip3(*v0, mean, |v, m| b.mul_add(m, v));
    }
    Ok(())
}

/// The Wisdom–Holman integrator state, in backend `R` (default `f64`).
#[derive(Debug, Clone)]
pub struct Whfast<R: Real = f64> {
    cfg: WhConfig,
    jac: Jacobi<R>,
    beta: Vec<R>,
    t0: R,
    h: R,
    k: u64,
    xj: Vec<V3<R>>,
    vj: Vec<V3<R>>,
    /// The kernel state is half a drift ahead of the step boundary.
    lazy: bool,
    out_x: Vec<R>,
    out_v: Vec<R>,
    last: Option<(Kernel<R>, bool)>,
    scratch: Scratch<R>,
    stats: WhStats,
}

#[expect(
    clippy::cast_precision_loss,
    reason = "step indices stay far below 2^53"
)]
fn index_f64(k: u64) -> f64 {
    k as f64
}

impl<R: Real> Whfast<R> {
    /// A new integrator at `(t0, x0, v0)` (inertial, component-major `z[k·n + i]`, body 0 the
    /// central body) with GMs `gm` and fixed step `h` (negative integrates backwards). The real
    /// initial state is mapped to kernel coordinates with the configured corrector, which needs
    /// the force model `sys`.
    ///
    /// # Errors
    /// `FA-E0402` for an invalid setting, step or GM; `FA-E0403` for slices of the wrong length;
    /// any error of the force model.
    pub fn new<S: SecondOrderSystem<R> + ?Sized>(
        sys: &S,
        gm: &[R],
        t0: R,
        x0: &[R],
        v0: &[R],
        h: R,
        cfg: WhConfig,
    ) -> Result<Self, IntegratorError> {
        let jac = Jacobi::new(gm)?;
        let n = jac.len();
        for (what, got) in [
            ("initial position", x0.len()),
            ("initial velocity", v0.len()),
        ] {
            if got != 3 * n {
                return Err(IntegratorError::Dimension {
                    what,
                    got,
                    expected: 3 * n,
                });
            }
        }
        if sys.dimension() != 3 * n {
            return Err(IntegratorError::Dimension {
                what: "force model",
                got: sys.dimension(),
                expected: 3 * n,
            });
        }
        if !(h.is_finite() && h != R::ZERO && t0.is_finite()) {
            return Err(cfg_err(
                "Wisdom-Holman step",
                h.to_f64(),
                "finite and non-zero",
            ));
        }
        let beta = cfg
            .corrector
            .table()
            .iter()
            .map(|&(p, q)| {
                // |p|, |q| < 2^53 (checked by the generator), so both convert exactly.
                #[expect(clippy::cast_precision_loss, reason = "exact below 2^53")]
                let (pf, qf) = (p as f64, q as f64);
                R::from_f64(pf) / R::from_f64(qf)
            })
            .collect();
        let mut wh = Self {
            cfg,
            jac,
            beta,
            t0,
            h,
            k: 0,
            xj: vec![[R::ZERO; 3]; n],
            vj: vec![[R::ZERO; 3]; n],
            lazy: false,
            out_x: x0.to_vec(),
            out_v: v0.to_vec(),
            last: None,
            scratch: Scratch::new(n),
            stats: WhStats::default(),
        };
        wh.jac.jacobi_of(x0, &mut wh.xj);
        wh.jac.jacobi_of(v0, &mut wh.vj);
        let (mut xj, mut vj) = (wh.xj.clone(), wh.vj.clone());
        wh.corrector(sys, t0, &mut xj, &mut vj, TO_KERNEL)?;
        wh.xj = xj;
        wh.vj = vj;
        Ok(wh)
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

    /// Counters.
    #[must_use]
    pub const fn stats(&self) -> WhStats {
        self.stats
    }

    /// Settings.
    #[must_use]
    pub const fn config(&self) -> WhConfig {
        self.cfg
    }

    /// Number of bodies.
    #[must_use]
    pub fn bodies(&self) -> usize {
        self.jac.len()
    }

    /// Physical (corrected) inertial positions at the current time with
    /// [`WhConfig::output_every_step`] (default); otherwise those of the last
    /// [`Whfast::synchronize`] (or the initial state).
    #[must_use]
    pub fn positions(&self) -> &[R] {
        &self.out_x
    }

    /// Velocities, as [`Whfast::positions`].
    #[must_use]
    pub fn velocities(&self) -> &[R] {
        &self.out_v
    }

    /// Apply the corrector (sign +1 or −1) to a Jacobi state.
    fn corrector<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t: R,
        xj: &mut [V3<R>],
        vj: &mut [V3<R>],
        sign: i32,
    ) -> Result<(), IntegratorError> {
        let h = self.h;
        let sgn = R::from_i32(sign);
        for (i, &b) in self.beta.clone().iter().enumerate() {
            let a = R::from_i32(i32::try_from(i + 1).unwrap_or(i32::MAX)) * h;
            let bb = sgn * b * h;
            for (aa, kb) in [(a, bb), (-a, -bb)] {
                self.jac.drift(xj, vj, aa)?;
                kick(&self.jac, sys, t, xj, vj, kb, &mut self.scratch)?;
                self.stats.force_evaluations += 1;
                self.jac.drift(xj, vj, -aa)?;
            }
        }
        Ok(())
    }

    /// The kernel state synchronised to the current time (a copy), Jacobi.
    fn synced_kernel(&self) -> Result<Kernel<R>, IntegratorError> {
        let (mut xj, mut vj) = (self.xj.clone(), self.vj.clone());
        if self.lazy {
            self.jac.drift(&mut xj, &mut vj, -self.h * R::HALF)?;
        }
        Ok((xj, vj))
    }

    /// The uncorrected (kernel) inertial state at the current time.
    ///
    /// # Errors
    /// `FA-E0403` for slices of the wrong length; `FA-E0400` if the drift fails.
    pub fn kernel_state(&self, x: &mut [R], v: &mut [R]) -> Result<(), IntegratorError> {
        self.check_out(x, v)?;
        let (xj, vj) = self.synced_kernel()?;
        self.jac.inertial_of(&xj, x);
        self.jac.inertial_of(&vj, v);
        Ok(())
    }

    fn check_out(&self, x: &[R], v: &[R]) -> Result<(), IntegratorError> {
        let m = 3 * self.jac.len();
        for (what, got) in [("output position", x.len()), ("output velocity", v.len())] {
            if got != m {
                return Err(IntegratorError::Dimension {
                    what,
                    got,
                    expected: m,
                });
            }
        }
        Ok(())
    }

    /// The physical (corrected) inertial state at the current time.
    ///
    /// # Errors
    /// `FA-E0403` for slices of the wrong length; any error of the force model or a drift.
    pub fn physical_state<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        x: &mut [R],
        v: &mut [R],
    ) -> Result<(), IntegratorError> {
        self.check_out(x, v)?;
        let (mut xj, mut vj) = self.synced_kernel()?;
        let t = self.time();
        self.corrector(sys, t, &mut xj, &mut vj, -TO_KERNEL)?;
        self.jac.inertial_of(&xj, x);
        self.jac.inertial_of(&vj, v);
        Ok(())
    }

    /// Recompute [`Whfast::positions`] and [`Whfast::velocities`] for the current time.
    ///
    /// # Errors
    /// As [`Whfast::physical_state`].
    pub fn synchronize<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<(), IntegratorError> {
        let (mut x, mut v) = (
            vec![R::ZERO; self.out_x.len()],
            vec![R::ZERO; self.out_v.len()],
        );
        self.physical_state(sys, &mut x, &mut v)?;
        self.out_x = x;
        self.out_v = v;
        Ok(())
    }

    /// One step of `h`.
    ///
    /// # Errors
    /// `FA-E0400` for a non-finite acceleration or a failed drift; any error of the force model.
    pub fn step<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
    ) -> Result<WhStepReport<R>, IntegratorError> {
        let h = self.h;
        match &mut self.last {
            Some(((lx, lv), ll)) => {
                lx.copy_from_slice(&self.xj);
                lv.copy_from_slice(&self.vj);
                *ll = self.lazy;
            }
            None => self.last = Some(((self.xj.clone(), self.vj.clone()), self.lazy)),
        }
        if !self.lazy {
            self.jac.drift(&mut self.xj, &mut self.vj, h * R::HALF)?;
        }
        let t_mid = h.mul_add(R::HALF, self.time());
        kick(
            &self.jac,
            sys,
            t_mid,
            &self.xj,
            &mut self.vj,
            h,
            &mut self.scratch,
        )?;
        self.stats.force_evaluations += 1;
        self.jac.drift(&mut self.xj, &mut self.vj, h)?;
        self.lazy = true;
        self.k += 1;
        self.stats.steps += 1;
        if self.cfg.output_every_step {
            self.synchronize(sys)?;
        }
        Ok(WhStepReport {
            t: self.time(),
            dt: h,
        })
    }

    /// The time span `(t_k, t_{k+1})` of the last step (`None` before the first step and after
    /// a restore).
    #[must_use]
    pub fn last_step_span(&self) -> Option<(R, R)> {
        self.last.as_ref()?;
        let k = self.k.checked_sub(1)?;
        Some((self.grid(k), self.grid(self.k)))
    }

    fn no_dense() -> IntegratorError {
        cfg_err(
            "Wisdom-Holman dense output",
            0.0,
            "at least one step taken since the start or the last restore",
        )
    }

    /// Dense output (NUM-010) at `t` inside the last step: from the step's start, one
    /// Wisdom–Holman substep `D(τ/2) K(τ) D(τ/2)` of `τ = t − t_k`, then the corrector to real
    /// coordinates. Its error is that of a WH step of length τ (O(ε τ²)); at `t_{k+1}` it is the
    /// step's own output.
    ///
    /// # Errors
    /// `FA-E0402` before the first step or for `t` outside the last step; `FA-E0403` for slices of
    /// the wrong length; any error of the force model.
    pub fn dense_state<S: SecondOrderSystem<R> + ?Sized>(
        &mut self,
        sys: &S,
        t: R,
        x: &mut [R],
        v: &mut [R],
    ) -> Result<(), IntegratorError> {
        self.check_out(x, v)?;
        let (t0, _) = self.last_step_span().ok_or_else(Self::no_dense)?;
        let theta = (t - t0) / self.h;
        // The grid times t₀ + k·h are rounded, so θ at the step end can differ from 1 by ~k ulp.
        let slack = R::EPSILON.scale(16.0) * (t.abs() / self.h.abs()).max(R::ONE);
        if !(theta >= -slack && theta <= R::ONE + slack) {
            return Err(cfg_err(
                "dense-output time (fraction of the last step)",
                theta.to_f64(),
                "0 <= theta <= 1",
            ));
        }
        if t == self.time() || theta >= R::ONE {
            return self.physical_state(sys, x, v);
        }
        let ((mut xj, mut vj), lazy) = self.last.clone().ok_or_else(Self::no_dense)?;
        if lazy {
            self.jac.drift(&mut xj, &mut vj, -self.h * R::HALF)?;
        }
        let tau = t - t0;
        if tau != R::ZERO {
            self.jac.drift(&mut xj, &mut vj, tau * R::HALF)?;
            kick(
                &self.jac,
                sys,
                tau.mul_add(R::HALF, t0),
                &xj,
                &mut vj,
                tau,
                &mut self.scratch,
            )?;
            self.jac.drift(&mut xj, &mut vj, tau * R::HALF)?;
        }
        self.corrector(sys, t, &mut xj, &mut vj, -TO_KERNEL)?;
        self.jac.inertial_of(&xj, x);
        self.jac.inertial_of(&vj, v);
        Ok(())
    }

    /// Event location (NUM-010): a sign change of `g(t, x, v)` inside the last step, by the
    /// Illinois variant of regula falsi on [`Whfast::dense_state`]; `None` if `g` has the same sign
    /// at both ends.
    ///
    /// # Errors
    /// As [`Whfast::dense_state`].
    pub fn locate_event<S, G>(&mut self, sys: &S, g: G) -> Result<Option<R>, IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        G: Fn(R, &[R], &[R]) -> R,
    {
        let (t0, t1) = self.last_step_span().ok_or_else(Self::no_dense)?;
        let m = 3 * self.jac.len();
        let (mut x, mut v) = (vec![R::ZERO; m], vec![R::ZERO; m]);
        let h = self.h;
        let to_t = |theta: R| {
            if theta >= R::ONE {
                t1
            } else {
                theta.mul_add(h, t0)
            }
        };
        let mut at = |this: &mut Self, theta: R| -> Result<R, IntegratorError> {
            let t = to_t(theta);
            this.dense_state(sys, t, &mut x, &mut v)?;
            Ok(g(t, &x, &v))
        };
        let (mut a, mut b) = (R::ZERO, R::ONE);
        let (mut fa, mut fb) = (at(self, a)?, at(self, b)?);
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
            let fc = at(self, c)?;
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

    /// Step until `t_end` is reached or passed, calling `observer` after each step.
    ///
    /// # Errors
    /// Any error of [`Whfast::step`], or the first error returned by `observer`.
    pub fn integrate_to<S, F>(
        &mut self,
        sys: &S,
        t_end: R,
        mut observer: F,
    ) -> Result<(), IntegratorError>
    where
        S: SecondOrderSystem<R> + ?Sized,
        F: FnMut(&mut Self, &WhStepReport<R>) -> Result<(), IntegratorError>,
    {
        if !t_end.is_finite() {
            return Err(cfg_err("t_end", t_end.to_f64(), "finite"));
        }
        while (t_end - self.time()) * self.h > R::ZERO {
            let rep = self.step(sys)?;
            observer(self, &rep)?;
        }
        Ok(())
    }
}

/// Refuse a force model the Wisdom–Holman kick cannot integrate: velocity-dependent terms (EIH,
/// 1PN, Lense–Thirring), tiers other than Major, a geocentric Earth–Moon pair, or a central body
/// (the Sun, NAIF 10) that is not stored first. Full EIH is not supported in WH (NUM-004).
///
/// # Errors
/// `FA-E0402` naming the first unsupported item.
pub fn check_model<F: Frame, R: Real>(model: &NBodyModel<F, R>) -> Result<(), IntegratorError> {
    for t in model.terms() {
        if !SUPPORTED_TERMS.contains(&t.name()) {
            return Err(IntegratorError::Config {
                name: t.name(),
                value: 0.0,
                valid: "Wisdom-Holman supports velocity-independent terms only (newtonian-mutual, \
                        solar-j2-with-reaction, gr-potential-sun); full EIH is not supported in \
                        WH (NUM-004)",
            });
        }
    }
    let layout = model.layout();
    if layout.moon_pair().is_some() {
        return Err(cfg_err(
            "Wisdom-Holman with geocentric Moon variables",
            0.0,
            "barycentric slots only (MoonVariables::Barycentric)",
        ));
    }
    if layout.tiers().iter().any(|t| *t != Tier::Major) {
        return Err(cfg_err(
            "Wisdom-Holman with Minor or Ring bodies",
            0.0,
            "Major bodies only",
        ));
    }
    if layout.index_of(NaifId::SUN) != Some(0) {
        return Err(cfg_err(
            "Wisdom-Holman central body position",
            0.0,
            "the Sun (NAIF 10) stored first",
        ));
    }
    Ok(())
}

/// Everything a [`Whfast`] carries between steps, as exact `(hi, lo)` binary64 pairs (CI-010).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct WhSnapshot {
    /// [`Real::NAME`] of the backend.
    pub backend: String,
    /// Settings.
    pub config: WhConfig,
    /// GMs.
    pub gm: Vec<Parts>,
    /// Initial time of the grid.
    pub t0: Parts,
    /// Fixed step.
    pub h: Parts,
    /// Steps on the grid.
    pub steps_on_grid: u64,
    /// Kernel Jacobi positions.
    pub xj: Vec<[Parts; 3]>,
    /// Kernel Jacobi velocities.
    pub vj: Vec<[Parts; 3]>,
    /// Kernel state half a drift ahead.
    pub lazy: bool,
    /// Output positions.
    pub out_x: Vec<Parts>,
    /// Output velocities.
    pub out_v: Vec<Parts>,
    /// Counters.
    pub stats: WhStats,
}

fn ck(what: &'static str, problem: impl Into<String>) -> IntegratorError {
    IntegratorError::Checkpoint {
        what,
        problem: problem.into(),
    }
}

fn to_p<R: Real>(what: &'static str, z: R) -> Result<Parts, IntegratorError> {
    let (hi, lo) = z.to_parts();
    if R::from_parts(hi, lo) == z {
        Ok([hi, lo])
    } else {
        Err(ck(what, "value does not split exactly"))
    }
}

fn from_p<R: Real>(what: &'static str, [hi, lo]: Parts) -> Result<R, IntegratorError> {
    let z = R::from_parts(hi, lo);
    if !z.is_finite() || z.to_parts() != (hi, lo) {
        return Err(ck(
            what,
            format!("({hi:e}, {lo:e}) is not a normalised value"),
        ));
    }
    Ok(z)
}

impl<R: Real> Whfast<R> {
    /// The state carried between steps, for a checkpoint (CI-010).
    ///
    /// # Errors
    /// `FA-E0405` if a value cannot be split exactly.
    pub fn snapshot(&self) -> Result<WhSnapshot, IntegratorError> {
        let v3 = |what, z: &[V3<R>]| -> Result<Vec<[Parts; 3]>, IntegratorError> {
            z.iter()
                .map(|p| Ok([to_p(what, p[0])?, to_p(what, p[1])?, to_p(what, p[2])?]))
                .collect()
        };
        let vs = |what, z: &[R]| -> Result<Vec<Parts>, IntegratorError> {
            z.iter().map(|&w| to_p(what, w)).collect()
        };
        Ok(WhSnapshot {
            backend: R::NAME.to_owned(),
            config: self.cfg,
            gm: vs("GM", &self.jac.gm)?,
            t0: to_p("initial time", self.t0)?,
            h: to_p("step", self.h)?,
            steps_on_grid: self.k,
            xj: v3("Jacobi positions", &self.xj)?,
            vj: v3("Jacobi velocities", &self.vj)?,
            lazy: self.lazy,
            out_x: vs("output positions", &self.out_x)?,
            out_v: vs("output velocities", &self.out_v)?,
            stats: self.stats,
        })
    }

    /// Rebuild an integrator from [`Whfast::snapshot`]'s output; stepping on gives bit for bit
    /// the steps the original would have taken.
    ///
    /// # Errors
    /// `FA-E0405` for a snapshot of another backend or a malformed one.
    pub fn restore(snap: &WhSnapshot) -> Result<Self, IntegratorError> {
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
        let gm: Vec<R> = snap
            .gm
            .iter()
            .map(|&p| from_p("GM", p))
            .collect::<Result<_, _>>()?;
        let jac = Jacobi::new(&gm)?;
        let n = jac.len();
        let v3 = |what, z: &[[Parts; 3]]| -> Result<Vec<V3<R>>, IntegratorError> {
            if z.len() != n {
                return Err(ck(what, format!("has {} bodies, expected {n}", z.len())));
            }
            z.iter()
                .map(|p| {
                    Ok([
                        from_p(what, p[0])?,
                        from_p(what, p[1])?,
                        from_p(what, p[2])?,
                    ])
                })
                .collect()
        };
        let vs = |what, z: &[Parts]| -> Result<Vec<R>, IntegratorError> {
            if z.len() != 3 * n {
                return Err(ck(
                    what,
                    format!("has {} values, expected {}", z.len(), 3 * n),
                ));
            }
            z.iter().map(|&p| from_p(what, p)).collect()
        };
        let h: R = from_p("step", snap.h)?;
        if h == R::ZERO {
            return Err(ck("step", "is zero"));
        }
        let beta = snap
            .config
            .corrector
            .table()
            .iter()
            .map(|&(p, q)| {
                #[expect(clippy::cast_precision_loss, reason = "exact below 2^53")]
                let (pf, qf) = (p as f64, q as f64);
                R::from_f64(pf) / R::from_f64(qf)
            })
            .collect();
        Ok(Self {
            cfg: snap.config,
            jac,
            beta,
            t0: from_p("initial time", snap.t0)?,
            h,
            k: snap.steps_on_grid,
            xj: v3("Jacobi positions", &snap.xj)?,
            vj: v3("Jacobi velocities", &snap.vj)?,
            lazy: snap.lazy,
            out_x: vs("output positions", &snap.out_x)?,
            out_v: vs("output velocities", &snap.out_v)?,
            last: None,
            scratch: Scratch::new(n),
            stats: snap.stats,
        })
    }
}
