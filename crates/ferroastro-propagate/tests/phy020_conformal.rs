// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-020: the conformal-gravity comparison model (ADR-0016) reproduces the analytic Gauss
//! secular rates, its uniform γ0 centring leaves the heliocentric rate unchanged, it adds to GR
//! to first order, conserves energy with its potential, and assembles from `[forces.conformal]`
//! beside every relativistic switch with the "comparison model" label.
//!
//! # Oracles (decision D7)
//! Only the closed-form Gauss averages, per orbit, with `β = GM☉/c²` and the relative orbit's
//! `a`, `e`:
//! - linear term `−(γc²/2) r̂`: `Δϖ = −πγa²√(1−e²)/β`;
//! - quadratic term `+κc² r`: `Δϖ = +3πκa³√(1−e²)/β`.
//!
//! The published fit values (OI-79) are never used. The parameters are **exaggerated** and
//! scale-free: units with `GM☉ = 1 km³/s²` and `a = 1 km` (period 2π s), a synthetic `c`, and the
//! perturbing acceleration a fraction `ε = 1e-5` of `GM☉/a²` at `a`. The integrations are
//! two-body (or three-body) Newtonian N-body models with the term added, IAS15 with dense output
//! (NUM-010), 20 orbits started at a generic true anomaly; the per-orbit advance is the
//! least-squares slope of the perihelion longitude (root of `r·v`, rising) against the passage
//! number, minus the slope of the same run without the term.
//!
//! # Tolerance
//! The Gauss average is first order in `ε` and uses the initial osculating `a`, `e`, which differ
//! from the mean elements by O(ε). The neglected terms are therefore O(ε) relative to the rate:
//! the bound `TOL_REL = 1e-3` = 100 ε leaves a margin of about 8 over the observed
//! deviations (≤ 1.3e-4, printed; they flip sign with the perturbation, as second-order terms
//! do), while the IAS15 floor of the difference method (≈ 1e-15 rad per
//! orbit) is 10 orders below the 6e-5 rad/orbit signal. Sultana, Kazanas & Said's printed
//! eccentricity factor `(1−e²)²` instead of `√(1−e²)` would be off by 7 % at e = 0.2 and 35 % at
//! e = 0.5, so the bound discriminates the two forms.
//!
//! All tests are PR-tier (well under a second each); none is `#[ignore]`. The scenario tests at
//! the end need the cached header.440 (CI-005).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              reference arithmetic is written plainly"
)]

use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::{GravParam, Velocity};
use ferroastro_eih::HeliocentricSchwarzschild;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ConformalCentral, ConformalPotential, ConformalUniformGamma0, ForceTerm, MoonVariables, NBody,
    NBodyModel, NewtonianMutual, Pole, SchwarzschildPpn, TestParticle,
};
use ferroastro_integrators::{Ias15, Ias15Config, SecondOrderSystem};
use ferroastro_propagate::{DataSources, refuse_unsupported, run};
use ferroastro_scenario::{Conformal, Gamma0Centre, IntegratorName, ScenarioSpec};
use ferroastro_system::Tier;
use std::f64::consts::PI;
use std::path::PathBuf;

/// `GM☉` of the scale-free units (km³/s²).
const GM: f64 = 1.0;
/// Semi-major axis of the relative orbit (km).
const A: f64 = 1.0;
/// Synthetic speed of light (km/s): `β = GM/c² = 1e-6 km`.
const C: f64 = 1.0e3; // lint: allow-literal(synthetic c of the scale-free units)
/// The perturbing acceleration at `a` as a fraction of `GM/a²`.
const EPS: f64 = 1.0e-5; // lint: allow-literal(exaggeration of the perturbation, see module docs)
/// Relative bound on |numerical/analytic − 1| (module docs: 100 ε).
const TOL_REL: f64 = 1.0e-3; // lint: allow-literal(O(eps) tolerance, see module docs)
/// Orbits per run.
const ORBITS: usize = 20;
/// Initial true anomaly (rad): generic, so the first passage is a clean root.
const F0: f64 = 1.0;
/// Mercury-like eccentricity.
const E_LOW: f64 = 0.2; // lint: allow-literal(structural eccentricity)
/// High eccentricity, where `√(1−e²)` and `(1−e²)²` differ by 35 %.
const E_HIGH: f64 = 0.5;
/// Mass ratio of the planet (and of the outer perturber), finite so the reaction matters.
const Q: f64 = 1.0e-3; // lint: allow-literal(structural mass ratio)
/// Direction of the uniform field (any non-zero vector; normalised by `Pole`).
const N_UNIFORM: [f64; 3] = [0.6, -0.2, 0.5]; // lint: allow-literal(structural direction)

/// `β = GM☉/c²` (km).
fn beta() -> f64 {
    GM / (C * C)
}

/// γ (1/km) whose acceleration `γc²/2` is `ε_signed · GM/a²`.
fn gamma_for(eps: f64) -> f64 {
    2.0 * eps * GM / (C * C * A * A)
}

/// κ (1/km²) whose acceleration `κc² a` is `ε_signed · GM/a²`.
fn kappa_for(eps: f64) -> f64 {
    eps * GM / (C * C * A * A * A)
}

/// The PHY-020 linear-term oracle, rad per orbit.
fn gauss_linear(gamma: f64, e: f64) -> f64 {
    -PI * gamma * A * A * e.mul_add(-e, 1.0).sqrt() / beta()
}

/// The PHY-020 quadratic-term oracle, rad per orbit.
fn gauss_quadratic(kappa: f64, e: f64) -> f64 {
    3.0 * PI * kappa * A * A * A * e.mul_add(-e, 1.0).sqrt() / beta()
}

fn speed() -> Velocity {
    Velocity::from_km_per_s(C).unwrap()
}

/// Relative Kepler state (μ = GM(1 + q)) at true anomaly `F0`.
fn kepler_state(e: f64, mu: f64, a: f64, phase: f64) -> ([f64; 3], [f64; 3]) {
    let p = a * e.mul_add(-e, 1.0);
    let f = F0 + phase;
    let r = p / e.mul_add(f.cos(), 1.0);
    let w = (mu / p).sqrt();
    (
        [r * f.cos(), r * f.sin(), 0.0],
        [-w * f.sin(), w * (e + f.cos()), 0.0],
    )
}

/// A planetary system: the Sun, a body of mass ratio `q` on the (a = 1, e) orbit and optionally
/// a circular outer perturber; barycentric component-major positions and velocities.
struct System {
    bodies: Vec<NBody>,
    x: Vec<f64>,
    v: Vec<f64>,
}

fn system(e: f64, q: f64, outer: bool) -> System {
    let mut bodies = vec![
        NBody {
            id: NaifId::SUN,
            gm: GM,
            tier: Tier::Major,
        },
        NBody {
            id: NaifId::MERCURY_BARYCENTER,
            gm: q * GM,
            tier: Tier::Major,
        },
    ];
    let (r1, w1) = kepler_state(e, GM * (1.0 + q), A, 0.0);
    let mut rel = vec![(r1, w1, q)];
    if outer {
        let q3 = Q;
        bodies.push(NBody {
            id: NaifId::JUPITER_BARYCENTER,
            gm: q3 * GM,
            tier: Tier::Major,
        });
        // lint: allow-literal(structural outer orbit: a = 3, circular, other phase)
        let (r3, w3) = kepler_state(0.0, GM * (1.0 + q3), 3.0, 2.0);
        rel.push((r3, w3, q3));
    }
    let n = bodies.len();
    let total: f64 = bodies.iter().map(|b| b.gm).sum();
    // Heliocentric → barycentric: shift every body by −Σ mᵢ rᵢ / M.
    let mut com = [[0.0; 3]; 2];
    for (r, w, q) in &rel {
        for k in 0..3 {
            com[0][k] += q * GM * r[k] / total;
            com[1][k] += q * GM * w[k] / total;
        }
    }
    let mut x = vec![0.0; 3 * n];
    let mut v = vec![0.0; 3 * n];
    for k in 0..3 {
        x[k * n] = -com[0][k];
        v[k * n] = -com[1][k];
        for (j, (r, w, _)) in rel.iter().enumerate() {
            x[k * n + j + 1] = r[k] - com[0][k];
            v[k * n + j + 1] = w[k] - com[1][k];
        }
    }
    System { bodies, x, v }
}

/// Heliocentric relative state of body `i` (storage index) in an `n`-body flat state.
fn relative(x: &[f64], v: &[f64], n: usize, i: usize) -> ([f64; 3], [f64; 3]) {
    let r = [0, 1, 2].map(|k| x[k * n + i] - x[k * n]);
    let w = [0, 1, 2].map(|k| v[k * n + i] - v[k * n]);
    (r, w)
}

/// Least-squares slope of `y` against `0, 1, 2, …`.
fn slope(y: &[f64]) -> f64 {
    let m = y.len() as f64;
    let xm = (m - 1.0) / 2.0;
    let ym = y.iter().sum::<f64>() / m;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (k, yk) in y.iter().enumerate() {
        let dx = k as f64 - xm;
        sxy += dx * (yk - ym);
        sxx += dx * dx;
    }
    sxy / sxx
}

/// Per-orbit advance (rad) of body 1's perihelion longitude: IAS15 with dense output over
/// `ORBITS` orbits, passages at the rising roots of `r·v` (relative to storage index 0, or the
/// state itself for a 3-dimensional test particle).
fn advance_per_orbit<S: SecondOrderSystem>(sys: &S, x0: &[f64], v0: &[f64]) -> f64 {
    let dim = sys.dimension();
    let n = dim / 3;
    let rel = |x: &[f64], v: &[f64]| -> ([f64; 3], [f64; 3]) {
        if n == 1 {
            ([x[0], x[1], x[2]], [v[0], v[1], v[2]])
        } else {
            relative(x, v, n, 1)
        }
    };
    let g = |_t: f64, x: &[f64], v: &[f64]| {
        let (r, w) = rel(x, v);
        r[0] * w[0] + r[1] * w[1] + r[2] * w[2]
    };
    let cfg = Ias15Config {
        dense_output: true,
        ..Ias15Config::default()
    };
    let mut ias = Ias15::new(dim, 0.0, x0, v0, 0.01, cfg).unwrap(); // lint: allow-literal(first trial step, s)
    let t_end = 2.0 * PI * (ORBITS as f64 + 1.5); // lint: allow-literal(margin of 1.5 periods for the last passage)
    let (mut xs, mut vs) = (vec![0.0; dim], vec![0.0; dim]);
    let mut longitudes: Vec<f64> = Vec::new();
    let mut g_prev = g(0.0, x0, v0);
    while ias.time() < t_end && longitudes.len() < ORBITS + 1 {
        ias.step(sys, t_end).unwrap();
        let g_now = g(ias.time(), ias.positions(), ias.velocities());
        if g_prev < 0.0 && g_now >= 0.0 {
            let t = ias.locate_event(g).unwrap().expect("root in the step");
            ias.dense_state(t, &mut xs, &mut vs).unwrap();
            let (r, _) = rel(&xs, &vs);
            let mut l = r[1].atan2(r[0]);
            if let Some(&prev) = longitudes.last() {
                while l - prev > PI {
                    l -= 2.0 * PI;
                }
                while l - prev < -PI {
                    l += 2.0 * PI;
                }
            }
            longitudes.push(l);
        }
        g_prev = g_now;
    }
    assert_eq!(longitudes.len(), ORBITS + 1, "passages found");
    slope(&longitudes)
}

fn model(s: &System, extra: Vec<Box<dyn ForceTerm<Icrf>>>) -> NBodyModel<Icrf> {
    let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
    terms.extend(extra);
    NBodyModel::new(&s.bodies, MoonVariables::Barycentric, terms).unwrap()
}

fn sun_term(gamma: f64, kappa: f64) -> Box<dyn ForceTerm<Icrf>> {
    Box::new(ConformalPotential::new(gamma, 0.0, kappa, speed()).unwrap())
}

fn uniform_term(gamma0: f64) -> Box<dyn ForceTerm<Icrf>> {
    let n = Pole::from_components(N_UNIFORM).unwrap();
    Box::new(ConformalUniformGamma0::new(gamma0, speed(), n).unwrap())
}

/// The advance of `extra` minus the Newtonian run's (difference method).
fn measured(s: &System, extra: Vec<Box<dyn ForceTerm<Icrf>>>) -> f64 {
    let base = advance_per_orbit(&model(s, Vec::new()), &s.x, &s.v);
    advance_per_orbit(&model(s, extra), &s.x, &s.v) - base
}

fn check(label: &str, got: f64, want: f64) {
    let rel = got / want - 1.0;
    println!("{label}: {got:+.10e} rad/orbit, Gauss {want:+.10e}, relative deviation {rel:+.2e}");
    assert!(rel.abs() < TOL_REL, "{label}: {rel:e}");
}

/// PHY-020 (a): the Sun-sourced linear term, exaggerated, of both signs, matches
/// `−πγa²√(1−e²)/β` per orbit (γ > 0 retrograde). The planet has a finite mass ratio 1e-3, so
/// the Sun's reaction (1 + m/M☉) and the two-body μ of the Gauss average must cancel with
/// `β = GM☉/c²`; e = 0.5 separates `√(1−e²)` from Sultana et al.'s printed `(1−e²)²`.
///
/// Verifies: PHY-020
#[test]
fn linear_term_matches_the_gauss_rate_for_both_signs() {
    for (e, q) in [(E_LOW, Q), (E_HIGH, Q), (E_LOW, 0.0)] {
        let s = system(e, q, false);
        for sign in [1.0, -1.0] {
            let gamma = gamma_for(sign * EPS);
            let got = measured(&s, vec![sun_term(gamma, 0.0)]);
            let want = gauss_linear(gamma, e);
            check(
                &format!("linear, e = {e}, m/M = {q}, gamma {sign:+}"),
                got,
                want,
            );
            assert_eq!(got.signum(), -gamma.signum(), "gamma > 0 is retrograde");
            let sultana = -PI * gamma * A * A * e.mul_add(-e, 1.0).powi(2) / beta();
            assert!(
                (got / sultana - 1.0).abs() > 10.0 * TOL_REL,
                "(1-e^2)^2 form rejected"
            );
        }
    }
}

/// PHY-020 (b): the quadratic term with an exaggerated κ > 0 (repulsive) matches
/// `+3πκa³√(1−e²)/β` per orbit and is prograde; κ < 0 reverses it.
///
/// Verifies: PHY-020
#[test]
fn quadratic_term_matches_the_gauss_rate_and_is_prograde() {
    for e in [E_LOW, E_HIGH] {
        let s = system(e, Q, false);
        for sign in [1.0, -1.0] {
            let kappa = kappa_for(sign * EPS);
            let got = measured(&s, vec![sun_term(0.0, kappa)]);
            check(
                &format!("quadratic, e = {e}, kappa {sign:+}"),
                got,
                gauss_quadratic(kappa, e),
            );
            assert_eq!(got.signum(), kappa.signum(), "kappa > 0 is prograde");
        }
    }
}

/// The test-particle form (`ConformalCentral`, the Rung-lab term) gives the same Gauss rates for
/// both terms together (first order: the rates add).
///
/// Verifies: PHY-020
#[test]
fn central_term_matches_the_gauss_rates() {
    let (gamma, kappa) = (gamma_for(EPS), kappa_for(-EPS));
    let mu = GravParam::from_km3_per_s2(GM).unwrap();
    let (r0, v0) = kepler_state(E_LOW, GM, A, 0.0);
    let newton = TestParticle::<Icrf>::new(mu, Vec::new()).unwrap();
    let cg = TestParticle::<Icrf>::new(
        mu,
        vec![Box::new(
            ConformalCentral::new(gamma, kappa, speed()).unwrap(),
        )],
    )
    .unwrap();
    let got = advance_per_orbit(&cg, &r0, &v0) - advance_per_orbit(&newton, &r0, &v0);
    check(
        "central, linear + quadratic",
        got,
        gauss_linear(gamma, E_LOW) + gauss_quadratic(kappa, E_LOW),
    );
}

/// PHY-020 (c): with `gamma0_centre = "uniform"` an exaggerated γ0 (the same ε as the
/// Sun-centred runs, so the Sun-centred reading would give −2πε√(1−e²) ≈ 6e-5 rad/orbit) leaves
/// the heliocentric perihelion rate unchanged within the integrator floor, also with a third
/// (massive) body, while the barycentre accelerates.
///
/// Verifies: PHY-020
#[test]
fn uniform_gamma0_leaves_the_heliocentric_rate_unchanged() {
    let s = system(E_LOW, Q, true);
    let gamma0 = gamma_for(EPS);
    let sun_centred = measured(
        &s,
        vec![Box::new(
            ConformalPotential::new(0.0, gamma0, 0.0, speed()).unwrap(),
        )],
    );
    let uniform = measured(&s, vec![uniform_term(gamma0)]);
    // The floor: the same difference for a pure Newtonian rerun is exactly 0; the uniform field
    // changes only the rounding of the barycentric positions (|x| grows by g t²/2 ≈ 0.4 a).
    let floor = 1.0e-12; // lint: allow-literal(integrator floor bound, rad/orbit: 1e-7 of the signal)
    println!(
        "uniform gamma0: {uniform:+.3e} rad/orbit (Sun-centred {sun_centred:+.6e}, Gauss {:+.6e})",
        gauss_linear(gamma0, E_LOW)
    );
    assert!(uniform.abs() < floor, "{uniform:e}");
    // The Sun-centred reading of the same γ0 is resolved (3-body: Gauss to 1 % only, the outer
    // body perturbs the orbit at the 1e-3 level).
    assert!((sun_centred / gauss_linear(gamma0, E_LOW) - 1.0).abs() < 1.0e-2); // lint: allow-literal(3-body tolerance)
    // The field is applied, not dropped: the barycentre's acceleration is g.
    let m = model(&s, vec![uniform_term(gamma0)]);
    let n = s.bodies.len();
    let mut a = vec![0.0; 3 * n];
    m.acceleration(0.0, &s.x, &s.v, &mut a).unwrap();
    let total: f64 = s.bodies.iter().map(|b| b.gm).sum();
    let acm: Vec<f64> = (0..3)
        .map(|k| (0..n).map(|i| s.bodies[i].gm * a[k * n + i]).sum::<f64>() / total)
        .collect();
    let g = acm[0].hypot(acm[1]).hypot(acm[2]);
    let want = gamma0 * C * C / 2.0;
    assert!((g / want - 1.0).abs() < 1.0e-12, "{g:e} vs {want:e}"); // lint: allow-literal(round-off budget)
}

/// PHY-020 (e): GR 1PN in PPN isotropic/harmonic gauge (the Sun-only N-body term of
/// `gr_1pn_central`) plus the conformal term assembles without FA-E0502, and the advances add to
/// first order: rate(GR + CG) = rate(GR) + rate(CG) up to the cross term, O(`ε_GR · ε_CG`).
///
/// Verifies: PHY-020, NUM-017
#[test]
fn gr_1pn_and_conformal_assemble_and_add_to_first_order() {
    let s = system(E_LOW, 1.0e-9, false); // lint: allow-literal(near test-particle mass ratio)
    let gamma = gamma_for(EPS);
    let gr = || -> Box<dyn ForceTerm<Icrf>> {
        Box::new(HeliocentricSchwarzschild::new(speed(), 1.0, 1.0).unwrap())
    };
    let both = model(&s, vec![gr(), sun_term(gamma, 0.0)]);
    assert!(both.has_comparison_model());
    assert!(
        both.describe()
            .contains("PPN isotropic/harmonic; + comparison model")
    );
    let r_gr = measured(&s, vec![gr()]);
    let r_cg = measured(&s, vec![sun_term(gamma, 0.0)]);
    let r_both = measured(&s, vec![gr(), sun_term(gamma, 0.0)]);
    let pn = 6.0 * PI * beta() / (A * E_LOW.mul_add(-E_LOW, 1.0));
    println!(
        "GR {r_gr:+.6e} (6 pi beta/p = {pn:+.6e}), CG {r_cg:+.6e}, GR + CG {r_both:+.6e}, \
         non-additivity {:+.2e}",
        (r_both - r_gr - r_cg) / r_both
    );
    assert!((r_gr / pn - 1.0).abs() < TOL_REL, "GR rung");
    check("CG beside nothing", r_cg, gauss_linear(gamma, E_LOW));
    // Cross term ~ ε_GR ε_CG ≪ TOL_REL · |rate| (ε_GR ≈ 3β/a ≈ 3e-6).
    assert!(((r_both - r_gr - r_cg) / r_both).abs() < TOL_REL);
    // Test-particle form: 1PN Schwarzschild (PPN gauge) + ConformalCentral.
    let mu = GravParam::from_km3_per_s2(GM).unwrap();
    let tp = TestParticle::<Icrf>::new(
        mu,
        vec![
            Box::new(SchwarzschildPpn::new(mu, speed(), 1.0, 1.0).unwrap()),
            Box::new(ConformalCentral::new(gamma, 0.0, speed()).unwrap()),
        ],
    );
    assert!(tp.is_ok());
}

/// The conformal terms conserve the total energy with their potentials (NUM-023): over 20 orbits
/// of the exaggerated three-body system with both the Sun-sourced (γ, κ) and the uniform γ0
/// terms, IAS15's relative energy error stays at round-off level.
///
/// Verifies: PHY-020, NUM-023
#[test]
fn energy_is_conserved_with_the_conformal_potential() {
    let s = system(E_LOW, Q, true);
    let m = model(
        &s,
        vec![
            sun_term(gamma_for(EPS), kappa_for(EPS)),
            uniform_term(gamma_for(-EPS)),
        ],
    );
    let e0 = m.energy_over_g(&s.x, &s.v).unwrap();
    let mut ias = Ias15::new(s.x.len(), 0.0, &s.x, &s.v, 0.01, Ias15Config::default()).unwrap(); // lint: allow-literal(first trial step, s)
    let t_end = 2.0 * PI * ORBITS as f64;
    let mut worst = 0.0_f64;
    while ias.time() < t_end {
        ias.step(&m, t_end).unwrap();
        let e = m.energy_over_g(ias.positions(), ias.velocities()).unwrap();
        worst = worst.max(((e - e0) / e0).abs());
    }
    println!("max |dE/E| over {ORBITS} orbits = {worst:.3e}");
    assert!(worst < 1.0e-12, "{worst:e}"); // lint: allow-literal(IAS15 round-off level, Brouwer growth over 20 orbits)
}

// ---------------------------------------------------------------------------------------------
// Scenario level (`[forces.conformal]`): needs the cached header.440 (CI-005).

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn with_conformal(mut s: ScenarioSpec, centre: Gamma0Centre) -> ScenarioSpec {
    // Exaggerated CGS coefficients of no particular meaning (not the OI-79 fit values).
    s.forces.conformal = Some(Conformal {
        gamma0: 1.0e-26,     // lint: allow-literal(exaggerated gamma0, 1/cm)
        gamma_star: 2.0e-27, // lint: allow-literal(exaggerated gamma_star, 1/cm)
        kappa: 3.0e-40,      // lint: allow-literal(exaggerated kappa, 1/cm^2)
        gamma0_centre: centre,
        gamma0_direction: [0.0, 0.0, 1.0],
    });
    s.span.years = 0.01; // lint: allow-literal(a few steps: the model string is what is checked)
    s.span.stop_after_passages = None;
    s.output.cadence_days = None;
    s
}

/// PHY-020 (e), (f): `[forces.conformal]` assembles beside EIH, the Sun-only 1PN rung and (with
/// `WHFast`) the GR effective potential without FA-E0502; the model string of every run carries
/// the "comparison model" label and the term's parameters in km units; `"sun"` puts γ0 into the
/// Sun-sourced term, `"uniform"` adds the uniform field. The table is refused in test-particle
/// mode (FA-E1201).
///
/// Verifies: PHY-020, PHY-015, CI-002, REF-011
#[test]
fn conformal_scenarios_assemble_with_every_relativistic_switch() {
    if De440::load_cached_or_skip("conformal_scenarios_assemble_with_every_relativistic_switch")
        .unwrap()
        .is_none()
    {
        return;
    }
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let data = DataSources::none().with_header(&h, &prov);
    for (file, centre, gauge) in [
        (
            "sun-mercury-eih-header440.toml",
            Gamma0Centre::Sun,
            "PPN isotropic/harmonic",
        ),
        (
            "sun-mercury-eih-header440.toml",
            Gamma0Centre::Uniform,
            "PPN isotropic/harmonic",
        ),
        (
            "wh-sun-mercury-grpotential-100yr.toml",
            Gamma0Centre::Uniform,
            "GR effective potential",
        ),
        (
            "sun-mercury-newton-header440.toml",
            Gamma0Centre::Sun,
            "comparison model",
        ),
    ] {
        let s = with_conformal(spec(file), centre);
        assert!(s.forces.requested().contains(&"conformal"));
        refuse_unsupported(&s).unwrap();
        let out = run(&s, &data).unwrap();
        println!("{file} ({}): {}", centre.label(), out.model);
        assert!(
            out.model.starts_with(&format!("N-body model [{gauge}")),
            "{}",
            out.model
        );
        assert!(
            out.model.contains("comparison model (ADR-0016)"),
            "{}",
            out.model
        );
        assert!(
            out.model.contains("conformal-potential-sun"),
            "{}",
            out.model
        );
        // gamma0 = 1e-26 /cm = 1e-21 /km goes into the Sun-sourced term only for "sun".
        let no_sun_share = out.model.contains("gamma0 (Sun-centred share) = 0 1/km");
        assert_eq!(
            no_sun_share,
            centre == Gamma0Centre::Uniform,
            "{}",
            out.model
        );
        assert_eq!(
            out.model.contains("conformal-gamma0-uniform"),
            centre == Gamma0Centre::Uniform,
            "{}",
            out.model
        );
        if s.integrator.name == IntegratorName::Whfast {
            assert!(out.model.contains("Wisdom-Holman"), "{}", out.model);
        }
    }
    // gr_1pn_central in n-body mode (the Sun-only rung) + conformal.
    let mut s1 = with_conformal(spec("sun-mercury-eih-header440.toml"), Gamma0Centre::Sun);
    s1.forces.gr_eih = false;
    s1.forces.gr_1pn_central = true;
    let out = run(&s1, &data).unwrap();
    assert!(
        out.model
            .starts_with("N-body model [PPN isotropic/harmonic; + comparison model"),
        "{}",
        out.model
    );
    // Refused in test-particle mode.
    let tp = with_conformal(spec("sun-mercury-gr1pn-header440.toml"), Gamma0Centre::Sun);
    let e = refuse_unsupported(&tp).unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E1201");
    assert!(e.to_string().contains("forces.conformal"), "{e}");
}

/// PHY-020 (c) at scenario level: Sun + Mercury with DE440 initial conditions and an
/// exaggerated γ0 (1e-26 cm⁻¹, ≈ 3000× the unverified fit value's order) gives the same perihelion
/// passages with `"uniform"` as without the table, to the integrator floor, while `"sun"` moves
/// them.
///
/// Verifies: PHY-020
#[test]
fn uniform_gamma0_scenario_leaves_the_heliocentric_passages_unchanged() {
    if De440::load_cached_or_skip(
        "uniform_gamma0_scenario_leaves_the_heliocentric_passages_unchanged",
    )
    .unwrap()
    .is_none()
    {
        return;
    }
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    let data = DataSources::none().with_header(&h, &prov);
    let mut base = spec("sun-mercury-newton-header440.toml");
    base.span.years = 5.0; // lint: allow-literal(about 20 Mercury orbits)
    base.span.stop_after_passages = None;
    base.output.cadence_days = None;
    let longitude = |s: &ScenarioSpec| -> Vec<f64> {
        run(s, &data)
            .unwrap()
            .passages
            .iter()
            .map(|p| {
                let r = p.r.map(|c| c[0] + c[1]);
                r[1].atan2(r[0])
            })
            .collect()
    };
    let pick = |centre| {
        let mut s = base.clone();
        let mut c = with_conformal(s.clone(), centre).forces.conformal.unwrap();
        c.gamma_star = 0.0;
        c.kappa = 0.0;
        s.forces.conformal = Some(c);
        s
    };
    let l0 = longitude(&base);
    let lu = longitude(&pick(Gamma0Centre::Uniform));
    let ls = longitude(&pick(Gamma0Centre::Sun));
    assert!(l0.len() >= 20 && l0.len() == lu.len() && l0.len() == ls.len());
    let d_uniform = l0
        .iter()
        .zip(&lu)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let d_sun = (ls.last().unwrap() - l0.last().unwrap()).abs();
    println!(
        "{} passages: max |dl| uniform {d_uniform:.3e} rad, last-passage |dl| Sun-centred {d_sun:.3e} rad",
        l0.len()
    );
    assert!(d_uniform < 1.0e-12, "{d_uniform:e}"); // lint: allow-literal(passage-location floor, rad)
    assert!(d_sun > 1.0e3 * d_uniform.max(1.0e-15)); // lint: allow-literal(signal well above floor)
}
