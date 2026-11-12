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
//! **Sums over all bodies (WP22, maintainer decision D1 of 2026-10-21, NUM-009, NUM-020, PHY-014).**
//! The correction is evaluated for every moving body (Major and Minor: the Sun, the planets, the
//! Moon, the asteroids and the KBOs), each with its own 1PN terms. The sums run over the pairs the
//! tiered Newtonian model contains, the symmetric interaction graph `E`: Major–Major,
//! Major–Minor, Major–Ring and Minor–Ring (no Minor–Minor, no Ring–Ring pair). For a receiver
//! `i`, the sources `j` and the potential `U_i` run over `E(i)`; `U_j` runs over `E(j)`. Ring
//! bodies are fixed masses: they are sources with `v_j = a_j = 0` and receive nothing. With
//! [`MinorReaction::Off`] the Minor bodies are test particles (GM 0 as a source). Restricting
//! every sum of the EIH Lagrangian to a symmetric graph keeps Eq. 27's form with restricted sums,
//! so the 1PN centre of mass (with `U` over `E`) is still conserved to O(c⁻⁴) (WP22 check E3). The
//! two set rules NUM-009 left open, no Minor sources for a Minor receiver and Ring masses as
//! sources, are WP22's reading (NEEDS HUMAN REVIEW).
//!
//! `a_j` on the right-hand side is the Newtonian point-mass acceleration of body `j` over `E(j)`
//! ([`RhsAcceleration::Newtonian`], NUM-018 default; DE430 p. 12) or that plus the default c⁻²
//! correction ([`RhsAcceleration::Iterated`], one fixed-point step).

#![expect(
    clippy::indexing_slicing,
    reason = "every body index is < n, the layout's body count: per-body vectors have length n, \
              the tier lists hold storage indices, and 3-vectors are indexed by 0..3"
)]

use ferroastro_core::frames::Frame;
use ferroastro_core::units::Velocity;
use ferroastro_forces::{
    Accelerations, ForceError, ForceTerm, Gauge, Layout, MinorReaction, TermParameter, View,
};
use ferroastro_real::Real;
use ferroastro_system::Tier;

/// The accelerations `a_j` used on the right-hand side of Eq. 27 (NUM-018).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RhsAcceleration {
    /// The Newtonian point-mass accelerations of the tiered model (default).
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

/// The c⁻² part of the EIH acceleration (Eq. 27) for every moving body, with PPN β, γ, summed over
/// the tiered model's interaction graph (module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eih<R: Real = f64> {
    c2: R,
    beta: R,
    gamma: R,
    rhs: RhsAcceleration,
    minor: MinorReaction,
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
    /// The term with the speed of light `c`, PPN `β`, `γ` and the right-hand-side mode. The Minor
    /// bodies act on the Majors ([`MinorReaction::On`], as `ferroastro_forces::MinorFromMajors`'
    /// default); see [`Eih::with_minor_reaction`].
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
            minor: MinorReaction::On,
        })
    }

    /// The same term with the Minor bodies' reaction setting, which must match the model's
    /// `MinorFromMajors` term: with [`MinorReaction::Off`] the Minor bodies are test particles
    /// (GM 0 as a 1PN source of anything).
    #[must_use]
    pub const fn with_minor_reaction(mut self, minor: MinorReaction) -> Self {
        self.minor = minor;
        self
    }

    /// The right-hand-side mode.
    #[must_use]
    pub const fn rhs(&self) -> RhsAcceleration {
        self.rhs
    }

    /// The Minor bodies' reaction setting.
    #[must_use]
    pub const fn minor_reaction(&self) -> MinorReaction {
        self.minor
    }

    /// The c⁻² corrections (km/s²) of every body, in storage order; Ring bodies get 0.
    ///
    /// # Errors
    /// `FA-E0501` if two bodies joined by the interaction graph coincide; `FA-E0500` for a view
    /// whose layout does not match its state.
    pub fn corrections(&self, t: R, view: &View<'_, R>) -> Result<Vec<[R; 3]>, ForceError> {
        let g = Graph::new(t, view, self.minor)?;
        let first = self.correction_with(&g, &g.newton);
        Ok(match self.rhs {
            RhsAcceleration::Newtonian => first,
            RhsAcceleration::Iterated => {
                let rhs: Vec<[R; 3]> = g
                    .newton
                    .iter()
                    .zip(&first)
                    .map(|(n, d)| [n[0] + d[0], n[1] + d[1], n[2] + d[2]])
                    .collect();
                self.correction_with(&g, &rhs)
            }
        })
    }

    /// Eq. 27 minus its Newtonian part over the graph's edges, with the right-hand-side
    /// accelerations `acc` (0 for Ring bodies).
    fn correction_with(&self, g: &Graph<R>, acc: &[[R; 3]]) -> Vec<[R; 3]> {
        let (beta, gamma) = (self.beta, self.gamma);
        let inv_c2 = R::ONE / self.c2;
        let one = R::ONE;
        // Constant coefficients of B_ij and S2/S3.
        let k = Coefficients {
            gamma,
            three_halves: R::from_f64(1.5),
            k_ui: -(R::TWO * (beta + gamma)),
            k_uj: -(R::TWO * beta - one),
            k_vj2: one + gamma,
            k_vivj: -(R::TWO * (one + gamma)),
            k_s2i: R::TWO + R::TWO * gamma,
            k_s2j: one + R::TWO * gamma,
            k_s3: (R::from_f64(3.0) + R::from_f64(4.0) * gamma) * R::HALF,
        };
        let mut out = vec![[R::ZERO; 3]; g.len()];
        g.for_each_edge(|i, j| {
            for (rec, src) in [(i, j), (j, i)] {
                if g.moves[rec] && g.gm[src] != R::ZERO {
                    let s = term(&k, g, rec, src, acc[src]);
                    for c in 0..3 {
                        out[rec][c] += s[c];
                    }
                }
            }
        });
        for o in &mut out {
            *o = o.map(|x| x * inv_c2);
        }
        out
    }
}

/// The constant coefficients of Eq. 27's c⁻² part.
#[derive(Clone, Copy)]
struct Coefficients<R: Real> {
    gamma: R,
    three_halves: R,
    k_ui: R,
    k_uj: R,
    k_vj2: R,
    k_vivj: R,
    k_s2i: R,
    k_s2j: R,
    k_s3: R,
}

/// The c⁻² contribution (times c²) of source `j` to receiver `i`: S1 + S2 + S3 of the module docs.
fn term<R: Real>(k: &Coefficients<R>, p: &Graph<R>, i: usize, j: usize, aj: [R; 3]) -> [R; 3] {
    let Coefficients {
        gamma,
        three_halves,
        k_ui,
        k_uj,
        k_vj2,
        k_vivj,
        k_s2i,
        k_s2j,
        k_s3,
    } = *k;
    let (ri, vi) = (p.pos[i], p.vel[i]);
    let vi2 = dot(vi, vi);
    let gj = p.gm[j];
    let (rj, vj) = (p.pos[j], p.vel[j]);
    let d_ji = sub(rj, ri); // r_j − r_i
    let d_ij = sub(ri, rj); // r_i − r_j
    let rij = dot(d_ij, d_ij).sqrt();
    let inv_r3 = R::ONE / (rij * rij * rij);
    let proj = dot(d_ij, vj) / rij;
    let b =
        k_ui * p.u[i] + k_uj * p.u[j] + gamma * vi2 + k_vj2 * dot(vj, vj) + k_vivj * dot(vi, vj)
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
    let mut s = [R::ZERO; 3];
    for c in 0..3 {
        s[c] = s1 * d_ji[c] + s2 * (vi[c] - vj[c]) + s3 * aj[c];
    }
    s
}

/// Every body's position, velocity, source GM, potential `U` over its graph neighbours and
/// Newtonian point-mass acceleration over them (0 for Ring bodies), plus the tier lists that
/// define the edges.
struct Graph<R: Real> {
    pos: Vec<[R; 3]>,
    vel: Vec<[R; 3]>,
    /// GM as a source: the body's GM, or 0 for a Minor body under [`MinorReaction::Off`].
    gm: Vec<R>,
    /// True for Major and Minor bodies (the receivers).
    moves: Vec<bool>,
    /// `U_i = Σ_{k ∈ E(i)} GM_k / r_ik`.
    u: Vec<R>,
    /// `Σ_{k ∈ E(i)} GM_k (r_k − r_i) / r_ik³` for moving bodies, 0 for Ring bodies.
    newton: Vec<[R; 3]>,
    major: Vec<usize>,
    minor: Vec<usize>,
    ring: Vec<usize>,
}

impl<R: Real> Graph<R> {
    fn new(t: R, view: &View<'_, R>, reaction: MinorReaction) -> Result<Self, ForceError> {
        let layout: &Layout<R> = view.layout();
        let n = layout.len();
        let pos = (0..n)
            .map(|i| view.position(i))
            .collect::<Result<Vec<[R; 3]>, _>>()?;
        let vel = (0..n)
            .map(|i| view.velocity(i))
            .collect::<Result<Vec<[R; 3]>, _>>()?;
        let gm: Vec<R> = (0..n)
            .map(|i| match layout.tiers()[i] {
                Tier::Minor if reaction == MinorReaction::Off => R::ZERO,
                _ => layout.gm(i),
            })
            .collect();
        let moves: Vec<bool> = (0..n).map(|i| layout.moves(i)).collect();
        let mut g = Self {
            pos,
            vel,
            gm,
            moves,
            u: vec![R::ZERO; n],
            newton: vec![[R::ZERO; 3]; n],
            major: layout.indices(Tier::Major).to_vec(),
            minor: layout.indices(Tier::Minor).to_vec(),
            ring: layout.indices(Tier::Ring).to_vec(),
        };
        let mut singular = None;
        let (mut u, mut newton) = (vec![R::ZERO; n], vec![[R::ZERO; 3]; n]);
        g.for_each_edge(|i, j| {
            let d = sub(g.pos[j], g.pos[i]); // r_j − r_i
            let r2 = dot(d, d);
            if r2 == R::ZERO {
                singular.get_or_insert((i, j));
                return;
            }
            let r = r2.sqrt();
            u[i] += g.gm[j] / r;
            u[j] += g.gm[i] / r;
            let inv_r3 = R::ONE / (r2 * r);
            if g.moves[i] {
                let s = g.gm[j] * inv_r3;
                for c in 0..3 {
                    newton[i][c] += s * d[c];
                }
            }
            if g.moves[j] {
                let s = g.gm[i] * inv_r3;
                for c in 0..3 {
                    newton[j][c] -= s * d[c];
                }
            }
        });
        if let Some((i, j)) = singular {
            return Err(ForceError::Singular {
                i,
                j,
                t: t.to_f64(),
            });
        }
        g.u = u;
        g.newton = newton;
        Ok(g)
    }

    fn len(&self) -> usize {
        self.pos.len()
    }

    /// Visit every edge of the interaction graph once, in a fixed order: Major–Major (`i < j` in
    /// storage order), Major–Minor, Major–Ring, Minor–Ring.
    fn for_each_edge(&self, mut f: impl FnMut(usize, usize)) {
        for (a, &i) in self.major.iter().enumerate() {
            for &j in &self.major[a + 1..] {
                f(i, j);
            }
            for &j in self.minor.iter().chain(&self.ring) {
                f(i, j);
            }
        }
        for &i in &self.minor {
            for &j in &self.ring {
                f(i, j);
            }
        }
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
            TermParameter {
                name: "1PN sums over the tiered interaction graph, all moving bodies (WP22 D1)",
                value: 1.0,
                unit: "1",
            },
            TermParameter {
                name: "Minor bodies as 1PN sources (reaction on Majors)",
                value: match self.minor {
                    MinorReaction::On => 1.0,
                    MinorReaction::Off => 0.0,
                },
                unit: "1",
            },
        ]
    }
    fn tiers(&self) -> &'static [Tier] {
        &[Tier::Major, Tier::Minor]
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
        for (i, a) in self.corrections(t, view)?.into_iter().enumerate() {
            if layout.moves(i) {
                acc.add(i, a);
            }
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
            "EIH 1PN (Park et al. 2021 Eq. 27), all moving bodies over the tiered graph, beta = {}, \
             gamma = {}, {}",
            self.beta.to_f64(),
            self.gamma.to_f64(),
            self.rhs.label()
        )
    }
}
