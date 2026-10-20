// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The Newtonian mutual attraction on the SoA kernel: [`newtonian_soa`] and the force term
//! [`NewtonianMutualSoa`].

use super::{Lane, Scalar, SoaBackend, SoaBodies, inv_r3_exact, load3, splat3};
use crate::nbody::{Accelerations, Layout, NewtonianMutual, View};
use crate::{ForceError, ForceTerm, Gauge, TermParameter};
use ferroastro_core::frames::Frame;
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// The kernel for one lane type: `out[c][k] = Σⱼ GMⱼ (xⱼ − xₖ)_c / r³` over the sources with a
/// different group label, `j` ascending. Returns the first coinciding pair `(k, j)` (body
/// positions in `b`) if two bodies of different groups are at the same position.
fn newton_lanes<L: Lane>(
    b: &SoaBodies<L::Elem>,
    out: &mut [Vec<L::Elem>; 3],
) -> Result<(), (usize, usize)> {
    let n = b.len();
    let len = b.gm.len();
    for o in out.iter_mut() {
        o.clear();
        o.resize(len, L::Elem::ZERO);
    }
    let zero = L::splat(L::Elem::ZERO);
    let one = L::splat(L::Elem::ONE);
    let mut k = 0;
    while k < n {
        let xi = load3::<L>(&b.pos, k);
        let gi = L::load(b.group.get(k..).unwrap_or(&[]));
        let mut a = [zero; 3];
        let mut bad = zero;
        for j in 0..n {
            let xj = splat3::<L>(&b.pos, j);
            let gj = L::splat(b.group.get(j).copied().unwrap_or(L::Elem::ZERO));
            let gm = L::splat(b.gm.get(j).copied().unwrap_or(L::Elem::ZERO));
            let d = [xj[0] - xi[0], xj[1] - xi[1], xj[2] - xi[2]];
            let r2 = (d[0] * d[0] + d[1] * d[1]) + d[2] * d[2];
            let s = inv_r3_exact(r2);
            for (ac, dc) in a.iter_mut().zip(d) {
                let t = gm * dc * s;
                *ac = *ac + L::select_eq(gi, gj, zero, t);
            }
            bad = bad + L::select_eq(gi, gj, zero, L::select_eq(r2, zero, one, zero));
        }
        for (o, ac) in out.iter_mut().zip(a) {
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
    Ok(())
}

/// The first body of another group at the same position as body `k` of `b` (`(k, k)` if none):
/// the cold path of the kernels' singularity report.
#[must_use]
pub fn coinciding<R: Real>(b: &SoaBodies<R>, k: usize) -> (usize, usize) {
    let at = |j: usize| -> [R; 3] {
        core::array::from_fn(|c| {
            b.pos
                .get(c)
                .and_then(|v| v.get(j))
                .copied()
                .unwrap_or(R::ZERO)
        })
    };
    let pk = at(k);
    let j = (0..b.len())
        .find(|&j| j != k && b.group.get(j) != b.group.get(k) && at(j) == pk)
        .unwrap_or(k);
    (k, j)
}

/// The Newtonian accelerations of every body of `b` by the bodies of other groups, into `out`
/// (three padded component arrays), with the lane type `backend` selects for `f64` (double-double
/// and other backends use [`Scalar`]).
///
/// # Errors
/// The positions `(k, j)` in `b` of two bodies of different groups at the same place.
pub fn newtonian_soa<R: Real>(
    b: &SoaBodies<R>,
    backend: SoaBackend,
    out: &mut [Vec<R>; 3],
) -> Result<(), (usize, usize)> {
    #[cfg(feature = "simd")]
    if backend == SoaBackend::Simd {
        use core::any::Any;
        if let (Some(b64), Some(o64)) = (
            (b as &dyn Any).downcast_ref::<SoaBodies<f64>>(),
            (out as &mut dyn Any).downcast_mut::<[Vec<f64>; 3]>(),
        ) {
            return newton_lanes::<super::F64x4>(b64, o64);
        }
    }
    let _ = backend;
    newton_lanes::<Scalar<R>>(b, out)
}

/// [`NewtonianMutual`] on the SoA kernel: the same Major tier, pair rule (with a geocentric Moon
/// the Earth–Moon pair is skipped and the third-body terms reach the EMB only), summation order
/// (sources ascending) and potential; exact `1/r³` and no FMA instead of the pairwise kernel's
/// `1/(r²√r²)` and fused `r²`. Agrees with [`NewtonianMutual`] to a few ulp, and its scalar and
/// SIMD backends agree bit for bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NewtonianMutualSoa {
    backend: SoaBackend,
}

impl NewtonianMutualSoa {
    /// The term with the given lane backend (for `f64`).
    #[must_use]
    pub const fn new(backend: SoaBackend) -> Self {
        Self { backend }
    }

    /// The lane backend.
    #[must_use]
    pub const fn backend(&self) -> SoaBackend {
        self.backend
    }
}

impl<F: Frame, R: Real> ForceTerm<F, R> for NewtonianMutualSoa {
    fn name(&self) -> &'static str {
        "newtonian-mutual (SoA kernel, exact 1/r^3, no FMA)"
    }
    fn gauge(&self) -> Gauge {
        Gauge::Newtonian
    }
    fn parameters(&self) -> Vec<TermParameter> {
        Vec::new()
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
        let layout = view.layout();
        let major = layout.indices(Tier::Major);
        let pair = layout.moon_pair().map(|p| (p.earth, p.moon));
        let earth_k = pair.and_then(|(e, _)| major.iter().position(|&i| i == e));
        let group = |k: usize| -> usize {
            match (pair, major.get(k), earth_k) {
                (Some((e, m)), Some(&i), Some(ek)) if i == e || i == m => ek,
                _ => k,
            }
        };
        let b = SoaBodies::gather(view, major, group);
        let mut out: [Vec<R>; 3] = Default::default();
        newtonian_soa(&b, self.backend, &mut out).map_err(|(k, j)| ForceError::Singular {
            i: major.get(k).copied().unwrap_or(k),
            j: major.get(j).copied().unwrap_or(j),
            t: t.to_f64(),
        })?;
        for (k, &i) in major.iter().enumerate() {
            let a: [R; 3] = core::array::from_fn(|c| {
                out.get(c)
                    .and_then(|v| v.get(k))
                    .copied()
                    .unwrap_or(R::ZERO)
            });
            acc.add_emb_only(i, a);
        }
        Ok(())
    }
    fn potential_over_g(&self, view: &View<'_, R>) -> Option<R> {
        <NewtonianMutual as ForceTerm<F, R>>::potential_over_g(&NewtonianMutual, view)
    }
}
