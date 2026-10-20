// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! [`EihSoa`]: the [`Eih`] term (Park et al. 2021 Eq. 27 minus its Newtonian part) on the
//! structure-of-arrays kernel of `ferroastro_forces::soa` (NUM-021, WP26).
//!
//! The physics, the S1/S2/S3 grouping, the coefficients and the right-hand-side mode are those
//! of [`Eih`] (see its module documentation for the transcription). The kernel differs only in
//! arithmetic: the exact `1/r³` ([`inv_r3_exact`]), `r = √r²`, dot products without FMA, and the
//! sources of each target summed in storage order inside one lane. Pass A gives `U_i` and the
//! Newtonian `a_i` over all Major bodies, pass B the c⁻² correction. The scalar and SIMD
//! instantiations are bitwise identical ([`ferroastro_forces::soa::Lane`]).

use crate::eih::{Eih, RhsAcceleration};
use ferroastro_core::frames::Frame;
use ferroastro_forces::soa::{
    Lane, Scalar, SoaBackend, SoaBodies, coinciding, dot3, inv_r3_exact, load3, splat3,
};
use ferroastro_forces::{Accelerations, ForceError, ForceTerm, Gauge, Layout, TermParameter, View};
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// Constant coefficients of `B_ij`, S2 and S3 (as [`Eih`] forms them).
#[derive(Debug, Clone, Copy)]
struct Coefficients<R: Real> {
    inv_c2: R,
    gamma: R,
    k_ui: R,
    k_uj: R,
    k_vj2: R,
    k_vivj: R,
    k_s2i: R,
    k_s2j: R,
    k_s3: R,
}

impl<R: Real> Coefficients<R> {
    fn of(eih: &Eih<R>) -> Self {
        let (c2, beta, gamma) = eih.c2_beta_gamma();
        let one = R::ONE;
        Self {
            inv_c2: R::ONE / c2,
            gamma,
            k_ui: -(R::TWO * (beta + gamma)),
            k_uj: -(R::TWO * beta - one),
            k_vj2: one + gamma,
            k_vivj: -(R::TWO * (one + gamma)),
            k_s2i: R::TWO + R::TWO * gamma,
            k_s2j: one + R::TWO * gamma,
            k_s3: (R::from_f64(3.0) + R::from_f64(4.0) * gamma) * R::HALF,
        }
    }
}

/// Pass A for one lane type: `U_k = Σ_{j≠k} GM_j / r_kj` and the Newtonian `a_k`, sources
/// ascending. Returns the first coinciding pair `(k, j)`.
#[expect(
    clippy::type_complexity,
    reason = "(potentials, three acceleration components) of one pass"
)]
fn pass_a<L: Lane>(
    b: &SoaBodies<L::Elem>,
) -> Result<(Vec<L::Elem>, [Vec<L::Elem>; 3]), (usize, usize)> {
    let n = b.len();
    let len = b.gm.len();
    let mut u = vec![L::Elem::ZERO; len];
    let mut an: [Vec<L::Elem>; 3] = core::array::from_fn(|_| vec![L::Elem::ZERO; len]);
    let zero = L::splat(L::Elem::ZERO);
    let one = L::splat(L::Elem::ONE);
    let mut k = 0;
    while k < n {
        let xi = load3::<L>(&b.pos, k);
        let gi = L::load(b.group.get(k..).unwrap_or(&[]));
        let (mut uk, mut a, mut bad) = (zero, [zero; 3], zero);
        for j in 0..n {
            let xj = splat3::<L>(&b.pos, j);
            let gj = L::splat(b.group.get(j).copied().unwrap_or(L::Elem::ZERO));
            let gm = L::splat(b.gm.get(j).copied().unwrap_or(L::Elem::ZERO));
            let d = [xj[0] - xi[0], xj[1] - xi[1], xj[2] - xi[2]];
            let r2 = dot3(d, d);
            let s = r2.sqrt();
            let inv3 = inv_r3_exact(r2);
            uk = uk + L::select_eq(gi, gj, zero, gm / s);
            let g3 = gm * inv3;
            for (ac, dc) in a.iter_mut().zip(d) {
                *ac = *ac + L::select_eq(gi, gj, zero, g3 * dc);
            }
            bad = bad + L::select_eq(gi, gj, zero, L::select_eq(r2, zero, one, zero));
        }
        if let Some(dst) = u.get_mut(k..) {
            uk.store(dst);
        }
        for (o, ac) in an.iter_mut().zip(a) {
            if let Some(dst) = o.get_mut(k..) {
                ac.store(dst);
            }
        }
        let mut flags = [L::Elem::ZERO; 4];
        bad.store(&mut flags);
        if let Some(lane) = flags
            .iter()
            .take(L::WIDTH.min(n - k))
            .position(|&f| f != L::Elem::ZERO)
        {
            return Err(coinciding(b, k + lane));
        }
        k += L::WIDTH;
    }
    Ok((u, an))
}

/// Pass B for one lane type: the c⁻² correction of every body with right-hand-side
/// accelerations `rhs` and potentials `u` (from pass A).
fn pass_b<L: Lane>(
    b: &SoaBodies<L::Elem>,
    u: &[L::Elem],
    rhs: &[Vec<L::Elem>; 3],
    k: &Coefficients<L::Elem>,
) -> [Vec<L::Elem>; 3] {
    let n = b.len();
    let len = b.gm.len();
    let mut out: [Vec<L::Elem>; 3] = core::array::from_fn(|_| vec![L::Elem::ZERO; len]);
    let zero = L::splat(L::Elem::ZERO);
    let half = L::splat(L::Elem::HALF);
    let three_halves = L::splat(L::Elem::from_f64(1.5));
    let c = |x: L::Elem| L::splat(x);
    let mut i = 0;
    while i < n {
        let ri = load3::<L>(&b.pos, i);
        let vi = load3::<L>(&b.vel, i);
        let gi = L::load(b.group.get(i..).unwrap_or(&[]));
        let ui = L::load(u.get(i..).unwrap_or(&[]));
        let vi2 = dot3(vi, vi);
        let mut s = [zero; 3];
        for j in 0..n {
            let rj = splat3::<L>(&b.pos, j);
            let vj = splat3::<L>(&b.vel, j);
            let aj = splat3::<L>(rhs, j);
            let gj = L::splat(b.group.get(j).copied().unwrap_or(L::Elem::ZERO));
            let gm = L::splat(b.gm.get(j).copied().unwrap_or(L::Elem::ZERO));
            let uj = L::splat(u.get(j).copied().unwrap_or(L::Elem::ZERO));
            let d_ji = [rj[0] - ri[0], rj[1] - ri[1], rj[2] - ri[2]];
            let d_ij = [ri[0] - rj[0], ri[1] - rj[1], ri[2] - rj[2]];
            let r2 = dot3(d_ji, d_ji);
            let rij = r2.sqrt();
            let inv3 = inv_r3_exact(r2);
            let proj = dot3(d_ij, vj) / rij;
            let bij = c(k.k_ui) * ui
                + c(k.k_uj) * uj
                + c(k.gamma) * vi2
                + c(k.k_vj2) * dot3(vj, vj)
                + c(k.k_vivj) * dot3(vi, vj)
                - three_halves * proj * proj
                + half * dot3(d_ji, aj);
            let w = [
                c(k.k_s2i) * vi[0] - c(k.k_s2j) * vj[0],
                c(k.k_s2i) * vi[1] - c(k.k_s2j) * vj[1],
                c(k.k_s2i) * vi[2] - c(k.k_s2j) * vj[2],
            ];
            let s1 = gm * inv3 * bij;
            let s2 = gm * inv3 * dot3(d_ij, w);
            let s3 = c(k.k_s3) * gm / rij;
            for (((sq, dq), (viq, vjq)), aq) in
                s.iter_mut().zip(d_ji).zip(vi.iter().zip(vj)).zip(aj)
            {
                let term = s1 * dq + s2 * (*viq - vjq) + s3 * aq;
                *sq = *sq + L::select_eq(gi, gj, zero, term);
            }
        }
        for (o, sq) in out.iter_mut().zip(s) {
            if let Some(dst) = o.get_mut(i..) {
                (sq * c(k.inv_c2)).store(dst);
            }
        }
        i += L::WIDTH;
    }
    out
}

/// Both passes for one lane type; `iterate` adds the fixed-point step.
fn corrections_lanes<L: Lane>(
    b: &SoaBodies<L::Elem>,
    k: &Coefficients<L::Elem>,
    iterate: bool,
) -> Result<[Vec<L::Elem>; 3], (usize, usize)> {
    let (u, an) = pass_a::<L>(b)?;
    let first = pass_b::<L>(b, &u, &an, k);
    if !iterate {
        return Ok(first);
    }
    let rhs: [Vec<L::Elem>; 3] = core::array::from_fn(|q| {
        an.get(q)
            .zip(first.get(q))
            .map(|(a, d)| a.iter().zip(d).map(|(&x, &y)| x + y).collect())
            .unwrap_or_default()
    });
    Ok(pass_b::<L>(b, &u, &rhs, k))
}

/// The EIH c⁻² term on the SoA kernel. Same physics, parameters and gauge as [`Eih`]; agrees
/// with it to a few ulp of the correction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EihSoa<R: Real = f64> {
    eih: Eih<R>,
    backend: SoaBackend,
}

impl<R: Real> EihSoa<R> {
    /// The term for an [`Eih`] configuration and the lane backend (for `f64`).
    #[must_use]
    pub const fn new(eih: Eih<R>, backend: SoaBackend) -> Self {
        Self { eih, backend }
    }

    /// The EIH configuration.
    #[must_use]
    pub const fn eih(&self) -> &Eih<R> {
        &self.eih
    }

    /// The c⁻² corrections of the Major bodies (km/s²), in `layout.indices(Tier::Major)` order.
    ///
    /// # Errors
    /// `FA-E0501` if two Major bodies coincide.
    pub fn corrections(&self, t: R, view: &View<'_, R>) -> Result<Vec<[R; 3]>, ForceError> {
        let major = view.layout().indices(Tier::Major);
        let b = SoaBodies::gather(view, major, |k| k);
        let k = Coefficients::of(&self.eih);
        let iterate = self.eih.rhs() == RhsAcceleration::Iterated;
        let out =
            dispatch(&b, &k, iterate, self.backend).map_err(|(i, j)| ForceError::Singular {
                i: major.get(i).copied().unwrap_or(i),
                j: major.get(j).copied().unwrap_or(j),
                t: t.to_f64(),
            })?;
        Ok((0..major.len())
            .map(|i| {
                core::array::from_fn(|q| {
                    out.get(q)
                        .and_then(|v| v.get(i))
                        .copied()
                        .unwrap_or(R::ZERO)
                })
            })
            .collect())
    }
}

/// The lane type `backend` selects for `f64`; [`Scalar`] otherwise.
fn dispatch<R: Real>(
    b: &SoaBodies<R>,
    k: &Coefficients<R>,
    iterate: bool,
    backend: SoaBackend,
) -> Result<[Vec<R>; 3], (usize, usize)> {
    #[cfg(feature = "simd")]
    if backend == SoaBackend::Simd {
        use core::any::Any;
        use ferroastro_forces::soa::F64x4;
        if let (Some(b64), Some(k64)) = (
            (b as &dyn Any).downcast_ref::<SoaBodies<f64>>(),
            (k as &dyn Any).downcast_ref::<Coefficients<f64>>(),
        ) {
            let out = corrections_lanes::<F64x4>(b64, k64, iterate)?;
            let boxed: Box<dyn Any> = Box::new(out);
            if let Ok(o) = boxed.downcast::<[Vec<R>; 3]>() {
                return Ok(*o);
            }
        }
    }
    let _ = backend;
    corrections_lanes::<Scalar<R>>(b, k, iterate)
}

impl<F: Frame, R: Real> ForceTerm<F, R> for EihSoa<R> {
    fn name(&self) -> &'static str {
        "eih-1pn (Park et al. 2021 Eq. 27; SoA kernel, exact 1/r^3, no FMA)"
    }
    fn gauge(&self) -> Gauge {
        Gauge::PpnIsotropicHarmonic
    }
    fn parameters(&self) -> Vec<TermParameter> {
        <Eih<R> as ForceTerm<F, R>>::parameters(&self.eih)
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major]
    }
    fn bind(&mut self, _layout: &Layout<R>) -> Result<(), ForceError> {
        Ok(())
    }
    fn accumulate(
        &self,
        t: R,
        view: &View<'_, R>,
        acc: &mut Accelerations<'_, R>,
    ) -> Result<(), ForceError> {
        let major = view.layout().indices(Tier::Major);
        for (&i, a) in major.iter().zip(self.corrections(t, view)?) {
            acc.add(i, a);
        }
        Ok(())
    }
    fn potential_over_g(&self, _view: &View<'_, R>) -> Option<R> {
        None
    }
}

impl<R: Real> std::fmt::Display for EihSoa<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (SoA kernel)", self.eih)
    }
}
