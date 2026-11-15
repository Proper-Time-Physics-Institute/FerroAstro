// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//! PHY-021: the Proper Time (Gill) comparison model (ADR-0016 decision D6). Two independent
//! routes give the same first-order perihelion advance, −1/6 of GR's:
//!
//! 1. the PPN 1PN field at (β, γ) = (1/2, −1) (`SchwarzschildPpn` for a test particle,
//!    `HeliocentricSchwarzschild` in an N-body model): velocity dependent, coordinate time,
//!    the 1PN expansion of the conformally flat metric `(1 − GM/(rc²))² η` (Bridge A);
//! 2. a direct integration of Gill's flat-space force law `a = −(∇V/m)(1 + V/(mc²))`
//!    (`ProperTimeCentral`, `ProperTimeKernel`): velocity independent, Gill's `K`-flow.
//!
//! The kernel replaces the GR 1PN sector (FA-E0502 beside GR terms), combines with the
//! conformal-gravity comparison terms, conserves energy with its potential, and assembles from
//! `forces.proper_time_kernel` under IAS15 and `WHFast` with the "comparison model" label.
//!
//! # Oracle
//! Only the closed form `Δϖ = −πGM/(c²a(1−e²))` per orbit
//! (`ferroastro_analytic::advance::proper_time_advance`, one negative 1PN unit; `(2 + 2γ − β)/3`
//! at (1/2, −1) times GR's `6πGM/(c²a(1−e²))`). No fitted or published number is an oracle; the
//! research value −7.163432586 ″/cy for Mercury is printed, not asserted.
//!
//! # Parameters and tolerance
//! Scale-free units as in PHY-020's tests: `GM☉ = 1 km³/s²`, `a = 1 km` (period 2π s) and an
//! **exaggerated** synthetic `c = 10³ km/s`, so `x = GM/(c²p) ≈ 1e-6` (Mercury: 2.6e-8). IAS15
//! with dense output, 20 orbits from a generic true anomaly, perihelion longitude at the rising
//! roots of `r·v`, least-squares slope per orbit, minus the Newtonian run's slope. Both routes and
//! the oracle agree to first order in `x`; the neglected terms (second-order PN terms, and the
//! O(x) gap between initial osculating and mean elements, which the velocity-dependent PPN route
//! has and the force law has only through `e`) are O(x) relative, ≈ 1e-6. `TOL_REL = 1e-4` = 100 x
//! leaves a margin of about 20 over the observed deviations (printed; the largest, 5e-6, is the
//! GR (1, 1) reference at e = 0.5; the two routes agree to ≤ 7e-7), while the IAS15 floor of
//! the difference method (≈ 1e-15 rad per orbit) is 9 orders below the 3e-6 rad/orbit signal. The
//! −1/6 ratio is the discriminating check: GR's +1 and Gill's −1/6 differ by 7/6.
//!
//! Gill's law is a flow in his parameter `τ_K`, with `dt/dτ_K = E/(mc²)` constant on each orbit
//! (research report §1.2), so integrating it in `t` gives the same orbit and advance per orbit; the
//! period differs by a constant `1 + O(x)`, which the per-orbit comparison does not see.
//!
//! All tests are PR-tier; none is `#[ignore]`. The scenario tests need the cached header.440
//! (CI-005).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::cast_precision_loss,
    clippy::suboptimal_flops,
    reason = "test: a failed unwrap is a test failure; indices are in range by construction; \
              reference arithmetic is written plainly"
)]

use ferroastro_analytic::advance::{proper_time_advance, schwarzschild_advance};
use ferroastro_constants::De440;
use ferroastro_constants::header440::Header;
use ferroastro_core::frames::Icrf;
use ferroastro_core::units::{GravParam, Length, Velocity};
use ferroastro_eih::HeliocentricSchwarzschild;
use ferroastro_ephem::NaifId;
use ferroastro_forces::{
    ConformalPotential, ForceModel, ForceTerm, Gauge, GrPotential, MoonVariables, NBody,
    NBodyModel, NewtonianMutual, ProperTimeCentral, ProperTimeKernel, SchwarzschildPpn,
    TestParticle,
};
use ferroastro_integrators::{Ias15, Ias15Config, SecondOrderSystem};
use ferroastro_propagate::{DataSources, refuse_unsupported, run};
use ferroastro_scenario::{IntegratorName, Ppn, ScenarioSpec};
use ferroastro_system::Tier;
use std::f64::consts::PI;
use std::path::PathBuf;

/// `GM☉` of the scale-free units (km³/s²).
const GM: f64 = 1.0;
/// Semi-major axis of the relative orbit (km).
const A: f64 = 1.0;
/// Synthetic, exaggerated speed of light (km/s): `GM/c² = 1e-6 km`.
const C: f64 = 1.0e3; // lint: allow-literal(synthetic c of the scale-free units)
/// Relative bound on |numerical/analytic − 1| (module docs: 100 x).
const TOL_REL: f64 = 1.0e-4; // lint: allow-literal(O(x) tolerance, see module docs)
/// Orbits per run.
const ORBITS: usize = 20;
/// Initial true anomaly (rad): generic, so the first passage is a clean root.
const F0: f64 = 1.0;
/// Mercury-like eccentricity.
const E_LOW: f64 = 0.2; // lint: allow-literal(structural eccentricity)
/// High eccentricity.
const E_HIGH: f64 = 0.5;
/// Mass ratio of the planet, finite so the Sun's reaction matters.
const Q: f64 = 1.0e-3; // lint: allow-literal(structural mass ratio)
/// The Proper Time / GR ratio of the first-order advance, `(2 + 2γ − β)/3` at (1/2, −1).
const RATIO: f64 = -1.0 / 6.0;

/// The PHY-021 oracle, rad per orbit.
fn oracle(e: f64) -> f64 {
    proper_time_advance(
        GravParam::from_km3_per_s2(GM).unwrap(),
        speed(),
        Length::from_km(A).unwrap(),
        e,
    )
    .unwrap()
    .radians()
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

fn kernel() -> Box<dyn ForceTerm<Icrf>> {
    Box::new(ProperTimeKernel::new(speed()).unwrap())
}

fn ppn_field(beta: f64, gamma: f64) -> Box<dyn ForceTerm<Icrf>> {
    Box::new(HeliocentricSchwarzschild::new(speed(), beta, gamma).unwrap())
}

/// The advance of `extra` minus the Newtonian run's (difference method).
fn measured(s: &System, extra: Vec<Box<dyn ForceTerm<Icrf>>>) -> f64 {
    let base = advance_per_orbit(&model(s, Vec::new()), &s.x, &s.v);
    advance_per_orbit(&model(s, extra), &s.x, &s.v) - base
}

fn check(label: &str, got: f64, want: f64) {
    let rel = got / want - 1.0;
    println!("{label}: {got:+.10e} rad/orbit, oracle {want:+.10e}, relative deviation {rel:+.2e}");
    assert!(rel.abs() < TOL_REL, "{label}: {rel:e}");
}

/// Route 2 in an N-body model: the Sun-sourced kernel with the reaction on the Sun matches
/// `−πGM☉/(c²a(1−e²))` per orbit (retrograde) for a finite mass ratio (the reaction factor
/// cancels against the two-body `L²`, so the oracle uses the Sun's GM only) and for a test body.
///
/// Verifies: PHY-021
#[test]
fn kernel_matches_the_closed_form_in_an_n_body_model() {
    for (e, q) in [(E_LOW, Q), (E_HIGH, Q), (E_LOW, 0.0)] {
        let s = system(e, q, false);
        let got = measured(&s, vec![kernel()]);
        check(&format!("kernel, e = {e}, m/M = {q}"), got, oracle(e));
        assert!(got < 0.0, "the Proper Time kernel is retrograde");
    }
}

/// PHY-021 acceptance, test-particle form: the PPN (1/2, −1) 1PN field (route 1) and Gill's
/// force law (route 2) agree, each equals the closed form, and each is −1/6 of the GR (1, 1)
/// field's measured advance, at Mercury-like and high eccentricity.
///
/// Verifies: PHY-021, PHY-006
#[test]
fn ppn_half_minus_one_and_gill_force_law_agree_at_minus_one_sixth_of_gr() {
    let mu = GravParam::from_km3_per_s2(GM).unwrap();
    for e in [E_LOW, E_HIGH] {
        let (r0, v0) = kepler_state(e, GM, A, 0.0);
        let run = |terms: Vec<Box<dyn ferroastro_forces::CentralTerm<Icrf>>>| {
            advance_per_orbit(&TestParticle::<Icrf>::new(mu, terms).unwrap(), &r0, &v0)
        };
        let newton = run(Vec::new());
        let gr = run(vec![Box::new(
            SchwarzschildPpn::new(mu, speed(), 1.0, 1.0).unwrap(),
        )]) - newton;
        let ppn = run(vec![Box::new(
            SchwarzschildPpn::new(mu, speed(), 0.5, -1.0).unwrap(),
        )]) - newton;
        let gill = run(vec![Box::new(ProperTimeCentral::new(mu, speed()).unwrap())]) - newton;
        let gr_oracle = schwarzschild_advance(mu, speed(), Length::from_km(A).unwrap(), e, mu)
            .unwrap()
            .per_orbit
            .radians();
        check(&format!("GR (1, 1), e = {e}"), gr, gr_oracle);
        check(&format!("route 1 PPN (1/2, -1), e = {e}"), ppn, oracle(e));
        check(&format!("route 2 Gill force law, e = {e}"), gill, oracle(e));
        for (label, x) in [("PPN", ppn), ("Gill", gill)] {
            let ratio = x / gr;
            println!("{label} / GR = {ratio:+.8} (want {RATIO:+.8})");
            assert!((ratio / RATIO - 1.0).abs() < TOL_REL, "{label}: {ratio}");
        }
        let gap = (ppn - gill) / gill;
        println!("route gap (PPN - Gill)/Gill = {gap:+.2e}");
        assert!(gap.abs() < TOL_REL, "routes disagree: {gap:e}");
    }
}

/// Route 1 in an N-body model (`HeliocentricSchwarzschild` at (1/2, −1), the viewer's GR bit with
/// the PPN preset) agrees with the kernel (route 2) for a near-test-particle planet.
///
/// Verifies: PHY-021
#[test]
fn n_body_ppn_route_matches_the_kernel() {
    let s = system(E_LOW, 1.0e-9, false); // lint: allow-literal(near test-particle mass ratio)
    let ppn = measured(&s, vec![ppn_field(0.5, -1.0)]);
    let gill = measured(&s, vec![kernel()]);
    check("n-body route 1 PPN (1/2, -1)", ppn, oracle(E_LOW));
    check("n-body route 2 kernel", gill, oracle(E_LOW));
    assert!(((ppn - gill) / gill).abs() < TOL_REL);
}

/// The kernel's gauge is a relativistic comparison gauge: it is refused beside the GR 1PN field,
/// the GR effective potential and (test-particle form) the 1PN Schwarzschild term (FA-E0502); it
/// combines with the conformal-gravity terms, and the model then names both comparison models.
///
/// Verifies: PHY-021, PHY-020, NUM-017
#[test]
fn kernel_replaces_the_gr_sector_and_combines_with_conformal_gravity() {
    let g = Gauge::ProperTimeKernel;
    assert!(g.is_relativistic() && g.is_comparison_model());
    assert!(g.label().contains("comparison model") && g.label().contains("Gill"));
    let s = system(E_LOW, Q, false);
    let build = |t: Vec<Box<dyn ForceTerm<Icrf>>>| {
        let mut terms: Vec<Box<dyn ForceTerm<Icrf>>> = vec![Box::new(NewtonianMutual)];
        terms.extend(t);
        NBodyModel::new(&s.bodies, MoonVariables::Barycentric, terms)
    };
    for other in [
        ppn_field(1.0, 1.0),
        Box::new(GrPotential::new(speed()).unwrap()) as Box<dyn ForceTerm<Icrf>>,
    ] {
        let e = build(vec![kernel(), other]).unwrap_err();
        println!("{e}");
        assert_eq!(e.code().to_string(), "FA-E0502");
    }
    let alone = build(vec![kernel()]).unwrap();
    assert_eq!(alone.gauge(), Gauge::ProperTimeKernel);
    assert!(alone.has_comparison_model());
    assert!(
        alone.describe().contains("proper-time-kernel-sun"),
        "{}",
        alone.describe()
    );
    let with_cg = build(vec![
        kernel(),
        Box::new(ConformalPotential::new(1.0e-12, 0.0, 0.0, speed()).unwrap()), // lint: allow-literal(arbitrary gamma, 1/km)
    ])
    .unwrap();
    let label = with_cg.gauge_label();
    println!("{label}");
    assert!(label.starts_with("comparison model (ADR-0016): Proper Time (Gill)"));
    assert!(label.contains("; + comparison model (ADR-0016): conformal-gravity"));
    let mu = GravParam::from_km3_per_s2(GM).unwrap();
    let e = TestParticle::<Icrf>::new(
        mu,
        vec![
            Box::new(SchwarzschildPpn::new(mu, speed(), 0.5, -1.0).unwrap()),
            Box::new(ProperTimeCentral::new(mu, speed()).unwrap()),
        ],
    )
    .unwrap_err();
    assert_eq!(e.code().to_string(), "FA-E0502");
    let tp = TestParticle::<Icrf>::new(
        mu,
        vec![Box::new(ProperTimeCentral::new(mu, speed()).unwrap())],
    )
    .unwrap();
    assert_eq!(tp.gauge(), Gauge::ProperTimeKernel);
    assert!(tp.describe().contains("comparison model"));
}

/// The kernel conserves the total energy with its potential `+(GM☉)²/(2c²r²)` (NUM-023), with a
/// massive outer perturber, and its acceleration is minus the gradient of that potential.
///
/// Verifies: PHY-021, NUM-023
#[test]
fn energy_is_conserved_with_the_kernel_potential() {
    let s = system(E_HIGH, Q, true);
    let m = model(&s, vec![kernel()]);
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
    // Central form: a = -grad(phi) by central differences.
    let mu = GravParam::from_km3_per_s2(GM).unwrap();
    let pt = ProperTimeCentral::<Icrf>::new(mu, speed()).unwrap();
    let r = [0.7, -0.4, 0.2]; // lint: allow-literal(generic test point, km)
    let mut a = [0.0; 3];
    ferroastro_forces::CentralTerm::<Icrf>::add_acceleration(&pt, r, [0.0; 3], &mut a);
    let h = 1.0e-5; // lint: allow-literal(finite-difference step, km)
    for k in 0..3 {
        let (mut rp, mut rm) = (r, r);
        rp[k] += h;
        rm[k] -= h;
        let grad = (pt.potential(rp) - pt.potential(rm)) / (2.0 * h);
        assert!((a[k] + grad).abs() <= 1.0e-8 * a[k].abs().max(1e-12), "{k}"); // lint: allow-literal(O(h^2) finite-difference bound)
    }
}

// ---------------------------------------------------------------------------------------------
// Scenario level (`forces.proper_time_kernel`): needs the cached header.440 (CI-005).

fn spec(file: &str) -> ScenarioSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ci/scenarios")
        .join(file);
    ScenarioSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn header_data() -> (Header, String) {
    let f = ferroastro_testdata::locate_product("de440-header").expect("cached header.440");
    let prov = f.provenance();
    let h = Header::parse(&std::fs::read_to_string(&f.path).unwrap()).unwrap();
    (h, prov)
}

/// `forces.proper_time_kernel` assembles under IAS15 and `WHFast` with the "comparison model"
/// label in the model string, and is refused in test-particle mode (FA-E1201).
///
/// Verifies: PHY-021, PHY-015, CI-002, REF-011, NUM-004
#[test]
fn proper_time_scenarios_assemble_under_ias15_and_whfast() {
    if De440::load_cached_or_skip("proper_time_scenarios_assemble_under_ias15_and_whfast")
        .unwrap()
        .is_none()
    {
        return;
    }
    let (h, prov) = header_data();
    let data = DataSources::none().with_header(&h, &prov);
    let mut wh = spec("wh-sun-mercury-grpotential-100yr.toml");
    wh.forces.gr_potential = false;
    for mut s in [spec("sun-mercury-newton-header440.toml"), wh] {
        s.forces.proper_time_kernel = true;
        s.span.years = 0.01; // lint: allow-literal(a few steps: the model string is what is checked)
        s.span.stop_after_passages = None;
        s.output.cadence_days = None;
        assert!(s.forces.requested().contains(&"proper_time_kernel"));
        refuse_unsupported(&s).unwrap();
        let out = run(&s, &data).unwrap();
        println!("{}: {}", s.name, out.model);
        assert!(
            out.model
                .starts_with("N-body model [comparison model (ADR-0016): Proper Time (Gill)"),
            "{}",
            out.model
        );
        assert!(
            out.model.contains("proper-time-kernel-sun"),
            "{}",
            out.model
        );
        if s.integrator.name == IntegratorName::Whfast {
            assert!(out.model.contains("Wisdom-Holman"), "{}", out.model);
        }
    }
    let mut tp = spec("sun-mercury-gr1pn-header440.toml");
    tp.forces.gr_1pn_central = false;
    tp.forces.proper_time_kernel = true;
    let e = refuse_unsupported(&tp).unwrap_err();
    println!("{e}");
    assert_eq!(e.code().to_string(), "FA-E1201");
    assert!(e.to_string().contains("proper_time_kernel"), "{e}");
}

/// PHY-021 acceptance on the Mercury configuration: Sun + Mercury from the DE440 initial
/// conditions, 20 yr (≈ 83 orbits). The PPN (1/2, −1) run (`gr_1pn_central` with
/// `ppn = { beta = 0.5, gamma = -1 }`, IAS15) and the kernel run agree, and both are −1/6 of the
/// GR (1, 1) run, each as the slope of the in-plane perihelion longitude per orbit minus the
/// Newtonian run's. At Mercury `x ≈ 2.6e-8`, so the routes differ only at that order and by the
/// reaction factor `m/M☉ ≈ 1.7e-7` (route 1 is Sun-only); the passage floor over 83 passages is
/// ≈ 1e-14 rad/orbit against an 8e-8 rad/orbit signal. `TOL_REL` therefore holds with a wide
/// margin; the printed ″/cy value is compared with the research report's −7.163, not asserted.
///
/// Verifies: PHY-021
#[test]
fn mercury_ppn_run_and_kernel_run_agree_at_minus_one_sixth_of_gr() {
    if De440::load_cached_or_skip("mercury_ppn_run_and_kernel_run_agree_at_minus_one_sixth_of_gr")
        .unwrap()
        .is_none()
    {
        return;
    }
    let (h, prov) = header_data();
    let data = DataSources::none().with_header(&h, &prov);
    let mut base = spec("sun-mercury-newton-header440.toml");
    base.span.years = 20.0; // lint: allow-literal(about 83 Mercury orbits)
    base.span.stop_after_passages = None;
    base.output.cadence_days = None;
    // (slope of the in-plane perihelion longitude per orbit, mean orbit length in s)
    let slope_of = |s: &ScenarioSpec| -> (f64, f64) {
        let out = run(s, &data).unwrap();
        let rs: Vec<[f64; 3]> = out
            .passages
            .iter()
            .map(|p| p.r.map(|c| c[0] + c[1]))
            .collect();
        let vs0 = out.passages[0].v.map(|c| c[0] + c[1]);
        let r0 = rs[0];
        let n = cross(r0, vs0);
        let e1 = unit(r0);
        let e2 = unit(cross(n, e1));
        let mut l: Vec<f64> = Vec::new();
        for r in &rs {
            let mut x = dot(*r, e2).atan2(dot(*r, e1));
            if let Some(&prev) = l.last() {
                while x - prev > PI {
                    x -= 2.0 * PI;
                }
                while x - prev < -PI {
                    x += 2.0 * PI;
                }
            }
            l.push(x);
        }
        let t0 = out.passages[0].t[0] + out.passages[0].t[1];
        let tl = out.passages.last().map(|p| p.t[0] + p.t[1]).unwrap();
        (slope(&l), (tl - t0) / (l.len() - 1) as f64)
    };
    let with = |f: &dyn Fn(&mut ScenarioSpec)| {
        let mut s = base.clone();
        f(&mut s);
        s
    };
    let (newton, period) = slope_of(&base);
    let gr = slope_of(&with(&|s| {
        s.forces.gr_1pn_central = true;
        s.forces.ppn = Some(Ppn {
            beta: 1.0,
            gamma: 1.0,
        });
    }))
    .0 - newton;
    let ppn = slope_of(&with(&|s| {
        s.forces.gr_1pn_central = true;
        s.forces.ppn = Some(Ppn {
            beta: 0.5,
            gamma: -1.0,
        });
    }))
    .0 - newton;
    let gill = slope_of(&with(&|s| s.forces.proper_time_kernel = true)).0 - newton;
    let per_cy = 36_525.0 * 86_400.0 / period * 180.0 / PI * 3600.0; // lint: allow-literal(Julian century in s, rad to arcsec)
    println!(
        "Mercury, {} s orbit: GR {:+.6} ″/cy, PPN (1/2, -1) {:+.6} ″/cy, Gill kernel {:+.6} ″/cy \
         (research report: -7.163432586)",
        period,
        gr * per_cy,
        ppn * per_cy,
        gill * per_cy
    );
    for (label, x) in [("PPN (1/2, -1)", ppn), ("Gill kernel", gill)] {
        let ratio = x / gr;
        println!("{label} / GR = {ratio:+.9}");
        assert!((ratio / RATIO - 1.0).abs() < TOL_REL, "{label}: {ratio}");
    }
    assert!(((ppn - gill) / gill).abs() < TOL_REL, "routes disagree");
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let n = dot(a, a).sqrt();
    a.map(|x| x / n)
}
