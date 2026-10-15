// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! Solar-system-barycentre (SSB) conditions, Newtonian and relativistic (NUM-018, SSB part).
//!
//! * Newtonian: Σ μᵢ rᵢ = 0 and Σ μᵢ vᵢ = 0.
//! * Relativistic, DE440 convention: the same with the "relativistic masses"
//!   μᵢ* = μᵢ [1 + vᵢ²/(2c²) − (1/(2c²)) Σⱼ≠ᵢ μⱼ/rᵢⱼ], the velocities being barycentric. The
//!   velocity condition neglects dμ*/dt (Σ μᵢ* vᵢ = 0), as stated.
//!
//! **Transcription status:** written from Folkner et al. 2014 (IPN PR 42-196) and Park et al.
//! 2021 (AJ 161, 105) Eqs. 1–2 as cited by NUM-018, **from memory, without re-opening the PDFs**
//! (IMPLEMENTATION-NOTES WP11, NEEDS HUMAN REVIEW). The 1/(2c²) structure is the standard 1PN
//! mass-energy weight (rest mass + kinetic − ½ potential energy per unit c²).
//!
//! Every body of the state with a catalogue GM contributes, whatever its tier.

use crate::{Catalogue, SystemError, SystemState};
use ferroastro_core::frames::{Barycentric, Displacement, Frame};
use ferroastro_core::units::{Length, Velocity};
use ferroastro_real::{Neumaier, Real};

/// Which masses weight the barycentre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassModel {
    /// μᵢ (Newtonian centre of mass).
    Newtonian,
    /// μᵢ* (DE440 relativistic SSB, NUM-018).
    RelativisticDe440,
}

impl MassModel {
    /// Report label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            MassModel::Newtonian => "Newtonian mu_i",
            MassModel::RelativisticDe440 => {
                "relativistic mu_i* = mu_i [1 + v_i^2/(2c^2) - sum_j mu_j/(2c^2 r_ij)] (DE440)"
            }
        }
    }
}

/// The mass-weighted centre of a system relative to its origin.
#[derive(Debug, Clone, PartialEq)]
pub struct BarycentreOffset<R: Real = f64> {
    /// Σ wᵢ rᵢ / Σ wᵢ, km.
    pub position_km: [R; 3],
    /// Σ wᵢ vᵢ / Σ wᵢ, km/s.
    pub velocity_km_s: [R; 3],
    /// Σ wᵢ, km³/s².
    pub total_gm: R,
    /// Number of bodies weighted.
    pub bodies: usize,
    /// Mass model.
    pub model: MassModel,
}

impl<R: Real> BarycentreOffset<R> {
    /// |position offset| in km (f64, for reports).
    #[must_use]
    pub fn distance_km(&self) -> f64 {
        let [x, y, z] = self.position_km.map(Real::to_f64);
        x.hypot(y).hypot(z)
    }

    /// |velocity offset| in km/s (f64, for reports).
    #[must_use]
    pub fn speed_km_s(&self) -> f64 {
        let [x, y, z] = self.velocity_km_s.map(Real::to_f64);
        x.hypot(y).hypot(z)
    }
}

/// The weights wᵢ (km³/s²) of every body of `state`, in storage order.
///
/// # Errors
/// `FA-E1100` if a body has no catalogue GM; `FA-E1102` for coincident bodies under
/// [`MassModel::RelativisticDe440`].
pub fn mass_weights<F: Frame, R: Real>(
    catalogue: &Catalogue,
    state: &SystemState<F, Barycentric, R>,
    model: MassModel,
    c: Velocity,
) -> Result<Vec<R>, SystemError> {
    let mu: Vec<R> = state
        .ids()
        .iter()
        .map(|&id| catalogue.gm(id).map(|g| R::from_f64(g.km3_per_s2())))
        .collect::<Result<_, _>>()?;
    if model == MassModel::Newtonian {
        return Ok(mu);
    }
    let (p, _) = state.components();
    let c2 = R::from_f64(c.km_per_s()) * R::from_f64(c.km_per_s());
    let two_c2 = c2 + c2;
    let n = mu.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (Some(&mui), Some(raw)) = (mu.get(i), state.raw_at(i)) else {
            continue;
        };
        let ([rx, ry, rz], [vx, vy, vz]) = raw;
        let v2 = vx * vx + vy * vy + vz * vz;
        let [px, py, pz] = p;
        let mut pot = Neumaier::<R>::new();
        for (j, &muj) in mu.iter().enumerate() {
            if j == i {
                continue;
            }
            let dj = |c: &[R], rk: R| c.get(j).copied().unwrap_or(R::ZERO) - rk;
            let r = dj(px, rx).hypot(dj(py, ry)).hypot(dj(pz, rz));
            if r.to_f64() <= 0.0 {
                return Err(SystemError::Barycentre {
                    problem: format!(
                        "bodies {} and {} coincide",
                        state.ids().get(i).map_or(-1, |x| x.0),
                        state.ids().get(j).map_or(-1, |x| x.0)
                    ),
                    fix: "check the initial conditions",
                });
            }
            pot.add(muj / r);
        }
        out.push(mui * (R::ONE + (v2 - pot.total()) / two_c2));
    }
    Ok(out)
}

/// The barycentre of `state` under `model` (relative to the state's origin, the nominal SSB).
///
/// # Errors
/// As [`mass_weights`]; `FA-E1102` if the total weight is not positive.
pub fn barycentre<F: Frame, R: Real>(
    catalogue: &Catalogue,
    state: &SystemState<F, Barycentric, R>,
    model: MassModel,
    c: Velocity,
) -> Result<BarycentreOffset<R>, SystemError> {
    let w = mass_weights(catalogue, state, model, c)?;
    let (p, v) = state.components();
    let mut total = Neumaier::<R>::new();
    let mut sp = [Neumaier::<R>::new(), Neumaier::new(), Neumaier::new()];
    let mut sv = [Neumaier::<R>::new(), Neumaier::new(), Neumaier::new()];
    for (i, &wi) in w.iter().enumerate() {
        total.add(wi);
        for (s, c) in sp
            .iter_mut()
            .zip(p.iter())
            .chain(sv.iter_mut().zip(v.iter()))
        {
            s.add(wi * c.get(i).copied().unwrap_or(R::ZERO));
        }
    }
    let m = total.total();
    if m.to_f64() <= 0.0 {
        return Err(SystemError::Barycentre {
            problem: format!("total GM {} km^3/s^2 is not positive", m.to_f64()),
            fix: "include at least one body with a positive GM",
        });
    }
    Ok(BarycentreOffset {
        position_km: sp.map(|s| s.total() / m),
        velocity_km_s: sv.map(|s| s.total() / m),
        total_gm: m,
        bodies: w.len(),
        model,
    })
}

/// Move the origin of `state` to its barycentre under `model`: every body is translated by the
/// barycentre offset, `iterations` times (μ* depends on the velocities, so the relativistic
/// condition needs a fixed-point pass or two; translation leaves the rᵢⱼ unchanged). Returns the
/// offset that remains after the last pass.
///
/// # Errors
/// As [`barycentre`].
pub fn rebarycentre<F: Frame, R: Real>(
    catalogue: &Catalogue,
    state: &mut SystemState<F, Barycentric, R>,
    model: MassModel,
    c: Velocity,
    iterations: usize,
) -> Result<BarycentreOffset<R>, SystemError> {
    for _ in 0..iterations {
        let b = barycentre(catalogue, state, model, c)?;
        let nf = |what: &str, x: R| SystemError::non_finite(what.to_owned(), x.to_f64());
        let d = b
            .position_km
            .map(|x| Length::from_km(x).map_err(|_| nf("barycentre offset (km)", x)));
        let dv = b
            .velocity_km_s
            .map(|x| Velocity::from_km_per_s(x).map_err(|_| nf("barycentre velocity (km/s)", x)));
        let [d0, d1, d2] = d;
        let [v0, v1, v2] = dv;
        state.translate(&Displacement::new(d0?, d1?, d2?), [v0?, v1?, v2?]);
    }
    barycentre(catalogue, state, model, c)
}
