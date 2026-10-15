// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! The "Park2017" precession estimator (NUM-011, ADR-0011): perihelion passages of a **sampled**
//! heliocentric trajectory (e.g. a JPL ephemeris), the perihelion direction projected onto a
//! fixed reference plane (by default Mercury's mean-orbit frame), and a least-squares fit of
//! `B + ϖ̇t + Qt² + Σ(Sᵢ sin νᵢt + Cᵢ cos νᵢt)` with formal and jack-knife errors.
//!
//! The procedure follows Park et al. 2017 (AJ 153, 121), §4:
//!
//! 1. **Frame.** z = the mean-orbit pole, by default α = 280.9876°, δ = 61.4481° (ICRF);
//!    y = z × `x̂_ICRF` (normalised); x = y × z ("the y-axis is determined by the cross product of the
//!    z-axis with ICRF x-axis; the orbit frame x-axis completes the triad").
//! 2. **Passages.** "Compute the times of Mercury's perihelion passages": here, the roots of `r·v`
//!    with `r·v` rising, located by bisection on a Lagrange interpolant of the sampled `r·v`
//!    ([`PassageConfig::order`] points). Position and velocity at the root come from the same
//!    interpolant. A second interpolant two points shorter gives a per-passage error estimate.
//! 3. **Longitude.** "Compute the position vector and project it onto the frame": the angle of
//!    the projected passage position from the frame's x-axis, unwrapped
//!    ([`LongitudeDefinition::Projected`]). Park measures it "with respect to the initial position
//!    vector", which only shifts `B`. [`LongitudeDefinition::DogLeg`] gives instead the classical
//!    `ϖ = Ω + ω` of the osculating orbit at the passage (node on the reference plane). That is
//!    the quantity tabulated by Standish & Williams (SW92), and it is **not** the same angle.
//! 4. **Fit.** `t` in Julian centuries from J2000.0 TDB (Park: "with the epoch of 2000 January
//!    1"), angles in arcseconds, Householder QR ([`crate::lsq`]). Frequencies are supplied (e.g.
//!    Park17 Table 2's 14 combinations of mean motions) or found by [`fft_frequencies`].
//! 5. **Errors.** The formal σ of ϖ̇ comes from the residual variance. The jack-knife σ is a
//!    delete-one-block jack-knife over `G` contiguous blocks of passages, because the residuals are
//!    serially correlated and the formal σ understates the uncertainty.
//!
//! Sampled once per orbit, a term `k·n_M + j·n_P` is aliased to `≈ j·n_P + k·ϖ̇`: at every passage
//! Mercury's mean longitude equals its perihelion longitude. Two Table-2 terms that differ only in
//! `k` are therefore nearly collinear (e.g. `1n_M − 2n_J` and `2n_J`). The fit stays full rank,
//! but their individual amplitudes are poorly separated. The secular rate, which is orthogonal to
//! both, is not affected.

use crate::PrecessionError;
use crate::analytic::rate_arcsec_per_century;
use crate::kepler::{cross, dot, norm};
use crate::lsq::{LsqSolution, lstsq};
use crate::report::{RateReport, RateReportBuilder};
use crate::spectral::fft_in_place;
use ferroastro_core::frames::{Center, Frame, Heliocentric, Icrf, State};
use ferroastro_core::time::{DAYS_PER_JULIAN_CENTURY, Epoch, J2000_JD, SECONDS_PER_DAY, Tdb};
use ferroastro_core::units::{Angle, AngularRate, TimeSpan};
use std::f64::consts::{PI, TAU};
use std::fmt;
use std::marker::PhantomData;

/// Right ascension of Mercury's mean-orbit pole, degrees, ICRF (Park et al. 2017 §4: the mean
/// orbital angular momentum over 1000 January 1 – 3000 January 1).
pub const PARK2017_POLE_RA_DEG: f64 = 280.9876;
/// Declination of Mercury's mean-orbit pole, degrees, ICRF (Park et al. 2017 §4).
pub const PARK2017_POLE_DEC_DEG: f64 = 61.4481;

/// Definition string of this estimator, for reports (NUM-012).
pub const ESTIMATOR: &str = "Park2017 (NUM-011): heliocentric perihelion passages (r.v = 0 rising, \
     Lagrange-interpolated samples), perihelion direction referred to a fixed reference plane";

const SECONDS_PER_CENTURY: f64 = DAYS_PER_JULIAN_CENTURY * SECONDS_PER_DAY;

fn input(message: impl Into<String>) -> PrecessionError {
    PrecessionError::EstimatorInput(message.into())
}

fn arcsec_per_rad() -> f64 {
    180.0 * 3600.0 / PI
}

/// A heliocentric trajectory sampled at increasing TDB epochs, with axes `F`.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledOrbit<F: Frame> {
    /// TDB seconds from J2000.0.
    t_s: Vec<f64>,
    r: Vec<[f64; 3]>,
    v: Vec<[f64; 3]>,
    _frame: PhantomData<F>,
}

impl<F: Frame> SampledOrbit<F> {
    /// Build from raw arrays: Julian dates (TDB), heliocentric positions (km) and velocities
    /// (km/s) in frame `F`.
    ///
    /// # Errors
    /// `FA-E0603` for unequal lengths, fewer than 16 samples, non-finite values or epochs that are
    /// not strictly increasing.
    pub fn from_raw(
        jd_tdb: &[f64],
        r_km: &[[f64; 3]],
        v_km_s: &[[f64; 3]],
    ) -> Result<Self, PrecessionError> {
        let t_s: Vec<f64> = jd_tdb
            .iter()
            .map(|jd| (jd - J2000_JD) * SECONDS_PER_DAY)
            .collect();
        Self::checked(t_s, r_km.to_vec(), v_km_s.to_vec())
    }

    /// Build from typed epochs and heliocentric states.
    ///
    /// # Errors
    /// As [`SampledOrbit::from_raw`].
    pub fn from_states(
        epochs: &[Epoch<Tdb>],
        states: &[State<F, Heliocentric>],
    ) -> Result<Self, PrecessionError> {
        let t_s = epochs
            .iter()
            .map(|e| {
                let (d, f) = e.jd_parts();
                ((d - J2000_JD) + f) * SECONDS_PER_DAY
            })
            .collect();
        let r = states
            .iter()
            .map(|s| {
                s.position()
                    .components()
                    .map(ferroastro_core::units::Length::km)
            })
            .collect();
        let v = states
            .iter()
            .map(|s| s.velocity().map(ferroastro_core::units::Velocity::km_per_s))
            .collect();
        Self::checked(t_s, r, v)
    }

    fn checked(t_s: Vec<f64>, r: Vec<[f64; 3]>, v: Vec<[f64; 3]>) -> Result<Self, PrecessionError> {
        if t_s.len() != r.len() || t_s.len() != v.len() {
            return Err(input(format!(
                "{} epochs, {} positions, {} velocities",
                t_s.len(),
                r.len(),
                v.len()
            )));
        }
        if t_s.len() < 16 {
            return Err(input(format!("only {} samples (need >= 16)", t_s.len())));
        }
        let finite = t_s.iter().all(|x| x.is_finite())
            && r.iter().chain(&v).flatten().all(|x| x.is_finite());
        if !finite {
            return Err(input("non-finite epoch or state"));
        }
        if t_s.windows(2).any(|w| matches!(w, [a, b] if b <= a)) {
            return Err(input("epochs are not strictly increasing"));
        }
        Ok(Self {
            t_s,
            r,
            v,
            _frame: PhantomData,
        })
    }

    /// Number of samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.t_s.len()
    }

    /// True if there are no samples (never, for a constructed orbit).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.t_s.is_empty()
    }

    /// Every `k`-th sample (`k ≥ 1`), for sampling-sensitivity checks.
    ///
    /// # Errors
    /// `FA-E0603` for `k = 0` or too few remaining samples.
    pub fn decimate(&self, k: usize) -> Result<Self, PrecessionError> {
        if k == 0 {
            return Err(input("decimation factor 0"));
        }
        let pick = |n: usize| n % k == 0;
        let t = self.t_s.iter().enumerate().filter(|(i, _)| pick(*i));
        let r = self.r.iter().enumerate().filter(|(i, _)| pick(*i));
        let v = self.v.iter().enumerate().filter(|(i, _)| pick(*i));
        Self::checked(
            t.map(|(_, x)| *x).collect(),
            r.map(|(_, x)| *x).collect(),
            v.map(|(_, x)| *x).collect(),
        )
    }

    /// The samples with `jd_lo ≤ JD(TDB) ≤ jd_hi`.
    ///
    /// # Errors
    /// `FA-E0603` if fewer than 16 samples remain.
    pub fn window(&self, jd_lo: f64, jd_hi: f64) -> Result<Self, PrecessionError> {
        let (lo, hi) = (
            (jd_lo - J2000_JD) * SECONDS_PER_DAY,
            (jd_hi - J2000_JD) * SECONDS_PER_DAY,
        );
        let keep: Vec<usize> = (0..self.len())
            .filter(|&i| self.t_s.get(i).is_some_and(|t| (lo..=hi).contains(t)))
            .collect();
        let get = |v: &[[f64; 3]]| keep.iter().filter_map(|&i| v.get(i).copied()).collect();
        Self::checked(
            keep.iter()
                .filter_map(|&i| self.t_s.get(i).copied())
                .collect(),
            get(&self.r),
            get(&self.v),
        )
    }
}

/// A fixed plane with in-plane axes `x`, `y` and normal `z`, expressed in frame `F`.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePlane<F: Frame> {
    x: [f64; 3],
    y: [f64; 3],
    z: [f64; 3],
    label: String,
    _frame: PhantomData<F>,
}

impl ReferencePlane<Icrf> {
    /// The plane normal to the pole `(ra, dec)` in ICRF, with Park et al. 2017's axes:
    /// y = z × `x̂_ICRF` (normalised), x = y × z.
    ///
    /// # Errors
    /// `FA-E0603` if the pole lies along the ICRF x-axis (y undefined).
    pub fn from_pole(ra: Angle, dec: Angle, label: &str) -> Result<Self, PrecessionError> {
        let (sa, ca) = ra.radians().sin_cos();
        let (sd, cd) = dec.radians().sin_cos();
        let z = [cd * ca, cd * sa, sd];
        let y_raw = cross(z, [1.0, 0.0, 0.0]);
        let ny = norm(y_raw);
        if ny.is_nan() || ny < 1e-12 {
            return Err(input("pole along the ICRF x-axis: in-plane axes undefined"));
        }
        let y = y_raw.map(|c| c / ny);
        let x = cross(y, z);
        Ok(Self {
            x,
            y,
            z,
            label: format!(
                "{label}: plane normal to pole RA {:.4} deg, Dec {:.4} deg (ICRF); y = z x X_ICRF, x = y x z",
                ra.radians().to_degrees(),
                dec.radians().to_degrees()
            ),
            _frame: PhantomData,
        })
    }

    /// Mercury's mean-orbit frame of Park et al. 2017 (pole [`PARK2017_POLE_RA_DEG`],
    /// [`PARK2017_POLE_DEC_DEG`]).
    ///
    /// # Errors
    /// Never for the built-in pole; the `Result` mirrors [`ReferencePlane::from_pole`].
    pub fn park2017_mean_orbit() -> Result<Self, PrecessionError> {
        let deg = |d: f64| Angle::from_radians(d.to_radians()).map_err(|e| input(e.to_string()));
        Self::from_pole(
            deg(PARK2017_POLE_RA_DEG)?,
            deg(PARK2017_POLE_DEC_DEG)?,
            "Mercury mean-orbit frame (Park et al. 2017)",
        )
    }
}

impl<F: Frame> ReferencePlane<F> {
    /// The xy-plane of frame `F` itself (x-axis = the frame's x-axis), e.g. the J2000 ecliptic
    /// and equinox for [`ferroastro_core::frames::EclipticJ2000`].
    #[must_use]
    pub fn xy_plane() -> Self {
        Self {
            x: [1.0, 0.0, 0.0],
            y: [0.0, 1.0, 0.0],
            z: [0.0, 0.0, 1.0],
            label: format!(
                "{} xy-plane, longitudes from the {} x-axis",
                F::NAME,
                F::NAME
            ),
            _frame: PhantomData,
        }
    }

    /// Description for reports.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The unit normal (pole) in frame `F`.
    #[must_use]
    pub const fn pole(&self) -> [f64; 3] {
        self.z
    }

    /// The in-plane axes `(x, y)` in frame `F`.
    #[must_use]
    pub const fn axes(&self) -> ([f64; 3], [f64; 3]) {
        (self.x, self.y)
    }

    /// Angle (rad, `(−π, π]`) of the projection of `r` on the plane, from the x-axis.
    #[must_use]
    pub fn projected_angle(&self, r: [f64; 3]) -> f64 {
        dot(r, self.y).atan2(dot(r, self.x))
    }

    /// `Ω + ω` (rad, not reduced) of the osculating orbit through `(r, v)`: `Ω` is the longitude
    /// of the ascending node on this plane, `ω` the angle from the node to `r` in the orbit plane.
    ///
    /// # Errors
    /// `FA-E0600` if the orbit lies in the plane (node undefined) or `r × v = 0`.
    pub fn dog_leg_angle(&self, r: [f64; 3], v: [f64; 3]) -> Result<f64, PrecessionError> {
        let h = cross(r, v);
        let hn = norm(h);
        let node = cross(self.z, h);
        let nn = norm(node);
        if hn.is_nan() || nn.is_nan() || hn <= 0.0 || nn <= 1e-12 * hn {
            return Err(PrecessionError::Domain {
                what: "inclination to the reference plane (node undefined)",
                value: nn / hn,
            });
        }
        let big_omega = dot(node, self.y).atan2(dot(node, self.x));
        let n_hat = node.map(|c| c / nn);
        let h_hat = h.map(|c| c / hn);
        let small_omega = dot(cross(n_hat, r), h_hat).atan2(dot(n_hat, r));
        Ok(big_omega + small_omega)
    }
}

/// How the perihelion longitude is formed from a passage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongitudeDefinition {
    /// Angle of the passage position projected on the reference plane (Park et al. 2017).
    Projected,
    /// `Ω + ω` of the osculating orbit at the passage, node on the reference plane (SW92's
    /// "longitude of perihelion").
    DogLeg,
}

impl fmt::Display for LongitudeDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LongitudeDefinition::Projected => {
                "projected: angle of the passage position projected on the reference plane"
            }
            LongitudeDefinition::DogLeg => {
                "dog-leg: Omega + omega of the osculating orbit at the passage (node on the reference plane)"
            }
        })
    }
}

/// Settings for locating passages in sampled data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassageConfig {
    /// Points in the Lagrange interpolant (even, ≥ 4).
    pub order: usize,
    /// Minimum samples per orbit; a coarser table is rejected (`FA-E0603`).
    pub min_samples_per_orbit: usize,
}

impl Default for PassageConfig {
    fn default() -> Self {
        Self {
            order: 8,
            min_samples_per_orbit: 20,
        }
    }
}

/// One perihelion passage located in sampled data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampledPassage {
    /// TDB seconds from J2000.0.
    pub t_s: f64,
    /// Heliocentric position (km).
    pub r: [f64; 3],
    /// Heliocentric velocity (km/s).
    pub v: [f64; 3],
    /// Angle (rad) between the passage directions from the full and the shorter interpolant: an
    /// estimate of the interpolation error.
    pub direction_error_rad: f64,
}

/// Lagrange interpolation of `ys` (at `ts`) at `t`, with time scaled by `h` around `tc`.
fn lagrange<const N: usize>(ts: &[f64], ys: &[[f64; N]], t: f64) -> [f64; N] {
    let tc = ts.first().copied().unwrap_or(0.0);
    let h = match (ts.first(), ts.last()) {
        (Some(a), Some(b)) if b > a => b - a,
        _ => 1.0,
    };
    let u: Vec<f64> = ts.iter().map(|x| (x - tc) / h).collect();
    let ut = (t - tc) / h;
    let mut out = [0.0; N];
    for (j, (uj, yj)) in u.iter().zip(ys).enumerate() {
        let w: f64 = u
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != j)
            .map(|(_, uk)| (ut - uk) / (uj - uk))
            .product();
        for (o, y) in out.iter_mut().zip(yj) {
            *o = w.mul_add(*y, *o);
        }
    }
    out
}

/// Root of the interpolated `r·v` in `[a, b]` (where it rises from < 0 to ≥ 0), then `r`, `v`.
fn passage_in(
    ts: &[f64],
    rv: &[[f64; 1]],
    r: &[[f64; 3]],
    v: &[[f64; 3]],
    a: f64,
    b: f64,
) -> (f64, [f64; 3], [f64; 3]) {
    let (mut lo, mut hi) = (a, b);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        let [f] = lagrange(ts, rv, mid);
        if f < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = 0.5 * (lo + hi);
    (t, lagrange(ts, r, t), lagrange(ts, v, t))
}

/// Locate every perihelion passage (`r·v` rising through 0) that has a full interpolation window.
///
/// # Errors
/// `FA-E0603` for an odd or too small `order`, fewer than two passages, or fewer than
/// [`PassageConfig::min_samples_per_orbit`] samples between consecutive passages.
pub fn find_passages<F: Frame>(
    orbit: &SampledOrbit<F>,
    cfg: &PassageConfig,
) -> Result<Vec<SampledPassage>, PrecessionError> {
    if cfg.order < 4 || cfg.order % 2 != 0 {
        return Err(input(format!(
            "interpolation order {} (need even >= 4)",
            cfg.order
        )));
    }
    let h = cfg.order / 2;
    let rv: Vec<[f64; 1]> = orbit
        .r
        .iter()
        .zip(&orbit.v)
        .map(|(r, v)| [dot(*r, *v)])
        .collect();
    let n = orbit.len();
    let mut out = Vec::new();
    let mut last_index: Option<usize> = None;
    for i in h..n.saturating_sub(h) {
        let (Some([f0]), Some([f1])) = (rv.get(i - 1), rv.get(i)) else {
            continue;
        };
        if !(*f0 < 0.0 && *f1 >= 0.0) {
            continue;
        }
        if let Some(prev) = last_index {
            if i - prev < cfg.min_samples_per_orbit {
                return Err(input(format!(
                    "only {} samples between perihelion passages (need >= {}): sampling too coarse",
                    i - prev,
                    cfg.min_samples_per_orbit
                )));
            }
        }
        last_index = Some(i);
        let window = |k: usize| (i - k)..(i + k);
        let (Some(ts), Some(rvw), Some(rw), Some(vw)) = (
            orbit.t_s.get(window(h)),
            rv.get(window(h)),
            orbit.r.get(window(h)),
            orbit.v.get(window(h)),
        ) else {
            continue;
        };
        let (Some(&a), Some(&b)) = (orbit.t_s.get(i - 1), orbit.t_s.get(i)) else {
            continue;
        };
        let (t, r, v) = passage_in(ts, rvw, rw, vw, a, b);
        let k = h - 1;
        let err = match (
            orbit.t_s.get(window(k)),
            rv.get(window(k)),
            orbit.r.get(window(k)),
            orbit.v.get(window(k)),
        ) {
            (Some(ts2), Some(rv2), Some(r2), Some(v2)) => {
                let (_, r_short, _) = passage_in(ts2, rv2, r2, v2, a, b);
                norm(cross(r, r_short)) / (norm(r) * norm(r_short))
            }
            _ => f64::NAN,
        };
        out.push(SampledPassage {
            t_s: t,
            r,
            v,
            direction_error_rad: err,
        });
    }
    if out.len() < 2 {
        return Err(input(format!(
            "found {} perihelion passages (need >= 2)",
            out.len()
        )));
    }
    Ok(out)
}

/// Unwrapped perihelion longitudes (rad) of `passages` on `plane`.
///
/// # Errors
/// `FA-E0600` from [`ReferencePlane::dog_leg_angle`].
pub fn perihelion_longitudes<F: Frame>(
    passages: &[SampledPassage],
    plane: &ReferencePlane<F>,
    definition: LongitudeDefinition,
) -> Result<Vec<f64>, PrecessionError> {
    let raw: Vec<f64> = passages
        .iter()
        .map(|p| match definition {
            LongitudeDefinition::Projected => Ok(plane.projected_angle(p.r)),
            LongitudeDefinition::DogLeg => plane.dog_leg_angle(p.r, p.v),
        })
        .collect::<Result<_, _>>()?;
    let mut out = Vec::with_capacity(raw.len());
    let mut offset = 0.0;
    let mut prev: Option<f64> = None;
    for a in raw {
        if let Some(p) = prev {
            let d = a + offset - p;
            offset -= TAU * (d / TAU).round();
        }
        let x = a + offset;
        out.push(x);
        prev = Some(x);
    }
    Ok(out)
}

/// A periodic term's angular frequency.
#[derive(Debug, Clone, PartialEq)]
pub struct Frequency {
    /// Angular frequency, rad per Julian century.
    pub rad_per_century: f64,
    /// Label, e.g. `"1nM - 2nV"`.
    pub label: String,
}

/// The fit model `B + ϖ̇t (+ Qt²) + Σ(Sᵢ sin νᵢt + Cᵢ cos νᵢt)`.
#[derive(Debug, Clone, PartialEq)]
pub struct FitModel {
    /// Include `Qt²`.
    pub quadratic: bool,
    /// The `νᵢ`.
    pub frequencies: Vec<Frequency>,
}

impl FitModel {
    /// Description for reports (PHY-015).
    #[must_use]
    pub fn describe(&self) -> String {
        let poly = if self.quadratic {
            "B + varpi_dot*t + Q*t^2"
        } else {
            "B + varpi_dot*t"
        };
        let labels: Vec<&str> = self.frequencies.iter().map(|f| f.label.as_str()).collect();
        format!(
            "{poly} + sum_i (S_i sin nu_i t + C_i cos nu_i t), {} frequencies [{}]; t in Julian \
             centuries from J2000.0 TDB; unweighted least squares (Householder QR)",
            self.frequencies.len(),
            labels.join(", ")
        )
    }
}

/// A fitted periodic term.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodicTerm {
    /// The frequency.
    pub frequency: Frequency,
    /// Sine amplitude `Sᵢ` (arcsec).
    pub sin_arcsec: f64,
    /// Cosine amplitude `Cᵢ` (arcsec).
    pub cos_arcsec: f64,
}

/// Result of [`fit_longitudes`].
#[derive(Debug, Clone, PartialEq)]
pub struct Park2017Fit {
    /// ϖ̇ at J2000.0.
    pub rate: AngularRate,
    /// Formal one-sigma error of ϖ̇ (understates the true error for correlated residuals).
    pub formal_sigma: AngularRate,
    /// Delete-one-block jack-knife one-sigma error of ϖ̇.
    pub jackknife_sigma: AngularRate,
    /// Jack-knife blocks used.
    pub jackknife_blocks: usize,
    /// `B`, the fitted longitude at J2000.0.
    pub b: Angle,
    /// `Q` in arcsec per Julian century² (0 if not fitted).
    pub q_arcsec_per_cy2: f64,
    /// The periodic terms.
    pub terms: Vec<PeriodicTerm>,
    /// RMS of the post-fit residuals.
    pub rms_residual: Angle,
    /// Number of passages fitted.
    pub samples: usize,
    /// First and last passage, TDB seconds from J2000.0.
    pub span_s: (f64, f64),
    /// The fit model, as described by [`FitModel::describe`].
    pub fit_model: String,
}

impl Park2017Fit {
    /// ϖ̇ in ″/cy.
    #[must_use]
    pub fn arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.rate)
    }

    /// Formal σ in ″/cy.
    #[must_use]
    pub fn formal_sigma_arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.formal_sigma)
    }

    /// Jack-knife σ in ″/cy.
    #[must_use]
    pub fn jackknife_sigma_arcsec_per_century(&self) -> f64 {
        rate_arcsec_per_century(self.jackknife_sigma)
    }
}

fn design(t_cy: &[f64], model: &FitModel) -> Vec<Vec<f64>> {
    let mut cols = vec![vec![1.0; t_cy.len()], t_cy.to_vec()];
    if model.quadratic {
        cols.push(t_cy.iter().map(|t| t * t).collect());
    }
    for f in &model.frequencies {
        cols.push(t_cy.iter().map(|t| (f.rad_per_century * t).sin()).collect());
        cols.push(t_cy.iter().map(|t| (f.rad_per_century * t).cos()).collect());
    }
    cols
}

fn solve(t_cy: &[f64], y: &[f64], model: &FitModel) -> Result<LsqSolution, PrecessionError> {
    lstsq(&design(t_cy, model), y)
}

fn rate_from_arcsec_cy(x: f64) -> Result<AngularRate, PrecessionError> {
    AngularRate::from_rad_per_s(x / arcsec_per_rad() / SECONDS_PER_CENTURY)
        .map_err(|e| PrecessionError::Fit(e.to_string()))
}

/// Fit the model to longitudes (rad) at times (TDB s from J2000.0), with a `blocks`-block
/// jack-knife.
///
/// # Errors
/// `FA-E0601` for mismatched lengths, `blocks < 2`, a block too small to leave the fit
/// over-determined, or a degenerate design.
pub fn fit_longitudes(
    t_s: &[f64],
    longitudes_rad: &[f64],
    model: &FitModel,
    blocks: usize,
) -> Result<Park2017Fit, PrecessionError> {
    let n = t_s.len();
    if n != longitudes_rad.len() {
        return Err(PrecessionError::Fit(format!(
            "{n} times but {} longitudes",
            longitudes_rad.len()
        )));
    }
    let p = 2 + usize::from(model.quadratic) + 2 * model.frequencies.len();
    if blocks < 2 || n / blocks < 1 || n - n.div_ceil(blocks) <= p {
        return Err(PrecessionError::Fit(format!(
            "jack-knife with {blocks} blocks over {n} samples and {p} parameters is ill-posed"
        )));
    }
    let t_cy: Vec<f64> = t_s.iter().map(|t| t / SECONDS_PER_CENTURY).collect();
    let y: Vec<f64> = longitudes_rad
        .iter()
        .map(|a| a * arcsec_per_rad())
        .collect();
    let full = solve(&t_cy, &y, model)?;
    let coef = |i: usize| full.coef.get(i).copied().unwrap_or(f64::NAN);
    // Jack-knife: delete one contiguous block at a time.
    let mut est = Vec::with_capacity(blocks);
    for g in 0..blocks {
        let (lo, hi) = (g * n / blocks, (g + 1) * n / blocks);
        let keep = |v: &[f64]| -> Vec<f64> {
            v.iter()
                .enumerate()
                .filter(|(i, _)| *i < lo || *i >= hi)
                .map(|(_, x)| *x)
                .collect()
        };
        let s = solve(&keep(&t_cy), &keep(&y), model)?;
        est.push(s.coef.get(1).copied().unwrap_or(f64::NAN));
    }
    #[expect(clippy::cast_precision_loss, reason = "block counts are small")]
    let g = blocks as f64;
    let mean = est.iter().sum::<f64>() / g;
    let jk = ((g - 1.0) / g * est.iter().map(|e| (e - mean).powi(2)).sum::<f64>()).sqrt();
    let first_periodic = 2 + usize::from(model.quadratic);
    let terms = model
        .frequencies
        .iter()
        .enumerate()
        .map(|(i, f)| PeriodicTerm {
            frequency: f.clone(),
            sin_arcsec: coef(first_periodic + 2 * i),
            cos_arcsec: coef(first_periodic + 2 * i + 1),
        })
        .collect();
    #[expect(
        clippy::cast_precision_loss,
        reason = "sample counts are far below 2^53"
    )]
    let rms = (full.rss / n as f64).sqrt();
    Ok(Park2017Fit {
        rate: rate_from_arcsec_cy(coef(1))?,
        formal_sigma: rate_from_arcsec_cy(full.sigma(1))?,
        jackknife_sigma: rate_from_arcsec_cy(jk)?,
        jackknife_blocks: blocks,
        b: Angle::from_arcsec(coef(0)).map_err(|e| PrecessionError::Fit(e.to_string()))?,
        q_arcsec_per_cy2: if model.quadratic { coef(2) } else { 0.0 },
        terms,
        rms_residual: Angle::from_arcsec(rms).map_err(|e| PrecessionError::Fit(e.to_string()))?,
        samples: n,
        span_s: (
            t_s.first().copied().unwrap_or(f64::NAN),
            t_s.last().copied().unwrap_or(f64::NAN),
        ),
        fit_model: model.describe(),
    })
}

/// Residual sum of squares of `[1, t, t², sin ωt, cos ωt]` fitted to `y`.
fn sinusoid_rss(t: &[f64], y: &[f64], omega: f64) -> Result<f64, PrecessionError> {
    let cols = vec![
        vec![1.0; t.len()],
        t.to_vec(),
        t.iter().map(|x| x * x).collect(),
        t.iter().map(|x| (omega * x).sin()).collect(),
        t.iter().map(|x| (omega * x).cos()).collect(),
    ];
    Ok(lstsq(&cols, y)?.rss)
}

/// Minimise [`sinusoid_rss`] over `[lo, hi]` by golden-section search. Unlike the peak of the DFT
/// power, the least-squares optimum is not pulled by the sinusoid's negative-frequency image or by
/// the polynomial, which matters when a term has only a few cycles over the span.
fn refine_sinusoid(t: &[f64], y: &[f64], lo: f64, hi: f64) -> Result<f64, PrecessionError> {
    let g = (5.0_f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = (lo.max(0.0), hi);
    let mut x1 = b - g * (b - a);
    let mut x2 = a + g * (b - a);
    let (mut f1, mut f2) = (sinusoid_rss(t, y, x1)?, sinusoid_rss(t, y, x2)?);
    for _ in 0..60 {
        if f1 > f2 {
            a = x1;
            x1 = x2;
            f1 = f2;
            x2 = a + g * (b - a);
            f2 = sinusoid_rss(t, y, x2)?;
        } else {
            b = x2;
            x2 = x1;
            f2 = f1;
            x1 = b - g * (b - a);
            f1 = sinusoid_rss(t, y, x1)?;
        }
    }
    Ok(f64::midpoint(a, b))
}

/// Settings of [`fft_frequencies`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FftConfig {
    /// Stop when the next term's fitted amplitude `√(S² + C²)` is below this (Park et al. 2017
    /// kept the terms that "contribute 0.5″").
    pub min_amplitude: Angle,
    /// At most this many terms.
    pub max_terms: usize,
    /// Ignore frequencies with fewer cycles than this over the span (they are degenerate with
    /// the polynomial).
    pub min_cycles: f64,
    /// Zero-padding factor of the FFT (power of two).
    pub oversample: usize,
    /// Include `Qt²` in the polynomial part while searching.
    pub quadratic: bool,
}

/// Find periodic terms by iterative prewhitening.
///
/// The residuals of the current model are treated as uniformly spaced in passage index and
/// zero-padded FFT'd. The strongest peak above [`FftConfig::min_cycles`] is refined by minimising
/// the residual sum of squares of a single sinusoid plus quadratic at the true (non-uniform) times
/// (golden section within ±1 bin) and added to the model. The model is
/// refitted, and the loop repeats until the new term's fitted amplitude falls below
/// [`FftConfig::min_amplitude`] (that term is dropped) or [`FftConfig::max_terms`] is reached.
/// Frequencies are found in the band `[0, π/Δt]` of the once-per-orbit sampling, so they are the
/// aliases of the physical frequencies; at the sample times the two are equivalent.
///
/// # Errors
/// `FA-E0601` from the fits, `FA-E0603` for mismatched lengths or a bad `oversample`.
pub fn fft_frequencies(
    t_s: &[f64],
    longitudes_rad: &[f64],
    cfg: &FftConfig,
) -> Result<Vec<Frequency>, PrecessionError> {
    let n = t_s.len();
    if n != longitudes_rad.len() || n < 16 {
        return Err(input("fft_frequencies needs >= 16 equal-length samples"));
    }
    if !cfg.oversample.is_power_of_two() {
        return Err(input("FFT oversampling factor must be a power of two"));
    }
    let t_cy: Vec<f64> = t_s.iter().map(|t| t / SECONDS_PER_CENTURY).collect();
    let y: Vec<f64> = longitudes_rad
        .iter()
        .map(|a| a * arcsec_per_rad())
        .collect();
    let (Some(&t0), Some(&t1)) = (t_cy.first(), t_cy.last()) else {
        return Err(input("empty sample"));
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "sample counts are far below 2^53"
    )]
    let dt = (t1 - t0) / (n - 1) as f64;
    let m = (n * cfg.oversample).next_power_of_two();
    #[expect(clippy::cast_precision_loss, reason = "FFT sizes are far below 2^53")]
    let (mf, nf) = (m as f64, n as f64);
    let mut model = FitModel {
        quadratic: cfg.quadratic,
        frequencies: Vec::new(),
    };
    while model.frequencies.len() < cfg.max_terms {
        let resid = solve(&t_cy, &y, &model)?.residuals;
        let mut re = resid.clone();
        re.resize(m, 0.0);
        let mut im = vec![0.0; m];
        if !fft_in_place(&mut re, &mut im) {
            return Err(input("FFT size is not a power of two"));
        }
        // Bin k has k·n/m cycles over the span.
        let k_min = (cfg.min_cycles * mf / nf).ceil();
        let best = re
            .iter()
            .zip(&im)
            .enumerate()
            .take(m / 2)
            .filter(|(k, _)| {
                #[expect(clippy::cast_precision_loss, reason = "bin index below 2^53")]
                let kf = *k as f64;
                kf >= k_min
            })
            .map(|(k, (a, b))| (k, a.mul_add(*a, b * b)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((k, _)) = best else { break };
        #[expect(clippy::cast_precision_loss, reason = "bin index below 2^53")]
        let kf = k as f64;
        let omega = |kk: f64| TAU * kk / (mf * dt);
        let w = refine_sinusoid(&t_cy, &resid, omega(kf - 1.0), omega(kf + 1.0))?;
        let mut trial = model.clone();
        trial.frequencies.push(Frequency {
            rad_per_century: w,
            label: format!(
                "fft{} (period {:.4} yr)",
                model.frequencies.len() + 1,
                100.0 * TAU / w
            ),
        });
        let s = solve(&t_cy, &y, &trial)?;
        let last = s.coef.len();
        let amp = match (
            s.coef.get(last.wrapping_sub(2)),
            s.coef.get(last.wrapping_sub(1)),
        ) {
            (Some(a), Some(b)) => a.hypot(*b),
            _ => 0.0,
        };
        if amp < cfg.min_amplitude.arcsec() {
            break;
        }
        model = trial;
    }
    Ok(model.frequencies)
}

/// The estimator: plane, longitude definition, passage settings, fit model and jack-knife size.
#[derive(Debug, Clone, PartialEq)]
pub struct Park2017<F: Frame> {
    /// Reference plane.
    pub plane: ReferencePlane<F>,
    /// Longitude definition.
    pub definition: LongitudeDefinition,
    /// Passage location settings.
    pub passages: PassageConfig,
    /// Fit model.
    pub model: FitModel,
    /// Jack-knife blocks.
    pub jackknife_blocks: usize,
}

/// Everything the estimator produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Park2017Result {
    /// The fit.
    pub fit: Park2017Fit,
    /// The passages.
    pub passages: Vec<SampledPassage>,
    /// Unwrapped longitudes (rad) at the passages.
    pub longitudes_rad: Vec<f64>,
    /// Largest per-passage interpolation error estimate (rad).
    pub max_direction_error_rad: f64,
    /// Reference-plane label.
    pub frame: String,
    /// Longitude definition.
    pub definition: LongitudeDefinition,
}

impl<F: Frame> Park2017<F> {
    /// Run passages → longitudes → fit.
    ///
    /// # Errors
    /// As [`find_passages`], [`perihelion_longitudes`] and [`fit_longitudes`].
    pub fn estimate(&self, orbit: &SampledOrbit<F>) -> Result<Park2017Result, PrecessionError> {
        let passages = find_passages(orbit, &self.passages)?;
        let longitudes_rad = perihelion_longitudes(&passages, &self.plane, self.definition)?;
        let t: Vec<f64> = passages.iter().map(|p| p.t_s).collect();
        let fit = fit_longitudes(&t, &longitudes_rad, &self.model, self.jackknife_blocks)?;
        let max_direction_error_rad = passages
            .iter()
            .map(|p| p.direction_error_rad)
            .fold(0.0, f64::max);
        Ok(Park2017Result {
            fit,
            passages,
            longitudes_rad,
            max_direction_error_rad,
            frame: self.plane.label().to_owned(),
            definition: self.definition,
        })
    }
}

impl Park2017Result {
    /// A [`RateReport`] carrying frame, epoch (J2000.0 TDB, the fit epoch), fit span, fit model,
    /// constants, gauge, estimator and dynamics (PHY-015).
    ///
    /// # Errors
    /// `FA-E0602` if a caller-supplied field is blank.
    pub fn report<C: Center>(
        &self,
        rung: &str,
        constants: &str,
        gauge: &str,
        dynamics: &str,
    ) -> Result<RateReport, PrecessionError> {
        let span = TimeSpan::from_seconds(self.fit.span_s.1 - self.fit.span_s.0)
            .map_err(|_| PrecessionError::MissingMetadata { field: "fit span" })?;
        RateReportBuilder::new()
            .rung(rung)
            .rate(self.fit.rate)
            .frame(format!("{} (centre {})", self.frame, C::NAME))
            .epoch(Epoch::<Tdb>::j2000())
            .span(span)
            .fit_model(self.fit.fit_model.clone())
            .constants(constants)
            .gauge(gauge)
            .estimator(format!("{ESTIMATOR}; longitude {}", self.definition))
            .dynamics(dynamics)
            .build()
    }
}
