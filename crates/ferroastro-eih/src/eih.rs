// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The EIH point-mass acceleration, Park et al. 2021 (DE440) Eq. 27.
//!
//! As typeset (bold = vectors, `r_ij = |r_i − r_j|`, sums over all bodies):
//!
//! ```text
//! a_i = Σ_{j≠i} GM_j (r_j − r_i)/r_ij³ { 1 − 2(β+γ)/c² Σ_{k≠i} GM_k/r_ik − (2β−1)/c² Σ_{k≠j} GM_k/r_jk
//!                                       + γ (v_i/c)² + (1+γ)(v_j/c)² − 2(1+γ)/c² v_i·v_j
//!                                       − 3/(2c²) [(r_i − r_j)·v_j / r_ij]² + 1/(2c²) (r_j − r_i)·a_j }
//!     + 1/c² Σ_{j≠i} GM_j/r_ij³ {[r_i − r_j]·[(2+2γ) v_i − (1+2γ) v_j]} (v_i − v_j)
//!     + (3+4γ)/(2c²) Σ_{j≠i} GM_j a_j / r_ij
//! ```
//!
//! [`Eih`] adds `a_i` minus its Newtonian part `Σ GM_j (r_j − r_i)/r_ij³`, i.e. three c⁻² sums:
//!
//! * **S1** `Σ_j GM_j (r_j − r_i)/r_ij³ · B_ij / c²` with
//!   `B_ij = −2(β+γ) U_i − (2β−1) U_j + γ v_i² + (1+γ) v_j² − 2(1+γ) v_i·v_j
//!   − (3/2) [(r_i − r_j)·v_j / r_ij]² + (1/2) (r_j − r_i)·a_j`,
//!   `U_i = Σ_{k≠i} GM_k/r_ik`;
//! * **S2** `Σ_j GM_j/r_ij³ · [(r_i − r_j)·((2+2γ) v_i − (1+2γ) v_j)] (v_i − v_j) / c²`;
//! * **S3** `(3+4γ)/(2c²) Σ_j GM_j a_j / r_ij`.
//!
//! The sums run over the model's Major bodies (sources `j`, potentials `k`); the term changes the
//! motion of the Major bodies only. Minor and Ring bodies stay Newtonian (interpretation, NEEDS
//! HUMAN REVIEW before WP22). `a_j` on the right-hand side is the Newtonian point-mass
//! acceleration of body `j` by the other Majors ([`RhsAcceleration::Newtonian`], NUM-018 default)
//! or that plus the default c⁻² correction ([`RhsAcceleration::Iterated`], one fixed-point step).

#![expect(
    clippy::indexing_slicing,
    reason = "every index is < m, the number of Major bodies: per-body vectors have length m, \
              the distance matrix m * m, and 3-vectors are indexed by 0..3"
)]

use ferroastro_core::frames::Frame;
use ferroastro_core::units::Velocity;
use ferroastro_forces::{Accelerations, ForceError, ForceTerm, Gauge, Layout, TermParameter, View};
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// The accelerations `a_j` used on the right-hand side of Eq. 27 (NUM-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RhsAcceleration {
    /// The Newtonian point-mass accelerations by the Major bodies (default).
    #[default]
    Newtonian,
    /// One fixed-point iteration: the Newtonian accelerations plus the c⁻² correction computed
    /// with Newtonian right-hand sides. Differs from the default at O(c⁻⁴).
    Iterated,
}

impl RhsAcceleration {
    const fn label(self) -> &'static str {
        match self {
            RhsAcceleration::Newtonian => "Newtonian a_j",
            RhsAcceleration::Iterated => "a_j iterated once",
        }
    }
}

/// The c⁻² part of the EIH acceleration (Eq. 27) for the Major bodies, with PPN β, γ.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eih<R: Real = f64> {
    c2: R,
    beta: R,
    gamma: R,
    rhs: RhsAcceleration,
}

fn finite<R: Real>(name: &'static str, value: R) -> Result<R, ForceError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(ForceError::Invalid {
            name,
            value: value.to_f64(),
            valid: "finite",
        })
    }
}

impl<R: Real> Eih<R> {
    /// The term with the speed of light `c`, PPN `β`, `γ` and the right-hand-side mode.
    ///
    /// # Errors
    /// `FA-E0500` if `c` is not positive and finite or β, γ are not finite.
    pub fn new(
        c: Velocity<R>,
        beta: R,
        gamma: R,
        rhs: RhsAcceleration,
    ) -> Result<Self, ForceError> {
        let cc = c.km_per_s();
        if !(cc.is_finite() && cc > R::ZERO) {
            return Err(ForceError::Invalid {
                name: "c (km/s)",
                value: cc.to_f64(),
                valid: "finite and > 0",
            });
        }
        Ok(Self {
            c2: cc * cc,
            beta: finite("PPN beta", beta)?,
            gamma: finite("PPN gamma", gamma)?,
            rhs,
        })
    }

    /// The right-hand-side mode.
    #[must_use]
    pub const fn rhs(&self) -> RhsAcceleration {
        self.rhs
    }

    /// `(c², β, γ)` for the SoA kernel ([`crate::EihSoa`]).
    pub(crate) const fn c2_beta_gamma(&self) -> (R, R, R) {
        (self.c2, self.beta, self.gamma)
    }

    /// The c⁻² corrections of the Major bodies (km/s²), in `layout.indices(Tier::Major)` order.
    ///
    /// # Errors
    /// `FA-E0501` if two Major bodies coincide.
    pub fn corrections(&self, t: R, view: &View<'_, R>) -> Result<Vec<[R; 3]>, ForceError> {
        let pairs = Pairs::new(t, view)?;
        let newton = pairs.newtonian();
        let first = self.correction_with(&pairs, &newton);
        Ok(match self.rhs {
            RhsAcceleration::Newtonian => first,
            RhsAcceleration::Iterated => {
                let rhs: Vec<[R; 3]> = newton
                    .iter()
                    .zip(&first)
                    .map(|(n, d)| [n[0] + d[0], n[1] + d[1], n[2] + d[2]])
                    .collect();
                self.correction_with(&pairs, &rhs)
            }
        })
    }

    /// Eq. 27 minus its Newtonian part, with the right-hand-side accelerations `acc`.
    fn correction_with(&self, p: &Pairs<R>, acc: &[[R; 3]]) -> Vec<[R; 3]> {
        let (beta, gamma) = (self.beta, self.gamma);
        let inv_c2 = R::ONE / self.c2;
        let one = R::ONE;
        let three_halves = R::from_f64(1.5);
        // Constant coefficients of B_ij and S2/S3.
        let k_ui = -(R::TWO * (beta + gamma));
        let k_uj = -(R::TWO * beta - one);
        let k_vj2 = one + gamma;
        let k_vivj = -(R::TWO * (one + gamma));
        let k_s2i = R::TWO + R::TWO * gamma;
        let k_s2j = one + R::TWO * gamma;
        let k_s3 = (R::from_f64(3.0) + R::from_f64(4.0) * gamma) * R::HALF;
        let m = p.len();
        let mut out = vec![[R::ZERO; 3]; m];
        for (i, out_i) in out.iter_mut().enumerate() {
            let (ri, vi) = (p.pos[i], p.vel[i]);
            let vi2 = dot(vi, vi);
            let mut s = [R::ZERO; 3];
            for j in (0..m).filter(|&j| j != i) {
                let gj = p.gm[j];
                if gj == R::ZERO {
                    continue;
                }
                let (rj, vj, aj) = (p.pos[j], p.vel[j], acc[j]);
                let rij = p.dist(i, j);
                let inv_r3 = p.inv_r3(i, j);
                let d_ji = sub(rj, ri); // r_j − r_i
                let d_ij = sub(ri, rj); // r_i − r_j
                let proj = dot(d_ij, vj) / rij;
                let b = k_ui * p.u[i]
                    + k_uj * p.u[j]
                    + gamma * vi2
                    + k_vj2 * dot(vj, vj)
                    + k_vivj * dot(vi, vj)
                    - three_halves * proj * proj
                    + R::HALF * dot(d_ji, aj);
                let w = [
                    k_s2i * vi[0] - k_s2j * vj[0],
                    k_s2i * vi[1] - k_s2j * vj[1],
                    k_s2i * vi[2] - k_s2j * vj[2],
                ];
                let s1 = gj * inv_r3 * b; // multiplies (r_j − r_i)
                let s2 = gj * inv_r3 * dot(d_ij, w); // multiplies (v_i − v_j)
                let s3 = k_s3 * gj / rij; // multiplies a_j
                for c in 0..3 {
                    s[c] += s1 * d_ji[c] + s2 * (vi[c] - vj[c]) + s3 * aj[c];
                }
            }
            *out_i = s.map(|x| x * inv_c2);
        }
        out
    }
}

/// Positions, velocities, GMs, pair distances and potentials of the Major bodies.
struct Pairs<R: Real> {
    pos: Vec<[R; 3]>,
    vel: Vec<[R; 3]>,
    gm: Vec<R>,
    /// Row-major `m × m` distances (0 on the diagonal).
    r: Vec<R>,
    /// `U_i = Σ_{k≠i} GM_k / r_ik`.
    u: Vec<R>,
}

impl<R: Real> Pairs<R> {
    fn new(t: R, view: &View<'_, R>) -> Result<Self, ForceError> {
        let layout = view.layout();
        let major = layout.indices(Tier::Major);
        let m = major.len();
        let pos: Vec<[R; 3]> = major.iter().map(|&i| view.position(i)).collect();
        let vel: Vec<[R; 3]> = major.iter().map(|&i| view.velocity(i)).collect();
        let gm: Vec<R> = major.iter().map(|&i| layout.gm(i)).collect();
        let mut r = vec![R::ZERO; m * m];
        for i in 0..m {
            for j in (i + 1)..m {
                let d = sub(pos[j], pos[i]);
                let rij = dot(d, d).sqrt();
                if rij == R::ZERO {
                    return Err(ForceError::Singular {
                        i: major[i],
                        j: major[j],
                        t: t.to_f64(),
                    });
                }
                r[i * m + j] = rij;
                r[j * m + i] = rij;
            }
        }
        let u = (0..m)
            .map(|i| {
                (0..m)
                    .filter(|&k| k != i)
                    .fold(R::ZERO, |acc, k| acc + gm[k] / r[i * m + k])
            })
            .collect();
        Ok(Self { pos, vel, gm, r, u })
    }

    fn len(&self) -> usize {
        self.gm.len()
    }

    fn dist(&self, i: usize, j: usize) -> R {
        self.r[i * self.len() + j]
    }

    fn inv_r3(&self, i: usize, j: usize) -> R {
        let r = self.dist(i, j);
        R::ONE / (r * r * r)
    }

    /// Newtonian point-mass accelerations by the other Majors.
    fn newtonian(&self) -> Vec<[R; 3]> {
        let m = self.len();
        (0..m)
            .map(|j| {
                let mut a = [R::ZERO; 3];
                for k in (0..m).filter(|&k| k != j) {
                    let s = self.gm[k] * self.inv_r3(j, k);
                    let d = sub(self.pos[k], self.pos[j]);
                    for c in 0..3 {
                        a[c] += s * d[c];
                    }
                }
                a
            })
            .collect()
    }
}

fn sub<R: Real>(a: [R; 3], b: [R; 3]) -> [R; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot<R: Real>(a: [R; 3], b: [R; 3]) -> R {
    a[0].mul_add(b[0], a[1].mul_add(b[1], a[2] * b[2]))
}

impl<F: Frame, R: Real> ForceTerm<F, R> for Eih<R> {
    fn name(&self) -> &'static str {
        "eih-1pn (Park et al. 2021 Eq. 27)"
    }
    fn gauge(&self) -> Gauge {
        Gauge::PpnIsotropicHarmonic
    }
    fn parameters(&self) -> Vec<TermParameter> {
        vec![
            TermParameter {
                name: "PPN beta",
                value: self.beta.to_f64(),
                unit: "1",
            },
            TermParameter {
                name: "PPN gamma",
                value: self.gamma.to_f64(),
                unit: "1",
            },
            TermParameter {
                name: "c",
                value: self.c2.sqrt().to_f64(),
                unit: "km/s",
            },
            TermParameter {
                name: match self.rhs {
                    RhsAcceleration::Newtonian => "RHS a_j fixed-point iterations (Newtonian a_j)",
                    RhsAcceleration::Iterated => {
                        "RHS a_j fixed-point iterations (a_j iterated once)"
                    }
                },
                value: match self.rhs {
                    RhsAcceleration::Newtonian => 0.0,
                    RhsAcceleration::Iterated => 1.0,
                },
                unit: "1",
            },
            TermParameter {
                name: "time argument TDB (NUM-017)",
                value: 1.0,
                unit: "1",
            },
        ]
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
        // The 1PN energy is not a sum of term potentials; the model energy is unavailable.
        None
    }
}

impl<R: Real> std::fmt::Display for Eih<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EIH 1PN (Park et al. 2021 Eq. 27), beta = {}, gamma = {}, {}",
            self.beta.to_f64(),
            self.gamma.to_f64(),
            self.rhs.label()
        )
    }
}
